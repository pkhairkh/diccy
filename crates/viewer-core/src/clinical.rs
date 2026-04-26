//! Clinical workflow API surface for measurements, segmentation, fusion, RT overlays, and
//! volumetric controls.

use crate::{ImagePoint, MeasurementKind, MeasurementUnit};
use std::fmt;

/// Deterministic clinical workflow errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClinicalError {
    /// Referenced object id was not found.
    NotFound {
        /// Missing object identifier.
        id: String,
    },
    /// Requested mutation is blocked by a lock.
    Locked {
        /// Locked object identifier.
        id: String,
    },
    /// Input payload failed deterministic validation.
    InvalidInput {
        /// Validation failure detail.
        detail: String,
    },
    /// Requested operation is disabled by capability gating.
    CapabilityDisabled {
        /// Capability key.
        capability: &'static str,
    },
}

impl fmt::Display for ClinicalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClinicalError::NotFound { id } => write!(f, "clinical object not found: {id}"),
            ClinicalError::Locked { id } => write!(f, "clinical object is locked: {id}"),
            ClinicalError::InvalidInput { detail } => write!(f, "invalid clinical input: {detail}"),
            ClinicalError::CapabilityDisabled { capability } => {
                write!(f, "clinical capability disabled: {capability}")
            }
        }
    }
}

impl std::error::Error for ClinicalError {}

/// Immutable clinical audit event emitted by workflow mutations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClinicalAuditEvent {
    /// Deterministic event code.
    pub event_code: &'static str,
    /// Optional subject identifier.
    pub subject_id: Option<String>,
    /// Logical monotonic tick for deterministic ordering.
    pub tick: u64,
}

/// Persisted measurement record.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasurementRecord {
    /// Stable measurement identifier.
    pub id: String,
    /// Measurement kind.
    pub kind: MeasurementKind,
    /// Ordered measurement points.
    pub points: Vec<ImagePoint>,
    /// Optional computed measurement value.
    pub value: Option<f64>,
    /// Measurement unit.
    pub unit: MeasurementUnit,
    /// Monotonic create tick.
    pub created_at_tick: u64,
    /// Monotonic update tick.
    pub updated_at_tick: u64,
    /// Monotonic revision.
    pub revision: u32,
    /// Soft-delete marker.
    pub deleted: bool,
}

/// Deterministic export bundle for measurement workflows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportBundle {
    /// CSV export payload.
    pub csv: String,
    /// JSON export payload.
    pub json: String,
}

/// SR-compatible measurement payload stub for writeback integrations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrMeasurementPayload {
    /// Source marker for downstream routing.
    pub source: &'static str,
    /// Number of active measurements serialized into this payload.
    pub measurement_count: usize,
}

/// Deterministic measurement lifecycle store with undo/redo support.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasurementStore {
    records: Vec<MeasurementRecord>,
    undo_stack: Vec<Vec<MeasurementRecord>>,
    redo_stack: Vec<Vec<MeasurementRecord>>,
    next_id: u64,
    tick: u64,
    audit_events: Vec<ClinicalAuditEvent>,
}

impl Default for MeasurementStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MeasurementStore {
    /// Create an empty measurement store.
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            next_id: 0,
            tick: 0,
            audit_events: Vec::new(),
        }
    }

    /// Return active measurements in deterministic id order.
    pub fn active_measurements(&self) -> Vec<&MeasurementRecord> {
        let mut out = self
            .records
            .iter()
            .filter(|record| !record.deleted)
            .collect::<Vec<_>>();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    /// Lookup one measurement by id.
    pub fn measurement(&self, id: &str) -> Option<&MeasurementRecord> {
        self.records.iter().find(|record| record.id == id)
    }

    /// Create a measurement entry and return its deterministic id.
    pub fn create_measurement(
        &mut self,
        kind: MeasurementKind,
        points: Vec<ImagePoint>,
        value: Option<f64>,
        unit: MeasurementUnit,
    ) -> Result<String, ClinicalError> {
        validate_points(&points)?;
        validate_optional_number(value)?;
        self.capture_undo_state();
        self.tick = self.tick.saturating_add(1);
        let id = format!("m{}", self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.records.push(MeasurementRecord {
            id: id.clone(),
            kind,
            points,
            value,
            unit,
            created_at_tick: self.tick,
            updated_at_tick: self.tick,
            revision: 0,
            deleted: false,
        });
        self.audit_events.push(ClinicalAuditEvent {
            event_code: "viewer.measurement.created",
            subject_id: Some(id.clone()),
            tick: self.tick,
        });
        Ok(id)
    }

    /// Update points/value/unit for an existing measurement id.
    pub fn update_measurement(
        &mut self,
        id: &str,
        points: Vec<ImagePoint>,
        value: Option<f64>,
        unit: MeasurementUnit,
    ) -> Result<(), ClinicalError> {
        validate_points(&points)?;
        validate_optional_number(value)?;
        let index = self
            .records
            .iter()
            .position(|record| record.id == id)
            .ok_or_else(|| ClinicalError::NotFound { id: id.to_string() })?;
        if self.records[index].deleted {
            return Err(ClinicalError::NotFound { id: id.to_string() });
        }
        self.capture_undo_state();
        self.tick = self.tick.saturating_add(1);
        let record = &mut self.records[index];
        record.points = points;
        record.value = value;
        record.unit = unit;
        record.updated_at_tick = self.tick;
        record.revision = record.revision.saturating_add(1);
        self.audit_events.push(ClinicalAuditEvent {
            event_code: "viewer.measurement.updated",
            subject_id: Some(id.to_string()),
            tick: self.tick,
        });
        Ok(())
    }

    /// Soft-delete a measurement.
    pub fn delete_measurement(&mut self, id: &str) -> Result<(), ClinicalError> {
        let index = self
            .records
            .iter()
            .position(|record| record.id == id)
            .ok_or_else(|| ClinicalError::NotFound { id: id.to_string() })?;
        if self.records[index].deleted {
            return Ok(());
        }
        self.capture_undo_state();
        self.tick = self.tick.saturating_add(1);
        let record = &mut self.records[index];
        record.deleted = true;
        record.updated_at_tick = self.tick;
        record.revision = record.revision.saturating_add(1);
        self.audit_events.push(ClinicalAuditEvent {
            event_code: "viewer.measurement.deleted",
            subject_id: Some(id.to_string()),
            tick: self.tick,
        });
        Ok(())
    }

    /// Undo the latest measurement mutation.
    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo_stack.pop() else {
            return false;
        };
        self.redo_stack.push(self.records.clone());
        self.records = previous;
        self.tick = self.tick.saturating_add(1);
        true
    }

    /// Redo the latest undone measurement mutation.
    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo_stack.pop() else {
            return false;
        };
        self.undo_stack.push(self.records.clone());
        self.records = next;
        self.tick = self.tick.saturating_add(1);
        true
    }

    /// Return an image-space jump target for a measurement id.
    pub fn jump_target(&self, id: &str) -> Option<ImagePoint> {
        self.measurement(id)
            .filter(|record| !record.deleted)
            .and_then(|record| record.points.first().copied())
    }

    /// Export active measurements as deterministic CSV and JSON payloads.
    pub fn export_bundle(&mut self) -> ExportBundle {
        let active = self
            .active_measurements()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        self.tick = self.tick.saturating_add(1);
        self.audit_events.push(ClinicalAuditEvent {
            event_code: "viewer.measurement.exported",
            subject_id: None,
            tick: self.tick,
        });
        ExportBundle {
            csv: to_measurement_csv(&active),
            json: to_measurement_json(&active),
        }
    }

    /// Snapshot all records for deterministic persistence/restore.
    pub fn snapshot(&self) -> Vec<MeasurementRecord> {
        self.records.clone()
    }

    /// Restore full measurement state from a snapshot.
    pub fn restore_from_snapshot(&mut self, snapshot: Vec<MeasurementRecord>) {
        self.records = snapshot;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.tick = self.tick.saturating_add(1);
    }

    /// Build SR-compatible measurement writeback payload metadata.
    pub fn sr_payload(&self) -> SrMeasurementPayload {
        SrMeasurementPayload {
            source: "viewer-core.measurements",
            measurement_count: self.active_measurements().len(),
        }
    }

    /// Return immutable clinical audit events emitted by this store.
    pub fn audit_events(&self) -> &[ClinicalAuditEvent] {
        &self.audit_events
    }

    fn capture_undo_state(&mut self) {
        self.undo_stack.push(self.records.clone());
        self.redo_stack.clear();
    }
}

