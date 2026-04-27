//! Deterministic volume assembly primitives.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Deterministic volume assembly configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VolumeAssemblyConfig {
    /// Allow non-uniform slice spacing.
    pub allow_non_uniform_spacing: bool,
}

/// Backward-compatible alias for [`VolumeAssemblyConfig`].
#[deprecated(since = "0.14.0", note = "Use VolumeAssemblyConfig instead")]
pub type VolumeAssemblyOptions = VolumeAssemblyConfig;

/// A source slice used to assemble a volume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlicePlane {
    /// SOP Instance UID (used as stable tie-breaker).
    pub instance_uid: String,
    /// Slice position along normal in patient space.
    pub position_mm: i64,
    /// Slice spacing to previous slice in micrometers.
    pub spacing_um: u64,
    /// Slice width in pixels.
    pub width: usize,
    /// Slice height in pixels.
    pub height: usize,
    /// Pixel values in row-major order after modality transform.
    pub pixels: Vec<i32>,
}

/// Volume assembly failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VolumeError {
    /// No slices were provided.
    EmptyInput,
    /// Slice dimensions were inconsistent.
    InconsistentDimensions,
    /// Slice pixel buffer length was invalid.
    InvalidSlicePixels,
    /// Non-uniform spacing was detected and not allowed.
    NonUniformSpacing,
    /// Requested dimensions overflowed allocation limits.
    SizeOverflow,
}

impl VolumeError {
    /// Stable error code for API surfaces exposing volume assembly failures.
    pub fn code(&self) -> &'static str {
        match self {
            VolumeError::EmptyInput => "DVF.VOLUME.EMPTY_INPUT",
            VolumeError::InconsistentDimensions => "DVF.VOLUME.INCONSISTENT_DIMENSIONS",
            VolumeError::InvalidSlicePixels => "DVF.VOLUME.INVALID_SLICE_PIXELS",
            VolumeError::NonUniformSpacing => "DVF.VOLUME.NON_UNIFORM_SPACING",
            VolumeError::SizeOverflow => "DVF.VOLUME.SIZE_OVERFLOW",
        }
    }
}

impl fmt::Display for VolumeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VolumeError::EmptyInput => write!(f, "volume assembly requires at least one slice"),
            VolumeError::InconsistentDimensions => {
                write!(f, "slice dimensions must be identical for volume assembly")
            }
            VolumeError::InvalidSlicePixels => {
                write!(f, "slice pixel buffer length does not match dimensions")
            }
            VolumeError::NonUniformSpacing => {
                write!(
                    f,
                    "non-uniform spacing is not enabled for this volume assembly"
                )
            }
            VolumeError::SizeOverflow => write!(f, "volume dimensions exceed allocation limits"),
        }
    }
}

impl std::error::Error for VolumeError {}

/// Optional patient-space metadata attached to a `VolumeGrid`.
///
/// Geometry is represented as integer micrometer vectors so mapping remains
/// deterministic and serializable across targets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatientGeometry {
    /// Patient-space origin in micrometers.
    pub origin_um: [i64; 3],
    /// Patient-space displacement vectors per voxel axis `[x, y, z]` in micrometers.
    pub basis_vectors_um: [[i64; 3]; 3],
    /// Optional frame-of-reference UID.
    pub frame_of_reference_uid: Option<String>,
}

impl PatientGeometry {
    fn from_volume_axes(
        spacing_um: [u64; 3],
        orientation_ras: [[i8; 3]; 3],
        frame_of_reference_uid: Option<String>,
    ) -> Self {
        let mut basis_vectors_um = [[0i64; 3]; 3];
        for axis in 0..3 {
            let step = spacing_um[axis] as i64;
            for component in 0..3 {
                basis_vectors_um[axis][component] = orientation_ras[axis][component] as i64 * step;
            }
        }
        Self {
            origin_um: [0, 0, 0],
            basis_vectors_um,
            frame_of_reference_uid,
        }
    }
}

/// Legacy serialized shape for `VolumeGrid` before patient metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyVolumeGrid {
    /// Volume dimensions as `[x, y, z]`.
    pub dimensions: [usize; 3],
    /// Voxel spacing as `[x, y, z]` in micrometers.
    pub spacing_um: [u64; 3],
    /// Orientation basis in RAS axes.
    pub orientation_ras: [[i8; 3]; 3],
    /// Source SOP Instance UIDs.
    pub source_instance_uids: Vec<String>,
    /// Voxel values in deterministic row-major order.
    pub voxels: Vec<i32>,
    /// Non-uniform spacing indicator.
    pub non_uniform_spacing: bool,
}

