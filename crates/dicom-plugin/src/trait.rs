//! Core plugin trait and extension point definitions.
//!
//! Every DiCCY plugin must implement the [`DiccyPlugin`] trait and declare
//! which extension points it provides via [`ExtensionPoint`].

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Core plugin trait
// ---------------------------------------------------------------------------

/// Core plugin trait that all DiCCY plugins must implement.
///
/// Plugins are loaded at runtime (either as native dynamic libraries or
/// WebAssembly modules) and must be `Send + Sync` so they can be shared
/// across threads safely.
pub trait DiccyPlugin: Send + Sync {
    /// Unique plugin identifier.
    ///
    /// A reverse-DNS naming scheme is recommended, e.g.
    /// `"com.example.diccy-measure-tool"`.
    fn name(&self) -> &str;

    /// Semantic version of the plugin (e.g. `"1.2.0"`).
    fn version(&self) -> &str;

    /// Called when the plugin is loaded into the runtime.
    ///
    /// The [`PluginContext`] provides runtime services and configuration
    /// that the plugin may need during initialisation.
    fn on_load(&mut self, context: &PluginContext) -> Result<(), PluginError>;

    /// Called before the plugin is unloaded.
    ///
    /// Implementations should release any resources held by the plugin.
    fn on_unload(&mut self) -> Result<(), PluginError>;

    /// Declared extension points this plugin provides.
    ///
    /// Each variant maps to a specific extension surface in the DiCCY
    /// runtime. The plugin manager will register each returned extension
    /// point in the appropriate registry slot.
    fn extension_points(&self) -> Vec<ExtensionPoint>;

    /// Plugin metadata for display and logging.
    ///
    /// The default implementation returns an empty metadata object;
    /// plugins are encouraged to override this with meaningful values.
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata::default()
    }
}

// ---------------------------------------------------------------------------
// Extension point enum
// ---------------------------------------------------------------------------

/// Extension point types a plugin can implement.
///
/// Each variant wraps a trait object that the runtime will dispatch to
/// when the corresponding surface is exercised.
pub enum ExtensionPoint {
    /// Viewer tool — adds interactive tools to the image viewer.
    ViewerTool(Box<dyn ViewerTool>),
    /// Image processor — transforms pixel data.
    ImageProcessor(Box<dyn ImageProcessor>),
    /// Workflow hook — reacts to clinical workflow events.
    WorkflowHook(Box<dyn WorkflowHook>),
    /// Storage backend — provides alternative DICOM object storage.
    StorageBackend(Box<dyn StorageBackendPlugin>),
}

// ---------------------------------------------------------------------------
// ViewerTool extension point
// ---------------------------------------------------------------------------

/// Viewer tool extension — adds interactive tools to the viewer.
///
/// Implementors define a tool that appears in the viewer toolbar and
/// responds to activation/deactivation events.
pub trait ViewerTool: Send + Sync {
    /// Unique tool identifier within the plugin's namespace.
    fn tool_id(&self) -> &str;

    /// Human-readable display name for the UI.
    fn display_name(&self) -> &str;

    /// Icon name for the toolbar button (defaults to `"default"`).
    fn icon_name(&self) -> &str {
        "default"
    }

    /// Called when the tool is activated (selected by the user).
    fn on_activate(&self, context: &ToolContext) -> Result<(), PluginError>;

    /// Called when the tool is deactivated.
    fn on_deactivate(&self) -> Result<(), PluginError>;
}

// ---------------------------------------------------------------------------
// ImageProcessor extension point
// ---------------------------------------------------------------------------

/// Pixel format descriptor for image processor I/O negotiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// 8-bit grayscale.
    Luma8,
    /// 16-bit grayscale.
    Luma16,
    /// 8-bit RGBA.
    Rgba8,
}