/// Segmentation source type.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SegmentationSource {
    /// Imported from SEG payload.
    SegImport,
    /// Created from labelmap tool.
    LabelMap,
}

/// Mutable segmentation style controls.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct SegmentationStyle {
    /// Layer visibility.
    pub visible: bool,
    /// Layer opacity in `[0, 1]`.
    pub opacity: f32,
    /// RGB color triplet.
    pub color_rgb: [u8; 3],
    /// Active-edit flag.
    pub active: bool,
}

impl Default for SegmentationStyle {
    fn default() -> Self {
        Self {
            visible: true,
            opacity: 0.6,
            color_rgb: [255, 64, 64],
            active: false,
        }
    }
}

/// Deterministic segmentation change descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentationDiff {
    /// Target segmentation id.
    pub segment_id: String,
    /// Revision after the change.
    pub revision: u32,
    /// Changed field names.
    pub changed_fields: Vec<String>,
    /// Logical monotonic tick.
    pub tick: u64,
}

/// Persisted segmentation record.
#[derive(Debug, Clone, PartialEq)]
pub struct SegmentationRecord {
    /// Stable segmentation id.
    pub id: String,
    /// Operator-visible label.
    pub label: String,
    /// Source kind.
    pub source: SegmentationSource,
    /// Lock state.
    pub locked: bool,
    /// Current style controls.
    pub style: SegmentationStyle,
    /// Monotonic revision counter.
    pub revision: u32,
    /// Soft-delete marker.
    pub deleted: bool,
    /// Last update tick.
    pub updated_at_tick: u64,
}

/// Segmentation lifecycle store with deterministic revisions and diffs.
#[derive(Debug, Clone, PartialEq)]
pub struct SegmentationStore {
    segments: Vec<SegmentationRecord>,
    next_id: u64,
    tick: u64,
    audit_events: Vec<ClinicalAuditEvent>,
}

impl Default for SegmentationStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SegmentationStore {
    /// Create an empty segmentation store.
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
            next_id: 0,
            tick: 0,
            audit_events: Vec::new(),
        }
    }

    /// Import one segmentation layer from SEG.
    pub fn import_seg(&mut self, label: &str) -> Result<String, ClinicalError> {
        self.create(label, SegmentationSource::SegImport)
    }

    /// Create one segmentation layer from a labelmap workflow.
    pub fn create_labelmap(&mut self, label: &str) -> Result<String, ClinicalError> {
        self.create(label, SegmentationSource::LabelMap)
    }

    /// Return active segmentations in deterministic id order.
    pub fn active_segments(&self) -> Vec<&SegmentationRecord> {
        let mut out = self
            .segments
            .iter()
            .filter(|record| !record.deleted)
            .collect::<Vec<_>>();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    /// Lookup one segmentation by id.
    pub fn segment(&self, id: &str) -> Option<&SegmentationRecord> {
        self.segments.iter().find(|record| record.id == id)
    }

    /// Update segmentation style and emit a deterministic diff.
    pub fn update_style(
        &mut self,
        id: &str,
        style: SegmentationStyle,
    ) -> Result<SegmentationDiff, ClinicalError> {
        if !(0.0..=1.0).contains(&style.opacity) {
            return Err(ClinicalError::InvalidInput {
                detail: "segmentation opacity must be in [0,1]".to_string(),
            });
        }
        let index = self
            .segments
            .iter()
            .position(|record| record.id == id)
            .ok_or_else(|| ClinicalError::NotFound { id: id.to_string() })?;
        if self.segments[index].locked {
            return Err(ClinicalError::Locked { id: id.to_string() });
        }
        self.tick = self.tick.saturating_add(1);
        let record = &mut self.segments[index];
        let mut changed_fields = Vec::new();
        if record.style.visible != style.visible {
            changed_fields.push("visible".to_string());
        }
        if (record.style.opacity - style.opacity).abs() > f32::EPSILON {
            changed_fields.push("opacity".to_string());
        }
        if record.style.color_rgb != style.color_rgb {
            changed_fields.push("color_rgb".to_string());
        }
        if record.style.active != style.active {
            changed_fields.push("active".to_string());
        }
        record.style = style;
        record.revision = record.revision.saturating_add(1);
        record.updated_at_tick = self.tick;
        let diff = SegmentationDiff {
            segment_id: id.to_string(),
            revision: record.revision,
            changed_fields,
            tick: self.tick,
        };
        self.audit_events.push(ClinicalAuditEvent {
            event_code: "viewer.segmentation.updated",
            subject_id: Some(id.to_string()),
            tick: self.tick,
        });
        Ok(diff)
    }

    /// Lock/unlock one segmentation.
    pub fn set_locked(&mut self, id: &str, locked: bool) -> Result<(), ClinicalError> {
        let index = self
            .segments
            .iter()
            .position(|record| record.id == id)
            .ok_or_else(|| ClinicalError::NotFound { id: id.to_string() })?;
        self.tick = self.tick.saturating_add(1);
        let record = &mut self.segments[index];
        record.locked = locked;
        record.revision = record.revision.saturating_add(1);
        record.updated_at_tick = self.tick;
        self.audit_events.push(ClinicalAuditEvent {
            event_code: "viewer.segmentation.lock_changed",
            subject_id: Some(id.to_string()),
            tick: self.tick,
        });
        Ok(())
    }

    /// Soft-delete one segmentation.
    pub fn remove(&mut self, id: &str) -> Result<(), ClinicalError> {
        let index = self
            .segments
            .iter()
            .position(|record| record.id == id)
            .ok_or_else(|| ClinicalError::NotFound { id: id.to_string() })?;
        self.tick = self.tick.saturating_add(1);
        let record = &mut self.segments[index];
        record.deleted = true;
        record.revision = record.revision.saturating_add(1);
        record.updated_at_tick = self.tick;
        self.audit_events.push(ClinicalAuditEvent {
            event_code: "viewer.segmentation.deleted",
            subject_id: Some(id.to_string()),
            tick: self.tick,
        });
        Ok(())
    }

    /// Return immutable audit events emitted by this store.
    pub fn audit_events(&self) -> &[ClinicalAuditEvent] {
        &self.audit_events
    }

    fn create(&mut self, label: &str, source: SegmentationSource) -> Result<String, ClinicalError> {
        if label.trim().is_empty() {
            return Err(ClinicalError::InvalidInput {
                detail: "segmentation label must be non-empty".to_string(),
            });
        }
        self.tick = self.tick.saturating_add(1);
        let id = format!("seg{}", self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.segments.push(SegmentationRecord {
            id: id.clone(),
            label: label.to_string(),
            source,
            locked: false,
            style: SegmentationStyle::default(),
            revision: 0,
            deleted: false,
            updated_at_tick: self.tick,
        });
        self.audit_events.push(ClinicalAuditEvent {
            event_code: "viewer.segmentation.created",
            subject_id: Some(id.clone()),
            tick: self.tick,
        });
        Ok(id)
    }
}

/// Volumetric workflow capability contract.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct VolumeWorkflowCapabilities {
    /// MPR reformat capability.
    pub mpr: bool,
    /// MIP projection capability.
    pub mip: bool,
    /// 3D volume rendering capability.
    pub volume_3d: bool,
}