/// Deterministic voxel volume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeGrid {
    dimensions: [usize; 3],
    spacing_um: [u64; 3],
    orientation_ras: [[i8; 3]; 3],
    source_instance_uids: Vec<String>,
    voxels: Vec<i32>,
    non_uniform_spacing: bool,
    #[serde(default)]
    patient_geometry: Option<PatientGeometry>,
}

impl VolumeGrid {
    /// Create a zero-initialized volume with explicit dimensions/spacing.
    pub fn new(dimensions: [usize; 3], spacing_um: [u64; 3]) -> Result<Self, VolumeError> {
        let voxel_count = dimensions[0]
            .checked_mul(dimensions[1])
            .and_then(|value| value.checked_mul(dimensions[2]))
            .ok_or(VolumeError::SizeOverflow)?;
        Ok(Self {
            dimensions,
            spacing_um,
            orientation_ras: [[1, 0, 0], [0, 1, 0], [0, 0, 1]],
            source_instance_uids: Vec::new(),
            voxels: vec![0; voxel_count],
            non_uniform_spacing: false,
            patient_geometry: None,
        })
    }

    /// Assemble a deterministic volume from ordered slices.
    ///
    /// Slices are sorted by `(position_mm, instance_uid)` and then stacked as `z`.
    pub fn from_slices(
        slices: &[SlicePlane],
        options: VolumeAssemblyConfig,
    ) -> Result<Self, VolumeError> {
        // Stage 1+2: validate input and build deterministic ordering plan.
        let plan = plan_volume_assembly(slices, options)?;
        // Stage 3: allocate and assemble deterministic voxel buffer.
        let mut grid = Self::new(
            [plan.width, plan.height, plan.depth],
            [
                1_000, // fallback in-plane spacing baseline for synthetic assembly
                1_000,
                plan.spacing_um.max(1),
            ],
        )?;
        grid.non_uniform_spacing = plan.non_uniform_spacing;
        grid.source_instance_uids = plan
            .ordered
            .iter()
            .map(|slice| slice.instance_uid.clone())
            .collect();
        populate_voxels_from_ordered_slices(&mut grid, &plan.ordered);
        // Stage 4: deterministic cache-key derivation hook for higher-level cache layers.
        let _cache_key = volume_cache_key_for_plan(&plan);
        Ok(grid)
    }

    /// Return dimensions as `[x, y, z]`.
    pub fn dimensions(&self) -> [usize; 3] {
        self.dimensions
    }

    /// Return voxel spacing in micrometers as `[x, y, z]`.
    pub fn spacing_um(&self) -> [u64; 3] {
        self.spacing_um
    }

    /// Return orientation basis in RAS axes.
    pub fn orientation_ras(&self) -> [[i8; 3]; 3] {
        self.orientation_ras
    }

    /// Return true when assembled from non-uniform spacing input.
    pub fn non_uniform_spacing(&self) -> bool {
        self.non_uniform_spacing
    }

    /// Return source SOP Instance UIDs in deterministic assembly order.
    pub fn source_instance_uids(&self) -> &[String] {
        &self.source_instance_uids
    }

    /// Return voxel buffer in deterministic `z/y/x` row-major layout.
    pub fn voxels(&self) -> &[i32] {
        &self.voxels
    }

    /// Return patient-space metadata when available.
    pub fn patient_geometry(&self) -> Option<&PatientGeometry> {
        self.patient_geometry.as_ref()
    }

    /// Attach or replace patient-space metadata.
    pub fn set_patient_geometry(&mut self, geometry: PatientGeometry) {
        self.patient_geometry = Some(geometry);
    }

    /// Attach deterministic default patient-space metadata from legacy fields.
    pub fn migrate_patient_geometry_from_legacy(&mut self, frame_of_reference_uid: Option<String>) {
        self.patient_geometry = Some(PatientGeometry::from_volume_axes(
            self.spacing_um,
            self.orientation_ras,
            frame_of_reference_uid,
        ));
    }

    /// Return a patient-space-capable copy from a legacy volume instance.
    pub fn migrated_from_legacy(
        legacy: LegacyVolumeGrid,
        frame_of_reference_uid: Option<String>,
    ) -> Self {
        let mut volume = Self::from(legacy);
        volume.migrate_patient_geometry_from_legacy(frame_of_reference_uid);
        volume
    }

