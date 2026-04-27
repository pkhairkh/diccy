#![cfg(feature = "rendering")]

use dicom_core::{Dataset, Element, ErrorKind, Tag, Value, Vr};
use dicom_pixel::{DisplayFrame, PixelFormat};
use pack_gsps::PresentationState;

const TAG_SHUTTER_SHAPE: Tag = Tag(0x0018, 0x1600);
const TAG_SHUTTER_VERTICES: Tag = Tag(0x0018, 0x1620);
const TAG_SHUTTER_PRESENTATION_VALUE: Tag = Tag(0x0018, 0x1622);
const TAG_GRAPHIC_ANNOTATION_SEQUENCE: Tag = Tag(0x0070, 0x0001);
const TAG_GRAPHIC_OBJECT_SEQUENCE: Tag = Tag(0x0070, 0x0009);
const TAG_GRAPHIC_DATA: Tag = Tag(0x0070, 0x0022);
const TAG_GRAPHIC_TYPE: Tag = Tag(0x0070, 0x0023);

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

#[test]
fn gsps_polygon_shutter_and_graphics_apply_deterministically() {
    // REQ-HI-407, REQ-HI-412
    let shutter = build_polygon_dataset("2\\2\\4\\2\\4\\4\\2\\4", 0);
    let state = PresentationState::from_dataset(&shutter).expect("parse shutter");

    let mut frame_a = DisplayFrame {
        width: 5,
        height: 5,
        format: PixelFormat::Luma8,
        bytes: vec![9; 25],
    };
    let mut frame_b = frame_a.clone();
    state.apply_to(&mut frame_a).expect("apply a");
    state.apply_to(&mut frame_b).expect("apply b");
    assert_eq!(frame_a.bytes, frame_b.bytes);

    let graphics = build_graphic_dataset_with_type("POLYLINE", "1\\1\\2\\1\\2\\2");
    let graphic_state = PresentationState::from_dataset(&graphics).expect("parse graphics");
    let mut g_frame_a = DisplayFrame {
        width: 3,
        height: 3,
        format: PixelFormat::Luma8,
        bytes: vec![0; 9],
    };
    let mut g_frame_b = g_frame_a.clone();
    graphic_state
        .apply_to(&mut g_frame_a)
        .expect("apply graphics a");
    graphic_state
        .apply_to(&mut g_frame_b)
        .expect("apply graphics b");
    assert_eq!(g_frame_a.bytes, g_frame_b.bytes);
    assert!(g_frame_a.bytes.iter().any(|value| *value != 0));
}

#[test]
fn gsps_rejects_invalid_polygon_vertices_and_unsupported_graphic_types() {
    // REQ-HI-408, REQ-HI-409, REQ-HI-411
    let invalid_polygon = build_polygon_dataset("1\\1\\2\\2", 0);
    let polygon_err = PresentationState::from_dataset(&invalid_polygon).expect_err("polygon");
    assert!(matches!(
        polygon_err.kind(),
        ErrorKind::InvalidTagValue { .. }
    ));

    let unsupported = build_graphic_dataset_with_type("BEZIER", "1\\1\\2\\2");
    let graphic_err = PresentationState::from_dataset(&unsupported).expect_err("graphic");
    assert!(matches!(
        graphic_err.kind(),
        ErrorKind::InvalidTagValue { .. }
    ));

    let malformed_pairs = build_graphic_dataset_with_type("POLYLINE", "1\\1\\2");
    let pair_err = PresentationState::from_dataset(&malformed_pairs).expect_err("pairs");
    assert!(matches!(pair_err.kind(), ErrorKind::InvalidTagValue { .. }));
}

#[test]
fn gsps_rejects_invalid_circle_and_ellipse_geometry() {
    // REQ-HI-410
    let invalid_circle = build_graphic_dataset_with_type("CIRCLE", "2\\2");
    let circle_err = PresentationState::from_dataset(&invalid_circle).expect_err("circle");
    assert!(matches!(
        circle_err.kind(),
        ErrorKind::InvalidTagValue { .. }
    ));

    let invalid_ellipse = build_graphic_dataset_with_type("ELLIPSE", "1\\1\\3\\1\\1\\3\\4\\4");
    let ellipse_err = PresentationState::from_dataset(&invalid_ellipse).expect_err("ellipse");
    assert!(matches!(
        ellipse_err.kind(),
        ErrorKind::InvalidTagValue { .. }
    ));
}
