#![deny(missing_docs)]

//! Study/series assembly and deterministic ordering.

use dicom_core::{Error, ErrorKind, Result, Tag};
use std::collections::BTreeMap;

/// SOP Class UID for CT Image Storage.
pub const SOP_CLASS_CT: &str = "1.2.840.10008.5.1.4.1.1.2";
const SOP_CLASS_MR: &str = "1.2.840.10008.5.1.4.1.1.4";
/// SOP Class UID for Multi-frame Single Bit Secondary Capture.
pub const SOP_CLASS_SC_MF_BYTE: &str = "1.2.840.10008.5.1.4.1.1.7.2";
const SOP_CLASS_SC_MF_WORD: &str = "1.2.840.10008.5.1.4.1.1.7.3";
const SOP_CLASS_SC_MF_COLOR: &str = "1.2.840.10008.5.1.4.1.1.7.4";

const TAG_STUDY_UID: Tag = Tag(0x0020, 0x000D);
const TAG_SERIES_UID: Tag = Tag(0x0020, 0x000E);
const TAG_SOP_UID: Tag = Tag(0x0008, 0x0018);
const TAG_SOP_CLASS_UID: Tag = Tag(0x0008, 0x0016);
const TAG_IMAGE_POSITION: Tag = Tag(0x0020, 0x0032);
const TAG_IMAGE_ORIENTATION: Tag = Tag(0x0020, 0x0037);

const GEOM_EPSILON: f64 = 1.0e-6;
const IOP_EPSILON: f64 = 1.0e-3;

/// Minimal instance header used for grouping and ordering.
#[derive(Debug, Clone, PartialEq)]
pub struct InstanceHeader {
    /// Study Instance UID.
    pub study_instance_uid: Option<String>,
    /// Series Instance UID.
    pub series_instance_uid: Option<String>,
    /// SOP Instance UID.
    pub sop_instance_uid: Option<String>,
    /// SOP Class UID.
    pub sop_class_uid: Option<String>,
    /// Optional Instance Number.
    pub instance_number: Option<i32>,
    /// Image Position (Patient), if available.
    pub image_position: Option<[f64; 3]>,
    /// Image Orientation (Patient), if available.
    pub image_orientation: Option<[f64; 6]>,
    /// Number of frames (defaults to 1 if absent).
    pub number_of_frames: Option<u32>,
    /// Pixel data byte length, if known.
    pub pixel_data_len: Option<u64>,
    /// Stable source order (path sort or input order).
    pub source_order: u64,
    /// Indicates whether the instance is in-envelope.
    pub in_envelope: bool,
}

/// A study grouping.
#[derive(Debug, Clone, PartialEq)]
pub struct Study {
    /// Study Instance UID.
    pub study_instance_uid: String,
    /// Series contained in this study.
    pub series: Vec<Series>,
}

/// A series warning with deterministic ordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesWarning {
    /// Warning kind.
    pub kind: SeriesWarningKind,
    /// Warning detail.
    pub detail: String,
}

/// Warning kinds emitted during series assembly.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SeriesWarningKind {
    /// Duplicate SOP Instance UID encountered.
    DuplicateSopInstanceUid,
    /// Non-monotonic slice order detected.
    NonMonotonicSliceOrder,
}

/// A series grouping.
#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    /// Series Instance UID.
    pub series_instance_uid: String,
    /// Frames belonging to the series.
    frames: Vec<FrameRef>,
    /// Series warnings.
    pub warnings: Vec<SeriesWarning>,
    /// Whether the series is eligible for volume assembly.
    pub volume_eligible: bool,
}

impl Series {
    /// Create an empty series.
    pub fn new(series_instance_uid: impl Into<String>) -> Self {
        Self {
            series_instance_uid: series_instance_uid.into(),
            frames: Vec::new(),
            warnings: Vec::new(),
            volume_eligible: true,
        }
    }

    /// Insert a frame reference.
    pub fn push_frame(&mut self, frame: FrameRef) {
        self.frames.push(frame);
    }

    /// Iterate over frames in deterministic order.
    pub fn frames(&self) -> impl Iterator<Item = &FrameRef> {
        self.frames.iter()
    }
}

/// A stable frame key for identifying a frame within a series.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FrameKey {
    /// Series UID.
    pub series_uid: String,
    /// Instance UID.
    pub instance_uid: String,
    /// Frame index (0-based).
    pub frame_index: u32,
}

/// A frame reference used by the viewer pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameRef {
    /// Frame key.
    pub key: FrameKey,
}

