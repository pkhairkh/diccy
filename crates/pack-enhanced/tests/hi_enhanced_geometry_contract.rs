use dicom_core::{Dataset, Element, Limits, Value, Vr};
use pack_enhanced::{
    extract_frame_geometry, extract_frame_rescale, select_frame_groups, EnhancedRescale,
    TAG_FRAME_CONTENT_SEQUENCE, TAG_IMAGE_ORIENTATION, TAG_IMAGE_POSITION,
    TAG_IN_STACK_POSITION_NUMBER, TAG_NUMBER_OF_FRAMES, TAG_PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
    TAG_PIXEL_MEASURES_SEQUENCE, TAG_PIXEL_SPACING, TAG_PIXEL_VALUE_TRANSFORM_SEQUENCE,
    TAG_PLANE_ORIENTATION_SEQUENCE, TAG_PLANE_POSITION_SEQUENCE, TAG_RESCALE_INTERCEPT,
    TAG_RESCALE_SLOPE, TAG_SHARED_FUNCTIONAL_GROUPS_SEQUENCE, TAG_SLICE_THICKNESS,
};

fn build_enhanced_dataset_with_values(
    number_of_frames: &str,
    pixel_spacing: &str,
    slice_thickness: Option<&str>,
    in_stack_position: &str,
) -> Dataset {
    let mut shared = Dataset::new();

    let mut pixel_measures = Dataset::new();
    pixel_measures.insert(Element {
        tag: TAG_PIXEL_SPACING,
        vr: Vr::Ds,
        value: Value::Str(pixel_spacing.to_string()),
    });
    if let Some(value) = slice_thickness {
        pixel_measures.insert(Element {
            tag: TAG_SLICE_THICKNESS,
            vr: Vr::Ds,
            value: Value::Str(value.to_string()),
        });
    }
    shared.insert(Element {
        tag: TAG_PIXEL_MEASURES_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![pixel_measures]),
    });

    let mut transform = Dataset::new();
    transform.insert(Element {
        tag: TAG_RESCALE_SLOPE,
        vr: Vr::Ds,
        value: Value::Str("2".to_string()),
    });
    transform.insert(Element {
        tag: TAG_RESCALE_INTERCEPT,
        vr: Vr::Ds,
        value: Value::Str("5".to_string()),
    });
    shared.insert(Element {
        tag: TAG_PIXEL_VALUE_TRANSFORM_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![transform]),
    });

    let mut per_frame = Dataset::new();
    let mut plane_position = Dataset::new();
    plane_position.insert(Element {
        tag: TAG_IMAGE_POSITION,
        vr: Vr::Ds,
        value: Value::Str("0\\0\\0".to_string()),
    });
    per_frame.insert(Element {
        tag: TAG_PLANE_POSITION_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![plane_position]),
    });

    let mut plane_orientation = Dataset::new();
    plane_orientation.insert(Element {
        tag: TAG_IMAGE_ORIENTATION,
        vr: Vr::Ds,
        value: Value::Str("1\\0\\0\\0\\1\\0".to_string()),
    });
    per_frame.insert(Element {
        tag: TAG_PLANE_ORIENTATION_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![plane_orientation]),
    });

    let mut frame_content = Dataset::new();
    frame_content.insert(Element {
        tag: TAG_IN_STACK_POSITION_NUMBER,
        vr: Vr::Is,
        value: Value::Str(in_stack_position.to_string()),
    });
    per_frame.insert(Element {
        tag: TAG_FRAME_CONTENT_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![frame_content]),
    });

    let mut dataset = Dataset::new();
    dataset.insert(Element {
        tag: TAG_NUMBER_OF_FRAMES,
        vr: Vr::Is,
        value: Value::Str(number_of_frames.to_string()),
    });
    dataset.insert(Element {
        tag: TAG_SHARED_FUNCTIONAL_GROUPS_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![shared]),
    });
    dataset.insert(Element {
        tag: TAG_PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![per_frame]),
    });
    dataset
}

#[test]
fn enhanced_group_selection_enforces_number_of_frames_consistency() {
    // REQ-HI-402
    let dataset = build_enhanced_dataset_with_values("2", "0.5\\0.5", None, "1");
    let err = select_frame_groups(&dataset, 0, &Limits::default()).expect_err("count mismatch");
    assert_eq!(err.code, "DVF.DICOM.INVALID_TAG_VALUE");
}

#[test]
fn enhanced_geometry_validation_fails_closed_for_invalid_spacing_and_frame_content() {
    // REQ-HI-403, REQ-HI-404, REQ-HI-405
    let spacing_invalid = build_enhanced_dataset_with_values("1", "0\\1", None, "1");
    let groups = select_frame_groups(&spacing_invalid, 0, &Limits::default()).expect("groups");
    let spacing_err = extract_frame_geometry(&groups, &Limits::default()).expect_err("spacing");
    assert_eq!(spacing_err.code, "DVF.DICOM.INVALID_TAG_VALUE");

    let frame_content_invalid = build_enhanced_dataset_with_values("1", "1\\1", None, "0");
    let groups =
        select_frame_groups(&frame_content_invalid, 0, &Limits::default()).expect("groups");
    let frame_err = extract_frame_geometry(&groups, &Limits::default()).expect_err("frame");
    assert_eq!(frame_err.code, "DVF.DICOM.INVALID_TAG_VALUE");
}

#[test]
fn enhanced_geometry_and_rescale_outputs_are_deterministic() {
    // REQ-HI-406
    let dataset = build_enhanced_dataset_with_values("1", "0.5\\0.5", Some("1.5"), "1");
    let limits = Limits::default();
    let groups_a = select_frame_groups(&dataset, 0, &limits).expect("groups a");
    let groups_b = select_frame_groups(&dataset, 0, &limits).expect("groups b");

    let geom_a = extract_frame_geometry(&groups_a, &limits).expect("geometry a");
    let geom_b = extract_frame_geometry(&groups_b, &limits).expect("geometry b");
    assert_eq!(geom_a, geom_b);

    let rescale_a = extract_frame_rescale(&groups_a, &limits).expect("rescale a");
    let rescale_b = extract_frame_rescale(&groups_b, &limits).expect("rescale b");
    assert_eq!(
        rescale_a,
        Some(EnhancedRescale {
            slope: 2.0,
            intercept: 5.0,
        })
    );
    assert_eq!(rescale_a, rescale_b);
}
