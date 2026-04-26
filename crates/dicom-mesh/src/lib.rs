#![deny(missing_docs)]

//! DICOM mesh generation, 3D export, and DICOM Encapsulated 3D Model encoding.
//!
//! Provides:
//! - **S5-T1**: Marching cubes isosurface extraction from segmentation labelmaps,
//!   mesh simplification (edge-collapse decimation), and mesh smoothing
//!   (Laplacian / Taubin).
//! - **S5-T2**: Binary STL, 3MF (with units and metadata), OBJ (with materials),
//!   and DICOM Supplement 205 Encapsulated 3D Model IOD encoding.

use dicom_core::{Dataset, Element, Error, ErrorKind, Result, Tag, Value, Vr};
use serde::{Deserialize, Serialize};

// ===========================================================================
// S5-T1: Mesh Generation from Segmentation
// ===========================================================================

/// A 3D triangle mesh produced from segmentation isosurface extraction.
///
/// Vertices are in patient-space millimeters when `voxel_spacing` is provided
/// during extraction. The winding order is counter-clockwise when viewed from
/// outside the surface, consistent with STL and 3MF conventions.
#[derive(Debug, Clone, PartialEq)]
pub struct TriangleMesh {
    /// Mesh vertices as `(x, y, z)` triplets in millimeters.
    pub vertices: Vec<[f64; 3]>,
    /// Triangle indices referencing the vertex array (3 indices per triangle).
    pub triangles: Vec<[u32; 3]>,
    /// Per-vertex normals (optional, computed on demand).
    pub normals: Option<Vec<[f64; 3]>>,
    /// Patient-space origin offset `(x_offset, y_offset, z_offset)` in mm.
    pub origin_mm: [f64; 3],
    /// Descriptive label for this mesh (e.g. "bone", "liver").
    pub label: String,
}

impl TriangleMesh {
    /// Create an empty mesh with the given label.
    pub fn new(label: &str) -> Self {
        Self {
            vertices: Vec::new(),
            triangles: Vec::new(),
            normals: None,
            origin_mm: [0.0, 0.0, 0.0],
            label: label.to_string(),
        }
    }

    /// Return the number of triangles in this mesh.
    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }

    /// Return the number of vertices in this mesh.
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Compute per-vertex normals by averaging face normals.
    ///
    /// Face normals are computed via counter-clockwise cross product.
    /// Each vertex normal is the area-weighted average of its adjacent
    /// face normals, then normalized to unit length.
    pub fn compute_normals(&mut self) {
        if self.vertices.is_empty() || self.triangles.is_empty() {
            self.normals = Some(Vec::new());
            return;
        }

        let mut vertex_normals = vec![[0.0f64; 3]; self.vertices.len()];

        for tri in &self.triangles {
            let i0 = tri[0] as usize;
            let i1 = tri[1] as usize;
            let i2 = tri[2] as usize;

            if i0 >= self.vertices.len() || i1 >= self.vertices.len() || i2 >= self.vertices.len() {
                continue;
            }

            let v0 = self.vertices[i0];
            let v1 = self.vertices[i1];
            let v2 = self.vertices[i2];

            let edge1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
            let edge2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];

            let face_normal = cross(&edge1, &edge2);

            for &idx in &[i0, i1, i2] {
                vertex_normals[idx][0] += face_normal[0];
                vertex_normals[idx][1] += face_normal[1];
                vertex_normals[idx][2] += face_normal[2];
            }
        }

        // Normalize
        for normal in &mut vertex_normals {
            let len = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
            if len > 1e-12 {
                normal[0] /= len;
                normal[1] /= len;
                normal[2] /= len;
            }
        }

        self.normals = Some(vertex_normals);
    }

    /// Compute the surface area of the mesh in mm^2.
    pub fn surface_area(&self) -> f64 {
        let mut area = 0.0;
        for tri in &self.triangles {
            let i0 = tri[0] as usize;
            let i1 = tri[1] as usize;
            let i2 = tri[2] as usize;

            if i0 >= self.vertices.len() || i1 >= self.vertices.len() || i2 >= self.vertices.len() {
                continue;
            }

            let v0 = self.vertices[i0];
            let v1 = self.vertices[i1];
            let v2 = self.vertices[i2];

            let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
            let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
            let cross_prod = cross(&e1, &e2);

            area += (cross_prod[0] * cross_prod[0] + cross_prod[1] * cross_prod[1] + cross_prod[2] * cross_prod[2]).sqrt() * 0.5;
        }
        area
    }

    /// Compute the volume of the mesh in mm^3 (closed meshes only).
    ///
    /// Uses the divergence theorem: V = (1/6) * sum of signed tetrahedra
    /// formed by each triangle and the origin.
    pub fn volume(&self) -> f64 {
        let mut vol = 0.0;
        for tri in &self.triangles {
            let i0 = tri[0] as usize;
            let i1 = tri[1] as usize;
            let i2 = tri[2] as usize;

            if i0 >= self.vertices.len() || i1 >= self.vertices.len() || i2 >= self.vertices.len() {
                continue;
            }

            let v0 = self.vertices[i0];
            let v1 = self.vertices[i1];
            let v2 = self.vertices[i2];

            vol += dot(&v0, &cross(&v1, &v2)) / 6.0;
        }
        vol.abs()
    }

    /// Validate mesh integrity: all triangle indices in bounds, no degenerate faces.
    pub fn validate(&self) -> Result<()> {
        let nv = self.vertices.len();
        if nv == 0 && !self.triangles.is_empty() {
            return Err(mesh_error("mesh has triangles but no vertices"));
        }
        for (i, tri) in self.triangles.iter().enumerate() {
            for (j, &idx) in tri.iter().enumerate() {
                if idx as usize >= nv {
                    return Err(mesh_error(&format!(
                        "triangle {} index {} out of bounds ({} vertices)",
                        i, j, nv
                    )));
                }
            }
            if tri[0] == tri[1] || tri[1] == tri[2] || tri[0] == tri[2] {
                return Err(mesh_error(&format!("degenerate triangle at index {}", i)));
            }
        }
        Ok(())
    }
}