    /// Return one voxel value if coordinates are in range.
    pub fn voxel(&self, x: usize, y: usize, z: usize) -> Option<i32> {
        let [dx, dy, dz] = self.dimensions;
        if x >= dx || y >= dy || z >= dz {
            return None;
        }
        let stride_xy = dx * dy;
        let index = z * stride_xy + y * dx + x;
        self.voxels.get(index).copied()
    }

    /// Set one voxel value if coordinates are in range.
    pub fn set_voxel(&mut self, x: usize, y: usize, z: usize, value: i32) -> bool {
        let [dx, dy, dz] = self.dimensions;
        if x >= dx || y >= dy || z >= dz {
            return false;
        }
        let stride_xy = dx * dy;
        let index = z * stride_xy + y * dx + x;
        if let Some(slot) = self.voxels.get_mut(index) {
            *slot = value;
            return true;
        }
        false
    }

    /// Convert voxel coordinates to patient-space micrometers when geometry exists.
    pub fn voxel_to_patient_um(&self, x: usize, y: usize, z: usize) -> Option<[i64; 3]> {
        let geometry = self.patient_geometry.as_ref()?;
        let [dx, dy, dz] = self.dimensions;
        if x >= dx || y >= dy || z >= dz {
            return None;
        }
        let coords = [x as i64, y as i64, z as i64];
        let mut out = geometry.origin_um;
        for (axis, coordinate) in coords.iter().enumerate() {
            for (component, component_out) in out.iter_mut().enumerate() {
                let delta = geometry.basis_vectors_um[axis][component].checked_mul(*coordinate)?;
                *component_out = component_out.checked_add(delta)?;
            }
        }
        Some(out)
    }

    /// Convert patient-space micrometers to voxel-space floating coordinates.
    pub fn patient_to_voxel_f64(&self, patient_um: [f64; 3]) -> Option<[f64; 3]> {
        let geometry = self.patient_geometry.as_ref()?;
        let matrix = basis_to_matrix(geometry.basis_vectors_um);
        let inverse = invert_3x3(matrix)?;
        let rhs = [
            patient_um[0] - geometry.origin_um[0] as f64,
            patient_um[1] - geometry.origin_um[1] as f64,
            patient_um[2] - geometry.origin_um[2] as f64,
        ];
        Some(mul_3x3_vec(inverse, rhs))
    }

    #[cfg(test)]
    pub(crate) fn voxels_mut_for_test(&mut self) -> &mut [i32] {
        &mut self.voxels
    }
}

#[derive(Debug, Clone)]
struct VolumeAssemblyPlan {
    ordered: Vec<SlicePlane>,
    width: usize,
    height: usize,
    depth: usize,
    spacing_um: u64,
    non_uniform_spacing: bool,
}

fn plan_volume_assembly(
    slices: &[SlicePlane],
    options: VolumeAssemblyConfig,
) -> Result<VolumeAssemblyPlan, VolumeError> {
    if slices.is_empty() {
        return Err(VolumeError::EmptyInput);
    }
    let mut ordered = slices.to_vec();
    ordered.sort_by(|lhs, rhs| {
        lhs.position_mm
            .cmp(&rhs.position_mm)
            .then(lhs.instance_uid.cmp(&rhs.instance_uid))
    });
    let width = ordered[0].width;
    let height = ordered[0].height;
    let expected_pixels = width.checked_mul(height).ok_or(VolumeError::SizeOverflow)?;
    for slice in &ordered {
        if slice.width != width || slice.height != height {
            return Err(VolumeError::InconsistentDimensions);
        }
        if slice.pixels.len() != expected_pixels {
            return Err(VolumeError::InvalidSlicePixels);
        }
    }

    let mut non_uniform_spacing = false;
    if ordered.len() > 1 {
        let baseline = ordered[0].spacing_um;
        for slice in &ordered[1..] {
            if slice.spacing_um != baseline {
                non_uniform_spacing = true;
                break;
            }
        }
        if non_uniform_spacing && !options.allow_non_uniform_spacing {
            return Err(VolumeError::NonUniformSpacing);
        }
    }
    let depth = ordered.len();
    Ok(VolumeAssemblyPlan {
        spacing_um: ordered[0].spacing_um,
        ordered,
        width,
        height,
        depth,
        non_uniform_spacing,
    })
}

fn populate_voxels_from_ordered_slices(grid: &mut VolumeGrid, ordered: &[SlicePlane]) {
    let mut cursor = 0usize;
    for slice in ordered {
        for value in &slice.pixels {
            grid.voxels[cursor] = *value;
            cursor += 1;
        }
    }
}

