// Auto-extracted from /home/z/diccy/crates/dicom-mesh/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_mesh::*;
    use dicom_core::{Tag, Value};

    fn test_mesh() -> TriangleMesh {
        let mut mesh = TriangleMesh::empty("bone");
        mesh.vertices_mut().push([0.0, 0.0, 0.0]);
        mesh.vertices_mut().push([1.0, 0.0, 0.0]);
        mesh.vertices_mut().push([0.0, 1.0, 0.0]);
        mesh.vertices_mut().push([1.0, 1.0, 0.0]);
        mesh.triangles_mut().push([0, 1, 2]);
        mesh.triangles_mut().push([1, 3, 2]);
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
        let mut mesh = TriangleMesh::empty("bad");
        mesh.triangles_mut().push([0, 1, 2]); // no vertices
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