/// Image processor extension — transforms pixel data.
///
/// Implementors receive raw pixel buffers and return transformed buffers.
/// The runtime uses [`input_format`] / [`output_format`] to negotiate
/// pipeline compatibility before calling [`process`].
pub trait ImageProcessor: Send + Sync {
    /// Unique processor identifier.
    fn processor_id(&self) -> &str;

    /// Input pixel format expected by this processor.
    fn input_format(&self) -> PixelFormat;

    /// Output pixel format produced by this processor.
    fn output_format(&self) -> PixelFormat;

    /// Process a pixel buffer.
    ///
    /// The `input` buffer must contain `width * height` pixels in the
    /// format returned by [`input_format`].
    fn process(
        &self,
        input: &[u8],
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, PluginError>;
}

// ---------------------------------------------------------------------------
// WorkflowHook extension point
// ---------------------------------------------------------------------------

/// Workflow event types that a hook can subscribe to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkflowEventType {
    /// A new study has been received.
    StudyReceived,
    /// A structured report has been created.
    ReportCreated,
    /// A measurement has been added to a study.
    MeasurementAdded,
    /// A study has been archived.
    StudyArchived,
    /// A DICOM object store operation completed.
    StoreCompleted,
    /// A worklist entry was updated.
    WorklistUpdated,
}

/// Action returned by a workflow hook after processing an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookAction {
    /// Continue the workflow without modification.
    Continue,
    /// Abort the workflow — stop processing this event.
    Abort,
    /// Modify the event payload and continue.
    Modify(HashMap<String, String>),
}

/// A workflow event delivered to a hook.
#[derive(Debug, Clone)]
pub struct WorkflowEvent {
    /// The event type.
    pub event_type: WorkflowEventType,
    /// Event payload as key/value pairs.
    pub payload: HashMap<String, String>,
    /// Optional Study Instance UID.
    pub study_instance_uid: Option<String>,
    /// Optional Series Instance UID.
    pub series_instance_uid: Option<String>,
    /// Optional SOP Instance UID.
    pub sop_instance_uid: Option<String>,
}

impl WorkflowEvent {
    /// Create a minimal workflow event with just a type.
    pub fn new(event_type: WorkflowEventType) -> Self {
        Self {
            event_type,
            payload: HashMap::new(),
            study_instance_uid: None,
            series_instance_uid: None,
            sop_instance_uid: None,
        }
    }
}

/// Workflow hook extension — reacts to clinical workflow events.
///
/// Implementors declare which event types they are interested in and
/// return a [`HookAction`] for each event they receive.
pub trait WorkflowHook: Send + Sync {
    /// Unique hook identifier.
    fn hook_id(&self) -> &str;

    /// The event types this hook subscribes to.
    fn event_types(&self) -> Vec<WorkflowEventType>;

    /// Handle a workflow event.
    fn on_event(&self, event: &WorkflowEvent) -> Result<HookAction, PluginError>;
}

// ---------------------------------------------------------------------------
// StorageBackend extension point
// ---------------------------------------------------------------------------

/// Storage backend extension — provides alternative DICOM object storage.
///
/// Implementors provide a key/value-style storage interface that the
/// runtime can use as an alternative to the default VNA/S3 backend.
pub trait StorageBackendPlugin: Send + Sync {
    /// Unique backend identifier.
    fn backend_id(&self) -> &str;

    /// Store data under the given key.
    fn put(&self, key: &str, data: &[u8]) -> Result<(), PluginError>;

    /// Retrieve data by key, returning `None` if not found.
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, PluginError>;

    /// Delete data by key. Returns whether the key existed.
    fn delete(&self, key: &str) -> Result<bool, PluginError>;

    /// List keys with the given prefix.
    fn list(&self, prefix: &str) -> Result<Vec<String>, PluginError>;
}

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

/// Context provided to a plugin during [`DiccyPlugin::on_load`].
#[derive(Debug, Clone)]
pub struct PluginContext {
    /// Runtime configuration as key/value pairs.
    pub config: HashMap<String, String>,
    /// Path to the plugin's data directory.
    pub data_dir: Option<String>,
    /// DiCCY core version the plugin is running under.
    pub diccy_version: String,
}

