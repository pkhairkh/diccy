#![cfg(feature = "rendering")]

use dicom_core::{Dataset, Element, ErrorKind, Tag, Value, Vr};
use dicom_pixel::{DisplayFrame, PixelFormat};
use pack_seg::{SegReference, Segmentation};

const TAG_ROWS: Tag = Tag(0x0028, 0x0010);
const TAG_COLUMNS: Tag = Tag(0x0028, 0x0011);
const TAG_NUMBER_OF_FRAMES: Tag = Tag(0x0028, 0x0008);
const TAG_FRAME_OF_REFERENCE_UID: Tag = Tag(0x0020, 0x0052);
const TAG_SEGMENTATION_TYPE: Tag = Tag(0x0062, 0x0001);
const TAG_SEGMENT_NUMBER: Tag = Tag(0x0062, 0x0004);
const TAG_BITS_ALLOCATED: Tag = Tag(0x0028, 0x0100);
const TAG_BITS_STORED: Tag = Tag(0x0028, 0x0101);
const TAG_HIGH_BIT: Tag = Tag(0x0028, 0x0102);
const TAG_PIXEL_DATA: Tag = Tag(0x7FE0, 0x0010);
const TAG_REFERENCED_SERIES_SEQUENCE: Tag = Tag(0x0008, 0x1115);
const TAG_REFERENCED_INSTANCE_SEQUENCE: Tag = Tag(0x0008, 0x114A);
const TAG_REFERENCED_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x1155);

struct SegFixture<'a> {
    rows: u16,
    cols: u16,
    frames: u16,
    seg_type: &'a str,
    bits_allocated: u16,
    bits_stored: u16,
    high_bit: u16,
    pixel_data: Vec<u8>,
    frame_of_reference_uid: &'a str,
    referenced_uid: &'a str,
    segment_number: u16,
}

