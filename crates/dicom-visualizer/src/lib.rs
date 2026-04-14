#![deny(missing_docs)]

//! Export-control primitives for provenance-safe preview/report workflows.

use dicom_core::{validate_uid_strict, Dataset, Error, ErrorKind, Result, Tag};
use std::collections::BTreeSet;

const TAG_STUDY_UID: Tag = Tag(0x0020, 0x000D);
const TAG_SERIES_UID: Tag = Tag(0x0020, 0x000E);
const TAG_INSTANCE_UID: Tag = Tag(0x0008, 0x0018);

/// Source object context bound to export/report artifacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportContextTuple {
    /// Study Instance UID.
    pub study_uid: String,
    /// Series Instance UID.
    pub series_uid: String,
    /// SOP Instance UID.
    pub instance_uid: String,
    /// Optional frame index for multi-frame provenance.
    pub frame_index: Option<u32>,
}

/// Measurement-to-source binding for export/report provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasurementBinding {
    /// Stable measurement identifier.
    pub measurement_id: String,
    /// Source context tuple referenced by the measurement.
    pub source_context: ExportContextTuple,
}

/// Export/report plan requiring explicit context and measurement provenance bindings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportPlan {
    /// Active source context selected in the interface.
    pub active_context: ExportContextTuple,
    /// Measurement bindings that must match the active context.
    pub bindings: Vec<MeasurementBinding>,
}

/// Supported derived-object layer types for UI workflows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DerivedObjectKind {
    /// GSPS presentation state.
    Gsps,
    /// DICOM segmentation.
    Seg,
    /// Structured report.
    Sr,
    /// RT derived content.
    Rt,
    /// Unsupported derived-object class.
    Unsupported(String),
}

/// Derived-object layer metadata rendered in the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedObjectLayerView {
    /// Layer kind.
    pub kind: DerivedObjectKind,
    /// Source object context.
    pub source_context: ExportContextTuple,
    /// Provenance metadata identifier.
    pub provenance_id: String,
    /// Whether the layer is interactive.
    pub interactive: bool,
}

/// Derived-object activation decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DerivedObjectLayerDecision {
    /// Derived object is supported and interactive.
    Supported(DerivedObjectLayerView),
    /// Derived object is blocked fail-closed and non-interactive.
    UnsupportedBlocked {
        /// Unsupported object label.
        kind: String,
        /// Structured block rationale.
        rationale: String,
    },
}

/// Report finalization request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportFinalizationRequest {
    /// Report identifier.
    pub report_id: String,
    /// Signer identity.
    pub signer_identity: String,
    /// Finalization timestamp.
    pub finalized_epoch_secs: u64,
    /// Application version context.
    pub application_version: String,
}

/// Finalized report metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalizedReportRecord {
    /// Report identifier.
    pub report_id: String,
    /// Signer identity.
    pub signer_identity: String,
    /// Finalization timestamp.
    pub finalized_epoch_secs: u64,
    /// Application version context.
    pub application_version: String,
}

/// Data-classification status for export previews.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataClassification {
    /// Identified export content.
    Identified,
    /// De-identified export content.
    DeIdentified,
}

/// Export preview metadata shown before execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportPreview {
    /// Classification status.
    pub classification: DataClassification,
    /// Optional de-identification profile identifier.
    pub deidentification_profile: Option<String>,
}

/// Export execution authorization request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportAuthorizationRequest {
    /// Preview metadata.
    pub preview: ExportPreview,
    /// Elevated privilege state for identified exports.
    pub has_elevated_privilege: bool,
    /// Explicit operator confirmation state.
    pub operator_confirmed: bool,
}

/// Deterministic import diff entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportDiffEntry {
    /// Stable key identifier.
    pub key: String,
    /// Previous value, if present.
    pub before: Option<String>,
    /// Incoming value, if present.
    pub after: Option<String>,
}

/// Report template governance metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportTemplateRecord {
    /// Template identifier.
    pub template_id: String,
    /// Template version.
    pub version: String,
    /// Approver identity.
    pub approved_by: String,
    /// Traceable checksum.
    pub checksum: String,
}

/// Bulk export state for progress/retry/cancellation UX.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkExportState {
    /// Total items scheduled.
    pub total_items: u32,
    /// Completed items.
    pub completed_items: u32,
    /// Retry count.
    pub retries: u32,
    /// Retry bound.
    pub max_retries: u32,
    /// Cancellation requested flag.
    pub cancellation_requested: bool,
}

/// Per-item delivery status for remote send workflows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteSendItemStatus {
    /// Source item identifier.
    pub item_id: String,
    /// Endpoint label.
    pub endpoint: String,
    /// Transport mode label.
    pub transport_mode: String,
    /// Delivery outcome.
    pub delivered: bool,
}

