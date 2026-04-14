use dicom_core::{Dataset, Element, ErrorKind, Tag, Value, Vr};
use pack_rt::{RtDoseGrid, RtPlanSummary, RtReferenceGeometry, RtStructureSet};

const TAG_ROWS: Tag = Tag(0x0028, 0x0010);
const TAG_COLUMNS: Tag = Tag(0x0028, 0x0011);
const TAG_NUMBER_OF_FRAMES: Tag = Tag(0x0028, 0x0008);
const TAG_PIXEL_SPACING: Tag = Tag(0x0028, 0x0030);
const TAG_IMAGE_POSITION: Tag = Tag(0x0020, 0x0032);
const TAG_IMAGE_ORIENTATION: Tag = Tag(0x0020, 0x0037);
const TAG_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x0018);
const TAG_FRAME_OF_REFERENCE_UID: Tag = Tag(0x0020, 0x0052);
const TAG_DOSE_GRID_SCALING: Tag = Tag(0x3004, 0x000E);
const TAG_GRID_FRAME_OFFSET_VECTOR: Tag = Tag(0x3004, 0x000C);
const TAG_BITS_ALLOCATED: Tag = Tag(0x0028, 0x0100);
const TAG_PIXEL_DATA: Tag = Tag(0x7FE0, 0x0010);
const TAG_ROI_CONTOUR_SEQUENCE: Tag = Tag(0x3006, 0x0039);
const TAG_CONTOUR_SEQUENCE: Tag = Tag(0x3006, 0x0040);
const TAG_CONTOUR_DATA: Tag = Tag(0x3006, 0x0050);
const TAG_CONTOUR_GEOMETRIC_TYPE: Tag = Tag(0x3006, 0x0042);
const TAG_NUMBER_OF_CONTOUR_POINTS: Tag = Tag(0x3006, 0x0046);
const TAG_ROI_DISPLAY_COLOR: Tag = Tag(0x3006, 0x002A);
const TAG_RT_PLAN_LABEL: Tag = Tag(0x300A, 0x0002);
const TAG_REFERENCED_STRUCTURE_SET_SEQUENCE: Tag = Tag(0x300C, 0x0060);
const TAG_REFERENCED_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x1155);

fn build_dose_dataset(
    frames: u16,
    grid_offsets: &str,
    scaling: &str,
    bits_allocated: u16,
    pixel_bytes: Vec<u8>,
) -> Dataset {
    let mut dataset = Dataset::new();
    dataset.insert(Element {
        tag: TAG_ROWS,
        vr: Vr::Us,
        value: Value::Str("1".to_string()),
    });
    dataset.insert(Element {
        tag: TAG_COLUMNS,
        vr: Vr::Us,
        value: Value::Str("1".to_string()),
    });
    dataset.insert(Element {
        tag: TAG_NUMBER_OF_FRAMES,
        vr: Vr::Is,
        value: Value::Str(frames.to_string()),
    });
    dataset.insert(Element {
        tag: TAG_PIXEL_SPACING,
        vr: Vr::Ds,
        value: Value::Str("1\\1".to_string()),
    });
    dataset.insert(Element {
        tag: TAG_IMAGE_POSITION,
        vr: Vr::Ds,
        value: Value::Str("0\\0\\0".to_string()),
    });
    dataset.insert(Element {
        tag: TAG_IMAGE_ORIENTATION,
        vr: Vr::Ds,
        value: Value::Str("1\\0\\0\\0\\1\\0".to_string()),
    });
    dataset.insert(Element {
        tag: TAG_FRAME_OF_REFERENCE_UID,
        vr: Vr::Ui,
        value: Value::Uid("1.2.3".to_string()),
    });
    dataset.insert(Element {
        tag: TAG_GRID_FRAME_OFFSET_VECTOR,
        vr: Vr::Ds,
        value: Value::Str(grid_offsets.to_string()),
    });
    dataset.insert(Element {
        tag: TAG_DOSE_GRID_SCALING,
        vr: Vr::Ds,
        value: Value::Str(scaling.to_string()),
    });
    dataset.insert(Element {
        tag: TAG_BITS_ALLOCATED,
        vr: Vr::Us,
        value: Value::Str(bits_allocated.to_string()),
    });
    dataset.insert(Element {
        tag: TAG_PIXEL_DATA,
        vr: Vr::Ob,
        value: Value::Bytes(pixel_bytes),
    });
    dataset
}

fn build_structure_dataset_with_type(points: &str, contour_type: &str) -> Dataset {
    let mut contour = Dataset::new();
    contour.insert(Element {
        tag: TAG_CONTOUR_GEOMETRIC_TYPE,
        vr: Vr::Cs,
        value: Value::Str(contour_type.to_string()),
    });
    contour.insert(Element {
        tag: TAG_NUMBER_OF_CONTOUR_POINTS,
        vr: Vr::Is,
        value: Value::Str("4".to_string()),
    });
    contour.insert(Element {
        tag: TAG_CONTOUR_DATA,
        vr: Vr::Ds,
        value: Value::Str(points.to_string()),
    });

    let mut roi = Dataset::new();
    roi.insert(Element {
        tag: TAG_ROI_DISPLAY_COLOR,
        vr: Vr::Is,
        value: Value::Str("255\\0\\0".to_string()),
    });
    roi.insert(Element {
        tag: TAG_CONTOUR_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![contour]),
    });

    let mut dataset = Dataset::new();
    dataset.insert(Element {
        tag: TAG_SOP_INSTANCE_UID,
        vr: Vr::Ui,
        value: Value::Uid("1.2.3.4".to_string()),
    });
    dataset.insert(Element {
        tag: TAG_FRAME_OF_REFERENCE_UID,
        vr: Vr::Ui,
        value: Value::Uid("9.8.7".to_string()),
    });
    dataset.insert(Element {
        tag: TAG_ROI_CONTOUR_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![roi]),
    });
    dataset
}

