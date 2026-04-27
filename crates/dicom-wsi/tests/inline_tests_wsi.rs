// Auto-extracted from /home/z/diccy/crates/dicom-wsi/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_wsi::*;
    use std::collections::BTreeMap;

    #[test]
    fn pyramid_level_creation() {
        let level = PyramidLevel::new(0, 100000, 80000, 256, 256);
        assert_eq!(level.level, 0);
        assert_eq!(level.width, 100000);
        assert_eq!(level.tile_width, 256);
    }

    #[test]
    fn pyramid_level_tile_counts() {
        let level = PyramidLevel::new(0, 1000, 800, 256, 256);
        assert_eq!(level.tile_columns(), 4); // ceil(1000/256)
        assert_eq!(level.tile_rows(), 4); // ceil(800/256)
        assert_eq!(level.total_tiles(), 16);
    }

    #[test]
    fn pyramid_magnification() {
        let level_0 = PyramidLevel::new(0, 100000, 80000, 256, 256);
        let level_1 = PyramidLevel::new(1, 50000, 40000, 256, 256);
        let level_2 = PyramidLevel::new(2, 25000, 20000, 256, 256);

        assert!((level_0.magnification() - 40.0).abs() < 1e-6);
        assert!((level_1.magnification() - 20.0).abs() < 1e-6);
        assert!((level_2.magnification() - 10.0).abs() < 1e-6);
    }

    #[test]
    fn wsi_metadata_creation() {
        let meta = WsiMetadata::new("1.2.3.4", "5.6.7", "8.9.0");
        assert_eq!(meta.sop_instance_uid, "1.2.3.4");
        assert_eq!(meta.objective_power, 40.0);
    }

    #[test]
    fn wsi_metadata_level_for_magnification() {
        let mut meta = WsiMetadata::new("1.2.3", "4.5.6", "7.8.9");
        meta.pyramid_levels = vec![
            PyramidLevel::new(0, 100000, 80000, 256, 256),
            PyramidLevel::new(1, 50000, 40000, 256, 256),
            PyramidLevel::new(2, 25000, 20000, 256, 256),
        ];

        let level = meta.level_for_magnification(20.0).unwrap();
        assert_eq!(level.level, 1);
    }

    #[test]
    fn wsi_viewport_default() {
        let vp = WsiViewport::default();
        assert_eq!(vp.magnification, 1.0);
        assert_eq!(vp.viewport_width, 1024);
    }

    #[test]
    fn wsi_viewport_zoom() {
        let mut vp = WsiViewport::new(50000.0, 40000.0, 40.0);
        vp.zoom(0.5);
        assert!((vp.magnification - 20.0).abs() < 1e-6);
    }

    #[test]
    fn cell_counting_simple() {
        let mut mask = vec![0u8; 20 * 20];
        // Place two separate cells
        mask[5 * 20 + 5] = 1;
        mask[5 * 20 + 6] = 1;
        mask[15 * 20 + 15] = 1;

        let count = count_cells(&mask, 20, 20);
        assert_eq!(count, 2);
    }

    #[test]
    fn cell_count_result_density() {
        let mut counts = BTreeMap::new();
        counts.insert("positive".to_string(), 50);
        counts.insert("negative".to_string(), 100);

        let result = CellCountResult::new(counts, 2.0);
        assert_eq!(result.total_count, 150);
        assert!((result.density - 75.0).abs() < 1e-6);
    }

    #[test]
    fn area_measurement_from_polygon() {
        let points = vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)];
        let measurement = AreaMeasurement::from_polygon(points, 0.00025);
        assert!(measurement.area_mm2 > 0.0);
        assert!(measurement.perimeter_mm > 0.0);
    }

    #[test]
    fn slide_store_register_and_retrieve() {
        let mut store = WsiSlideStore::new();
        let meta = WsiMetadata::new("1.2.3.4", "5.6.7", "8.9.0");
        store.register_slide(meta).unwrap();

        assert_eq!(store.slide_count(), 1);
        assert!(store.get_slide("1.2.3.4").is_some());
    }

    #[test]
    fn slide_store_tile_caching() {
        let mut store = WsiSlideStore::new();
        let tile = Tile {
            coord: TileCoordinate::new(0, 0, 0),
            data: vec![42u8; 100],
            data_size: 100,
        };
        store.store_tile("1.2.3", tile);

        assert_eq!(store.cached_tile_count(), 1);
        let retrieved = store.get_tile("1.2.3", &TileCoordinate::new(0, 0, 0));
        assert!(retrieved.is_some());
    }

    #[test]
    fn slide_store_rejects_empty_uid() {
        let mut store = WsiSlideStore::new();
        let meta = WsiMetadata::new("", "5.6.7", "8.9.0");
        assert!(store.register_slide(meta).is_err());
    }