/// Assemble studies and series from instance headers.
pub fn assemble(instances: &[InstanceHeader]) -> Result<Vec<Study>> {
    let mut study_map: BTreeMap<String, BTreeMap<String, Vec<&InstanceHeader>>> = BTreeMap::new();

    for instance in instances {
        let study_uid = require_uid(&instance.study_instance_uid, TAG_STUDY_UID)?;
        let series_uid = require_uid(&instance.series_instance_uid, TAG_SERIES_UID)?;
        let _ = require_uid(&instance.sop_instance_uid, TAG_SOP_UID)?;
        let _ = require_uid(&instance.sop_class_uid, TAG_SOP_CLASS_UID)?;

        study_map
            .entry(study_uid.to_string())
            .or_default()
            .entry(series_uid.to_string())
            .or_default()
            .push(instance);
    }

    let mut studies = Vec::new();

    for (study_uid, series_map) in study_map {
        let mut series_vec = Vec::new();
        for (series_uid, instances) in series_map {
            let series = assemble_series(&series_uid, &instances)?;
            series_vec.push(series);
        }
        studies.push(Study {
            study_instance_uid: study_uid,
            series: series_vec,
        });
    }

    Ok(studies)
}

fn assemble_series(series_uid: &str, instances: &[&InstanceHeader]) -> Result<Series> {
    let mut series = Series::new(series_uid);

    let mut by_sop: BTreeMap<&str, Vec<&InstanceHeader>> = BTreeMap::new();
    for instance in instances {
        let sop_uid = require_uid(&instance.sop_instance_uid, TAG_SOP_UID)?;
        by_sop.entry(sop_uid).or_default().push(*instance);
    }

    let mut selected_instances = Vec::new();
    let mut duplicate_uids = Vec::new();

    for (sop_uid, dupes) in by_sop {
        if dupes.len() > 1 {
            duplicate_uids.push(sop_uid.to_string());
        }
        let selected = select_best_instance(dupes)?;
        selected_instances.push(selected);
    }

    if !duplicate_uids.is_empty() {
        duplicate_uids.sort();
        series.warnings.push(SeriesWarning {
            kind: SeriesWarningKind::DuplicateSopInstanceUid,
            detail: format!("duplicate SOP Instance UIDs: {}", duplicate_uids.join(",")),
        });
    }

    if is_ct_or_mr_series(&selected_instances) && is_single_frame_series(&selected_instances) {
        order_ct_mr(series_uid, &mut series, &selected_instances)?;
        return Ok(series);
    }

    if is_sc_multiframe_series(&selected_instances) {
        order_sc_multiframe(series_uid, &mut series, &selected_instances);
        return Ok(series);
    }

    order_fallback(series_uid, &mut series, &selected_instances);
    Ok(series)
}

fn order_ct_mr(series_uid: &str, series: &mut Series, instances: &[&InstanceHeader]) -> Result<()> {
    let mut entries = Vec::new();

    for instance in instances {
        let iop = instance
            .image_orientation
            .ok_or_else(|| missing_tag(TAG_IMAGE_ORIENTATION))?;
        let ipp = instance
            .image_position
            .ok_or_else(|| missing_tag(TAG_IMAGE_POSITION))?;
        let row = [iop[0], iop[1], iop[2]];
        let col = [iop[3], iop[4], iop[5]];
        if !is_valid_iop(row, col) {
            return Err(Box::new(Error::from_kind(
                ErrorKind::InvalidGeometry {
                    detail: "invalid image orientation cosines".to_string(),
                },
                "invalid geometry",
            )));
        }
        let normal = cross(row, col);
        if !is_valid_normal(normal) {
            return Err(Box::new(Error::from_kind(
                ErrorKind::InvalidGeometry {
                    detail: "invalid image orientation normal".to_string(),
                },
                "invalid geometry",
            )));
        }
        let slice_coord = dot(normal, ipp);
        if !slice_coord.is_finite() {
            return Err(Box::new(Error::from_kind(
                ErrorKind::InvalidGeometry {
                    detail: "non-finite slice coordinate".to_string(),
                },
                "invalid geometry",
            )));
        }
        let instance_number = instance.instance_number.unwrap_or(0);
        let sop_uid = require_uid(&instance.sop_instance_uid, TAG_SOP_UID)?;
        entries.push((slice_coord, instance_number, sop_uid.to_string(), *instance));
    }

    entries.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.cmp(&b.1))
            .then(a.2.cmp(&b.2))
    });

    if !is_monotonic(&entries) {
        series.volume_eligible = false;
        series.warnings.push(SeriesWarning {
            kind: SeriesWarningKind::NonMonotonicSliceOrder,
            detail: "non-monotonic slice order".to_string(),
        });
    }

    for (_, _, _, instance) in entries {
        let instance_uid = require_uid(&instance.sop_instance_uid, TAG_SOP_UID)?;
        let frame_key = FrameKey {
            series_uid: series_uid.to_string(),
            instance_uid: instance_uid.to_string(),
            frame_index: 0,
        };
        series.push_frame(FrameRef { key: frame_key });
    }

    Ok(())
}