#[test]
fn rt_dose_overlay_contract_validates_scaling_alignment_and_frame_bounds() {
    // REQ-HI-420, REQ-HI-421, REQ-HI-422, REQ-HI-426
    let invalid_scaling = build_dose_dataset(1, "0", "0", 16, vec![0, 0]);
    let scaling_err = RtDoseGrid::from_dataset(&invalid_scaling).expect_err("scaling");
    assert!(matches!(
        scaling_err.kind,
        ErrorKind::InvalidTagValue { .. }
    ));

    let dose_dataset = build_dose_dataset(1, "0", "0.5", 16, vec![2, 0]);
    let dose = RtDoseGrid::from_dataset(&dose_dataset).expect("dose parse");
    let reference = RtReferenceGeometry {
        rows: 1,
        cols: 1,
        pixel_spacing: (1.0, 1.0),
        ipp: [0.0, 0.0, 0.0],
        iop: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        frame_of_reference_uid: "1.2.3".to_string(),
    };
    let first = dose.overlay_on(&reference, 0).expect("overlay first");
    let second = dose.overlay_on(&reference, 0).expect("overlay second");
    assert_eq!(first.bytes, second.bytes);

    let frame_err = dose.overlay_on(&reference, 1).expect_err("frame index");
    assert!(matches!(frame_err.kind, ErrorKind::InvalidTagValue { .. }));

    let mismatch = RtReferenceGeometry {
        rows: 1,
        cols: 1,
        pixel_spacing: (1.0, 1.0),
        ipp: [0.0, 0.0, 1.0],
        iop: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        frame_of_reference_uid: "1.2.3".to_string(),
    };
    let align_err = dose.overlay_on(&mismatch, 0).expect_err("alignment");
    assert!(matches!(align_err.kind, ErrorKind::InvalidGeometry { .. }));
}

#[test]
fn rt_structure_contract_rejects_invalid_contours_and_plane_mismatch() {
    // REQ-HI-423, REQ-HI-424, REQ-HI-425
    let unsupported = build_structure_dataset_with_type("0\\0\\0\\1\\0\\0", "BEZIER");
    let unsupported_err = RtStructureSet::from_dataset(&unsupported).expect_err("unsupported");
    assert!(matches!(
        unsupported_err.kind,
        ErrorKind::InvalidTagValue { .. }
    ));

    let bad_points = build_structure_dataset_with_type("0\\0\\0\\1", "CLOSED_PLANAR");
    let points_err = RtStructureSet::from_dataset(&bad_points).expect_err("point length");
    assert!(matches!(points_err.kind, ErrorKind::InvalidTagValue { .. }));

    let valid =
        build_structure_dataset_with_type("0\\0\\1\\1\\0\\1\\1\\1\\1\\0\\1\\1", "CLOSED_PLANAR");
    let structure = RtStructureSet::from_dataset(&valid).expect("structure parse");
    let reference = RtReferenceGeometry {
        rows: 2,
        cols: 2,
        pixel_spacing: (1.0, 1.0),
        ipp: [0.0, 0.0, 0.0],
        iop: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        frame_of_reference_uid: "9.8.7".to_string(),
    };
    let plane_err = structure
        .overlay_on(&reference)
        .expect_err("plane mismatch");
    assert!(matches!(plane_err.kind, ErrorKind::InvalidGeometry { .. }));
}

#[test]
fn rt_plan_summary_requires_matching_structure_set_reference() {
    // REQ-HI-427
    let structure = RtStructureSet::from_dataset(&build_structure_dataset_with_type(
        "0\\0\\0\\1\\0\\0\\1\\1\\0\\0\\1\\0",
        "CLOSED_PLANAR",
    ))
    .expect("structure parse");

    let mut plan = Dataset::new();
    plan.insert(Element {
        tag: TAG_FRAME_OF_REFERENCE_UID,
        vr: Vr::Ui,
        value: Value::Uid("9.8.7".to_string()),
    });
    plan.insert(Element {
        tag: TAG_RT_PLAN_LABEL,
        vr: Vr::Sh,
        value: Value::Str("PLAN".to_string()),
    });
    let mut ref_item = Dataset::new();
    ref_item.insert(Element {
        tag: TAG_REFERENCED_SOP_INSTANCE_UID,
        vr: Vr::Ui,
        value: Value::Uid("9.9.9.9".to_string()),
    });
    plan.insert(Element {
        tag: TAG_REFERENCED_STRUCTURE_SET_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![ref_item]),
    });

    let summary = RtPlanSummary::from_dataset(&plan).expect("plan parse");
    let err = summary
        .validate_structure_set(&structure)
        .expect_err("reference mismatch");
    assert!(matches!(err.kind, ErrorKind::InvalidTagValue { .. }));
}