/// Evaluate derived-object layer support and provenance access controls.
pub fn evaluate_derived_object_layer(
    kind: DerivedObjectKind,
    source_context: ExportContextTuple,
    provenance_id: &str,
) -> Result<DerivedObjectLayerDecision> {
    if provenance_id.trim().is_empty() {
        return Err(decode_error(
            "derived-object layers require provenance metadata identifiers",
        ));
    }
    match kind {
        DerivedObjectKind::Unsupported(label) => {
            Ok(DerivedObjectLayerDecision::UnsupportedBlocked {
                kind: label,
                rationale: "unsupported derived-object content is fail-closed and non-interactive"
                    .to_string(),
            })
        }
        supported => Ok(DerivedObjectLayerDecision::Supported(
            DerivedObjectLayerView {
                kind: supported,
                source_context,
                provenance_id: provenance_id.to_string(),
                interactive: true,
            },
        )),
    }
}

/// Finalize a report with signer, timestamp, and version metadata.
pub fn finalize_report(request: &ReportFinalizationRequest) -> Result<FinalizedReportRecord> {
    if request.report_id.trim().is_empty()
        || request.signer_identity.trim().is_empty()
        || request.application_version.trim().is_empty()
    {
        return Err(decode_error(
            "report finalization requires report_id, signer_identity, and application_version",
        ));
    }
    if request.finalized_epoch_secs == 0 {
        return Err(decode_error(
            "report finalization requires non-zero timestamp",
        ));
    }
    Ok(FinalizedReportRecord {
        report_id: request.report_id.clone(),
        signer_identity: request.signer_identity.clone(),
        finalized_epoch_secs: request.finalized_epoch_secs,
        application_version: request.application_version.clone(),
    })
}

/// Validate export authorization using classification, de-id profile, and privilege checks.
pub fn validate_export_authorization(request: &ExportAuthorizationRequest) -> Result<()> {
    match request.preview.classification {
        DataClassification::DeIdentified => {
            let profile = request
                .preview
                .deidentification_profile
                .as_deref()
                .ok_or_else(|| {
                    decode_error("de-identified export requires explicit profile selection")
                })?;
            if profile.trim().is_empty() {
                return Err(decode_error(
                    "de-identified export profile must not be empty",
                ));
            }
        }
        DataClassification::Identified => {
            if !request.has_elevated_privilege || !request.operator_confirmed {
                return Err(decode_error(
                    "identified export requires elevated privilege and explicit confirmation",
                ));
            }
        }
    }
    Ok(())
}

/// Build deterministic diff entries for external SR/GSPS import review.
pub fn deterministic_import_diff(
    before: &[(String, String)],
    after: &[(String, String)],
) -> Vec<ImportDiffEntry> {
    let mut before_map = BTreeSet::new();
    let mut after_map = BTreeSet::new();
    for (key, value) in before {
        before_map.insert((key.clone(), value.clone()));
    }
    for (key, value) in after {
        after_map.insert((key.clone(), value.clone()));
    }

    let mut keys = BTreeSet::new();
    for (key, _) in &before_map {
        keys.insert(key.clone());
    }
    for (key, _) in &after_map {
        keys.insert(key.clone());
    }

    let mut diff = Vec::new();
    for key in keys {
        let before_value = before
            .iter()
            .find(|(candidate, _)| candidate == &key)
            .map(|(_, value)| value.clone());
        let after_value = after
            .iter()
            .find(|(candidate, _)| candidate == &key)
            .map(|(_, value)| value.clone());
        if before_value != after_value {
            diff.push(ImportDiffEntry {
                key,
                before: before_value,
                after: after_value,
            });
        }
    }
    diff
}

/// Validate that import acceptance is gated on explicit review confirmation.
pub fn validate_import_acceptance(review_confirmed: bool, diff: &[ImportDiffEntry]) -> Result<()> {
    if diff.is_empty() {
        return Ok(());
    }
    if !review_confirmed {
        return Err(integrity_error(
            "external import requires deterministic diff review before acceptance",
        ));
    }
    Ok(())
}

/// Validate report template traceability metadata.
pub fn validate_report_template_record(template: &ReportTemplateRecord) -> Result<()> {
    if template.template_id.trim().is_empty()
        || template.version.trim().is_empty()
        || template.approved_by.trim().is_empty()
        || template.checksum.trim().is_empty()
    {
        return Err(decode_error(
            "report templates require version, approver, and traceable checksum metadata",
        ));
    }
    Ok(())
}