fn volume_cache_key_for_plan(plan: &VolumeAssemblyPlan) -> String {
    let mut key = format!(
        "{}x{}x{}:{}:{}",
        plan.width, plan.height, plan.depth, plan.spacing_um, plan.non_uniform_spacing
    );
    for slice in &plan.ordered {
        key.push('|');
        key.push_str(&slice.instance_uid);
    }
    key
}

impl From<LegacyVolumeGrid> for VolumeGrid {
    fn from(value: LegacyVolumeGrid) -> Self {
        Self {
            dimensions: value.dimensions,
            spacing_um: value.spacing_um,
            orientation_ras: value.orientation_ras,
            source_instance_uids: value.source_instance_uids,
            voxels: value.voxels,
            non_uniform_spacing: value.non_uniform_spacing,
            patient_geometry: None,
        }
    }
}

impl From<&VolumeGrid> for LegacyVolumeGrid {
    fn from(value: &VolumeGrid) -> Self {
        Self {
            dimensions: value.dimensions,
            spacing_um: value.spacing_um,
            orientation_ras: value.orientation_ras,
            source_instance_uids: value.source_instance_uids.clone(),
            voxels: value.voxels.clone(),
            non_uniform_spacing: value.non_uniform_spacing,
        }
    }
}

fn basis_to_matrix(basis_vectors_um: [[i64; 3]; 3]) -> [[f64; 3]; 3] {
    [
        [
            basis_vectors_um[0][0] as f64,
            basis_vectors_um[1][0] as f64,
            basis_vectors_um[2][0] as f64,
        ],
        [
            basis_vectors_um[0][1] as f64,
            basis_vectors_um[1][1] as f64,
            basis_vectors_um[2][1] as f64,
        ],
        [
            basis_vectors_um[0][2] as f64,
            basis_vectors_um[1][2] as f64,
            basis_vectors_um[2][2] as f64,
        ],
    ]
}

fn determinant_3x3(matrix: [[f64; 3]; 3]) -> f64 {
    matrix[0][0] * (matrix[1][1] * matrix[2][2] - matrix[1][2] * matrix[2][1])
        - matrix[0][1] * (matrix[1][0] * matrix[2][2] - matrix[1][2] * matrix[2][0])
        + matrix[0][2] * (matrix[1][0] * matrix[2][1] - matrix[1][1] * matrix[2][0])
}