impl Default for VolumeWorkflowCapabilities {
    fn default() -> Self {
        Self {
            mpr: true,
            mip: true,
            volume_3d: true,
        }
    }
}

/// MIP projection mode selection.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MipProjectionMode {
    /// Maximum intensity projection.
    MaxIntensity,
    /// Minimum intensity projection.
    MinIntensity,
}

/// Current volumetric workflow status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeWorkflowStatus {
    /// MPR toggle.
    pub mpr_enabled: bool,
    /// Optional MIP mode.
    pub mip_mode: Option<MipProjectionMode>,
    /// 3D volume toggle.
    pub volume_3d_enabled: bool,
    /// Optional deterministic fallback reason.
    pub fallback_reason: Option<String>,
}

/// Capability-gated volumetric workflow controls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeWorkflowState {
    capabilities: VolumeWorkflowCapabilities,
    status: VolumeWorkflowStatus,
}

impl VolumeWorkflowState {
    /// Build a capability-gated volumetric workflow state.
    pub fn new(capabilities: VolumeWorkflowCapabilities) -> Self {
        Self {
            capabilities,
            status: VolumeWorkflowStatus {
                mpr_enabled: false,
                mip_mode: None,
                volume_3d_enabled: false,
                fallback_reason: None,
            },
        }
    }

    /// Enable/disable MPR mode.
    pub fn set_mpr_enabled(&mut self, enabled: bool) -> Result<(), ClinicalError> {
        if enabled && !self.capabilities.mpr {
            self.status.fallback_reason = Some("mpr capability disabled".to_string());
            return Err(ClinicalError::CapabilityDisabled { capability: "mpr" });
        }
        self.status.mpr_enabled = enabled;
        self.status.fallback_reason = None;
        Ok(())
    }

    /// Set MIP mode.
    pub fn set_mip_mode(&mut self, mode: Option<MipProjectionMode>) -> Result<(), ClinicalError> {
        if mode.is_some() && !self.capabilities.mip {
            self.status.fallback_reason = Some("mip capability disabled".to_string());
            return Err(ClinicalError::CapabilityDisabled { capability: "mip" });
        }
        self.status.mip_mode = mode;
        self.status.fallback_reason = None;
        Ok(())
    }

    /// Enable/disable 3D volume mode.
    pub fn set_volume_3d_enabled(&mut self, enabled: bool) -> Result<(), ClinicalError> {
        if enabled && !self.capabilities.volume_3d {
            self.status.fallback_reason = Some("3d volume capability disabled".to_string());
            return Err(ClinicalError::CapabilityDisabled {
                capability: "volume_3d",
            });
        }
        self.status.volume_3d_enabled = enabled;
        self.status.fallback_reason = None;
        Ok(())
    }

    /// Return current volumetric workflow status.
    pub fn status(&self) -> &VolumeWorkflowStatus {
        &self.status
    }
}

/// Fusion registration state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FusionRegistrationState {
    /// Overlay is not registered.
    Unregistered,
    /// Overlay is registered.
    Registered,
    /// Registration failed with geometry mismatch details.
    GeometryMismatch {
        /// Deterministic geometry mismatch detail message.
        detail: String,
    },
}

/// Multimodality fusion overlay state.
#[derive(Debug, Clone, PartialEq)]
pub struct FusionOverlayState {
    /// Overlay visibility.
    pub visible: bool,
    /// Blend factor in `[0,1]`.
    pub blend: f32,
    /// Primary modality label.
    pub primary_modality: String,
    /// Secondary modality label.
    pub secondary_modality: String,
    /// Registration state.
    pub registration: FusionRegistrationState,
}

impl FusionOverlayState {
    /// Build a default fusion overlay model.
    pub fn new(primary_modality: &str, secondary_modality: &str) -> Self {
        Self {
            visible: false,
            blend: 0.5,
            primary_modality: primary_modality.to_string(),
            secondary_modality: secondary_modality.to_string(),
            registration: FusionRegistrationState::Unregistered,
        }
    }

    /// Set overlay visibility.
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Set blend factor deterministically.
    pub fn set_blend(&mut self, blend: f32) -> Result<(), ClinicalError> {
        if !(0.0..=1.0).contains(&blend) {
            return Err(ClinicalError::InvalidInput {
                detail: "fusion blend must be in [0,1]".to_string(),
            });
        }
        self.blend = blend;
        Ok(())
    }

    /// Mark registration success.
    pub fn mark_registered(&mut self) {
        self.registration = FusionRegistrationState::Registered;
    }

    /// Mark registration geometry mismatch with deterministic messaging.
    pub fn mark_geometry_mismatch(&mut self, detail: &str) {
        self.registration = FusionRegistrationState::GeometryMismatch {
            detail: detail.to_string(),
        };
    }
}

/// RT Dose overlay state.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct RtDoseOverlayState {
    /// Visibility toggle.
    pub visible: bool,
    /// Dose window lower bound.
    pub window_min: f64,
    /// Dose window upper bound.
    pub window_max: f64,
}

impl Default for RtDoseOverlayState {
    fn default() -> Self {
        Self {
            visible: false,
            window_min: 0.0,
            window_max: 1.0,
        }
    }
}

impl RtDoseOverlayState {
    /// Set RT dose window range.
    pub fn set_window(&mut self, min: f64, max: f64) -> Result<(), ClinicalError> {
        if !min.is_finite() || !max.is_finite() || min >= max {
            return Err(ClinicalError::InvalidInput {
                detail: "rt dose window must be finite with min < max".to_string(),
            });
        }
        self.window_min = min;
        self.window_max = max;
        Ok(())
    }
}

