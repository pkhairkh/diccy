// Auto-extracted from /home/z/diccy/crates/pack-gsps/src/lib.rs
// S13-T8: Move inline tests to tests/ directories

    use pack_gsps::*;
    use dicom_core::{Dataset, Element, Error, ErrorKind, Tag, Value, Vr};
    use dicom_pixel::{DisplayFrame, DisplayTransform, PixelFormat, PixelPipeline, PixelPipelineConfig, WindowLevel};

    const GSPS_MANIFEST: &str = include_str!("../manifest.toml");
    const TS_IMPLICIT_VR_LE: &str = "1.2.840.10008.1.2";

    const TAG_ROWS: Tag = Tag(0x0028, 0x0010);
    const TAG_COLUMNS: Tag = Tag(0x0028, 0x0011);
    const TAG_SAMPLES_PER_PIXEL: Tag = Tag(0x0028, 0x0002);
    const TAG_PHOTOMETRIC_INTERPRETATION: Tag = Tag(0x0028, 0x0004);
    const TAG_BITS_ALLOCATED: Tag = Tag(0x0028, 0x0100);
    const TAG_BITS_STORED: Tag = Tag(0x0028, 0x0101);
    const TAG_HIGH_BIT: Tag = Tag(0x0028, 0x0102);
    const TAG_PIXEL_REPRESENTATION: Tag = Tag(0x0028, 0x0103);
    const TAG_PIXEL_DATA: Tag = Tag(0x7FE0, 0x0010);
    const TAG_OVERLAY_ROWS: Tag = Tag(0x6000, 0x0010);
    const TAG_OVERLAY_COLUMNS: Tag = Tag(0x6000, 0x0011);
    const TAG_OVERLAY_BITS_ALLOCATED: Tag = Tag(0x6000, 0x0100);
    const TAG_OVERLAY_BIT_POSITION: Tag = Tag(0x6000, 0x0102);
    const TAG_OVERLAY_DATA: Tag = Tag(0x6000, 0x3000);

    fn parse_manifest_uids() -> Vec<String> {
        GSPS_MANIFEST
            .split('"')
            .enumerate()
            .filter_map(|(idx, part)| {
                if idx % 2 == 1 {
                    Some(part.to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    fn build_rect_dataset_with_bounds(
        left: i32,
        right: i32,
        upper: i32,
        lower: i32,
        value: u8,
    ) -> Dataset {
        let mut dataset = Dataset::new();
        dataset.insert(
            Element::new(
                TAG_SHUTTER_SHAPE,
                Vr::Cs,
                Value::Str("RECTANGULAR".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_SHUTTER_LEFT_VERT_EDGE,
                Vr::Is,
                Value::Str(left.to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_SHUTTER_RIGHT_VERT_EDGE,
                Vr::Is,
                Value::Str(right.to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_SHUTTER_UPPER_HORIZ_EDGE,
                Vr::Is,
                Value::Str(upper.to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_SHUTTER_LOWER_HORIZ_EDGE,
                Vr::Is,
                Value::Str(lower.to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_SHUTTER_PRESENTATION_VALUE,
                Vr::Us,
                Value::Str(value.to_string()),
            )
            .unwrap(),
        );
        dataset
    }

    fn build_rect_dataset() -> Dataset {
        build_rect_dataset_with_bounds(1, 1, 1, 1, 0)
    }

    fn build_polygon_dataset(vertices: &str, value: u8) -> Dataset {
        let mut dataset = Dataset::new();
        dataset.insert(
            Element::new(
                TAG_SHUTTER_SHAPE,
                Vr::Cs,
                Value::Str("POLYGONAL".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_SHUTTER_VERTICES,
                Vr::Is,
                Value::Str(vertices.to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_SHUTTER_PRESENTATION_VALUE,
                Vr::Us,
                Value::Str(value.to_string()),
            )
            .unwrap(),
        );
        dataset
    }

    fn build_graphic_dataset_with_type(graphic_type: &str, graphic_data: &str) -> Dataset {
        let mut graphic = Dataset::new();
        graphic.insert(
            Element::new(
                TAG_GRAPHIC_TYPE,
                Vr::Cs,
                Value::Str(graphic_type.to_string()),
            )
            .unwrap(),
        );
        graphic.insert(
            Element::new(
                TAG_GRAPHIC_DATA,
                Vr::Ds,
                Value::Str(graphic_data.to_string()),
            )
            .unwrap(),
        );

        let mut annotation = Dataset::new();
        annotation.insert(
            Element::new(
                TAG_GRAPHIC_OBJECT_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![graphic]),
            )
            .unwrap(),
        );

        let mut dataset = Dataset::new();
        dataset.insert(
            Element::new(
                TAG_GRAPHIC_ANNOTATION_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![annotation]),
            )
            .unwrap(),
        );
        dataset
    }

    fn build_graphic_dataset() -> Dataset {
        build_graphic_dataset_with_type("POLYLINE", "1\\1\\2\\1\\2\\2")
    }

    fn build_image_dataset(rows: u16, cols: u16, pixel_data: Vec<u8>) -> Dataset {
        let mut dataset = Dataset::new();
        dataset.insert(
            Element::new(TAG_ROWS, Vr::Us, Value::Bytes(rows.to_le_bytes().to_vec())).unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_COLUMNS,
                Vr::Us,
                Value::Bytes(cols.to_le_bytes().to_vec()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_SAMPLES_PER_PIXEL,
                Vr::Us,
                Value::Bytes(1u16.to_le_bytes().to_vec()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_PHOTOMETRIC_INTERPRETATION,
                Vr::Cs,
                Value::Str("MONOCHROME2".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_BITS_ALLOCATED,
                Vr::Us,
                Value::Bytes(8u16.to_le_bytes().to_vec()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_BITS_STORED,
                Vr::Us,
                Value::Bytes(8u16.to_le_bytes().to_vec()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_HIGH_BIT,
                Vr::Us,
                Value::Bytes(7u16.to_le_bytes().to_vec()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_PIXEL_REPRESENTATION,
                Vr::Us,
                Value::Bytes(0u16.to_le_bytes().to_vec()),
            )
            .unwrap(),
        );
        dataset.insert(Element::new(TAG_PIXEL_DATA, Vr::Ob, Value::Bytes(pixel_data)).unwrap());
        dataset
    }

    fn add_overlay(dataset: &mut Dataset, rows: u16, cols: u16, data: Vec<u8>) {
        dataset.insert(
            Element::new(
                TAG_OVERLAY_ROWS,
                Vr::Us,
                Value::Bytes(rows.to_le_bytes().to_vec()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_OVERLAY_COLUMNS,
                Vr::Us,
                Value::Bytes(cols.to_le_bytes().to_vec()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_OVERLAY_BITS_ALLOCATED,
                Vr::Us,
                Value::Bytes(1u16.to_le_bytes().to_vec()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_OVERLAY_BIT_POSITION,
                Vr::Us,
                Value::Bytes(0u16.to_le_bytes().to_vec()),
            )
            .unwrap(),
        );
        dataset.insert(Element::new(TAG_OVERLAY_DATA, Vr::Ob, Value::Bytes(data)).unwrap());
    }

    #[test]
    #[cfg(not(feature = "gsps"))]
    fn gsps_pack_disabled_rejects() {
        // REQ-FEAT-302
        assert!(!GspsPack::enabled());
        let err = GspsPack::ensure_supported(SOP_CLASS_GSPS).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.UNSUPPORTED_SOP");
    }

    #[test]
    #[cfg(feature = "gsps")]
    fn gsps_pack_enabled_allows() {
        // REQ-FEAT-302
        assert!(GspsPack::enabled());
        GspsPack::ensure_supported(SOP_CLASS_GSPS).expect("gsps pack enabled");
    }

    #[test]
    fn manifest_matches_constants() {
        // REQ-CONF-083
        let parsed = parse_manifest_uids();
        assert!(!parsed.is_empty());
        for uid in GSPS_SOP_CLASS_UIDS {
            assert!(parsed.contains(&uid.to_string()));
        }
    }

    #[test]
    fn rect_shutter_applied() {
        // REQ-UI-060, REQ-UI-061, REQ-GSPS-300, REQ-GSPS-301
        let dataset = build_rect_dataset();
        let state = PresentationState::from_dataset(&dataset).expect("parse");
        let mut frame = DisplayFrame {
            width: 2,
            height: 2,
            format: PixelFormat::Luma8,
            bytes: vec![10, 10, 10, 10],
        };
        state.apply_to(&mut frame).expect("apply");
        assert_eq!(frame.bytes, vec![10, 0, 0, 0]);
    }

    #[test]
    fn polygonal_shutter_applied() {
        // REQ-UI-060, REQ-UI-061, REQ-GSPS-300, REQ-GSPS-301
        let dataset = build_polygon_dataset("2\\2\\4\\2\\4\\4\\2\\4", 0);
        let state = PresentationState::from_dataset(&dataset).expect("parse");
        let mut frame = DisplayFrame {
            width: 5,
            height: 5,
            format: PixelFormat::Luma8,
            bytes: vec![9; 25],
        };
        state.apply_to(&mut frame).expect("apply");
        // Center pixel remains unchanged; outside corners are shuttered.
        assert_eq!(frame.bytes[12], 9);
        assert_eq!(frame.bytes[0], 0);
    }

    #[test]
    fn polyline_graphic_applied() {
        // REQ-UI-064, REQ-GSPS-303, REQ-CONF-091
        let dataset = build_graphic_dataset();
        let state = PresentationState::from_dataset(&dataset).expect("parse");
        let mut frame = DisplayFrame {
            width: 3,
            height: 3,
            format: PixelFormat::Luma8,
            bytes: vec![0; 9],
        };
        state.apply_to(&mut frame).expect("apply");
        assert!(frame.bytes.iter().any(|value| *value != 0));
    }

    #[test]
    fn point_graphic_applied() {
        // REQ-UI-064, REQ-GSPS-303, REQ-CONF-091
        let dataset = build_graphic_dataset_with_type("POINT", "2\\2");
        let state = PresentationState::from_dataset(&dataset).expect("parse");
        let mut frame = DisplayFrame {
            width: 3,
            height: 3,
            format: PixelFormat::Luma8,
            bytes: vec![0; 9],
        };
        state.apply_to(&mut frame).expect("apply");
        assert_eq!(frame.bytes[4], GRAPHIC_LUMA);
    }

    #[test]
    fn interpolated_graphic_applied() {
        // REQ-UI-064, REQ-GSPS-303, REQ-CONF-091
        let dataset = build_graphic_dataset_with_type("INTERPOLATED", "1\\3\\2\\2\\3\\1");
        let state = PresentationState::from_dataset(&dataset).expect("parse");
        let mut frame = DisplayFrame {
            width: 3,
            height: 3,
            format: PixelFormat::Luma8,
            bytes: vec![0; 9],
        };
        state.apply_to(&mut frame).expect("apply");
        assert!(frame.bytes.iter().any(|value| *value != 0));
    }

    #[test]
    fn circle_graphic_applied() {
        // REQ-UI-064, REQ-GSPS-303, REQ-CONF-091
        let dataset = build_graphic_dataset_with_type("CIRCLE", "3\\3\\5\\3");
        let state = PresentationState::from_dataset(&dataset).expect("parse");
        let mut frame = DisplayFrame {
            width: 7,
            height: 7,
            format: PixelFormat::Luma8,
            bytes: vec![0; 49],
        };
        state.apply_to(&mut frame).expect("apply");
        assert!(frame.bytes.iter().any(|value| *value != 0));
    }

    #[test]
    fn ellipse_graphic_applied() {
        // REQ-UI-064, REQ-GSPS-303, REQ-CONF-091
        let dataset = build_graphic_dataset_with_type("ELLIPSE", "2\\4\\6\\4\\4\\2\\4\\6");
        let state = PresentationState::from_dataset(&dataset).expect("parse");
        let mut frame = DisplayFrame {
            width: 7,
            height: 7,
            format: PixelFormat::Luma8,
            bytes: vec![0; 49],
        };
        state.apply_to(&mut frame).expect("apply");
        assert!(frame.bytes.iter().any(|value| *value != 0));
    }

    #[test]
    fn gsps_overlay_precedence_after_overlays() {
        // REQ-UI-062, REQ-GSPS-300, REQ-GSPS-302
        let mut dataset = build_image_dataset(2, 2, vec![128u8; 4]);
        add_overlay(&mut dataset, 1, 1, vec![0x01]);
        let gsps_dataset = build_rect_dataset_with_bounds(2, 2, 2, 2, 0);
        let state = PresentationState::from_dataset(&gsps_dataset).expect("parse");

        let config = PixelPipelineConfig {
            window_level: WindowLevel::Explicit {
                center: 128.0,
                width: 256.0,
            },
            ..PixelPipelineConfig::default()
        };
        let pipeline = PixelPipeline::new(config);

        let base = pipeline
            .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
            .expect("frame");
        assert_eq!(base.format, PixelFormat::Luma8);
        assert_eq!(base.bytes[0], 255);

        let mut manual = base.clone();
        state.apply_to(&mut manual).expect("apply");

        let combined = pipeline
            .decode_frame_with_transform(&dataset, TS_IMPLICIT_VR_LE, 0, &state)
            .expect("frame");
        assert_eq!(combined.bytes, manual.bytes);
        assert_eq!(combined.bytes[0], 0);
        assert_eq!(combined.bytes[3], 128);
    }

    #[test]
    fn unsupported_graphic_type_rejected() {
        // REQ-UI-065, REQ-GSPS-304, REQ-CONF-091
        let dataset = build_graphic_dataset_with_type("BEZIER", "1\\1\\2\\2");
        let err = PresentationState::from_dataset(&dataset).expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::InvalidTagValue { .. }));
    }

    #[test]
    fn invalid_circle_topology_rejected() {
        // REQ-UI-065, REQ-GSPS-304, REQ-CONF-091
        let dataset = build_graphic_dataset_with_type("CIRCLE", "2\\2");
        let err = PresentationState::from_dataset(&dataset).expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::InvalidTagValue { .. }));
    }

    #[test]
    fn invalid_ellipse_topology_rejected() {
        // REQ-UI-065, REQ-GSPS-304, REQ-CONF-091
        let dataset = build_graphic_dataset_with_type("ELLIPSE", "1\\1\\3\\1\\1\\3\\4\\4");
        let err = PresentationState::from_dataset(&dataset).expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::InvalidTagValue { .. }));
    }

    #[test]
    fn invalid_polygonal_shutter_rejected() {
        // REQ-UI-065, REQ-GSPS-304
        let dataset = build_polygon_dataset("1\\1\\2\\2", 0);
        let err = PresentationState::from_dataset(&dataset).expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::InvalidTagValue { .. }));
    }

    // -----------------------------------------------------------------------
    // Writeback (encoding) tests
    // -----------------------------------------------------------------------

    #[test]
    fn builder_creates_empty_state() {
        // REQ-GSPS-302
        let state = PresentationStateBuilder::new().build();
        assert!(state.shutter.is_none());
        assert!(state.graphics.is_empty());
        assert!(state.window_center.is_none());
        assert!(state.window_width.is_none());
        assert!(state.zoom.is_none());
        assert!(state.pan_x.is_none());
        assert!(state.pan_y.is_none());
        assert_eq!(state.rotation, Rotation::Q0);
        assert_eq!(state.flip, Flip::None);
    }

    #[test]
    fn builder_with_window_level() {
        // REQ-GSPS-302
        let state = PresentationStateBuilder::new()
            .with_window_level(40.0, 400.0)
            .build();
        assert_eq!(state.window_center, Some(40.0));
        assert_eq!(state.window_width, Some(400.0));
    }

    #[test]
    fn builder_with_spatial_transform() {
        // REQ-GSPS-302
        let state = PresentationStateBuilder::new()
            .with_zoom(2.5)
            .with_pan(10.0, -5.0)
            .with_rotation(Rotation::Q90)
            .with_flip(Flip::Horizontal)
            .build();
        assert_eq!(state.zoom, Some(2.5));
        assert_eq!(state.pan_x, Some(10.0));
        assert_eq!(state.pan_y, Some(-5.0));
        assert_eq!(state.rotation, Rotation::Q90);
        assert_eq!(state.flip, Flip::Horizontal);
    }

    #[test]
    fn builder_with_shutter_and_graphics() {
        // REQ-GSPS-302
        let state = PresentationStateBuilder::new()
            .with_shutter(Shutter::Rect {
                left: 1,
                right: 10,
                upper: 1,
                lower: 10,
                value: 0,
            })
            .with_graphic(GraphicObject::Point { point: (5.0, 5.0) })
            .with_graphic(GraphicObject::Polyline {
                points: vec![(1.0, 1.0), (2.0, 2.0)],
            })
            .build();
        assert!(state.shutter.is_some());
        assert_eq!(state.graphics.len(), 2);
    }

    #[test]
    fn encode_empty_state_has_sop_class_uid() {
        // REQ-GSPS-302
        let state = PresentationStateBuilder::new().build();
        let dataset = encode_presentation_state(&state);
        let sop_class = dataset.get_uid(TAG_SOP_CLASS_UID);
        assert_eq!(sop_class, Some(SOP_CLASS_GSPS));
    }

    #[test]
    fn encode_rect_shutter_roundtrip() {
        // REQ-GSPS-302
        let original = PresentationStateBuilder::new()
            .with_shutter(Shutter::Rect {
                left: 5,
                right: 20,
                upper: 3,
                lower: 15,
                value: 128,
            })
            .build();
        let dataset = encode_presentation_state(&original);
        let parsed = PresentationState::from_dataset(&dataset).expect("parse encoded");
        assert_eq!(parsed.shutter, original.shutter);
        assert!(parsed.graphics.is_empty());
    }

    #[test]
    fn encode_circular_shutter_roundtrip() {
        // REQ-GSPS-302
        let original = PresentationStateBuilder::new()
            .with_shutter(Shutter::Circular {
                center_x: 100,
                center_y: 200,
                radius: 50,
                value: 64,
            })
            .build();
        let dataset = encode_presentation_state(&original);
        let parsed = PresentationState::from_dataset(&dataset).expect("parse encoded");
        assert_eq!(parsed.shutter, original.shutter);
    }

    #[test]
    fn encode_polygon_shutter_roundtrip() {
        // REQ-GSPS-302
        let original = PresentationStateBuilder::new()
            .with_shutter(Shutter::Polygon {
                points: vec![(1, 1), (100, 1), (100, 100)],
                value: 0,
            })
            .build();
        let dataset = encode_presentation_state(&original);
        let parsed = PresentationState::from_dataset(&dataset).expect("parse encoded");
        assert_eq!(parsed.shutter, original.shutter);
    }

    #[test]
    fn encode_polyline_graphic_roundtrip() {
        // REQ-GSPS-302
        let original = PresentationStateBuilder::new()
            .with_graphic(GraphicObject::Polyline {
                points: vec![(1.0, 1.0), (2.0, 3.0), (4.0, 5.0)],
            })
            .build();
        let dataset = encode_presentation_state(&original);
        let parsed = PresentationState::from_dataset(&dataset).expect("parse encoded");
        assert_eq!(parsed.graphics, original.graphics);
    }

    #[test]
    fn encode_point_graphic_roundtrip() {
        // REQ-GSPS-302
        let original = PresentationStateBuilder::new()
            .with_graphic(GraphicObject::Point {
                point: (10.5, 20.3),
            })
            .build();
        let dataset = encode_presentation_state(&original);
        let parsed = PresentationState::from_dataset(&dataset).expect("parse encoded");
        assert_eq!(parsed.graphics, original.graphics);
    }

    #[test]
    fn encode_interpolated_graphic_roundtrip() {
        // REQ-GSPS-302
        let original = PresentationStateBuilder::new()
            .with_graphic(GraphicObject::Interpolated {
                points: vec![(1.0, 1.0), (5.0, 5.0), (10.0, 1.0)],
            })
            .build();
        let dataset = encode_presentation_state(&original);
        let parsed = PresentationState::from_dataset(&dataset).expect("parse encoded");
        assert_eq!(parsed.graphics, original.graphics);
    }

    #[test]
    fn encode_circle_graphic_roundtrip() {
        // REQ-GSPS-302
        let original = PresentationStateBuilder::new()
            .with_graphic(GraphicObject::Circle {
                center: (50.0, 50.0),
                edge: (60.0, 50.0),
            })
            .build();
        let dataset = encode_presentation_state(&original);
        let parsed = PresentationState::from_dataset(&dataset).expect("parse encoded");
        assert_eq!(parsed.graphics, original.graphics);
    }

    #[test]
    fn encode_window_level() {
        // REQ-GSPS-302
        let state = PresentationStateBuilder::new()
            .with_window_level(40.0, 400.0)
            .build();
        let dataset = encode_presentation_state(&state);
        let center = dataset
            .get(TAG_WINDOW_CENTER)
            .and_then(|e| match &e.value() {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        let width = dataset
            .get(TAG_WINDOW_WIDTH)
            .and_then(|e| match &e.value() {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        assert_eq!(center, Some("40"));
        assert_eq!(width, Some("400"));
    }

    #[test]
    fn encode_spatial_transform_no_flip() {
        // REQ-GSPS-302
        let state = PresentationStateBuilder::new()
            .with_rotation(Rotation::Q90)
            .with_flip(Flip::None)
            .build();
        let dataset = encode_presentation_state(&state);
        let flip = dataset
            .get(TAG_IMAGE_HORIZONTAL_FLIP)
            .and_then(|e| match &e.value() {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        let rotation = dataset
            .get(TAG_IMAGE_ROTATION)
            .and_then(|e| match &e.value() {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        assert_eq!(flip, Some("N"));
        assert_eq!(rotation, Some("90")); // quadrant 1 = 90°
    }

    #[test]
    fn encode_spatial_transform_horizontal_flip() {
        // REQ-GSPS-302
        let state = PresentationStateBuilder::new()
            .with_rotation(Rotation::Q0)
            .with_flip(Flip::Horizontal)
            .build();
        let dataset = encode_presentation_state(&state);
        let flip = dataset
            .get(TAG_IMAGE_HORIZONTAL_FLIP)
            .and_then(|e| match &e.value() {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        let rotation = dataset
            .get(TAG_IMAGE_ROTATION)
            .and_then(|e| match &e.value() {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        assert_eq!(flip, Some("Y")); // flip_x XOR flip_y = true
        assert_eq!(rotation, Some("0")); // no rotation + no flip_y
    }

    #[test]
    fn encode_spatial_transform_vertical_flip() {
        // REQ-GSPS-302: vertical flip = horizontal flip + 180° rotation
        let state = PresentationStateBuilder::new()
            .with_rotation(Rotation::Q0)
            .with_flip(Flip::Vertical)
            .build();
        let dataset = encode_presentation_state(&state);
        let flip = dataset
            .get(TAG_IMAGE_HORIZONTAL_FLIP)
            .and_then(|e| match &e.value() {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        let rotation = dataset
            .get(TAG_IMAGE_ROTATION)
            .and_then(|e| match &e.value() {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        assert_eq!(flip, Some("Y")); // flip_x XOR flip_y = true
        assert_eq!(rotation, Some("180")); // rotation_q + 2 = 2 quadrants
    }

    #[test]
    fn encode_spatial_transform_both_flips() {
        // REQ-GSPS-302: both flips = 180° rotation, no horizontal flip
        let state = PresentationStateBuilder::new()
            .with_rotation(Rotation::Q0)
            .with_flip(Flip::Both)
            .build();
        let dataset = encode_presentation_state(&state);
        let flip = dataset
            .get(TAG_IMAGE_HORIZONTAL_FLIP)
            .and_then(|e| match &e.value() {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        let rotation = dataset
            .get(TAG_IMAGE_ROTATION)
            .and_then(|e| match &e.value() {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        assert_eq!(flip, Some("N")); // flip_x XOR flip_y = false
        assert_eq!(rotation, Some("180")); // rotation_q + 2 (from flip_y)
    }

    #[test]
    fn encode_zoom_in_displayed_area() {
        // REQ-GSPS-302
        let state = PresentationStateBuilder::new().with_zoom(2.5).build();
        let dataset = encode_presentation_state(&state);
        let seq = dataset
            .get(TAG_DISPLAYED_AREA_SELECTION_SEQUENCE)
            .and_then(|e| match &e.value() {
                Value::Sequence(items) => Some(items.as_slice()),
                _ => None,
            });
        assert!(seq.is_some());
        let items = seq.unwrap();
        assert_eq!(items.len(), 1);
        let zoom_val = items[0]
            .get(TAG_PRESENTATION_PIXEL_MAGNIFICATION_RATIO)
            .and_then(|e| match &e.value() {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        assert_eq!(zoom_val, Some("2.5"));
    }

    #[test]
    fn encode_pan_in_displayed_area() {
        // REQ-GSPS-302
        let state = PresentationStateBuilder::new().with_pan(10.0, -5.0).build();
        let dataset = encode_presentation_state(&state);
        let seq = dataset
            .get(TAG_DISPLAYED_AREA_SELECTION_SEQUENCE)
            .and_then(|e| match &e.value() {
                Value::Sequence(items) => Some(items.as_slice()),
                _ => None,
            });
        assert!(seq.is_some());
        let items = seq.unwrap();
        assert_eq!(items.len(), 1);
        let pan_val = items[0]
            .get(TAG_DISPLAYED_AREA_TOP_LEFT)
            .and_then(|e| match &e.value() {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        // Format is "pan_y\\pan_x"
        assert_eq!(pan_val, Some("-5\\10"));
    }

    #[test]
    fn viewport_state_struct() {
        // REQ-GSPS-302
        let viewport = ViewportState {
            window_center: 40.0,
            window_width: 400.0,
            zoom: 1.5,
            pan_x: 5.0,
            pan_y: -3.0,
            rotation: Rotation::Q90,
            flip: Flip::Vertical,
            referenced_sop_instance_uid: "1.2.3.4.5".to_string(),
        };
        assert_eq!(viewport.window_center, 40.0);
        assert_eq!(viewport.zoom, 1.5);
        assert!(viewport.flip.is_vertical());
        assert_eq!(viewport.referenced_sop_instance_uid, "1.2.3.4.5");
    }

    #[test]
    fn encode_viewport_as_gsps_contains_reference() {
        // REQ-GSPS-302
        let viewport = ViewportState {
            window_center: 40.0,
            window_width: 400.0,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            rotation: Rotation::Q0,
            flip: Flip::None,
            referenced_sop_instance_uid: "1.2.840.10008.5.1.4.1.1.2".to_string(),
        };
        let dataset = encode_viewport_as_gsps(&viewport);

        // Should have SOP Class UID
        let sop_class = dataset.get_uid(TAG_SOP_CLASS_UID);
        assert_eq!(sop_class, Some(SOP_CLASS_GSPS));

        // Should have window center/width
        let center = dataset
            .get(TAG_WINDOW_CENTER)
            .and_then(|e| match &e.value() {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        assert_eq!(center, Some("40"));

        // Should have referenced series sequence with the SOP Instance UID
        let ref_series =
            dataset
                .get(TAG_REFERENCED_SERIES_SEQUENCE)
                .and_then(|e| match &e.value() {
                    Value::Sequence(items) => Some(items.as_slice()),
                    _ => None,
                });
        assert!(ref_series.is_some());
        let series_items = ref_series.unwrap();
        assert_eq!(series_items.len(), 1);
        let ref_images = series_items[0]
            .get(TAG_REFERENCED_IMAGE_SEQUENCE)
            .and_then(|e| match &e.value() {
                Value::Sequence(items) => Some(items.as_slice()),
                _ => None,
            });
        assert!(ref_images.is_some());
        let image_items = ref_images.unwrap();
        assert_eq!(image_items.len(), 1);
        let uid = image_items[0].get_uid(TAG_REFERENCED_SOP_INSTANCE_UID);
        assert_eq!(uid, Some("1.2.840.10008.5.1.4.1.1.2"));
    }

    #[test]
    fn encode_viewport_with_rotation_and_flip() {
        // REQ-GSPS-302
        let viewport = ViewportState {
            window_center: 500.0,
            window_width: 2000.0,
            zoom: 3.0,
            pan_x: 100.0,
            pan_y: 50.0,
            rotation: Rotation::Q180,
            flip: Flip::Horizontal,
            referenced_sop_instance_uid: "9.9.9".to_string(),
        };
        let dataset = encode_viewport_as_gsps(&viewport);

        // Check flip and rotation are encoded
        let flip = dataset
            .get(TAG_IMAGE_HORIZONTAL_FLIP)
            .and_then(|e| match &e.value() {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        let rotation = dataset
            .get(TAG_IMAGE_ROTATION)
            .and_then(|e| match &e.value() {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        assert_eq!(flip, Some("Y")); // flip_x=true, flip_y=false -> effective_flip_x=true
        assert_eq!(rotation, Some("180")); // rotation_q=2, flip_y=false -> 2*90=180

        // Check zoom
        let seq = dataset
            .get(TAG_DISPLAYED_AREA_SELECTION_SEQUENCE)
            .and_then(|e| match &e.value() {
                Value::Sequence(items) => Some(items.as_slice()),
                _ => None,
            })
            .expect("displayed area sequence");
        let zoom_val = seq[0]
            .get(TAG_PRESENTATION_PIXEL_MAGNIFICATION_RATIO)
            .and_then(|e| match &e.value() {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        assert_eq!(zoom_val, Some("3"));
    }

    #[test]
    fn encode_no_spatial_transform_when_none_set() {
        // REQ-GSPS-302
        let state = PresentationStateBuilder::new().build();
        let dataset = encode_presentation_state(&state);
        assert!(dataset.get(TAG_IMAGE_HORIZONTAL_FLIP).is_none());
        assert!(dataset.get(TAG_IMAGE_ROTATION).is_none());
        assert!(dataset.get(TAG_DISPLAYED_AREA_SELECTION_SEQUENCE).is_none());
    }

    #[test]
    fn format_ds_integer_values() {
        // REQ-GSPS-302
        assert_eq!(format_ds(40.0), "40");
        assert_eq!(format_ds(0.0), "0");
        assert_eq!(format_ds(-7.0), "-7");
    }

    #[test]
    fn format_ds_fractional_values() {
        // REQ-GSPS-302
        assert_eq!(format_ds(1.5), "1.5");
        assert_eq!(format_ds(0.001), "0.001");
    }