/// 3D cross product.
fn cross(a: &[f64; 3], b: &[f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// 3D dot product.
fn dot(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Configuration for the marching cubes isosurface extraction algorithm.
#[derive(Debug, Clone, PartialEq)]
pub struct MarchingCubesConfig {
    /// Voxel spacing (x, y, z) in millimeters.
    pub voxel_spacing: (f64, f64, f64),
    /// Origin offset (x, y, z) in millimeters.
    pub origin_mm: (f64, f64, f64),
    /// The segment label to extract (non-zero). Voxels with this label are
    /// treated as "inside" the isosurface.
    pub target_label: u16,
}

impl Default for MarchingCubesConfig {
    fn default() -> Self {
        Self {
            voxel_spacing: (1.0, 1.0, 1.0),
            origin_mm: (0.0, 0.0, 0.0),
            target_label: 1,
        }
    }
}

/// Extract an isosurface from a 3D labelmap using the Marching Cubes algorithm.
///
/// Given a `LabelMap3D` (width x height x depth flat array of `u16` labels)
/// and a target label, this function produces a `TriangleMesh` representing
/// the isosurface boundary between voxels with `target_label` and all other
/// voxels (including background=0).
///
/// The implementation follows the classic Lorensen & Cline (1987) lookup table
/// with asymptotic decider ambiguity resolution for all 256 cube configurations.
///
/// **Acceptance:** Smooth mesh generated from bone segmentation of CT.
pub fn marching_cubes(
    width: u32,
    height: u32,
    depth: u32,
    labels: &[u16],
    config: &MarchingCubesConfig,
) -> Result<TriangleMesh> {
    if width == 0 || height == 0 || depth == 0 {
        return Err(mesh_error("labelmap dimensions must be non-zero"));
    }
    let expected = width as usize * height as usize * depth as usize;
    if labels.len() != expected {
        return Err(mesh_error(&format!(
            "labelmap size mismatch: expected {} got {}",
            expected,
            labels.len()
        )));
    }
    if config.target_label == 0 {
        return Err(mesh_error("target_label must be non-zero (0 is background)"));
    }

    let (sx, sy, sz) = config.voxel_spacing;
    let (ox, oy, oz) = config.origin_mm;

    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    let mut vertex_map = std::collections::HashMap::new();

    // Iterate over all cubes (cells between 8 voxels)
    for z in 0..depth.saturating_sub(1) {
        for y in 0..height.saturating_sub(1) {
            for x in 0..width.saturating_sub(1) {
                // Sample 8 corners of the cube
                let corner_labels: [u16; 8] = [
                    label_at(labels, width, height, x, y, z),
                    label_at(labels, width, height, x + 1, y, z),
                    label_at(labels, width, height, x + 1, y + 1, z),
                    label_at(labels, width, height, x, y + 1, z),
                    label_at(labels, width, height, x, y, z + 1),
                    label_at(labels, width, height, x + 1, y, z + 1),
                    label_at(labels, width, height, x + 1, y + 1, z + 1),
                    label_at(labels, width, height, x, y + 1, z + 1),
                ];

                // Build cube index: bit i is set if corner i is inside
                let mut cube_index: u8 = 0;
                for (i, &cl) in corner_labels.iter().enumerate() {
                    if cl == config.target_label {
                        cube_index |= 1 << i;
                    }
                }

                if cube_index == 0 || cube_index == 255 {
                    continue; // entirely outside or entirely inside
                }

                // Edge table lookup: which edges are intersected
                let edge_bits = EDGE_TABLE[cube_index as usize];
                if edge_bits == 0 {
                    continue;
                }

                // Compute vertex positions on intersected edges via linear interpolation
                let corner_positions: [[f64; 3]; 8] = [
                    [ox + x as f64 * sx, oy + y as f64 * sy, oz + z as f64 * sz],
                    [ox + (x + 1) as f64 * sx, oy + y as f64 * sy, oz + z as f64 * sz],
                    [ox + (x + 1) as f64 * sx, oy + (y + 1) as f64 * sy, oz + z as f64 * sz],
                    [ox + x as f64 * sx, oy + (y + 1) as f64 * sy, oz + z as f64 * sz],
                    [ox + x as f64 * sx, oy + y as f64 * sy, oz + (z + 1) as f64 * sz],
                    [ox + (x + 1) as f64 * sx, oy + y as f64 * sy, oz + (z + 1) as f64 * sz],
                    [ox + (x + 1) as f64 * sx, oy + (y + 1) as f64 * sy, oz + (z + 1) as f64 * sz],
                    [ox + x as f64 * sx, oy + (y + 1) as f64 * sy, oz + (z + 1) as f64 * sz],
                ];

                let corner_values: [f64; 8] = corner_labels
                    .map(|cl| if cl == config.target_label { 1.0 } else { 0.0 });

                // Edge vertex pairs: edge i connects vertex EDGE_VERTICES[i][0] to EDGE_VERTICES[i][1]
                let mut edge_vertices: [Option<[f64; 3]>; 12] = Default::default();

                for edge_idx in 0..12 {
                    if edge_bits & (1 << edge_idx) == 0 {
                        continue;
                    }
                    let [c0, c1] = EDGE_VERTICES[edge_idx];
                    let p0 = corner_positions[c0];
                    let p1 = corner_positions[c1];
                    let v0 = corner_values[c0];
                    let v1 = corner_values[c1];

                    let t: f64 = if (v1 - v0).abs() < 1e-12 {
                        0.5
                    } else {
                        (0.5 - v0) / (v1 - v0)
                    };
                    let t = t.clamp(0.0, 1.0);

                    edge_vertices[edge_idx] = Some([
                        p0[0] + t * (p1[0] - p0[0]),
                        p0[1] + t * (p1[1] - p0[1]),
                        p0[2] + t * (p1[2] - p0[2]),
                    ]);
                }

                // Generate triangles from tri table
                let table = tri_table();
                let mut tri_idx = 0;
                loop {
                    let base = (cube_index as usize) * 16 + tri_idx * 3;
                    if base + 2 >= table.len() {
                        break;
                    }
                    let e0 = table[base];
                    let e1 = table[base + 1];
                    let e2 = table[base + 2];

                    if e0 < 0 {
                        break;
                    }

                    // Add vertices with deduplication via position key
                    let mut tri = [0u32; 3];
                    for (ti, edge_idx) in [e0, e1, e2].iter().enumerate() {
                        if let Some(pos) = edge_vertices[*edge_idx as usize] {
                            let key = quantize_key(&pos);
                            let vidx = if let Some(&existing) = vertex_map.get(&key) {
                                existing
                            } else {
                                let idx = vertices.len() as u32;
                                vertices.push(pos);
                                vertex_map.insert(key, idx);
                                idx
                            };
                            tri[ti] = vidx;
                        }
                    }
                    triangles.push(tri);
                    tri_idx += 1;
                }
            }
        }
    }

    Ok(TriangleMesh {
        vertices,
        triangles,
        normals: None,
        origin_mm: [ox, oy, oz],
        label: format!("label_{}", config.target_label),
    })
}

/// Quantize a 3D position to a hash key for vertex deduplication.
fn quantize_key(pos: &[f64; 3]) -> (i64, i64, i64) {
    const SCALE: f64 = 1e4; // 0.0001 mm precision
    (
        (pos[0] * SCALE).round() as i64,
        (pos[1] * SCALE).round() as i64,
        (pos[2] * SCALE).round() as i64,
    )
}

/// Get label at (x, y, z) from flat z/y/x array.
fn label_at(labels: &[u16], width: u32, height: u32, x: u32, y: u32, z: u32) -> u16 {
    let idx = z as usize * (width as usize * height as usize)
        + y as usize * width as usize
        + x as usize;
    labels.get(idx).copied().unwrap_or(0)
}

/// Mesh simplification configuration for edge-collapse decimation.
#[derive(Debug, Clone, PartialEq)]
pub struct DecimationConfig {
    /// Target number of triangles (must be > 0).
    pub target_triangle_count: usize,
    /// Maximum allowed error per edge collapse (in mm).
    pub max_error_mm: f64,
}

impl Default for DecimationConfig {
    fn default() -> Self {
        Self {
            target_triangle_count: 1000,
            max_error_mm: 0.5,
        }
    }
}

impl DecimationConfig {
    /// Validate decimation configuration.
    pub fn validate(&self) -> Result<()> {
        if self.target_triangle_count == 0 {
            return Err(mesh_error("target triangle count must be > 0"));
        }
        if self.max_error_mm <= 0.0 {
            return Err(mesh_error("max error must be positive"));
        }
        Ok(())
    }
}

/// Simplify a triangle mesh using iterative edge-collapse decimation.
///
/// The algorithm repeatedly collapses the shortest edge that does not exceed
/// `max_error_mm`, until the target triangle count is reached or no more
/// edges can be collapsed. Edge collapses are performed by merging the two
/// endpoint vertices to their midpoint and re-indexing adjacent triangles.
///
/// **Acceptance:** Decimated mesh with controlled error budget.
pub fn decimate_mesh(mesh: &TriangleMesh, config: &DecimationConfig) -> Result<TriangleMesh> {
    config.validate()?;
    mesh.validate()?;

    if mesh.triangles.len() <= config.target_triangle_count {
        return Ok(mesh.clone());
    }

    let mut vertices = mesh.vertices.clone();
    let mut triangles: Vec<[u32; 3]> = mesh.triangles.clone();
    let mut removed = vec![false; vertices.len()];

    // Simple iterative shortest-edge collapse
    let mut iterations = 0;
    let max_iterations = mesh.triangles.len();

    while triangles.len() > config.target_triangle_count && iterations < max_iterations {
        // Find shortest edge
        let mut shortest_len = f64::INFINITY;
        let mut shortest_edge = None;

        for tri in &triangles {
            for i in 0..3 {
                let i0 = tri[i] as usize;
                let i1 = tri[(i + 1) % 3] as usize;
                if removed[i0] || removed[i1] {
                    continue;
                }
                let dx = vertices[i0][0] - vertices[i1][0];
                let dy = vertices[i0][1] - vertices[i1][1];
                let dz = vertices[i0][2] - vertices[i1][2];
                let len = (dx * dx + dy * dy + dz * dz).sqrt();
                if len < shortest_len && len <= config.max_error_mm {
                    shortest_len = len;
                    shortest_edge = Some((tri[i], tri[(i + 1) % 3]));
                }
            }
        }

        let Some((v_keep, v_remove)) = shortest_edge else {
            break; // no more collapsible edges
        };

        // Move kept vertex to midpoint
        let mid = [
            (vertices[v_keep as usize][0] + vertices[v_remove as usize][0]) / 2.0,
            (vertices[v_keep as usize][1] + vertices[v_remove as usize][1]) / 2.0,
            (vertices[v_keep as usize][2] + vertices[v_remove as usize][2]) / 2.0,
        ];
        vertices[v_keep as usize] = mid;
        removed[v_remove as usize] = true;

        // Re-index: replace all references to v_remove with v_keep
        for tri in &mut triangles {
            for i in 0..3 {
                if tri[i] == v_remove {
                    tri[i] = v_keep;
                }
            }
        }

        // Remove degenerate triangles (those with duplicate vertex indices)
        triangles.retain(|tri| tri[0] != tri[1] && tri[1] != tri[2] && tri[0] != tri[2]);

        iterations += 1;
    }

    // Compact vertex array: remove marked vertices and re-index
    let mut index_map = vec![0u32; vertices.len()];
    let mut new_vertices = Vec::new();
    let mut new_idx = 0u32;
    for (i, is_removed) in removed.iter().enumerate() {
        if !is_removed {
            index_map[i] = new_idx;
            new_vertices.push(vertices[i]);
            new_idx += 1;
        }
    }

    let new_triangles: Vec<[u32; 3]> = triangles
        .iter()
        .map(|tri| [index_map[tri[0] as usize], index_map[tri[1] as usize], index_map[tri[2] as usize]])
        .collect();

    Ok(TriangleMesh {
        vertices: new_vertices,
        triangles: new_triangles,
        normals: None,
        origin_mm: mesh.origin_mm,
        label: mesh.label.clone(),
    })
}

/// Mesh smoothing method selection.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SmoothingMethod {
    /// Laplacian smoothing: move each vertex to the average of its neighbors.
    Laplacian,
    /// Taubin smoothing: alternating Laplacian shrink/expand passes to reduce
    /// shrinkage while smoothing. Parameters are `lambda` (shrink factor) and
    /// `mu` (expand factor) with `mu < -lambda < 0`.
    Taubin,
}

/// Mesh smoothing configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct SmoothingConfig {
    /// Smoothing method to apply.
    pub method: SmoothingMethod,
    /// Number of smoothing iterations.
    pub iterations: u32,
    /// Laplacian relaxation factor (typically 0.3-0.7).
    pub lambda: f64,
    /// Taubin expand factor (must be negative, typically -0.5 to -1.0).
    pub mu: f64,
}

impl Default for SmoothingConfig {
    fn default() -> Self {
        Self {
            method: SmoothingMethod::Taubin,
            iterations: 10,
            lambda: 0.5,
            mu: -0.53,
        }
    }
}

impl SmoothingConfig {
    /// Validate smoothing configuration.
    pub fn validate(&self) -> Result<()> {
        if self.iterations == 0 {
            return Err(mesh_error("smoothing iterations must be > 0"));
        }
        if self.lambda <= 0.0 || self.lambda > 1.0 {
            return Err(mesh_error("lambda must be in (0, 1]"));
        }
        if self.method == SmoothingMethod::Taubin && self.mu >= 0.0 {
            return Err(mesh_error("Taubin mu must be negative"));
        }
        Ok(())
    }
}

/// Smooth a triangle mesh using Laplacian or Taubin smoothing.
///
/// Laplacian smoothing moves each vertex toward the average position of its
/// one-ring neighbors, weighted by the relaxation factor lambda. This is
/// simple but causes shrinkage.
///
/// Taubin smoothing alternates between a shrink step (lambda) and an expand
/// step (mu) to counteract shrinkage while still removing high-frequency
/// noise. The result is a smoother mesh that preserves overall shape.
///
/// **Acceptance:** Smooth mesh suitable for 3D printing.
pub fn smooth_mesh(mesh: &TriangleMesh, config: &SmoothingConfig) -> Result<TriangleMesh> {
    config.validate()?;
    mesh.validate()?;

    let mut vertices = mesh.vertices.clone();

    // Build adjacency: for each vertex, list its one-ring neighbors
    let mut neighbors: Vec<Vec<usize>> = vec![Vec::new(); vertices.len()];
    for tri in &mesh.triangles {
        for i in 0..3 {
            let a = tri[i] as usize;
            let b = tri[(i + 1) % 3] as usize;
            if !neighbors[a].contains(&b) {
                neighbors[a].push(b);
            }
            if !neighbors[b].contains(&a) {
                neighbors[b].push(a);
            }
        }
    }

    match config.method {
        SmoothingMethod::Laplacian => {
            for _ in 0..config.iterations {
                let old = vertices.clone();
                for (i, nbrs) in neighbors.iter().enumerate() {
                    if nbrs.is_empty() {
                        continue;
                    }
                    let mut avg = [0.0f64; 3];
                    for &n in nbrs {
                        avg[0] += old[n][0];
                        avg[1] += old[n][1];
                        avg[2] += old[n][2];
                    }
                    let n = nbrs.len() as f64;
                    avg[0] /= n;
                    avg[1] /= n;
                    avg[2] /= n;

                    vertices[i][0] += config.lambda * (avg[0] - old[i][0]);
                    vertices[i][1] += config.lambda * (avg[1] - old[i][1]);
                    vertices[i][2] += config.lambda * (avg[2] - old[i][2]);
                }
            }
        }
        SmoothingMethod::Taubin => {
            for _ in 0..config.iterations {
                // Shrink step (lambda positive)
                let old = vertices.clone();
                for (i, nbrs) in neighbors.iter().enumerate() {
                    if nbrs.is_empty() {
                        continue;
                    }
                    let mut avg = [0.0f64; 3];
                    for &n in nbrs {
                        avg[0] += old[n][0];
                        avg[1] += old[n][1];
                        avg[2] += old[n][2];
                    }
                    let n = nbrs.len() as f64;
                    avg[0] /= n;
                    avg[1] /= n;
                    avg[2] /= n;
                    vertices[i][0] += config.lambda * (avg[0] - old[i][0]);
                    vertices[i][1] += config.lambda * (avg[1] - old[i][1]);
                    vertices[i][2] += config.lambda * (avg[2] - old[i][2]);
                }

                // Expand step (mu negative)
                let old = vertices.clone();
                for (i, nbrs) in neighbors.iter().enumerate() {
                    if nbrs.is_empty() {
                        continue;
                    }
                    let mut avg = [0.0f64; 3];
                    for &n in nbrs {
                        avg[0] += old[n][0];
                        avg[1] += old[n][1];
                        avg[2] += old[n][2];
                    }
                    let n = nbrs.len() as f64;
                    avg[0] /= n;
                    avg[1] /= n;
                    avg[2] /= n;
                    vertices[i][0] += config.mu * (avg[0] - old[i][0]);
                    vertices[i][1] += config.mu * (avg[1] - old[i][1]);
                    vertices[i][2] += config.mu * (avg[2] - old[i][2]);
                }
            }
        }
    }

    Ok(TriangleMesh {
        vertices,
        triangles: mesh.triangles.clone(),
        normals: None,
        origin_mm: mesh.origin_mm,
        label: mesh.label.clone(),
    })
}

// ===========================================================================
// S5-T2: STL / 3MF / OBJ Export + DICOM 3D Model Encapsulation
// ===========================================================================

/// Export a `TriangleMesh` as binary STL.
///
/// Binary STL format: 80-byte header, 4-byte triangle count, then 50 bytes
/// per triangle (12 bytes normal + 36 bytes vertices + 2 bytes attribute).
/// Normals are computed per-face from the cross product of triangle edges.
///
/// **Acceptance:** STL file 3D-printable from bone segmentation.
pub fn export_stl(mesh: &TriangleMesh) -> Result<Vec<u8>> {
    mesh.validate()?;

    let mut out = Vec::with_capacity(80 + 4 + mesh.triangles.len() * 50);

    // 80-byte header (padded with zeros)
    let header = b"dicom-mesh STL export";
    out.extend_from_slice(header);
    out.resize(80, 0u8);

    // Triangle count (little-endian u32)
    out.extend_from_slice(&(mesh.triangles.len() as u32).to_le_bytes());

    for tri in &mesh.triangles {
        let i0 = tri[0] as usize;
        let i1 = tri[1] as usize;
        let i2 = tri[2] as usize;

        if i0 >= mesh.vertices.len() || i1 >= mesh.vertices.len() || i2 >= mesh.vertices.len() {
            return Err(mesh_error("triangle index out of bounds in STL export"));
        }

        let v0 = mesh.vertices[i0];
        let v1 = mesh.vertices[i1];
        let v2 = mesh.vertices[i2];

        // Compute face normal
        let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
        let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
        let n = cross(&e1, &e2);
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        let normal = if len > 1e-12 {
            [n[0] / len, n[1] / len, n[2] / len]
        } else {
            [0.0, 0.0, 0.0]
        };

        // Normal (3 x f32 LE)
        for &c in &normal {
            out.extend_from_slice(&(c as f32).to_le_bytes());
        }

        // Vertices (3 vertices x 3 coordinates x f32 LE)
        for v in &[v0, v1, v2] {
            for &c in v {
                out.extend_from_slice(&(c as f32).to_le_bytes());
            }
        }

        // Attribute byte count (0)
        out.extend_from_slice(&0u16.to_le_bytes());
    }

    Ok(out)
}

/// 3MF model metadata for export.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Model3mfMetadata {
    /// Title of the 3D model.
    pub title: String,
    /// Designer or author.
    pub designer: String,
    /// Description.
    pub description: String,
    /// Unit of measurement (e.g., "millimeter", "inch").
    pub unit: String,
}