/// RTSS contour overlay state.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RtssOverlayState {
    /// Visibility toggle.
    pub visible: bool,
    /// Plane index used for contour intersection.
    pub plane_index: u32,
    /// Clipping mode toggle.
    pub clipping_enabled: bool,
}

impl Default for RtssOverlayState {
    fn default() -> Self {
        Self {
            visible: false,
            plane_index: 0,
            clipping_enabled: true,
        }
    }
}

/// 3D annotation record.
#[derive(Debug, Clone, PartialEq)]
pub struct Annotation3d {
    /// Stable annotation identifier.
    pub id: String,
    /// Annotation label.
    pub label: String,
    /// Physical coordinate in millimeters.
    pub position_mm: [f64; 3],
    /// Monotonic create tick.
    pub created_at_tick: u64,
    /// Monotonic update tick.
    pub updated_at_tick: u64,
}

/// Deterministic 3D annotation store.
#[derive(Debug, Clone, PartialEq)]
pub struct Annotation3dStore {
    annotations: Vec<Annotation3d>,
    next_id: u64,
    tick: u64,
}

impl Default for Annotation3dStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Annotation3dStore {
    /// Create an empty annotation store.
    pub fn new() -> Self {
        Self {
            annotations: Vec::new(),
            next_id: 0,
            tick: 0,
        }
    }

    /// Return annotations in deterministic id order.
    pub fn annotations(&self) -> Vec<&Annotation3d> {
        let mut out = self.annotations.iter().collect::<Vec<_>>();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    /// Add a new annotation and return its id.
    pub fn add(&mut self, label: &str, position_mm: [f64; 3]) -> Result<String, ClinicalError> {
        validate_position3(position_mm)?;
        if label.trim().is_empty() {
            return Err(ClinicalError::InvalidInput {
                detail: "annotation label must be non-empty".to_string(),
            });
        }
        self.tick = self.tick.saturating_add(1);
        let id = format!("a{}", self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.annotations.push(Annotation3d {
            id: id.clone(),
            label: label.to_string(),
            position_mm,
            created_at_tick: self.tick,
            updated_at_tick: self.tick,
        });
        Ok(id)
    }

    /// Update one annotation by id.
    pub fn update(
        &mut self,
        id: &str,
        label: &str,
        position_mm: [f64; 3],
    ) -> Result<(), ClinicalError> {
        validate_position3(position_mm)?;
        let index = self
            .annotations
            .iter()
            .position(|annotation| annotation.id == id)
            .ok_or_else(|| ClinicalError::NotFound { id: id.to_string() })?;
        self.tick = self.tick.saturating_add(1);
        let annotation = &mut self.annotations[index];
        annotation.label = label.to_string();
        annotation.position_mm = position_mm;
        annotation.updated_at_tick = self.tick;
        Ok(())
    }

    /// Remove one annotation by id.
    pub fn remove(&mut self, id: &str) -> Result<(), ClinicalError> {
        let index = self
            .annotations
            .iter()
            .position(|annotation| annotation.id == id)
            .ok_or_else(|| ClinicalError::NotFound { id: id.to_string() })?;
        self.annotations.remove(index);
        self.tick = self.tick.saturating_add(1);
        Ok(())
    }

    /// Snapshot annotations for persistence.
    pub fn snapshot(&self) -> Vec<Annotation3d> {
        self.annotations.clone()
    }

    /// Restore annotations from snapshot.
    pub fn restore_from_snapshot(&mut self, snapshot: Vec<Annotation3d>) {
        self.annotations = snapshot;
        self.tick = self.tick.saturating_add(1);
    }

    /// Export annotations as deterministic JSON.
    pub fn export_json(&self) -> String {
        let ordered = self.annotations();
        let mut out = String::from("[");
        for (index, annotation) in ordered.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "{{\"id\":\"{}\",\"label\":\"{}\",\"x\":{},\"y\":{},\"z\":{},\"created_at_tick\":{},\"updated_at_tick\":{}}}",
                escape_json(&annotation.id),
                escape_json(&annotation.label),
                annotation.position_mm[0],
                annotation.position_mm[1],
                annotation.position_mm[2],
                annotation.created_at_tick,
                annotation.updated_at_tick
            ));
        }
        out.push(']');
        out
    }
}

fn validate_points(points: &[ImagePoint]) -> Result<(), ClinicalError> {
    if points.is_empty() {
        return Err(ClinicalError::InvalidInput {
            detail: "measurement requires at least one point".to_string(),
        });
    }
    for point in points {
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(ClinicalError::InvalidInput {
                detail: "measurement points must be finite".to_string(),
            });
        }
    }
    Ok(())
}

fn validate_optional_number(value: Option<f64>) -> Result<(), ClinicalError> {
    if let Some(value) = value {
        if !value.is_finite() {
            return Err(ClinicalError::InvalidInput {
                detail: "measurement value must be finite".to_string(),
            });
        }
    }
    Ok(())
}

fn validate_position3(position: [f64; 3]) -> Result<(), ClinicalError> {
    if position.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(ClinicalError::InvalidInput {
            detail: "annotation position must be finite".to_string(),
        })
    }
}

fn to_measurement_csv(measurements: &[MeasurementRecord]) -> String {
    let mut out = String::from("id,kind,unit,value,revision,updated_at_tick\n");
    for measurement in measurements {
        let kind = match measurement.kind {
            MeasurementKind::Distance2D => "Distance2D",
            MeasurementKind::Angle2D => "Angle2D",
            MeasurementKind::Probe => "Probe",
        };
        let unit = match measurement.unit {
            MeasurementUnit::Pixel => "Pixel",
            MeasurementUnit::Millimeter => "Millimeter",
            MeasurementUnit::Degree => "Degree",
        };
        let value = measurement
            .value
            .map(|value| value.to_string())
            .unwrap_or_default();
        out.push_str(&format!(
            "{},{},{},{},{},{}\n",
            measurement.id, kind, unit, value, measurement.revision, measurement.updated_at_tick
        ));
    }
    out
}

fn to_measurement_json(measurements: &[MeasurementRecord]) -> String {
    let mut out = String::from("[");
    for (index, measurement) in measurements.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        let value = measurement
            .value
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string());
        let kind = match measurement.kind {
            MeasurementKind::Distance2D => "Distance2D",
            MeasurementKind::Angle2D => "Angle2D",
            MeasurementKind::Probe => "Probe",
        };
        let unit = match measurement.unit {
            MeasurementUnit::Pixel => "Pixel",
            MeasurementUnit::Millimeter => "Millimeter",
            MeasurementUnit::Degree => "Degree",
        };
        out.push_str(&format!(
            "{{\"id\":\"{}\",\"kind\":\"{}\",\"unit\":\"{}\",\"value\":{},\"revision\":{},\"updated_at_tick\":{}}}",
            escape_json(&measurement.id),
            kind,
            unit,
            value,
            measurement.revision,
            measurement.updated_at_tick
        ));
    }
    out.push(']');
    out
}

fn escape_json(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

// ---------------------------------------------------------------------------
// S2-T1: Interactive segmentation brush tool
// ---------------------------------------------------------------------------

/// Segmentation brush action mode.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BrushMode {
    /// Paint voxels into the active segment.
    Paint,
    /// Erase voxels from the active segment.
    Erase,
}