fn build_seg_dataset(fixture: SegFixture<'_>) -> Dataset {
    let mut ref_instance = Dataset::new();
    ref_instance.insert(
        Element::new(
            TAG_REFERENCED_SOP_INSTANCE_UID,
            Vr::Ui,
            Value::Uid(fixture.referenced_uid.to_string()),
        )
        .unwrap(),
    );
    let mut ref_series = Dataset::new();
    ref_series.insert(
        Element::new(
            TAG_REFERENCED_INSTANCE_SEQUENCE,
            Vr::Sq,
            Value::Sequence(vec![ref_instance]),
        )
        .unwrap(),
    );

    let mut dataset = Dataset::new();
    dataset.insert(Element::new(TAG_ROWS, Vr::Us, Value::Str(fixture.rows.to_string())).unwrap());
    dataset
        .insert(Element::new(TAG_COLUMNS, Vr::Us, Value::Str(fixture.cols.to_string())).unwrap());
    dataset.insert(
        Element::new(
            TAG_NUMBER_OF_FRAMES,
            Vr::Is,
            Value::Str(fixture.frames.to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            TAG_FRAME_OF_REFERENCE_UID,
            Vr::Ui,
            Value::Uid(fixture.frame_of_reference_uid.to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            TAG_SEGMENTATION_TYPE,
            Vr::Cs,
            Value::Str(fixture.seg_type.to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            TAG_SEGMENT_NUMBER,
            Vr::Us,
            Value::Str(fixture.segment_number.to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            TAG_BITS_ALLOCATED,
            Vr::Us,
            Value::Str(fixture.bits_allocated.to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            TAG_BITS_STORED,
            Vr::Us,
            Value::Str(fixture.bits_stored.to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            TAG_HIGH_BIT,
            Vr::Us,
            Value::Str(fixture.high_bit.to_string()),
        )
        .unwrap(),
    );
    dataset.insert(Element::new(TAG_PIXEL_DATA, Vr::Ob, Value::Bytes(fixture.pixel_data)).unwrap());
    dataset.insert(
        Element::new(
            TAG_REFERENCED_SERIES_SEQUENCE,
            Vr::Sq,
            Value::Sequence(vec![ref_series]),
        )
        .unwrap(),
    );
    dataset
}

fn base_frame(width: u32, height: u32) -> DisplayFrame {
    DisplayFrame {
        width,
        height,
        format: PixelFormat::Luma8,
        bytes: vec![0; (width * height) as usize],
    }
}

#[test]
fn seg_parser_enforces_binary_and_bit_constraints() {
    // REQ-HI-413
    let non_binary = build_seg_dataset(SegFixture {
        rows: 1,
        cols: 1,
        frames: 1,
        seg_type: "FRACTIONAL",
        bits_allocated: 1,
        bits_stored: 1,
        high_bit: 0,
        pixel_data: vec![1],
        frame_of_reference_uid: "1.2.3",
        referenced_uid: "1.2.3.4",
        segment_number: 1,
    });
    let type_err = Segmentation::from_dataset(&non_binary).expect_err("type");
    assert!(matches!(type_err.kind(), ErrorKind::InvalidTagValue { .. }));

    let bad_bits = build_seg_dataset(SegFixture {
        rows: 1,
        cols: 1,
        frames: 1,
        seg_type: "BINARY",
        bits_allocated: 8,
        bits_stored: 1,
        high_bit: 0,
        pixel_data: vec![1],
        frame_of_reference_uid: "1.2.3",
        referenced_uid: "1.2.3.4",
        segment_number: 1,
    });
    let bits_err = Segmentation::from_dataset(&bad_bits).expect_err("bits");
    assert!(matches!(bits_err.kind(), ErrorKind::InvalidTagValue { .. }));
}

#[test]
fn seg_overlay_alignment_and_frame_bounds_fail_closed() {
    // REQ-HI-414, REQ-HI-415, REQ-HI-417
    let dataset = build_seg_dataset(SegFixture {
        rows: 2,
        cols: 2,
        frames: 2,
        seg_type: "BINARY",
        bits_allocated: 1,
        bits_stored: 1,
        high_bit: 0,
        pixel_data: vec![0b0000_1111],
        frame_of_reference_uid: "1.2.3",
        referenced_uid: "1.2.3.4",
        segment_number: 5,
    });
    let seg = Segmentation::from_dataset(&dataset).expect("seg parse");
    let frame = base_frame(2, 2);

    let wrong_uid = SegReference {
        rows: 2,
        cols: 2,
        frame_of_reference_uid: "9.9.9",
        sop_instance_uid: "1.2.3.4",
    };
    let uid_err = seg.overlay_on(&frame, wrong_uid).expect_err("uid mismatch");
    assert!(matches!(uid_err.kind(), ErrorKind::InvalidGeometry { .. }));

    let wrong_grid = SegReference {
        rows: 3,
        cols: 2,
        frame_of_reference_uid: "1.2.3",
        sop_instance_uid: "1.2.3.4",
    };
    let grid_err = seg
        .overlay_on(&frame, wrong_grid)
        .expect_err("grid mismatch");
    assert!(matches!(grid_err.kind(), ErrorKind::InvalidGeometry { .. }));

    let valid_ref = SegReference {
        rows: 2,
        cols: 2,
        frame_of_reference_uid: "1.2.3",
        sop_instance_uid: "1.2.3.4",
    };
    let frame_err = seg
        .overlay_on_frame(&frame, valid_ref, 2)
        .expect_err("frame out of range");
    assert!(matches!(
        frame_err.kind(),
        ErrorKind::InvalidTagValue { .. }
    ));
}

#[test]
fn seg_overlay_color_and_mask_processing_are_deterministic() {
    // REQ-HI-416, REQ-HI-418, REQ-HI-419
    let dataset = build_seg_dataset(SegFixture {
        rows: 2,
        cols: 2,
        frames: 1,
        seg_type: "BINARY",
        bits_allocated: 1,
        bits_stored: 1,
        high_bit: 0,
        pixel_data: vec![0b0000_0011],
        frame_of_reference_uid: "1.2.3",
        referenced_uid: "1.2.3.4",
        segment_number: 7,
    });
    let seg = Segmentation::from_dataset(&dataset).expect("seg parse");
    let reference = SegReference {
        rows: 2,
        cols: 2,
        frame_of_reference_uid: "1.2.3",
        sop_instance_uid: "1.2.3.4",
    };
    let frame = base_frame(2, 2);
    let first = seg.overlay_on(&frame, reference).expect("overlay first");
    let second = seg.overlay_on(&frame, reference).expect("overlay second");
    assert_eq!(first.bytes, second.bytes);
    assert!(first.bytes.iter().any(|value| *value != 0));

    let too_short = build_seg_dataset(SegFixture {
        rows: 2,
        cols: 2,
        frames: 2,
        seg_type: "BINARY",
        bits_allocated: 1,
        bits_stored: 1,
        high_bit: 0,
        pixel_data: vec![],
        frame_of_reference_uid: "1.2.3",
        referenced_uid: "1.2.3.4",
        segment_number: 7,
    });
    let short_err = Segmentation::from_dataset(&too_short).expect_err("pixel data too short");
    assert!(matches!(
        short_err.kind(),
        ErrorKind::InvalidTagValue { .. }
    ));
}