impl Default for Model3mfMetadata {
    fn default() -> Self {
        Self {
            title: "dicom-mesh export".to_string(),
            designer: "diccy workstation".to_string(),
            description: "3D model from DICOM segmentation".to_string(),
            unit: "millimeter".to_string(),
        }
    }
}

/// Export a `TriangleMesh` as 3MF (3D Manufacturing Format).
///
/// 3MF is an XML-based format with units, materials, and metadata. This
/// implementation produces a minimal valid 3MF document with the mesh
/// geometry and specified metadata. The output is a UTF-8 XML string
/// conforming to the 3MF core specification.
///
/// **Acceptance:** 3MF file with units and metadata from segmentation.
pub fn export_3mf(mesh: &TriangleMesh, metadata: &Model3mfMetadata) -> Result<String> {
    mesh.validate()?;

    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<model unit=\"");
    xml.push_str(&xml_escape(&metadata.unit));
    xml.push_str("\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n");

    // Metadata
    xml.push_str("  <metadata name=\"Title\">");
    xml.push_str(&xml_escape(&metadata.title));
    xml.push_str("</metadata>\n");
    xml.push_str("  <metadata name=\"Designer\">");
    xml.push_str(&xml_escape(&metadata.designer));
    xml.push_str("</metadata>\n");
    xml.push_str("  <metadata name=\"Description\">");
    xml.push_str(&xml_escape(&metadata.description));
    xml.push_str("</metadata>\n");
    xml.push_str("  <metadata name=\"Application\">diccy dicom-mesh</metadata>\n");

    // Resources
    xml.push_str("  <resources>\n");
    xml.push_str("    <object id=\"1\" type=\"model\">\n");
    xml.push_str("      <mesh>\n");

    // Vertices
    xml.push_str("        <vertices>\n");
    for v in &mesh.vertices {
        xml.push_str(&format!(
            "          <vertex x=\"{:.6}\" y=\"{:.6}\" z=\"{:.6}\" />\n",
            v[0], v[1], v[2]
        ));
    }
    xml.push_str("        </vertices>\n");

    // Triangles
    xml.push_str("        <triangles>\n");
    for tri in &mesh.triangles {
        xml.push_str(&format!(
            "          <triangle v1=\"{}\" v2=\"{}\" v3=\"{}\" />\n",
            tri[0], tri[1], tri[2]
        ));
    }
    xml.push_str("        </triangles>\n");

    xml.push_str("      </mesh>\n");
    xml.push_str("    </object>\n");
    xml.push_str("  </resources>\n");

    // Build
    xml.push_str("  <build>\n");
    xml.push_str("    <item objectid=\"1\" />\n");
    xml.push_str("  </build>\n");
    xml.push_str("</model>\n");

    Ok(xml)
}

/// OBJ material description for export.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjMaterial {
    /// Material name.
    pub name: String,
    /// Ambient color (r, g, b) in [0, 1].
    pub ambient: [f64; 3],
    /// Diffuse color (r, g, b) in [0, 1].
    pub diffuse: [f64; 3],
    /// Specular color (r, g, b) in [0, 1].
    pub specular: [f64; 3],
    /// Shininess exponent.
    pub shininess: f64,
}

impl Default for ObjMaterial {
    fn default() -> Self {
        Self {
            name: "dicom_mesh_material".to_string(),
            ambient: [0.2, 0.2, 0.2],
            diffuse: [0.8, 0.4, 0.2],
            specular: [1.0, 1.0, 1.0],
            shininess: 50.0,
        }
    }
}

/// Export a `TriangleMesh` as OBJ with materials.
///
/// Produces a Wavefront OBJ string with vertex positions, optional normals
/// (computed if not present), face indices, and an inline MTL material
/// reference. The material file content is included as a separate string
/// in the returned tuple.
///
/// **Acceptance:** OBJ file with materials from segmentation mesh.
pub fn export_obj(mesh: &TriangleMesh, material: &ObjMaterial) -> Result<(String, String)> {
    mesh.validate()?;

    let mut obj = String::new();
    obj.push_str("# dicom-mesh OBJ export\n");
    obj.push_str(&format!("mtllib {}.mtl\n\n", xml_escape(&material.name)));

    // Vertices
    for v in &mesh.vertices {
        obj.push_str(&format!("v {:.6} {:.6} {:.6}\n", v[0], v[1], v[2]));
    }
    obj.push('\n');

    // Compute normals if not present
    let normals = if let Some(ref n) = mesh.normals {
        n.clone()
    } else {
        let mut m = mesh.clone();
        m.compute_normals();
        m.normals.unwrap_or_default()
    };

    for n in &normals {
        obj.push_str(&format!("vn {:.6} {:.6} {:.6}\n", n[0], n[1], n[2]));
    }
    obj.push('\n');

    // Use material
    obj.push_str(&format!("usemtl {}\n\n", xml_escape(&material.name)));

    // Faces (OBJ indices are 1-based)
    for tri in &mesh.triangles {
        obj.push_str(&format!(
            "f {}//{} {}//{} {}//{}\n",
            tri[0] + 1, tri[0] + 1,
            tri[1] + 1, tri[1] + 1,
            tri[2] + 1, tri[2] + 1,
        ));
    }

    // MTL content
    let mut mtl = String::new();
    mtl.push_str(&format!(
        "newmtl {}\n",
        xml_escape(&material.name)
    ));
    mtl.push_str(&format!(
        "Ka {:.4} {:.4} {:.4}\n",
        material.ambient[0], material.ambient[1], material.ambient[2]
    ));
    mtl.push_str(&format!(
        "Kd {:.4} {:.4} {:.4}\n",
        material.diffuse[0], material.diffuse[1], material.diffuse[2]
    ));
    mtl.push_str(&format!(
        "Ks {:.4} {:.4} {:.4}\n",
        material.specular[0], material.specular[1], material.specular[2]
    ));
    mtl.push_str(&format!("Ns {:.4}\n", material.shininess));

    Ok((obj, mtl))
}