/// Configuration for the segmentation brush tool.
#[derive(Debug, Clone, PartialEq)]
pub struct BrushConfig {
    /// Brush radius in voxels (must be >= 1).
    pub radius_voxels: u32,
    /// Current brush action mode.
    pub mode: BrushMode,
}

impl Default for BrushConfig {
    fn default() -> Self {
        Self {
            radius_voxels: 3,
            mode: BrushMode::Paint,
        }
    }
}

impl BrushConfig {
    /// Validate brush configuration.
    pub fn validate(&self) -> Result<(), ClinicalError> {
        if self.radius_voxels == 0 {
            return Err(ClinicalError::InvalidInput {
                detail: "brush radius must be >= 1 voxel".to_string(),
            });
        }
        if self.radius_voxels > 64 {
            return Err(ClinicalError::InvalidInput {
                detail: "brush radius must be <= 64 voxels".to_string(),
            });
        }
        Ok(())
    }
}

/// A single brush stroke on one slice of a labelmap volume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrushStroke {
    /// Segment id this stroke applies to.
    pub segment_id: String,
    /// Slice index within the labelmap volume (z axis).
    pub slice_index: u32,
    /// Center column of the brush in the slice.
    pub center_col: u32,
    /// Center row of the brush in the slice.
    pub center_row: u32,
    /// Brush radius in voxels.
    pub radius_voxels: u32,
    /// Brush mode (paint or erase).
    pub mode: BrushMode,
}

/// Deterministic 3D labelmap volume for interactive segmentation.
///
/// Each voxel stores a segment label (0 = background, >0 = segment index).
/// The labelmap dimensions must match the source volume dimensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelMap3D {
    /// Labelmap width (x dimension).
    pub width: u32,
    /// Labelmap height (y dimension).
    pub height: u32,
    /// Labelmap depth (z dimension, number of slices).
    pub depth: u32,
    /// Voxel labels in z/y/x row-major order.
    pub labels: Vec<u16>,
}

impl LabelMap3D {
    /// Create a zero-initialized (background) labelmap.
    pub fn new(width: u32, height: u32, depth: u32) -> Result<Self, ClinicalError> {
        if width == 0 || height == 0 || depth == 0 {
            return Err(ClinicalError::InvalidInput {
                detail: "labelmap dimensions must be non-zero".to_string(),
            });
        }
        let count = width as usize * height as usize * depth as usize;
        if count > 512 * 512 * 2048 {
            return Err(ClinicalError::InvalidInput {
                detail: "labelmap dimensions exceed allocation limit".to_string(),
            });
        }
        Ok(Self {
            width,
            height,
            depth,
            labels: vec![0u16; count],
        })
    }

    /// Return the label at a given voxel coordinate.
    pub fn label(&self, x: u32, y: u32, z: u32) -> Option<u16> {
        if x >= self.width || y >= self.height || z >= self.depth {
            return None;
        }
        let idx = z as usize * (self.width as usize * self.height as usize)
            + y as usize * self.width as usize
            + x as usize;
        self.labels.get(idx).copied()
    }

    /// Set the label at a given voxel coordinate. Returns false if out of bounds.
    pub fn set_label(&mut self, x: u32, y: u32, z: u32, label: u16) -> bool {
        if x >= self.width || y >= self.height || z >= self.depth {
            return false;
        }
        let idx = z as usize * (self.width as usize * self.height as usize)
            + y as usize * self.width as usize
            + x as usize;
        if let Some(slot) = self.labels.get_mut(idx) {
            *slot = label;
            return true;
        }
        false
    }

    /// Apply a single brush stroke deterministically.
    ///
    /// Paints or erases a circular region on one slice. When painting, all voxels
    /// within the brush radius are set to `segment_label`. When erasing, all voxels
    /// with `segment_label` within the brush radius are set to 0 (background).
    /// Returns the number of voxels modified.
    pub fn apply_brush_stroke(
        &mut self,
        stroke: &BrushStroke,
        segment_label: u16,
    ) -> Result<u64, ClinicalError> {
        if segment_label == 0 {
            return Err(ClinicalError::InvalidInput {
                detail: "segment label must be non-zero for painting".to_string(),
            });
        }
        if stroke.slice_index >= self.depth {
            return Err(ClinicalError::InvalidInput {
                detail: "brush stroke slice_index out of range".to_string(),
            });
        }
        if stroke.radius_voxels == 0 {
            return Err(ClinicalError::InvalidInput {
                detail: "brush radius must be >= 1".to_string(),
            });
        }
        let r2 = (stroke.radius_voxels as i64) * (stroke.radius_voxels as i64);
        let mut modified: u64 = 0;
        let r = stroke.radius_voxels as i32;
        let cx = stroke.center_col as i32;
        let cy = stroke.center_row as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                if (dx as i64) * (dx as i64) + (dy as i64) * (dy as i64) > r2 {
                    continue;
                }
                let x = cx + dx;
                let y = cy + dy;
                if x < 0 || y < 0 {
                    continue;
                }
                let ux = x as u32;
                let uy = y as u32;
                if ux >= self.width || uy >= self.height {
                    continue;
                }
                let idx = stroke.slice_index as usize
                    * (self.width as usize * self.height as usize)
                    + uy as usize * self.width as usize
                    + ux as usize;
                match stroke.mode {
                    BrushMode::Paint => {
                        if let Some(slot) = self.labels.get_mut(idx) {
                            if *slot != segment_label {
                                *slot = segment_label;
                                modified += 1;
                            }
                        }
                    }
                    BrushMode::Erase => {
                        if let Some(slot) = self.labels.get_mut(idx) {
                            if *slot == segment_label {
                                *slot = 0;
                                modified += 1;
                            }
                        }
                    }
                }
            }
        }
        Ok(modified)
    }

    /// Count the total number of voxels with a given label.
    pub fn count_voxels_with_label(&self, label: u16) -> u64 {
        self.labels.iter().filter(|&&v| v == label).count() as u64
    }

    /// Snapshot the labelmap for undo/restore.
    pub fn snapshot(&self) -> Vec<u16> {
        self.labels.clone()
    }

    /// Restore labels from a snapshot.
    pub fn restore_from_snapshot(&mut self, snapshot: Vec<u16>) {
        self.labels = snapshot;
    }
}

// ---------------------------------------------------------------------------
// S2-T2: 3D interpolation for segmentation
// ---------------------------------------------------------------------------

/// Interpolation method for between-slice segmentation.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum InterpolationMethod {
    /// Morphological interpolation: propagate and intersect adjacent slices.
    Morphological,
    /// Linear interpolation: blend labels between slices.
    Linear,
}

/// Region growing seed configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct RegionGrowSeed {
    /// Seed x coordinate (column).
    pub x: u32,
    /// Seed y coordinate (row).
    pub y: u32,
    /// Seed z coordinate (slice).
    pub z: u32,
    /// Lower HU bound for inclusion.
    pub hu_min: i32,
    /// Upper HU bound for inclusion.
    pub hu_max: i32,
}