fn order_sc_multiframe(series_uid: &str, series: &mut Series, instances: &[&InstanceHeader]) {
    let mut entries: Vec<&InstanceHeader> = instances.to_vec();
    entries.sort_by_key(|instance| instance.source_order);

    for instance in entries {
        let frames = instance.number_of_frames.unwrap_or(1);
        let instance_uid = instance
            .sop_instance_uid
            .as_deref()
            .unwrap_or_default()
            .to_string();
        for frame_index in 0..frames {
            let frame_key = FrameKey {
                series_uid: series_uid.to_string(),
                instance_uid: instance_uid.clone(),
                frame_index,
            };
            series.push_frame(FrameRef { key: frame_key });
        }
    }
}

fn order_fallback(series_uid: &str, series: &mut Series, instances: &[&InstanceHeader]) {
    let mut entries: Vec<&InstanceHeader> = instances.to_vec();
    entries.sort_by(|a, b| {
        a.source_order
            .cmp(&b.source_order)
            .then(
                a.instance_number
                    .unwrap_or(0)
                    .cmp(&b.instance_number.unwrap_or(0)),
            )
            .then(a.sop_instance_uid.cmp(&b.sop_instance_uid))
    });

    for instance in entries {
        let frames = instance.number_of_frames.unwrap_or(1);
        let instance_uid = instance
            .sop_instance_uid
            .as_deref()
            .unwrap_or_default()
            .to_string();
        for frame_index in 0..frames {
            let frame_key = FrameKey {
                series_uid: series_uid.to_string(),
                instance_uid: instance_uid.clone(),
                frame_index,
            };
            series.push_frame(FrameRef { key: frame_key });
        }
    }
}

fn select_best_instance(instances: Vec<&InstanceHeader>) -> Result<&InstanceHeader> {
    let mut best = instances[0];
    for candidate in instances.iter().skip(1) {
        let best_in_env = best.in_envelope;
        let cand_in_env = candidate.in_envelope;
        if cand_in_env && !best_in_env {
            best = candidate;
            continue;
        }
        if cand_in_env == best_in_env {
            let best_len = best.pixel_data_len.unwrap_or(0);
            let cand_len = candidate.pixel_data_len.unwrap_or(0);
            if cand_len > best_len {
                best = candidate;
                continue;
            }
            if cand_len == best_len && candidate.source_order < best.source_order {
                best = candidate;
            }
        }
    }
    Ok(best)
}

fn require_uid(value: &Option<String>, tag: Tag) -> Result<&str> {
    match value.as_deref() {
        Some(uid) if !uid.trim().is_empty() => Ok(uid),
        _ => Err(missing_tag(tag)),
    }
}

fn missing_tag(tag: Tag) -> Box<Error> {
    Box::new(Error::from_kind(
        ErrorKind::MissingRequiredTag { tag },
        "missing required tag",
    ))
}

fn is_ct_or_mr_series(instances: &[&InstanceHeader]) -> bool {
    instances.iter().all(|instance| {
        matches!(
            instance.sop_class_uid.as_deref(),
            Some(SOP_CLASS_CT) | Some(SOP_CLASS_MR)
        )
    })
}

fn is_single_frame_series(instances: &[&InstanceHeader]) -> bool {
    instances
        .iter()
        .all(|instance| instance.number_of_frames.unwrap_or(1) <= 1)
}

fn is_sc_multiframe_series(instances: &[&InstanceHeader]) -> bool {
    instances.iter().all(|instance| {
        matches!(
            instance.sop_class_uid.as_deref(),
            Some(SOP_CLASS_SC_MF_BYTE) | Some(SOP_CLASS_SC_MF_WORD) | Some(SOP_CLASS_SC_MF_COLOR)
        )
    })
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn is_valid_normal(normal: [f64; 3]) -> bool {
    if !normal[0].is_finite() || !normal[1].is_finite() || !normal[2].is_finite() {
        return false;
    }
    let norm = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    norm > GEOM_EPSILON
}

fn is_valid_iop(row: [f64; 3], col: [f64; 3]) -> bool {
    if !row[0].is_finite()
        || !row[1].is_finite()
        || !row[2].is_finite()
        || !col[0].is_finite()
        || !col[1].is_finite()
        || !col[2].is_finite()
    {
        return false;
    }
    let row_norm = (row[0] * row[0] + row[1] * row[1] + row[2] * row[2]).sqrt();
    let col_norm = (col[0] * col[0] + col[1] * col[1] + col[2] * col[2]).sqrt();
    let dot_rc = dot(row, col).abs();

    (row_norm - 1.0).abs() <= IOP_EPSILON
        && (col_norm - 1.0).abs() <= IOP_EPSILON
        && dot_rc <= IOP_EPSILON
}

fn is_monotonic(entries: &[(f64, i32, String, &InstanceHeader)]) -> bool {
    if entries.len() < 2 {
        return true;
    }
    let mut by_source: Vec<(u64, f64)> = entries
        .iter()
        .map(|(slice_coord, _, _, instance)| (instance.source_order, *slice_coord))
        .collect();
    by_source.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    });

    let mut prev = by_source[0].1;
    for (_, current) in by_source.iter().skip(1) {
        if *current + GEOM_EPSILON < prev {
            return false;
        }
        prev = *current;
    }
    true
}