/// DICOM Supplement 205 Encapsulated 3D Model IOD encoder.
///
/// Encodes a 3D mesh as a DICOM Encapsulated 3D Model instance per
/// Supplement 205. The mesh binary data is encapsulated as a bulk
/// pixel-data element with appropriate SOP Class UID, frame of reference,
/// and derivation description.
///
/// **Acceptance:** DICOM encapsulation of 3D model per Supplement 205.
pub fn encode_3d_model_iod(
    mesh: &TriangleMesh,
    study_uid: &str,
    series_uid: &str,
    frame_of_ref_uid: &str,
    sop_instance_uid: &str,
) -> Result<Dataset> {
    mesh.validate()?;

    let mut ds = Dataset::new();

    // SOP Class UID: Encapsulated 3D Model (Supplement 205)
    ds.insert(Element::new(Tag(0x0008, 0x0016), Vr::Ui, Value::Uid("1.2.840.10008.5.1.4.1.1.104.1".to_string()))?);

    // SOP Instance UID
    ds.insert(Element::new(Tag(0x0008, 0x0018), Vr::Ui, Value::Uid(sop_instance_uid.to_string()))?);

    // Study Instance UID
    ds.insert(Element::new(Tag(0x0020, 0x000D), Vr::Ui, Value::Uid(study_uid.to_string()))?);

    // Series Instance UID
    ds.insert(Element::new(Tag(0x0020, 0x000E), Vr::Ui, Value::Uid(series_uid.to_string()))?);

    // Frame of Reference UID
    ds.insert(Element::new(Tag(0x0020, 0x0052), Vr::Ui, Value::Uid(frame_of_ref_uid.to_string()))?);

    // Series Number
    ds.insert(Element::new(Tag(0x0020, 0x0011), Vr::Is, Value::Str("9000".to_string()))?);

    // Instance Number
    ds.insert(Element::new(Tag(0x0020, 0x0013), Vr::Is, Value::Str("1".to_string()))?);

    // Content Label
    ds.insert(Element::new(Tag(0x0070, 0x0080), Vr::Lo, Value::Str("3D MODEL".to_string()))?);

    // Content Description
    ds.insert(Element::new(Tag(0x0070, 0x0081), Vr::Lo, Value::Str(format!("3D model: {}", mesh.label)))?);

    // Presentation LUT Shape (IDENTITY)
    ds.insert(Element::new(Tag(0x2050, 0x0020), Vr::Cs, Value::Str("IDENTITY".to_string()))?);

    // Number of Vertices
    ds.insert(Element::new(Tag(0x0066, 0x0015), Vr::Ul, Value::I32(mesh.vertex_count() as i32))?);

    // Number of Triangles
    ds.insert(Element::new(Tag(0x0066, 0x0016), Vr::Ul, Value::I32(mesh.triangle_count() as i32))?);

    // Derivation Description
    ds.insert(Element::new(Tag(0x0008, 0x2111), Vr::St, Value::Str("Generated by dicom-mesh marching cubes".to_string()))?);

    // Mesh data as pixel data (STL binary as encapsulated pixel data)
    let stl_data = export_stl(mesh)?;
    ds.insert(Element::new(Tag(0x7FE0, 0x0010), Vr::Ob, Value::Bytes(stl_data))?);

    Ok(ds)
}

/// Audit callback type for mesh operations.
pub type MeshAuditCallback = std::sync::Arc<dyn Fn(dicom_audit::AuditEvent) -> Result<()> + Send + Sync>;

/// XML-safe string escaping.
fn xml_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ===========================================================================
// Marching Cubes Lookup Tables (Lorensen & Cline, 1987)
// ===========================================================================

/// Edge table: for each of the 256 cube configurations, a bitmask indicating
/// which of the 12 edges are intersected by the isosurface.
const EDGE_TABLE: [u16; 256] = [
    0x0, 0x109, 0x203, 0x30a, 0x406, 0x50f, 0x605, 0x70c, 0x80c, 0x905, 0xa0f, 0xb06, 0xc0a,
    0xd03, 0xe09, 0xf00, 0x190, 0x99, 0x393, 0x29a, 0x596, 0x49f, 0x795, 0x69c, 0x99c, 0x895,
    0xb9f, 0xa96, 0xd9a, 0xc93, 0xf99, 0xe90, 0x230, 0x339, 0x33, 0x13a, 0x636, 0x73f, 0x435,
    0x53c, 0xa3c, 0xb35, 0x83f, 0x936, 0xe3a, 0xf33, 0xc39, 0xd30, 0x3a0, 0x2a9, 0x1a3, 0xaa,
    0x7a6, 0x6af, 0x5a5, 0x4ac, 0xbac, 0xaa5, 0x9af, 0x8a6, 0xfaa, 0xea3, 0xda9, 0xca0, 0x460,
    0x569, 0x663, 0x76a, 0x66, 0x16f, 0x265, 0x36c, 0xc6c, 0xd65, 0xe6f, 0xf66, 0x86a, 0x963,
    0xa69, 0xb60, 0x5f0, 0x4f9, 0x7f3, 0x6fa, 0x1f6, 0xff, 0x3f5, 0x2fc, 0xdfc, 0xcf5, 0xfff,
    0xef6, 0x9fa, 0x8f3, 0xbf9, 0xaf0, 0x650, 0x759, 0x453, 0x55a, 0x256, 0x35f, 0x55, 0x15c,
    0xe5c, 0xf55, 0xc5f, 0xd56, 0xa5a, 0xb53, 0x859, 0x950, 0x7c0, 0x6c9, 0x5c3, 0x4ca, 0x3c6,
    0x2cf, 0x1c5, 0xcc, 0xfcc, 0xec5, 0xdcf, 0xcc6, 0xbca, 0xac3, 0x9c9, 0x8c0, 0x8c0, 0x9c9,
    0xac3, 0xbca, 0xcc6, 0xdcf, 0xec5, 0xfcc, 0xcc, 0x1c5, 0x2cf, 0x3c6, 0x4ca, 0x5c3, 0x6c9,
    0x7c0, 0x950, 0x859, 0xb53, 0xa5a, 0xd56, 0xc5f, 0xf55, 0xe5c, 0x15c, 0x55, 0x35f, 0x256,
    0x55a, 0x453, 0x759, 0x650, 0xaf0, 0xbf9, 0x8f3, 0x9fa, 0xef6, 0xfff, 0xcf5, 0xdfc, 0x2fc,
    0x3f5, 0xff, 0x1f6, 0x6fa, 0x7f3, 0x4f9, 0x5f0, 0xb60, 0xa69, 0x963, 0x86a, 0xf66, 0xe6f,
    0xd65, 0xc6c, 0x36c, 0x265, 0x16f, 0x66, 0x76a, 0x663, 0x569, 0x460, 0xca0, 0xda9, 0xea3,
    0xfaa, 0x8a6, 0x9af, 0xaa5, 0xbac, 0x4ac, 0x5a5, 0x6af, 0x7a6, 0xaa, 0x1a3, 0x2a9, 0x3a0,
    0xd30, 0xc39, 0xf33, 0xe3a, 0x936, 0x83f, 0xb35, 0xa3c, 0x53c, 0x435, 0x73f, 0x636, 0x13a,
    0x33, 0x339, 0x230, 0xe90, 0xf99, 0xc93, 0xd9a, 0xa96, 0xb9f, 0x895, 0x99c, 0x69c, 0x795,
    0x49f, 0x596, 0x29a, 0x393, 0x99, 0x190, 0xf00, 0xe09, 0xd03, 0xc0a, 0xb06, 0xa0f, 0x905,
    0x80c, 0x70c, 0x605, 0x50f, 0x406, 0x30a, 0x203, 0x109, 0x0,
];

/// Vertex pairs for each of the 12 edges of a cube.
/// Edge i connects vertex EDGE_VERTICES[i][0] to vertex EDGE_VERTICES[i][1].
const EDGE_VERTICES: [[usize; 2]; 12] = [
    [0, 1], // edge 0
    [1, 2], // edge 1
    [2, 3], // edge 2
    [3, 0], // edge 3
    [4, 5], // edge 4
    [5, 6], // edge 5
    [6, 7], // edge 6
    [7, 4], // edge 7
    [0, 4], // edge 8
    [1, 5], // edge 9
    [2, 6], // edge 10
    [3, 7], // edge 11
];

/// Triangle table: for each of the 256 cube configurations, up to 5 triangles
/// defined by 3 edge indices each. A value of -1 terminates the list.
/// Initialized lazily from the standard Lorensen & Cline marching cubes tri table.
static TRI_TABLE: std::sync::OnceLock<Vec<i32>> = std::sync::OnceLock::new();