impl Default for PluginContext {
    fn default() -> Self {
        Self {
            config: HashMap::new(),
            data_dir: None,
            diccy_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// Plugin metadata for display and logging.
#[derive(Debug, Clone, Default)]
pub struct PluginMetadata {
    /// Human-readable description.
    pub description: String,
    /// Author or organisation.
    pub author: String,
    /// Homepage or repository URL.
    pub url: String,
    /// Licence identifier (e.g. `"MIT"`, `"Apache-2.0"`).
    pub license: String,
}

/// Context provided to a viewer tool during activation.
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// The viewport identifier the tool was activated in.
    pub viewport_id: String,
    /// Current study UID, if available.
    pub study_instance_uid: Option<String>,
    /// Runtime configuration for the tool.
    pub config: HashMap<String, String>,
}

impl Default for ToolContext {
    fn default() -> Self {
        Self {
            viewport_id: String::new(),
            study_instance_uid: None,
            config: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin error type
// ---------------------------------------------------------------------------

/// Error type for plugin operations.
#[derive(Debug)]
pub enum PluginError {
    /// The plugin failed to load.
    LoadFailed {
        /// Name of the plugin that failed.
        plugin_name: String,
        /// Error description.
        reason: String,
    },
    /// The plugin failed to unload.
    UnloadFailed {
        /// Name of the plugin that failed.
        plugin_name: String,
        /// Error description.
        reason: String,
    },
    /// A required extension point was not found.
    ExtensionNotFound {
        /// Extension point name.
        extension_point: String,
    },
    /// The plugin attempted an operation outside its declared permissions.
    PermissionDenied {
        /// The permission that was required.
        required: String,
        /// Description of the denied operation.
        detail: String,
    },
    /// A manifest file was invalid or could not be parsed.
    InvalidManifest {
        /// Parse error description.
        reason: String,
    },
    /// A WASM binary failed validation.
    InvalidWasmBinary {
        /// Validation error description.
        reason: String,
    },
    /// A native library could not be loaded.
    NativeLoadError {
        /// Library path.
        path: String,
        /// Error description.
        reason: String,
    },
    /// An image processing operation failed.
    ProcessingError {
        /// Processor identifier.
        processor_id: String,
        /// Error description.
        reason: String,
    },
    /// A storage backend operation failed.
    StorageError {
        /// Backend identifier.
        backend_id: String,
        /// Error description.
        reason: String,
    },
    /// A generic / catch-all plugin error.
    Other {
        /// Error description.
        reason: String,
    },
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoadFailed { plugin_name, reason } => {
                write!(f, "plugin '{plugin_name}' failed to load: {reason}")
            }
            Self::UnloadFailed { plugin_name, reason } => {
                write!(f, "plugin '{plugin_name}' failed to unload: {reason}")
            }
            Self::ExtensionNotFound { extension_point } => {
                write!(f, "extension point not found: {extension_point}")
            }
            Self::PermissionDenied { required, detail } => {
                write!(f, "permission denied (required: {required}): {detail}")
            }
            Self::InvalidManifest { reason } => {
                write!(f, "invalid manifest: {reason}")
            }
            Self::InvalidWasmBinary { reason } => {
                write!(f, "invalid WASM binary: {reason}")
            }
            Self::NativeLoadError { path, reason } => {
                write!(f, "native library load error for '{path}': {reason}")
            }
            Self::ProcessingError {
                processor_id,
                reason,
            } => {
                write!(f, "processor '{processor_id}' error: {reason}")
            }
            Self::StorageError { backend_id, reason } => {
                write!(f, "storage backend '{backend_id}' error: {reason}")
            }
            Self::Other { reason } => write!(f, "{reason}"),
        }
    }
}

impl std::error::Error for PluginError {}