/// Threshold-based auto-segmentation configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThresholdConfig {
    /// Lower HU bound for inclusion.
    pub hu_min: i32,
    /// Upper HU bound for inclusion.
    pub hu_max: i32,
    /// Slice range start (z index).
    pub slice_start: u32,
    /// Slice range end (z index, exclusive).
    pub slice_end: u32,
}

impl ThresholdConfig {
    /// Validate threshold configuration.
    pub fn validate(&self) -> Result<(), ClinicalError> {
        if self.hu_min > self.hu_max {
            return Err(ClinicalError::InvalidInput {
                detail: "threshold hu_min must be <= hu_max".to_string(),
            });
        }
        if self.slice_start >= self.slice_end {
            return Err(ClinicalError::InvalidInput {
                detail: "threshold slice_start must be < slice_end".to_string(),
            });
        }
        Ok(())
    }
}

/// Apply morphological interpolation between two segmented slices.
///
/// Given a labelmap and two z-slice indices that already contain segmentation,
/// this fills intermediate slices with the intersection of the two boundary
/// slices, creating a smooth interpolation. Returns the number of voxels
/// written. Voxels are only written where both boundary slices agree on the
/// same non-zero label.
pub fn interpolate_slices_morphological(
    labelmap: &mut LabelMap3D,
    z_start: u32,
    z_end: u32,
    segment_label: u16,
) -> Result<u64, ClinicalError> {
    if z_start >= labelmap.depth || z_end >= labelmap.depth {
        return Err(ClinicalError::InvalidInput {
            detail: "interpolation slice indices out of range".to_string(),
        });
    }
    if segment_label == 0 {
        return Err(ClinicalError::InvalidInput {
            detail: "segment label must be non-zero for interpolation".to_string(),
        });
    }
    let z_lo = z_start.min(z_end);
    let z_hi = z_start.max(z_end);
    if z_lo == z_hi {
        return Ok(0);
    }
    let mut modified: u64 = 0;
    for z in (z_lo + 1)..z_hi {
        for y in 0..labelmap.height {
            for x in 0..labelmap.width {
                let label_lo = labelmap.label(x, y, z_lo).unwrap_or(0);
                let label_hi = labelmap.label(x, y, z_hi).unwrap_or(0);
                if label_lo == segment_label && label_hi == segment_label {
                    if labelmap.label(x, y, z) != Some(segment_label) {
                        labelmap.set_label(x, y, z, segment_label);
                        modified += 1;
                    }
                }
            }
        }
    }
    Ok(modified)
}

/// Apply linear interpolation between two segmented slices.
///
/// For intermediate slices, a voxel is assigned `segment_label` if the
/// corresponding voxel is present in at least one boundary slice and the
/// distance-weighted probability exceeds 0.5 (i.e., closer boundary slice
/// has the label set).
pub fn interpolate_slices_linear(
    labelmap: &mut LabelMap3D,
    z_start: u32,
    z_end: u32,
    segment_label: u16,
) -> Result<u64, ClinicalError> {
    if z_start >= labelmap.depth || z_end >= labelmap.depth {
        return Err(ClinicalError::InvalidInput {
            detail: "interpolation slice indices out of range".to_string(),
        });
    }
    if segment_label == 0 {
        return Err(ClinicalError::InvalidInput {
            detail: "segment label must be non-zero for interpolation".to_string(),
        });
    }
    let z_lo = z_start.min(z_end);
    let z_hi = z_start.max(z_end);
    if z_lo == z_hi {
        return Ok(0);
    }
    let total_dist = (z_hi - z_lo) as f64;
    let mut modified: u64 = 0;
    for z in (z_lo + 1)..z_hi {
        let t_lo = (z - z_lo) as f64 / total_dist;
        let t_hi = (z_hi - z) as f64 / total_dist;
        for y in 0..labelmap.height {
            for x in 0..labelmap.width {
                let label_lo = labelmap.label(x, y, z_lo).unwrap_or(0);
                let label_hi = labelmap.label(x, y, z_hi).unwrap_or(0);
                let w_lo = if label_lo == segment_label { t_hi } else { 0.0 };
                let w_hi = if label_hi == segment_label { t_lo } else { 0.0 };
                if w_lo + w_hi > 0.5 {
                    if labelmap.label(x, y, z) != Some(segment_label) {
                        labelmap.set_label(x, y, z, segment_label);
                        modified += 1;
                    }
                }
            }
        }
    }
    Ok(modified)
}

/// Apply threshold-based auto-segmentation to a volume.
///
/// Scans the voxel data and assigns `segment_label` to any voxel in the
/// configured slice range whose HU value falls within [hu_min, hu_max].
/// Returns the number of voxels segmented.
pub fn threshold_segment_volume(
    labelmap: &mut LabelMap3D,
    voxels: &[i32],
    config: &ThresholdConfig,
    segment_label: u16,
) -> Result<u64, ClinicalError> {
    config.validate()?;
    if segment_label == 0 {
        return Err(ClinicalError::InvalidInput {
            detail: "segment label must be non-zero for threshold segmentation".to_string(),
        });
    }
    let expected = labelmap.width as usize * labelmap.height as usize * labelmap.depth as usize;
    if voxels.len() != expected {
        return Err(ClinicalError::InvalidInput {
            detail: "voxel buffer length does not match labelmap dimensions".to_string(),
        });
    }
    let z_end = config.slice_end.min(labelmap.depth);
    let mut modified: u64 = 0;
    for z in config.slice_start..z_end {
        for y in 0..labelmap.height {
            for x in 0..labelmap.width {
                let idx = z as usize * (labelmap.width as usize * labelmap.height as usize)
                    + y as usize * labelmap.width as usize
                    + x as usize;
                let hu = voxels[idx];
                if hu >= config.hu_min && hu <= config.hu_max {
                    if labelmap.label(x, y, z) != Some(segment_label) {
                        labelmap.set_label(x, y, z, segment_label);
                        modified += 1;
                    }
                }
            }
        }
    }
    Ok(modified)
}