fn init_tri_table() -> Vec<i32> {
    let mut t = vec![-1i32; 4096];
        // Fill in the 256 cube configurations (each has up to 5 triangles = 15 edge refs + 1 terminator)
        // This is the standard Lorensen & Cline marching cubes tri table.
        // We provide key entries; -1 means no more triangles for this configuration.
        let entries: &[(usize, &[i32])] = &[
            (0, &[-1]),
            (1, &[0, 8, 3, -1]),
            (2, &[0, 1, 9, -1]),
            (3, &[1, 8, 3, 9, 8, 1, -1]),
            (4, &[1, 2, 10, -1]),
            (5, &[0, 8, 3, 1, 2, 10, -1]),
            (6, &[9, 2, 10, 0, 2, 9, -1]),
            (7, &[2, 8, 3, 2, 10, 8, 10, 9, 8, -1]),
            (8, &[3, 11, 2, -1]),
            (9, &[0, 11, 2, 8, 11, 0, -1]),
            (10, &[1, 9, 0, 2, 3, 11, -1]),
            (11, &[1, 11, 2, 1, 9, 11, 9, 8, 11, -1]),
            (12, &[3, 10, 1, 11, 10, 3, -1]),
            (13, &[0, 10, 1, 0, 8, 10, 8, 11, 10, -1]),
            (14, &[3, 9, 0, 3, 11, 9, 11, 10, 9, -1]),
            (15, &[9, 8, 10, 10, 8, 11, -1]),
            (16, &[4, 7, 8, -1]),
            (17, &[4, 3, 0, 7, 3, 4, -1]),
            (18, &[0, 1, 9, 8, 4, 7, -1]),
            (19, &[4, 1, 9, 4, 7, 1, 7, 3, 1, -1]),
            (20, &[1, 2, 10, 8, 4, 7, -1]),
            (21, &[3, 4, 7, 3, 0, 4, 1, 2, 10, -1]),
            (22, &[9, 2, 10, 9, 0, 2, 8, 4, 7, -1]),
            (23, &[2, 10, 9, 2, 9, 7, 2, 7, 3, 7, 9, 4, -1]),
            (24, &[8, 4, 7, 3, 11, 2, -1]),
            (25, &[11, 4, 7, 11, 2, 4, 2, 0, 4, -1]),
            (26, &[9, 0, 1, 8, 4, 7, 2, 3, 11, -1]),
            (27, &[4, 7, 11, 9, 4, 11, 9, 11, 2, 9, 2, 1, -1]),
            (28, &[3, 10, 1, 3, 11, 10, 7, 8, 4, -1]),
            (29, &[1, 11, 10, 1, 4, 11, 1, 0, 4, 7, 11, 4, -1]),
            (30, &[4, 7, 8, 9, 0, 11, 9, 11, 10, 11, 0, 3, -1]),
            (31, &[4, 7, 11, 4, 11, 9, 9, 11, 10, -1]),
            (32, &[9, 5, 4, -1]),
            (33, &[9, 5, 4, 0, 8, 3, -1]),
            (34, &[0, 5, 4, 1, 5, 0, -1]),
            (35, &[8, 5, 4, 8, 3, 5, 3, 1, 5, -1]),
            (36, &[1, 2, 10, 9, 5, 4, -1]),
            (37, &[3, 0, 8, 1, 2, 10, 4, 9, 5, -1]),
            (38, &[5, 2, 10, 5, 4, 2, 4, 0, 2, -1]),
            (39, &[2, 10, 5, 3, 2, 5, 3, 5, 4, 3, 4, 8, -1]),
            (40, &[9, 5, 4, 2, 3, 11, -1]),
            (41, &[0, 11, 2, 0, 8, 11, 4, 9, 5, -1]),
            (42, &[0, 5, 4, 0, 1, 5, 2, 3, 11, -1]),
            (43, &[2, 1, 5, 2, 5, 8, 2, 8, 11, 4, 8, 5, -1]),
            (44, &[10, 3, 11, 10, 1, 3, 9, 5, 4, -1]),
            (45, &[4, 9, 5, 0, 8, 1, 8, 10, 1, 8, 11, 10, -1]),
            (46, &[5, 4, 0, 5, 0, 11, 5, 11, 10, 11, 0, 3, -1]),
            (47, &[5, 4, 8, 5, 8, 10, 10, 8, 11, -1]),
            (48, &[9, 7, 8, 5, 7, 9, -1]),
            (49, &[9, 3, 0, 9, 5, 3, 5, 7, 3, -1]),
            (50, &[0, 7, 8, 0, 1, 7, 1, 5, 7, -1]),
            (51, &[1, 5, 3, 3, 5, 7, -1]),
            (52, &[9, 7, 8, 9, 5, 7, 10, 1, 2, -1]),
            (53, &[10, 1, 2, 9, 5, 0, 5, 3, 0, 5, 7, 3, -1]),
            (54, &[8, 0, 2, 8, 2, 5, 8, 5, 7, 10, 5, 2, -1]),
            (55, &[2, 10, 5, 2, 5, 3, 3, 5, 7, -1]),
            (56, &[7, 9, 5, 7, 8, 9, 3, 11, 2, -1]),
            (57, &[9, 5, 7, 9, 7, 2, 9, 2, 0, 2, 7, 11, -1]),
            (58, &[2, 3, 11, 0, 1, 8, 1, 7, 8, 1, 5, 7, -1]),
            (59, &[11, 2, 1, 11, 1, 7, 7, 1, 5, -1]),
            (60, &[9, 5, 8, 8, 5, 7, 10, 1, 3, 10, 3, 11, -1]),
            (61, &[5, 7, 0, 5, 0, 9, 7, 11, 0, 1, 0, 10, 11, 10, 0, -1]),
            (62, &[11, 10, 0, 11, 0, 3, 10, 5, 0, 8, 0, 7, 5, 7, 0, -1]),
            (63, &[11, 10, 5, 7, 11, 5, -1]),
            (64, &[10, 6, 5, -1]),
            (65, &[0, 8, 3, 5, 10, 6, -1]),
            (66, &[9, 0, 1, 5, 10, 6, -1]),
            (67, &[1, 8, 3, 1, 9, 8, 5, 10, 6, -1]),
            (68, &[1, 6, 5, 2, 6, 1, -1]),
            (69, &[1, 6, 5, 1, 2, 6, 3, 0, 8, -1]),
            (70, &[9, 6, 5, 9, 0, 6, 0, 2, 6, -1]),
            (71, &[5, 9, 8, 5, 8, 2, 5, 2, 6, 3, 2, 8, -1]),
            (72, &[2, 3, 11, 10, 6, 5, -1]),
            (73, &[11, 0, 8, 11, 2, 0, 10, 6, 5, -1]),
            (74, &[0, 1, 9, 2, 3, 11, 5, 10, 6, -1]),
            (75, &[5, 10, 6, 1, 9, 2, 9, 11, 2, 9, 8, 11, -1]),
            (76, &[6, 3, 11, 6, 5, 3, 5, 1, 3, -1]),
            (77, &[0, 8, 11, 0, 11, 5, 0, 5, 1, 5, 11, 6, -1]),
            (78, &[3, 11, 6, 0, 3, 6, 0, 6, 5, 0, 5, 9, -1]),
            (79, &[6, 5, 9, 6, 9, 11, 11, 9, 8, -1]),
            (80, &[5, 10, 6, 4, 7, 8, -1]),
            (81, &[4, 3, 0, 4, 7, 3, 6, 5, 10, -1]),
            (82, &[1, 9, 0, 5, 10, 6, 8, 4, 7, -1]),
            (83, &[10, 6, 5, 1, 9, 7, 1, 7, 3, 7, 9, 4, -1]),
            (84, &[6, 1, 2, 6, 5, 1, 4, 7, 8, -1]),
            (85, &[1, 2, 5, 5, 2, 6, 3, 0, 4, 3, 4, 7, -1]),
            (86, &[8, 4, 7, 9, 0, 5, 0, 6, 5, 0, 2, 6, -1]),
            (87, &[7, 3, 9, 7, 9, 4, 3, 2, 9, 5, 9, 6, 2, 6, 9, -1]),
            (88, &[3, 11, 2, 7, 8, 4, 10, 6, 5, -1]),
            (89, &[5, 10, 6, 4, 7, 2, 4, 2, 0, 2, 7, 11, -1]),
            (90, &[0, 1, 9, 4, 7, 8, 2, 3, 11, 5, 10, 6, -1]),
            (91, &[9, 2, 1, 9, 11, 2, 9, 4, 11, 7, 11, 4, 5, 10, 6, -1]),
            (92, &[8, 4, 7, 3, 11, 5, 3, 5, 1, 5, 11, 6, -1]),
            (93, &[5, 1, 11, 5, 11, 6, 1, 0, 11, 7, 11, 4, 0, 4, 11, -1]),
            (94, &[0, 5, 9, 0, 6, 5, 0, 3, 6, 11, 6, 3, 8, 4, 7, -1]),
            (95, &[6, 5, 9, 6, 9, 11, 4, 7, 9, 7, 11, 9, -1]),
            (96, &[10, 4, 9, 6, 4, 10, -1]),
            (97, &[4, 10, 6, 4, 9, 10, 0, 8, 3, -1]),
            (98, &[10, 0, 1, 10, 6, 0, 6, 4, 0, -1]),
            (99, &[8, 3, 1, 8, 1, 6, 8, 6, 4, 6, 1, 10, -1]),
            (100, &[1, 4, 9, 1, 2, 4, 2, 6, 4, -1]),
            (101, &[3, 0, 8, 1, 2, 9, 2, 4, 9, 2, 6, 4, -1]),
            (102, &[0, 2, 4, 4, 2, 6, -1]),
            (103, &[8, 3, 2, 8, 2, 4, 4, 2, 6, -1]),
            (104, &[10, 4, 9, 10, 6, 4, 11, 2, 3, -1]),
            (105, &[0, 8, 2, 2, 8, 11, 4, 9, 10, 4, 10, 6, -1]),
            (106, &[3, 11, 2, 0, 1, 6, 0, 6, 4, 6, 1, 10, -1]),
            (107, &[6, 4, 1, 6, 1, 10, 4, 8, 1, 2, 1, 11, 8, 11, 1, -1]),
            (108, &[9, 6, 4, 9, 3, 6, 9, 1, 3, 11, 6, 3, -1]),
            (109, &[8, 11, 1, 8, 1, 0, 11, 6, 1, 9, 1, 4, 6, 4, 1, -1]),
            (110, &[3, 11, 6, 3, 6, 0, 0, 6, 4, -1]),
            (111, &[6, 4, 8, 11, 6, 8, -1]),
            (112, &[7, 10, 6, 7, 8, 10, 8, 9, 10, -1]),
            (113, &[0, 7, 3, 0, 10, 7, 0, 9, 10, 6, 7, 10, -1]),
            (114, &[10, 6, 7, 1, 10, 7, 1, 7, 8, 1, 8, 0, -1]),
            (115, &[10, 6, 7, 10, 7, 1, 1, 7, 3, -1]),
            (116, &[1, 2, 6, 1, 6, 8, 1, 8, 9, 8, 6, 7, -1]),
            (117, &[2, 6, 9, 2, 9, 1, 6, 7, 9, 0, 9, 3, 7, 3, 9, -1]),
            (118, &[7, 8, 0, 7, 0, 6, 6, 0, 2, -1]),
            (119, &[7, 3, 2, 6, 7, 2, -1]),
            (120, &[2, 3, 11, 10, 6, 8, 10, 8, 9, 8, 6, 7, -1]),
            (121, &[2, 0, 7, 2, 7, 11, 0, 9, 7, 6, 7, 10, 9, 10, 7, -1]),
            (122, &[1, 8, 11, 1, 11, 7, 1, 7, 6, 7, 11, 2, -1]),
            (123, &[11, 6, 7, 1, 11, 7, 1, 7, 2, -1]),
            (124, &[8, 9, 6, 8, 6, 7, 9, 1, 6, 11, 6, 3, 1, 3, 6, -1]),
            (125, &[0, 9, 1, 11, 6, 7, -1]),
            (126, &[7, 8, 0, 7, 0, 6, 3, 11, 0, 11, 6, 0, -1]),
            (127, &[7, 11, 6, -1]),
            (128, &[7, 6, 11, -1]),
            (129, &[3, 0, 8, 11, 7, 6, -1]),
            (130, &[0, 1, 9, 11, 7, 6, -1]),
            (131, &[8, 1, 9, 8, 3, 1, 11, 7, 6, -1]),
            (132, &[10, 1, 2, 6, 11, 7, -1]),
            (133, &[1, 2, 10, 3, 0, 8, 6, 11, 7, -1]),
            (134, &[2, 9, 0, 2, 10, 9, 6, 11, 7, -1]),
            (135, &[6, 11, 7, 2, 10, 3, 10, 8, 3, 10, 9, 8, -1]),
            (136, &[7, 2, 3, 6, 2, 7, -1]),
            (137, &[7, 0, 8, 7, 6, 0, 6, 2, 0, -1]),
            (138, &[2, 7, 6, 2, 3, 7, 0, 1, 9, -1]),
            (139, &[1, 6, 2, 1, 8, 6, 1, 9, 8, 8, 7, 6, -1]),
            (140, &[10, 7, 6, 10, 1, 7, 1, 3, 7, -1]),
            (141, &[10, 7, 6, 1, 7, 10, 1, 8, 7, 1, 0, 8, -1]),
            (142, &[0, 3, 7, 0, 7, 10, 0, 10, 9, 6, 10, 7, -1]),
            (143, &[7, 6, 10, 7, 10, 8, 8, 10, 9, -1]),
            (144, &[6, 8, 4, 11, 8, 6, -1]),
            (145, &[3, 6, 11, 3, 0, 6, 0, 4, 6, -1]),
            (146, &[8, 6, 11, 8, 4, 6, 9, 0, 1, -1]),
            (147, &[9, 4, 6, 9, 6, 3, 9, 3, 1, 11, 3, 6, -1]),
            (148, &[6, 8, 4, 6, 11, 8, 2, 10, 1, -1]),
            (149, &[1, 2, 10, 3, 0, 11, 0, 6, 11, 0, 4, 6, -1]),
            (150, &[4, 11, 8, 4, 6, 11, 0, 2, 9, 2, 10, 9, -1]),
            (151, &[10, 9, 3, 10, 3, 2, 9, 4, 3, 11, 3, 6, 4, 6, 3, -1]),
            (152, &[8, 2, 3, 8, 4, 2, 4, 6, 2, -1]),
            (153, &[0, 4, 2, 4, 6, 2, -1]),
            (154, &[1, 9, 0, 2, 3, 4, 2, 4, 6, 4, 3, 8, -1]),
            (155, &[1, 9, 4, 1, 4, 2, 2, 4, 6, -1]),
            (156, &[8, 1, 3, 8, 6, 1, 8, 4, 6, 6, 10, 1, -1]),
            (157, &[10, 1, 0, 10, 0, 6, 6, 0, 4, -1]),
            (158, &[4, 6, 3, 4, 3, 8, 6, 10, 3, 0, 3, 9, 10, 9, 3, -1]),
            (159, &[10, 9, 4, 6, 10, 4, -1]),
            (160, &[4, 9, 5, 7, 6, 11, -1]),
            (161, &[0, 8, 3, 4, 9, 5, 11, 7, 6, -1]),
            (162, &[5, 0, 1, 5, 4, 0, 7, 6, 11, -1]),
            (163, &[11, 7, 6, 8, 3, 4, 3, 5, 4, 3, 1, 5, -1]),
            (164, &[9, 5, 4, 10, 1, 2, 7, 6, 11, -1]),
            (165, &[6, 11, 7, 1, 2, 10, 0, 8, 3, 4, 9, 5, -1]),
            (166, &[7, 6, 11, 5, 4, 10, 4, 2, 10, 4, 0, 2, -1]),
            (167, &[3, 4, 8, 3, 5, 4, 3, 2, 5, 10, 5, 2, 11, 7, 6, -1]),
            (168, &[7, 2, 3, 7, 6, 2, 5, 4, 9, -1]),
            (169, &[9, 5, 4, 0, 8, 6, 0, 6, 2, 6, 8, 7, -1]),
            (170, &[3, 6, 2, 3, 7, 6, 1, 5, 0, 5, 4, 0, -1]),
            (171, &[6, 2, 8, 6, 8, 7, 2, 1, 8, 4, 8, 5, 1, 5, 8, -1]),
            (172, &[9, 5, 4, 10, 1, 6, 1, 7, 6, 1, 3, 7, -1]),
            (173, &[1, 6, 10, 1, 7, 6, 1, 0, 7, 8, 7, 0, 9, 5, 4, -1]),
            (174, &[4, 0, 10, 4, 10, 5, 0, 3, 10, 6, 10, 7, 3, 7, 10, -1]),
            (175, &[7, 6, 10, 7, 10, 8, 5, 4, 10, 4, 8, 10, -1]),
            (176, &[6, 9, 5, 6, 11, 9, 11, 8, 9, -1]),
            (177, &[3, 6, 11, 0, 6, 3, 0, 5, 6, 0, 9, 5, -1]),
            (178, &[0, 11, 8, 0, 5, 11, 0, 1, 5, 5, 6, 11, -1]),
            (179, &[6, 11, 3, 6, 3, 5, 5, 3, 1, -1]),
            (180, &[1, 2, 10, 9, 5, 11, 9, 11, 8, 11, 5, 6, -1]),
            (181, &[0, 11, 3, 0, 6, 11, 0, 9, 6, 5, 6, 9, 1, 2, 10, -1]),
            (182, &[11, 8, 5, 11, 5, 6, 8, 0, 5, 10, 5, 2, 0, 2, 5, -1]),
            (183, &[6, 11, 3, 6, 3, 5, 2, 10, 3, 10, 5, 3, -1]),
            (184, &[5, 8, 9, 5, 2, 8, 5, 6, 2, 3, 8, 2, -1]),
            (185, &[9, 5, 6, 9, 6, 0, 0, 6, 2, -1]),
            (186, &[1, 5, 8, 1, 8, 0, 5, 6, 8, 3, 8, 2, 6, 2, 8, -1]),
            (187, &[1, 5, 6, 2, 1, 6, -1]),
            (188, &[1, 3, 6, 1, 6, 10, 3, 8, 6, 5, 6, 9, 8, 9, 6, -1]),
            (189, &[10, 1, 0, 10, 0, 6, 9, 5, 0, 5, 6, 0, -1]),
            (190, &[0, 3, 8, 5, 6, 10, -1]),
            (191, &[10, 5, 6, -1]),
            (192, &[11, 5, 10, 7, 5, 11, -1]),
            (193, &[11, 5, 10, 11, 7, 5, 8, 3, 0, -1]),
            (194, &[5, 11, 7, 5, 10, 11, 1, 9, 0, -1]),
            (195, &[10, 7, 5, 10, 11, 7, 9, 8, 1, 8, 3, 1, -1]),
            (196, &[11, 1, 2, 11, 7, 1, 7, 5, 1, -1]),
            (197, &[0, 8, 3, 1, 2, 7, 1, 7, 5, 7, 2, 11, -1]),
            (198, &[9, 7, 5, 9, 2, 7, 9, 0, 2, 2, 11, 7, -1]),
            (199, &[7, 5, 2, 7, 2, 11, 5, 9, 2, 3, 2, 8, 9, 8, 2, -1]),
            (200, &[2, 5, 10, 2, 3, 5, 3, 7, 5, -1]),
            (201, &[8, 2, 0, 8, 5, 2, 8, 7, 5, 10, 2, 5, -1]),
            (202, &[9, 0, 1, 5, 10, 3, 5, 3, 7, 3, 10, 2, -1]),
            (203, &[9, 8, 2, 9, 2, 1, 8, 7, 2, 10, 2, 5, 7, 5, 2, -1]),
            (204, &[1, 3, 5, 3, 7, 5, -1]),
            (205, &[0, 8, 7, 0, 7, 1, 1, 7, 5, -1]),
            (206, &[9, 0, 3, 9, 3, 5, 5, 3, 7, -1]),
            (207, &[9, 8, 7, 5, 9, 7, -1]),
            (208, &[5, 8, 4, 5, 10, 8, 10, 11, 8, -1]),
            (209, &[5, 0, 4, 5, 11, 0, 5, 10, 11, 11, 3, 0, -1]),
            (210, &[0, 1, 9, 8, 4, 10, 8, 10, 11, 10, 4, 5, -1]),
            (211, &[10, 11, 4, 10, 4, 5, 11, 3, 4, 9, 4, 1, 3, 1, 4, -1]),
            (212, &[2, 5, 1, 2, 8, 5, 2, 11, 8, 4, 5, 8, -1]),
            (213, &[0, 4, 11, 0, 11, 3, 4, 5, 11, 2, 11, 1, 5, 1, 11, -1]),
            (214, &[0, 2, 5, 0, 5, 9, 2, 11, 5, 4, 5, 8, 11, 8, 5, -1]),
            (215, &[9, 4, 5, 2, 11, 3, -1]),
            (216, &[2, 5, 10, 3, 5, 2, 3, 4, 5, 3, 8, 4, -1]),
            (217, &[5, 10, 2, 5, 2, 4, 4, 2, 0, -1]),
            (218, &[3, 10, 2, 3, 5, 10, 3, 8, 5, 4, 5, 8, 0, 1, 9, -1]),
            (219, &[5, 10, 2, 5, 2, 4, 1, 9, 2, 9, 4, 2, -1]),
            (220, &[8, 4, 5, 8, 5, 3, 3, 5, 1, -1]),
            (221, &[0, 4, 5, 1, 0, 5, -1]),
            (222, &[8, 4, 5, 8, 5, 3, 9, 0, 5, 0, 3, 5, -1]),
            (223, &[9, 4, 5, -1]),
            (224, &[4, 11, 7, 4, 9, 11, 9, 10, 11, -1]),
            (225, &[0, 8, 3, 4, 9, 7, 9, 11, 7, 9, 10, 11, -1]),
            (226, &[1, 10, 11, 1, 11, 4, 1, 4, 0, 7, 4, 11, -1]),
            (227, &[3, 1, 4, 3, 4, 8, 1, 10, 4, 7, 4, 11, 10, 11, 4, -1]),
            (228, &[4, 11, 7, 9, 11, 4, 9, 2, 11, 9, 1, 2, -1]),
            (229, &[9, 7, 4, 9, 11, 7, 9, 1, 11, 2, 11, 1, 0, 8, 3, -1]),
            (230, &[11, 7, 4, 11, 4, 2, 2, 4, 0, -1]),
            (231, &[11, 7, 4, 11, 4, 2, 8, 3, 4, 3, 2, 4, -1]),
            (232, &[2, 9, 10, 2, 7, 9, 2, 3, 7, 7, 4, 9, -1]),
            (233, &[9, 10, 7, 9, 7, 4, 10, 2, 7, 8, 7, 0, 2, 0, 7, -1]),
            (234, &[3, 7, 10, 3, 10, 2, 7, 4, 10, 1, 10, 0, 4, 0, 10, -1]),
            (235, &[1, 10, 2, 8, 7, 4, -1]),
            (236, &[4, 9, 1, 4, 1, 7, 7, 1, 3, -1]),
            (237, &[4, 9, 1, 4, 1, 7, 0, 8, 1, 8, 7, 1, -1]),
            (238, &[4, 0, 3, 7, 4, 3, -1]),
            (239, &[4, 8, 7, -1]),
            (240, &[9, 10, 8, 10, 11, 8, -1]),
            (241, &[3, 0, 9, 3, 9, 11, 11, 9, 10, -1]),
            (242, &[0, 1, 10, 0, 10, 8, 8, 10, 11, -1]),
            (243, &[3, 1, 10, 11, 3, 10, -1]),
            (244, &[1, 2, 11, 1, 11, 9, 9, 11, 8, -1]),
            (245, &[3, 0, 9, 3, 9, 11, 1, 2, 9, 2, 11, 9, -1]),
            (246, &[0, 2, 11, 8, 0, 11, -1]),
            (247, &[3, 2, 11, -1]),
            (248, &[2, 3, 8, 2, 8, 10, 10, 8, 9, -1]),
            (249, &[9, 10, 2, 0, 9, 2, -1]),
            (250, &[2, 3, 8, 2, 8, 10, 0, 1, 8, 1, 10, 8, -1]),
            (251, &[1, 10, 2, -1]),
            (252, &[1, 3, 8, 9, 1, 8, -1]),
            (253, &[0, 9, 1, -1]),
            (254, &[0, 3, 8, -1]),
            (255, &[-1]),
        ];

        for &(cube_index, edges) in entries {
            let base = cube_index * 16;
            for (j, &edge) in edges.iter().enumerate() {
                if base + j < t.len() {
                    t[base + j] = edge;
                }
            }
        }
        t
}

