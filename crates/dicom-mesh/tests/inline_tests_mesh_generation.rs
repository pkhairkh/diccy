// Auto-extracted from /home/z/diccy/crates/dicom-mesh/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_mesh::*;

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
        for v in mesh.vertices() {
            assert!(v[0] >= 100.0, "x should be in patient space");
            assert!(v[1] >= 200.0, "y should be in patient space");
            assert!(v[2] >= 300.0, "z should be in patient space");
        }
    }

    #[test]
    fn mesh_compute_normals() {
        let mut mesh = TriangleMesh::empty("test");
        mesh.vertices_mut().push([0.0, 0.0, 0.0]);
        mesh.vertices_mut().push([1.0, 0.0, 0.0]);
        mesh.vertices_mut().push([0.0, 1.0, 0.0]);
        mesh.triangles_mut().push([0, 1, 2]);
        mesh.compute_normals();
        assert!(mesh.normals_ref().is_computed());
        let normals = mesh.normals_ref().as_slice().unwrap();
        assert_eq!(normals.len(), 3);
        // Normal should point in +z direction for CCW triangle in xy plane
        for n in normals {
            assert!(n[2].abs() > 0.9, "normal should point in z direction");
        }
    }

    #[test]
    fn mesh_surface_area() {
        let mut mesh = TriangleMesh::empty("test");
        mesh.vertices_mut().push([0.0, 0.0, 0.0]);
        mesh.vertices_mut().push([1.0, 0.0, 0.0]);
        mesh.vertices_mut().push([0.0, 1.0, 0.0]);
        mesh.triangles_mut().push([0, 1, 2]);
        let area = mesh.surface_area();
        assert!((area - 0.5).abs() < 1e-10, "right triangle area = 0.5");
    }

    #[test]
    fn mesh_validate_rejects_bad_index() {
        let mut mesh = TriangleMesh::empty("test");
        mesh.vertices_mut().push([0.0, 0.0, 0.0]);
        mesh.triangles_mut().push([0, 1, 2]); // indices 1 and 2 don't exist
        assert!(mesh.validate().is_err());
    }

    #[test]
    fn mesh_validate_rejects_degenerate() {
        let mut mesh = TriangleMesh::empty("test");
        mesh.vertices_mut().push([0.0, 0.0, 0.0]);
        mesh.vertices_mut().push([1.0, 0.0, 0.0]);
        mesh.vertices_mut().push([2.0, 0.0, 0.0]);
        mesh.triangles_mut().push([0, 0, 1]); // vertex 0 used twice
        assert!(mesh.validate().is_err());
    }

    #[test]
    fn decimation_reduces_triangle_count() {
        // Create a mesh with many triangles
        let mut mesh = TriangleMesh::empty("test");
        for i in 0..20u32 {
            let x = i as f64 * 0.1;
            mesh.vertices_mut().push([x, 0.0, 0.0]);
            mesh.vertices_mut().push([x, 1.0, 0.0]);
            mesh.vertices_mut().push([x + 0.1, 0.5, 0.0]);
            mesh.triangles_mut().push([i * 3, i * 3 + 1, i * 3 + 2]);
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
        let mut mesh = TriangleMesh::empty("test");
        mesh.vertices_mut().push([0.0, 0.0, 0.0]);
        mesh.vertices_mut().push([1.0, 0.0, 0.0]);
        mesh.vertices_mut().push([0.0, 1.0, 0.0]);
        mesh.vertices_mut().push([1.0, 1.0, 0.0]);
        mesh.triangles_mut().push([0, 1, 2]);
        mesh.triangles_mut().push([1, 3, 2]);

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
        let mut mesh = TriangleMesh::empty("test");
        // Create a small noisy mesh
        mesh.vertices_mut().push([0.0, 0.0, 0.0]);
        mesh.vertices_mut().push([1.0, 0.1, 0.0]);
        mesh.vertices_mut().push([0.5, 1.0, 0.0]);
        mesh.vertices_mut().push([0.5, 0.5, 0.1]);
        mesh.triangles_mut().push([0, 1, 3]);
        mesh.triangles_mut().push([1, 2, 3]);
        mesh.triangles_mut().push([0, 3, 2]);
        mesh.triangles_mut().push([0, 2, 1]);

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