/// Evaluate bulk export runtime state with retry bounds and cancellation handling.
pub fn evaluate_bulk_export_state(state: &BulkExportState) -> Result<()> {
    if state.total_items == 0 {
        return Err(decode_error("bulk export requires at least one item"));
    }
    if state.completed_items > state.total_items {
        return Err(integrity_error(
            "bulk export completed_items cannot exceed total_items",
        ));
    }
    if state.retries > state.max_retries {
        return Err(integrity_error(
            "bulk export retries exceed configured bound",
        ));
    }
    if state.cancellation_requested && state.completed_items < state.total_items {
        return Err(integrity_error(
            "bulk export cancellation stops execution before completion",
        ));
    }
    Ok(())
}

/// Validate remote-send item status visibility metadata.
pub fn validate_remote_send_status(items: &[RemoteSendItemStatus]) -> Result<()> {
    for item in items {
        if item.item_id.trim().is_empty()
            || item.endpoint.trim().is_empty()
            || item.transport_mode.trim().is_empty()
        {
            return Err(decode_error(
                "remote-send status requires item_id, endpoint, and transport_mode",
            ));
        }
    }
    Ok(())
}

/// Extract a source context tuple from a dataset.
///
/// Missing or invalid UIDs fail closed.
pub fn extract_export_context(dataset: &Dataset) -> Result<ExportContextTuple> {
    let study_uid = require_uid(dataset, TAG_STUDY_UID)?;
    let series_uid = require_uid(dataset, TAG_SERIES_UID)?;
    let instance_uid = require_uid(dataset, TAG_INSTANCE_UID)?;
    Ok(ExportContextTuple {
        study_uid,
        series_uid,
        instance_uid,
        frame_index: Some(0),
    })
}

/// Validate export/report bindings against the active context.
///
/// Returns an integrity error when any binding points to a mismatched context,
/// and a decode error when measurement identifiers are missing.
pub fn validate_export_plan(plan: &ExportPlan) -> Result<()> {
    let mut seen_measurement_ids = BTreeSet::new();
    for binding in &plan.bindings {
        if binding.measurement_id.trim().is_empty() {
            return Err(decode_error("measurement_id must not be empty"));
        }
        if !seen_measurement_ids.insert(binding.measurement_id.as_str()) {
            return Err(integrity_error("duplicate measurement_id in export plan"));
        }
        validate_context_match(&plan.active_context, &binding.source_context)?;
    }
    Ok(())
}

/// Return deterministic measurement identifiers for rendering/export metadata.
pub fn deterministic_measurement_ids(bindings: &[MeasurementBinding]) -> Vec<String> {
    let mut ids = bindings
        .iter()
        .map(|binding| binding.measurement_id.clone())
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

fn require_uid(dataset: &Dataset, tag: Tag) -> Result<String> {
    let value = dataset
        .get_uid(tag)
        .ok_or_else(|| missing_required_tag(tag))?;
    validate_uid_strict(tag, value)?;
    Ok(value.to_string())
}

fn validate_context_match(active: &ExportContextTuple, source: &ExportContextTuple) -> Result<()> {
    if active.study_uid != source.study_uid
        || active.series_uid != source.series_uid
        || active.instance_uid != source.instance_uid
        || active.frame_index != source.frame_index
    {
        return Err(integrity_error(
            "export provenance context mismatch between active case and measurement source",
        ));
    }
    Ok(())
}

fn missing_required_tag(tag: Tag) -> Box<Error> {
    Error::from_kind(
        ErrorKind::MissingRequiredTag { tag },
        "missing required tag",
    )
    .into()
}

fn decode_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::DecodeError {
            stage: "dicom-visualizer".to_string(),
            detail: detail.into(),
        },
        "decode error",
    )
    .into()
}

fn integrity_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::IntegrityError {
            detail: detail.into(),
        },
        "integrity error",
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_core::{Element, Value, Vr};

    fn context_dataset(study: &str, series: &str, instance: &str) -> Dataset {
        let mut dataset = Dataset::new();
        dataset.insert(Element {
            tag: TAG_STUDY_UID,
            vr: Vr::Ui,
            value: Value::Uid(study.to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SERIES_UID,
            vr: Vr::Ui,
            value: Value::Uid(series.to_string()),
        });
        dataset.insert(Element {
            tag: TAG_INSTANCE_UID,
            vr: Vr::Ui,
            value: Value::Uid(instance.to_string()),
        });
        dataset
    }

    #[test]
    fn export_plan_rejects_duplicate_measurement_ids() {
        let active = extract_export_context(&context_dataset("1.2.3", "1.2.3.4", "1.2.3.4.5"))
            .expect("context");
        let plan = ExportPlan {
            active_context: active.clone(),
            bindings: vec![
                MeasurementBinding {
                    measurement_id: "m-1".to_string(),
                    source_context: active.clone(),
                },
                MeasurementBinding {
                    measurement_id: "m-1".to_string(),
                    source_context: active,
                },
            ],
        };

        let err = validate_export_plan(&plan).expect_err("duplicate id must fail");
        assert!(matches!(err.kind, ErrorKind::IntegrityError { .. }));
    }
}