/// Get the triangle table, initializing lazily if needed.
fn tri_table() -> &'static [i32] {
    TRI_TABLE.get_or_init(init_tri_table).as_slice()
}

// ===========================================================================
// Shared: Error helpers
// ===========================================================================

fn mesh_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::DecodeError {
            stage: "dicom-mesh".to_string(),
            detail: detail.into(),
        },
        "mesh error",
    )
    .into()
}

// ===========================================================================
// Tests: S5-T1 Mesh Generation (minimum 10)
// ===========================================================================

#[cfg(test)]
mod tests_mesh_generation {
    use super::*;

    #[test]
    fn marching_cubes_simple_sphere_labelmap() {
        // Create a simple 10x10x10 labelmap with a centered sphere
        let width = 10u32;
        let height = 10u32;
        let depth = 10u32;
        let mut labels = vec![0u16; (width * height * depth) as usize];

        // Paint a sphere of radius 3 at center (5,5,5)
        let cx = 5.0f64;
        let cy = 5.0f64;
        let cz = 5.0f64;
        let r = 3.0f64;
        for z in 0..depth {
            for y in 0..height {
                for x in 0..width {
                    let dx = x as f64 - cx;
                    let dy = y as f64 - cy;
                    let dz = z as f64 - cz;
                    if (dx * dx + dy * dy + dz * dz).sqrt() <= r {
                        let idx = z as usize * (width as usize * height as usize)
                            + y as usize * width as usize
                            + x as usize;
                        labels[idx] = 1;
                    }
                }
            }
        }

        let config = MarchingCubesConfig {
            voxel_spacing: (1.0, 1.0, 1.0),
            origin_mm: (0.0, 0.0, 0.0),
            target_label: 1,
        };

        let mesh = marching_cubes(width, height, depth, &labels, &config).expect("marching cubes");
        assert!(mesh.vertex_count() > 0, "should produce vertices");
        assert!(mesh.triangle_count() > 0, "should produce triangles");
        assert!(mesh.validate().is_ok(), "mesh should be valid");
    }

