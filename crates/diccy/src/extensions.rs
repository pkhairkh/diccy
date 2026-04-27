use crate::{Error, ErrorKind, Result};
use std::collections::{BTreeMap, BTreeSet};

/// Supported product profile for an extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionProfile {
    /// Headless framework profile.
    FrameworkCore,
    /// Interactive workstation profile.
    Workstation,
    /// Backend service profile.
    BackendServices,
}

/// Extension event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionEventKind {
    /// Measurement capture workflow event.
    MeasurementCaptured,
    /// Dataset indexing event.
    DatasetIndexed,
    /// Workflow state transition event.
    WorkflowStateChanged,
}

/// Extension event envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionEvent {
    /// Event kind.
    pub kind: ExtensionEventKind,
    /// Deterministic payload map.
    pub payload: BTreeMap<String, String>,
    /// Optional Study Instance UID.
    pub study_instance_uid: Option<String>,
    /// Optional Series Instance UID.
    pub series_instance_uid: Option<String>,
    /// Optional SOP Instance UID.
    pub sop_instance_uid: Option<String>,
}

/// Extension output status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionStatus {
    /// Event was accepted and handled.
    Accepted,
    /// Event was rejected fail-closed.
    Rejected,
    /// Extension explicitly performed no action.
    NoOp,
}

/// Result returned by an extension handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionResult {
    /// Status classification.
    pub status: ExtensionStatus,
    /// Deterministic output map.
    pub outputs: BTreeMap<String, String>,
    /// Optional stable error code.
    pub error_code: Option<&'static str>,
}

impl ExtensionResult {
    /// Build a no-op extension result.
    pub fn no_op() -> Self {
        Self {
            status: ExtensionStatus::NoOp,
            outputs: BTreeMap::new(),
            error_code: None,
        }
    }
}

/// Extension trait for deterministic workflow hooks.
pub trait WorkflowExtension {
    /// Stable extension identifier.
    fn id(&self) -> &'static str;
    /// Supported product profile for this extension.
    fn profile(&self) -> ExtensionProfile;
    /// Handle one event and return deterministic output.
    fn handle(&self, event: &ExtensionEvent) -> Result<ExtensionResult>;
}

/// Registered extension result row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredExtensionResult {
    /// Extension identifier.
    pub extension_id: &'static str,
    /// Extension response payload.
    pub result: ExtensionResult,
}

/// Deterministic extension registry.
pub struct ExtensionRegistry {
    extensions: Vec<Box<dyn WorkflowExtension + Send + Sync>>,
    ids: BTreeSet<&'static str>,
}

impl Default for ExtensionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ExtensionRegistry {
    /// Create an empty extension registry.
    pub fn new() -> Self {
        Self {
            extensions: Vec::new(),
            ids: BTreeSet::new(),
        }
    }

    /// Register one extension in deterministic order.
    pub fn register(&mut self, extension: Box<dyn WorkflowExtension + Send + Sync>) -> Result<()> {
        if self.ids.contains(extension.id()) {
            return Err(duplicate_extension_error(extension.id()));
        }
        self.ids.insert(extension.id());
        self.extensions.push(extension);
        Ok(())
    }

    /// Execute all registered extensions in registration order.
    pub fn run_all(&self, event: &ExtensionEvent) -> Result<Vec<RegisteredExtensionResult>> {
        let mut out = Vec::with_capacity(self.extensions.len());
        for extension in &self.extensions {
            out.push(RegisteredExtensionResult {
                extension_id: extension.id(),
                result: extension.handle(event)?,
            });
        }
        Ok(out)
    }
}

/// Sample deterministic extension implementation.
pub struct SampleEchoExtension;

impl WorkflowExtension for SampleEchoExtension {
    fn id(&self) -> &'static str {
        "sample.echo"
    }

    fn profile(&self) -> ExtensionProfile {
        ExtensionProfile::Workstation
    }

    fn handle(&self, event: &ExtensionEvent) -> Result<ExtensionResult> {
        if event.kind != ExtensionEventKind::MeasurementCaptured {
            return Ok(ExtensionResult::no_op());
        }
        let mut outputs = BTreeMap::new();
        outputs.insert("extension".to_string(), self.id().to_string());
        outputs.insert(
            "study_uid_present".to_string(),
            event.study_instance_uid.is_some().to_string(),
        );
        for (key, value) in &event.payload {
            outputs.insert(format!("payload.{key}"), value.clone());
        }
        Ok(ExtensionResult {
            status: ExtensionStatus::Accepted,
            outputs,
            error_code: None,
        })
    }
}

fn duplicate_extension_error(extension_id: &str) -> Box<Error> {
    Error::from_kind(
        ErrorKind::IntegrityError {
            detail: format!("duplicate extension id {extension_id}"),
        },
        "duplicate extension id",
    )
    .into()
}