fn invert_3x3(matrix: [[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let det = determinant_3x3(matrix);
    if !det.is_finite() || det.abs() < f64::EPSILON {
        return None;
    }
    let inv_det = 1.0 / det;
    Some([
        [
            (matrix[1][1] * matrix[2][2] - matrix[1][2] * matrix[2][1]) * inv_det,
            (matrix[0][2] * matrix[2][1] - matrix[0][1] * matrix[2][2]) * inv_det,
            (matrix[0][1] * matrix[1][2] - matrix[0][2] * matrix[1][1]) * inv_det,
        ],
        [
            (matrix[1][2] * matrix[2][0] - matrix[1][0] * matrix[2][2]) * inv_det,
            (matrix[0][0] * matrix[2][2] - matrix[0][2] * matrix[2][0]) * inv_det,
            (matrix[0][2] * matrix[1][0] - matrix[0][0] * matrix[1][2]) * inv_det,
        ],
        [
            (matrix[1][0] * matrix[2][1] - matrix[1][1] * matrix[2][0]) * inv_det,
            (matrix[0][1] * matrix[2][0] - matrix[0][0] * matrix[2][1]) * inv_det,
            (matrix[0][0] * matrix[1][1] - matrix[0][1] * matrix[1][0]) * inv_det,
        ],
    ])
}

fn mul_3x3_vec(matrix: [[f64; 3]; 3], rhs: [f64; 3]) -> [f64; 3] {
    [
        matrix[0][0] * rhs[0] + matrix[0][1] * rhs[1] + matrix[0][2] * rhs[2],
        matrix[1][0] * rhs[0] + matrix[1][1] * rhs[1] + matrix[1][2] * rhs[2],
        matrix[2][0] * rhs[0] + matrix[2][1] * rhs[1] + matrix[2][2] * rhs[2],
    ]
}

#[cfg(test)]
mod tests {
    use super::{LegacyVolumeGrid, SlicePlane, VolumeAssemblyConfig, VolumeError, VolumeGrid};

    fn slice(uid: &str, position_mm: i64, spacing_um: u64, pixels: [i32; 4]) -> SlicePlane {
        SlicePlane {
            instance_uid: uid.to_string(),
            position_mm,
            spacing_um,
            width: 2,
            height: 2,
            pixels: pixels.to_vec(),
        }
    }

    #[test]
    fn deterministic_slice_ordering_uses_position_then_uid() {
        let slices = vec![
            slice("1.2.3.b", 10, 1_000, [5, 6, 7, 8]),
            slice("1.2.3.a", 10, 1_000, [1, 2, 3, 4]),
            slice("1.2.3.c", 20, 1_000, [9, 10, 11, 12]),
        ];
        let volume =
            VolumeGrid::from_slices(&slices, VolumeAssemblyConfig::default()).expect("volume");
        assert_eq!(
            volume.source_instance_uids(),
            &[
                "1.2.3.a".to_string(),
                "1.2.3.b".to_string(),
                "1.2.3.c".to_string()
            ]
        );
        assert_eq!(volume.voxel(0, 0, 0), Some(1));
        assert_eq!(volume.voxel(1, 1, 2), Some(12));
    }

    #[test]
    fn non_uniform_spacing_fails_closed_by_default() {
        let slices = vec![
            slice("1.2.3.a", 0, 1_000, [1, 2, 3, 4]),
            slice("1.2.3.b", 1, 1_500, [5, 6, 7, 8]),
        ];
        let err = VolumeGrid::from_slices(&slices, VolumeAssemblyConfig::default())
            .expect_err("must fail");
        assert_eq!(err, VolumeError::NonUniformSpacing);
        assert_eq!(err.code(), "DVF.VOLUME.NON_UNIFORM_SPACING");
    }

    #[test]
    fn legacy_migration_adapter_adds_patient_geometry() {
        let legacy = LegacyVolumeGrid {
            dimensions: [2, 2, 1],
            spacing_um: [1000, 2000, 3000],
            orientation_ras: [[1, 0, 0], [0, 1, 0], [0, 0, 1]],
            source_instance_uids: vec!["1.2.3".to_string()],
            voxels: vec![1, 2, 3, 4],
            non_uniform_spacing: false,
        };
        let migrated = VolumeGrid::migrated_from_legacy(legacy, Some("1.2.3.4".to_string()));
        let geometry = migrated.patient_geometry().expect("patient geometry");
        assert_eq!(geometry.origin_um, [0, 0, 0]);
        assert_eq!(geometry.basis_vectors_um[0], [1000, 0, 0]);
        assert_eq!(geometry.basis_vectors_um[1], [0, 2000, 0]);
        assert_eq!(geometry.basis_vectors_um[2], [0, 0, 3000]);
    }

    #[test]
    fn voxel_to_patient_mapping_is_deterministic() {
        let mut volume = VolumeGrid::new([3, 3, 3], [1000, 2000, 3000]).expect("volume");
        volume.migrate_patient_geometry_from_legacy(None);
        assert_eq!(volume.voxel_to_patient_um(0, 0, 0), Some([0, 0, 0]));
        assert_eq!(
            volume.voxel_to_patient_um(2, 1, 1),
            Some([2000, 2000, 3000])
        );
        let voxel = volume
            .patient_to_voxel_f64([2000.0, 2000.0, 3000.0])
            .expect("patient->voxel");
        assert!((voxel[0] - 2.0).abs() < 1e-9);
        assert!((voxel[1] - 1.0).abs() < 1e-9);
        assert!((voxel[2] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn serialization_compatibility_across_schema_versions() {
        let legacy = LegacyVolumeGrid {
            dimensions: [2, 2, 1],
            spacing_um: [1000, 1000, 1000],
            orientation_ras: [[1, 0, 0], [0, 1, 0], [0, 0, 1]],
            source_instance_uids: vec!["1.2.3".to_string()],
            voxels: vec![10, 20, 30, 40],
            non_uniform_spacing: false,
        };
        let legacy_json = serde_json::to_string(&legacy).expect("serialize legacy");
        let upgraded: VolumeGrid = serde_json::from_str(&legacy_json).expect("upgrade deserialize");
        assert!(upgraded.patient_geometry().is_none());

        let migrated = VolumeGrid::migrated_from_legacy(legacy.clone(), None);
        let current_json = serde_json::to_string(&migrated).expect("serialize current");
        let roundtrip: VolumeGrid = serde_json::from_str(&current_json).expect("roundtrip current");
        assert_eq!(roundtrip, migrated);

        let legacy_roundtrip: LegacyVolumeGrid =
            serde_json::from_str(&legacy_json).expect("legacy roundtrip");
        assert_eq!(legacy_roundtrip, legacy);
    }
}