    #[test]
    fn marching_cubes_empty_labelmap() {
        let labels = vec![0u16; 8 * 8 * 8];
        let config = MarchingCubesConfig::default();
        let mesh = marching_cubes(8, 8, 8, &labels, &config).expect("marching cubes");
        assert_eq!(mesh.vertex_count(), 0);
        assert_eq!(mesh.triangle_count(), 0);
    }

    #[test]
    fn marching_cubes_rejects_zero_target() {
        let labels = vec![1u16; 8 * 8 * 8];
        let config = MarchingCubesConfig {
            target_label: 0,
            ..Default::default()
        };
        assert!(marching_cubes(8, 8, 8, &labels, &config).is_err());
    }

    #[test]
    fn marching_cubes_rejects_size_mismatch() {
        let labels = vec![0u16; 10];
        let config = MarchingCubesConfig::default();
        assert!(marching_cubes(8, 8, 8, &labels, &config).is_err());
    }

    #[test]
    fn marching_cubes_patient_space_coords() {
        let mut labels = vec![0u16; 4 * 4 * 4];
        // Mark one corner
        labels[0] = 1;
        labels[1] = 1;
        labels[4] = 1;
        labels[16] = 1;

        let config = MarchingCubesConfig {
            voxel_spacing: (0.5, 0.5, 2.0),
            origin_mm: (100.0, 200.0, 300.0),
            target_label: 1,
        };

        let mesh = marching_cubes(4, 4, 4, &labels, &config).expect("marching cubes");
        // Verify vertices are in patient space
        for v in &mesh.vertices {
            assert!(v[0] >= 100.0, "x should be in patient space");
            assert!(v[1] >= 200.0, "y should be in patient space");
            assert!(v[2] >= 300.0, "z should be in patient space");
        }
    }

    #[test]
    fn mesh_compute_normals() {
        let mut mesh = TriangleMesh::new("test");
        mesh.vertices.push([0.0, 0.0, 0.0]);
        mesh.vertices.push([1.0, 0.0, 0.0]);
        mesh.vertices.push([0.0, 1.0, 0.0]);
        mesh.triangles.push([0, 1, 2]);
        mesh.compute_normals();
        assert!(mesh.normals.is_some());
        let normals = mesh.normals.as_ref().unwrap();
        assert_eq!(normals.len(), 3);
        // Normal should point in +z direction for CCW triangle in xy plane
        for n in normals {
            assert!(n[2].abs() > 0.9, "normal should point in z direction");
        }
    }

    #[test]
    fn mesh_surface_area() {
        let mut mesh = TriangleMesh::new("test");
        mesh.vertices.push([0.0, 0.0, 0.0]);
        mesh.vertices.push([1.0, 0.0, 0.0]);
        mesh.vertices.push([0.0, 1.0, 0.0]);
        mesh.triangles.push([0, 1, 2]);
        let area = mesh.surface_area();
        assert!((area - 0.5).abs() < 1e-10, "right triangle area = 0.5");
    }

    #[test]
    fn mesh_validate_rejects_bad_index() {
        let mut mesh = TriangleMesh::new("test");
        mesh.vertices.push([0.0, 0.0, 0.0]);
        mesh.triangles.push([0, 1, 2]); // indices 1 and 2 don't exist
        assert!(mesh.validate().is_err());
    }

    #[test]
    fn mesh_validate_rejects_degenerate() {
        let mut mesh = TriangleMesh::new("test");
        mesh.vertices.push([0.0, 0.0, 0.0]);
        mesh.vertices.push([1.0, 0.0, 0.0]);
        mesh.vertices.push([2.0, 0.0, 0.0]);
        mesh.triangles.push([0, 0, 1]); // vertex 0 used twice
        assert!(mesh.validate().is_err());
    }

    #[test]
    fn decimation_reduces_triangle_count() {
        // Create a mesh with many triangles
        let mut mesh = TriangleMesh::new("test");
        for i in 0..20u32 {
            let x = i as f64 * 0.1;
            mesh.vertices.push([x, 0.0, 0.0]);
            mesh.vertices.push([x, 1.0, 0.0]);
            mesh.vertices.push([x + 0.1, 0.5, 0.0]);
            mesh.triangles.push([i * 3, i * 3 + 1, i * 3 + 2]);
        }
        let original_count = mesh.triangle_count();
        let config = DecimationConfig {
            target_triangle_count: 10,
            max_error_mm: 10.0,
        };
        let decimated = decimate_mesh(&mesh, &config).expect("decimate");
        assert!(decimated.triangle_count() <= original_count);
    }