/// Apply region growing from a seed point within a volume.
///
/// Uses a flood-fill approach: starting from the seed, expand to 6-connected
/// neighbors whose HU values fall within [hu_min, hu_max]. All connected
/// voxels are assigned `segment_label`. Returns the number of voxels segmented.
pub fn region_grow(
    labelmap: &mut LabelMap3D,
    voxels: &[i32],
    seed: &RegionGrowSeed,
    segment_label: u16,
) -> Result<u64, ClinicalError> {
    if seed.x >= labelmap.width || seed.y >= labelmap.height || seed.z >= labelmap.depth {
        return Err(ClinicalError::InvalidInput {
            detail: "region grow seed coordinates out of range".to_string(),
        });
    }
    if segment_label == 0 {
        return Err(ClinicalError::InvalidInput {
            detail: "segment label must be non-zero for region growing".to_string(),
        });
    }
    let expected = labelmap.width as usize * labelmap.height as usize * labelmap.depth as usize;
    if voxels.len() != expected {
        return Err(ClinicalError::InvalidInput {
            detail: "voxel buffer length does not match labelmap dimensions".to_string(),
        });
    }
    let idx0 = seed.z as usize * (labelmap.width as usize * labelmap.height as usize)
        + seed.y as usize * labelmap.width as usize
        + seed.x as usize;
    let hu0 = voxels[idx0];
    if hu0 < seed.hu_min || hu0 > seed.hu_max {
        return Ok(0);
    }
    // BFS flood fill using index-based queue
    let w = labelmap.width as usize;
    let h = labelmap.height as usize;
    let d = labelmap.depth as usize;
    let total = w * h * d;
    let mut visited = vec![false; total];
    let mut queue = std::collections::VecDeque::new();
    let start_idx = seed.z as usize * (w * h) + seed.y as usize * w + seed.x as usize;
    visited[start_idx] = true;
    queue.push_back(start_idx);
    let mut modified: u64 = 0;
    while let Some(idx) = queue.pop_front() {
        let hu = voxels[idx];
        if hu < seed.hu_min || hu > seed.hu_max {
            continue;
        }
        let z = idx / (w * h);
        let rem = idx % (w * h);
        let y = rem / w;
        let x = rem % w;
        labelmap.set_label(x as u32, y as u32, z as u32, segment_label);
        modified += 1;
        // 6-connected neighbors
        let neighbors = [
            if x > 0 { Some(idx - 1) } else { None },
            if x + 1 < w { Some(idx + 1) } else { None },
            if y > 0 { Some(idx - w) } else { None },
            if y + 1 < h { Some(idx + w) } else { None },
            if z > 0 { Some(idx - w * h) } else { None },
            if z + 1 < d { Some(idx + w * h) } else { None },
        ];
        for nb in neighbors.into_iter().flatten() {
            if !visited[nb] {
                visited[nb] = true;
                queue.push_back(nb);
            }
        }
    }
    Ok(modified)
}

// ---------------------------------------------------------------------------
// S2-T6: ROI Statistics and Histogram tool
// ---------------------------------------------------------------------------

/// ROI shape definition.
#[derive(Debug, Clone, PartialEq)]
pub enum RoiShape {
    /// Rectangular ROI defined by top-left and bottom-right corners.
    Rect {
        /// Left column (0-based).
        left: u32,
        /// Top row (0-based).
        top: u32,
        /// Right column (0-based, inclusive).
        right: u32,
        /// Bottom row (0-based, inclusive).
        bottom: u32,
    },
    /// Elliptical ROI defined by center and semi-axes.
    Ellipse {
        /// Center column.
        center_col: f64,
        /// Center row.
        center_row: f64,
        /// Semi-axis in columns.
        semi_col: f64,
        /// Semi-axis in rows.
        semi_row: f64,
    },
    /// Freehand ROI defined by a polygon of vertices.
    Freehand {
        /// Polygon vertices as (col, row) pairs.
        points: Vec<(f64, f64)>,
    },
}

/// Computed ROI statistics.
#[derive(Debug, Clone, PartialEq)]
pub struct RoiStatistics {
    /// Number of voxels in the ROI.
    pub voxel_count: u64,
    /// Mean voxel value.
    pub mean: f64,
    /// Standard deviation of voxel values.
    pub std_dev: f64,
    /// Minimum voxel value.
    pub min: f64,
    /// Maximum voxel value.
    pub max: f64,
    /// Area in square pixels.
    pub area_pixels: f64,
    /// Area in square millimeters (if calibrated).
    pub area_mm2: Option<f64>,
    /// Histogram bins (256 bins for 8-bit range).
    pub histogram: Vec<u64>,
}

/// Compute ROI statistics for a 2D slice of a volume.
///
/// Scans all voxels within the ROI shape on the given slice and computes
/// mean, std deviation, min, max, area, and a 256-bin histogram.
/// The `voxels` buffer must match `width * height * depth` in z/y/x order.
pub fn compute_roi_statistics(
    voxels: &[i32],
    width: u32,
    height: u32,
    depth: u32,
    slice_index: u32,
    roi: &RoiShape,
    pixel_spacing_mm: Option<(f64, f64)>,
) -> Result<RoiStatistics, ClinicalError> {
    if slice_index >= depth {
        return Err(ClinicalError::InvalidInput {
            detail: "ROI slice index out of range".to_string(),
        });
    }
    let slice_size = width as usize * height as usize;
    let slice_offset = slice_index as usize * slice_size;
    let mut values = Vec::new();
    let mut histogram = vec![0u64; 256];
    match roi {
        RoiShape::Rect {
            left,
            top,
            right,
            bottom,
        } => {
            if *right >= width || *bottom >= height {
                return Err(ClinicalError::InvalidInput {
                    detail: "ROI rect bounds exceed image dimensions".to_string(),
                });
            }
            for row in *top..=*bottom {
                for col in *left..=*right {
                    let idx = slice_offset + row as usize * width as usize + col as usize;
                    if let Some(&v) = voxels.get(idx) {
                        let vf = v as f64;
                        values.push(vf);
                        let bin = ((v - 0).clamp(0, 255)) as usize;
                        histogram[bin] += 1;
                    }
                }
            }
        }
        RoiShape::Ellipse {
            center_col,
            center_row,
            semi_col,
            semi_row,
        } => {
            if *semi_col <= 0.0 || *semi_row <= 0.0 {
                return Err(ClinicalError::InvalidInput {
                    detail: "ellipse semi-axes must be > 0".to_string(),
                });
            }
            let sc2 = semi_col * semi_col;
            let sr2 = semi_row * semi_row;
            let r_min = (*center_row - *semi_row).floor().max(0.0) as u32;
            let r_max = (*center_row + *semi_row).ceil().min(height as f64 - 1.0) as u32;
            let c_min = (*center_col - *semi_col).floor().max(0.0) as u32;
            let c_max = (*center_col + *semi_col).ceil().min(width as f64 - 1.0) as u32;
            for row in r_min..=r_max {
                for col in c_min..=c_max {
                    let dc = col as f64 - center_col;
                    let dr = row as f64 - center_row;
                    if dc * dc / sc2 + dr * dr / sr2 <= 1.0 {
                        let idx = slice_offset + row as usize * width as usize + col as usize;
                        if let Some(&v) = voxels.get(idx) {
                            let vf = v as f64;
                            values.push(vf);
                            let bin = ((v - 0).clamp(0, 255)) as usize;
                            histogram[bin] += 1;
                        }
                    }
                }
            }
        }
        RoiShape::Freehand { points } => {
            if points.len() < 3 {
                return Err(ClinicalError::InvalidInput {
                    detail: "freehand ROI requires at least 3 vertices".to_string(),
                });
            }
            let r_min = points
                .iter()
                .map(|p| p.1.floor().max(0.0) as u32)
                .min()
                .unwrap_or(0);
            let r_max = points
                .iter()
                .map(|p| p.1.ceil().min(height as f64 - 1.0) as u32)
                .max()
                .unwrap_or(0);
            let c_min = points
                .iter()
                .map(|p| p.0.floor().max(0.0) as u32)
                .min()
                .unwrap_or(0);
            let c_max = points
                .iter()
                .map(|p| p.0.ceil().min(width as f64 - 1.0) as u32)
                .max()
                .unwrap_or(0);
            for row in r_min..=r_max {
                for col in c_min..=c_max {
                    if point_in_freehand_roi(col as f64, row as f64, points) {
                        let idx = slice_offset + row as usize * width as usize + col as usize;
                        if let Some(&v) = voxels.get(idx) {
                            let vf = v as f64;
                            values.push(vf);
                            let bin = ((v - 0).clamp(0, 255)) as usize;
                            histogram[bin] += 1;
                        }
                    }
                }
            }
        }
    }
    let voxel_count = values.len() as u64;
    if voxel_count == 0 {
        return Ok(RoiStatistics {
            voxel_count: 0,
            mean: 0.0,
            std_dev: 0.0,
            min: 0.0,
            max: 0.0,
            area_pixels: 0.0,
            area_mm2: None,
            histogram,
        });
    }
    let sum: f64 = values.iter().copied().sum();
    let mean = sum / voxel_count as f64;
    let variance: f64 =
        values.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / voxel_count as f64;
    let std_dev = variance.sqrt();
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let area_pixels = voxel_count as f64;
    let area_mm2 = pixel_spacing_mm.map(|(row_mm, col_mm)| area_pixels * row_mm * col_mm);
    Ok(RoiStatistics {
        voxel_count,
        mean,
        std_dev,
        min,
        max,
        area_pixels,
        area_mm2,
        histogram,
    })
}