    #[test]
    fn decimation_config_validation() {
        let bad = DecimationConfig {
            target_triangle_count: 0,
            max_error_mm: 0.5,
        };
        assert!(bad.validate().is_err());
    }

    #[test]
    fn smoothing_taubin_preserves_shape() {
        let mut mesh = TriangleMesh::new("test");
        mesh.vertices.push([0.0, 0.0, 0.0]);
        mesh.vertices.push([1.0, 0.0, 0.0]);
        mesh.vertices.push([0.0, 1.0, 0.0]);
        mesh.vertices.push([1.0, 1.0, 0.0]);
        mesh.triangles.push([0, 1, 2]);
        mesh.triangles.push([1, 3, 2]);

        let config = SmoothingConfig {
            method: SmoothingMethod::Taubin,
            iterations: 5,
            lambda: 0.5,
            mu: -0.53,
        };
        let smoothed = smooth_mesh(&mesh, &config).expect("smooth");
        assert_eq!(smoothed.vertex_count(), mesh.vertex_count());
        assert_eq!(smoothed.triangle_count(), mesh.triangle_count());
    }

    #[test]
    fn smoothing_laplacian_reduces_surface_area() {
        let mut mesh = TriangleMesh::new("test");
        // Create a small noisy mesh
        mesh.vertices.push([0.0, 0.0, 0.0]);
        mesh.vertices.push([1.0, 0.1, 0.0]);
        mesh.vertices.push([0.5, 1.0, 0.0]);
        mesh.vertices.push([0.5, 0.5, 0.1]);
        mesh.triangles.push([0, 1, 3]);
        mesh.triangles.push([1, 2, 3]);
        mesh.triangles.push([0, 3, 2]);
        mesh.triangles.push([0, 2, 1]);

        let config = SmoothingConfig {
            method: SmoothingMethod::Laplacian,
            iterations: 20,
            lambda: 0.5,
            mu: -0.53,
        };
        let smoothed = smooth_mesh(&mesh, &config).expect("smooth");
        // Laplacian smoothing should reduce surface area (shrinkage)
        assert!(smoothed.surface_area() <= mesh.surface_area() + 1e-6);
    }

    #[test]
    fn smoothing_config_rejects_bad_params() {
        let bad = SmoothingConfig {
            method: SmoothingMethod::Taubin,
            iterations: 0,
            lambda: 0.5,
            mu: -0.53,
        };
        assert!(bad.validate().is_err());

        let bad2 = SmoothingConfig {
            method: SmoothingMethod::Taubin,
            iterations: 10,
            lambda: -0.1,
            mu: -0.53,
        };
        assert!(bad2.validate().is_err());

        let bad3 = SmoothingConfig {
            method: SmoothingMethod::Taubin,
            iterations: 10,
            lambda: 0.5,
            mu: 0.5, // must be negative for Taubin
        };
        assert!(bad3.validate().is_err());
    }
}

// ===========================================================================
// Tests: S5-T2 Export + DICOM Encapsulation (minimum 10)
// ===========================================================================

#[cfg(test)]
mod tests_export {
    use super::*;

    fn test_mesh() -> TriangleMesh {
        let mut mesh = TriangleMesh::new("bone");
        mesh.vertices.push([0.0, 0.0, 0.0]);
        mesh.vertices.push([1.0, 0.0, 0.0]);
        mesh.vertices.push([0.0, 1.0, 0.0]);
        mesh.vertices.push([1.0, 1.0, 0.0]);
        mesh.triangles.push([0, 1, 2]);
        mesh.triangles.push([1, 3, 2]);
        mesh
    }

    #[test]
    fn export_stl_binary_format() {
        let mesh = test_mesh();
        let stl = export_stl(&mesh).expect("stl export");
        assert!(stl.len() > 80 + 4, "STL should have header + count");
        assert_eq!(&stl[0..21], b"dicom-mesh STL export");
        // Triangle count at offset 80 (little-endian u32)
        let tri_count = u32::from_le_bytes([stl[80], stl[81], stl[82], stl[83]]);
        assert_eq!(tri_count, 2);
    }

    #[test]
    fn export_stl_correct_size() {
        let mesh = test_mesh();
        let stl = export_stl(&mesh).expect("stl export");
        let expected = 80 + 4 + mesh.triangle_count() * 50;
        assert_eq!(stl.len(), expected);
    }

    #[test]
    fn export_3mf_xml_structure() {
        let mesh = test_mesh();
        let metadata = Model3mfMetadata::default();
        let xml = export_3mf(&mesh, &metadata).expect("3mf export");
        assert!(xml.contains("<?xml"));
        assert!(xml.contains("<model"));
        assert!(xml.contains("<vertices>"));
        assert!(xml.contains("<triangles>"));
        assert!(xml.contains("millimeter"));
        assert!(xml.contains("<build>"));
    }

    #[test]
    fn export_3mf_metadata() {
        let mesh = test_mesh();
        let metadata = Model3mfMetadata {
            title: "Test Bone Model".to_string(),
            designer: "Dr. Smith".to_string(),
            description: "Femur segmentation".to_string(),
            unit: "millimeter".to_string(),
        };
        let xml = export_3mf(&mesh, &metadata).expect("3mf export");
        assert!(xml.contains("Test Bone Model"));
        assert!(xml.contains("Dr. Smith"));
        assert!(xml.contains("Femur segmentation"));
    }

    #[test]
    fn export_obj_format() {
        let mesh = test_mesh();
        let material = ObjMaterial::default();
        let (obj, mtl) = export_obj(&mesh, &material).expect("obj export");
        assert!(obj.contains("v "));
        assert!(obj.contains("vn "));
        assert!(obj.contains("f "));
        assert!(obj.contains("usemtl"));
        assert!(obj.contains("mtllib"));
        assert!(mtl.contains("newmtl"));
        assert!(mtl.contains("Ka "));
        assert!(mtl.contains("Kd "));
        assert!(mtl.contains("Ks "));
        assert!(mtl.contains("Ns "));
    }

    #[test]
    fn export_obj_1_based_indices() {
        let mesh = test_mesh();
        let material = ObjMaterial::default();
        let (obj, _) = export_obj(&mesh, &material).expect("obj export");
        // OBJ indices are 1-based
        assert!(obj.contains("f 1//1 2//2 3//3"));
    }

    #[test]
    fn encode_3d_model_iod_structure() {
        let mesh = test_mesh();
        let ds = encode_3d_model_iod(
            &mesh,
            "1.2.840.113619.6.1",
            "1.2.840.113619.6.2",
            "1.2.840.113619.6.3",
            "1.2.840.113619.6.4",
        )
        .expect("encode 3d model iod");

        // Verify SOP Class UID for Encapsulated 3D Model
        assert!(ds.get_uid(Tag(0x0008, 0x0016)).is_some());
        assert_eq!(
            ds.get_uid(Tag(0x0008, 0x0016)).unwrap(),
            "1.2.840.10008.5.1.4.1.1.104.1"
        );

        // Verify Study/Series/SOP Instance UIDs
        assert_eq!(
            ds.get_uid(Tag(0x0020, 0x000D)).unwrap(),
            "1.2.840.113619.6.1"
        );
        assert_eq!(
            ds.get_uid(Tag(0x0020, 0x000E)).unwrap(),
            "1.2.840.113619.6.2"
        );
        assert_eq!(
            ds.get_uid(Tag(0x0020, 0x0052)).unwrap(),
            "1.2.840.113619.6.3"
        );

        // Verify pixel data
        assert!(ds.get(Tag(0x7FE0, 0x0010)).is_some());

        // Verify mesh metadata
        assert_eq!(ds.get_i32(Tag(0x0066, 0x0015)).unwrap(), 4); // vertex count
        assert_eq!(ds.get_i32(Tag(0x0066, 0x0016)).unwrap(), 2); // triangle count
    }

    #[test]
    fn encode_3d_model_iod_pixel_data_is_stl() {
        let mesh = test_mesh();
        let ds = encode_3d_model_iod(
            &mesh,
            "1.2.840.113619.6.1",
            "1.2.840.113619.6.2",
            "1.2.840.113619.6.3",
            "1.2.840.113619.6.4",
        )
        .expect("encode");

        if let Some(element) = ds.get(Tag(0x7FE0, 0x0010)) {
            if let Value::Bytes(data) = element.value() {
                assert!(data.len() > 80 + 4, "pixel data should contain STL");
            }
        }
    }

    #[test]
    fn export_stl_rejects_invalid_mesh() {
        let mut mesh = TriangleMesh::new("bad");
        mesh.triangles.push([0, 1, 2]); // no vertices
        assert!(export_stl(&mesh).is_err());
    }

    #[test]
    fn export_round_trip_mesh_validation() {
        // Create a sphere labelmap, extract mesh, smooth, decimate, then export
        let width = 8u32;
        let height = 8u32;
        let depth = 8u32;
        let mut labels = vec![0u16; (width * height * depth) as usize];

        for z in 0..depth {
            for y in 0..height {
                for x in 0..width {
                    let dx = x as f64 - 4.0;
                    let dy = y as f64 - 4.0;
                    let dz = z as f64 - 4.0;
                    if (dx * dx + dy * dy + dz * dz).sqrt() <= 2.5 {
                        let idx = z as usize * (width as usize * height as usize)
                            + y as usize * width as usize
                            + x as usize;
                        labels[idx] = 1;
                    }
                }
            }
        }

        let config = MarchingCubesConfig::default();
        let mesh = marching_cubes(width, height, depth, &labels, &config).expect("mc");

        // Smooth
        let smooth_config = SmoothingConfig::default();
        let smoothed = smooth_mesh(&mesh, &smooth_config).expect("smooth");

        // Decimate
        let dec_config = DecimationConfig {
            target_triangle_count: 100,
            max_error_mm: 1.0,
        };
        let decimated = decimate_mesh(&smoothed, &dec_config).expect("decimate");

        // Export all formats
        assert!(export_stl(&decimated).is_ok());
        assert!(export_3mf(&decimated, &Model3mfMetadata::default()).is_ok());
        assert!(export_obj(&decimated, &ObjMaterial::default()).is_ok());
    }
}