/// Point-in-polygon test for freehand ROI using ray-casting algorithm.
fn point_in_freehand_roi(x: f64, y: f64, polygon: &[(f64, f64)]) -> bool {
    let mut inside = false;
    let n = polygon.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = polygon[i];
        let (xj, yj) = polygon[j];
        if ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measurement_store_crud_undo_redo_and_exports_are_deterministic() {
        let mut store = MeasurementStore::new();
        let id = store
            .create_measurement(
                MeasurementKind::Distance2D,
                vec![
                    ImagePoint { x: 10.0, y: 11.0 },
                    ImagePoint { x: 14.0, y: 15.0 },
                ],
                Some(5.0),
                MeasurementUnit::Millimeter,
            )
            .expect("create measurement");
        store
            .update_measurement(
                &id,
                vec![
                    ImagePoint { x: 10.0, y: 11.0 },
                    ImagePoint { x: 16.0, y: 19.0 },
                ],
                Some(9.0),
                MeasurementUnit::Millimeter,
            )
            .expect("update measurement");
        assert_eq!(
            store.jump_target(&id),
            Some(ImagePoint { x: 10.0, y: 11.0 })
        );
        let bundle = store.export_bundle();
        assert!(bundle
            .csv
            .contains("id,kind,unit,value,revision,updated_at_tick"));
        assert!(bundle.json.contains("\"id\":\"m0\""));
        let sr = store.sr_payload();
        assert_eq!(sr.source, "viewer-core.measurements");
        assert_eq!(sr.measurement_count, 1);
        store.delete_measurement(&id).expect("delete");
        assert_eq!(store.active_measurements().len(), 0);
        assert!(store.undo());
        assert_eq!(store.active_measurements().len(), 1);
        assert!(store.redo());
        assert_eq!(store.active_measurements().len(), 0);
    }

    #[test]
    fn segmentation_store_tracks_revisions_and_style_diffs() {
        let mut store = SegmentationStore::new();
        let id = store.import_seg("Tumor").expect("import");
        let diff = store
            .update_style(
                &id,
                SegmentationStyle {
                    visible: true,
                    opacity: 0.8,
                    color_rgb: [10, 20, 30],
                    active: true,
                },
            )
            .expect("style update");
        assert_eq!(diff.segment_id, id);
        assert_eq!(diff.revision, 1);
        assert!(diff.changed_fields.contains(&"opacity".to_string()));
        store.set_locked(&id, true).expect("lock");
        let locked_err = store
            .update_style(
                &id,
                SegmentationStyle {
                    visible: true,
                    opacity: 0.4,
                    color_rgb: [1, 2, 3],
                    active: false,
                },
            )
            .expect_err("locked segment should fail");
        assert!(matches!(locked_err, ClinicalError::Locked { .. }));
    }

    #[test]
    fn volumetric_fusion_and_rt_controls_fail_closed() {
        let mut volume = VolumeWorkflowState::new(VolumeWorkflowCapabilities {
            mpr: true,
            mip: false,
            volume_3d: false,
        });
        assert!(volume.set_mpr_enabled(true).is_ok());
        let mip_err = volume
            .set_mip_mode(Some(MipProjectionMode::MaxIntensity))
            .expect_err("mip disabled");
        assert!(matches!(
            mip_err,
            ClinicalError::CapabilityDisabled { capability: "mip" }
        ));
        let volume_err = volume.set_volume_3d_enabled(true).expect_err("3d disabled");
        assert!(matches!(
            volume_err,
            ClinicalError::CapabilityDisabled {
                capability: "volume_3d"
            }
        ));
        assert_eq!(
            volume.status().fallback_reason.as_deref(),
            Some("3d volume capability disabled")
        );

        let mut fusion = FusionOverlayState::new("CT", "PET");
        fusion.mark_geometry_mismatch("frame of reference mismatch");
        assert!(matches!(
            fusion.registration,
            FusionRegistrationState::GeometryMismatch { .. }
        ));

        let mut dose = RtDoseOverlayState::default();
        dose.set_window(0.1, 3.5).expect("valid dose window");
        let err = dose.set_window(2.0, 1.0).expect_err("invalid window");
        assert!(matches!(err, ClinicalError::InvalidInput { .. }));
    }

    #[test]
    fn annotation_store_round_trips_snapshot_and_json_export() {
        let mut store = Annotation3dStore::new();
        let id = store
            .add("Apex", [1.0, 2.0, 3.0])
            .expect("create annotation");
        store
            .update(&id, "Apex-1", [4.0, 5.0, 6.0])
            .expect("update annotation");
        let snapshot = store.snapshot();
        let mut restored = Annotation3dStore::new();
        restored.restore_from_snapshot(snapshot);
        let json = restored.export_json();
        assert!(json.contains("\"id\":\"a0\""));
        assert!(json.contains("\"label\":\"Apex-1\""));
    }

    #[test]
    fn regression_path_covers_measurement_segmentation_and_rt_overlay_artifacts() {
        let mut measurements = MeasurementStore::new();
        let _ = measurements
            .create_measurement(
                MeasurementKind::Distance2D,
                vec![ImagePoint { x: 2.0, y: 3.0 }, ImagePoint { x: 6.0, y: 9.0 }],
                Some(7.2),
                MeasurementUnit::Millimeter,
            )
            .expect("measurement create");
        let export = measurements.export_bundle();
        assert!(export.csv.contains("Distance2D"));

        let mut segments = SegmentationStore::new();
        let seg_id = segments.create_labelmap("Lesion").expect("segment create");
        let _ = segments
            .update_style(
                &seg_id,
                SegmentationStyle {
                    visible: true,
                    opacity: 0.7,
                    color_rgb: [200, 10, 10],
                    active: true,
                },
            )
            .expect("segment style update");
        assert_eq!(segments.active_segments().len(), 1);

        let mut dose = RtDoseOverlayState::default();
        dose.visible = true;
        dose.set_window(0.2, 2.6).expect("rt dose window");
        assert!(dose.visible);
    }
}
