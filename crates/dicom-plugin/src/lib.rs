//! Runtime plugin architecture for the DiCCY PACS workstation.
//!
//! This crate provides the plugin loading, sandboxing, and registration
//! infrastructure that allows third-party plugins to extend the DiCCY
//! runtime without core recompilation.
//!
//! # Architecture
//!
//! 1. **Discovery** — [`PluginManager::scan_directory`] finds `plugin.toml`
//!    manifests in a given directory tree.
//! 2. **Manifest parsing** — [`manifest::parse_manifest`] reads and
//!    validates each manifest.
//! 3. **Sandboxing** — [`sandbox::PluginSandbox`] is created from the
//!    declared permissions, enforcing least-privilege at runtime.
//! 4. **Loading** — [`PluginManager::load_plugin`] loads a native plugin
//!    (stubbed), or [`PluginManager::load_wasm_plugin`] loads a WASM
//!    module.
//! 5. **Registration** — Extension points from the plugin are registered
//!    in the [`registry::PluginRegistry`].
//!
//! # Extension Points
//!
//! - [`ViewerTool`] — interactive viewer tools
//! - [`ImageProcessor`] — pixel data transformations
//! - [`WorkflowHook`] — clinical workflow event handlers
//! - [`StorageBackendPlugin`] — alternative DICOM object storage

#![deny(missing_docs)]

pub mod manifest;
pub mod registry;
pub mod sandbox;
pub mod r#trait;
pub mod wasi_runtime;

// Re-export all public types for ergonomic access.
pub use manifest::{
    discover_manifests, load_manifest, parse_manifest, CompatibilitySection, PluginManifest,
    PluginPermissions, PluginSection, KNOWN_EXTENSION_POINTS,
};
pub use registry::{LoadedPlugin, PluginRegistry};
pub use sandbox::{PluginSandbox, ResourceLimits};
pub use r#trait::{
    DiccyPlugin, ExtensionPoint, HookAction, ImageProcessor, PixelFormat, PluginContext,
    PluginError, PluginMetadata, StorageBackendPlugin, ToolContext, ViewerTool, WorkflowEvent,
    WorkflowEventType, WorkflowHook,
};
pub use wasi_runtime::{is_wasm_binary, validate_wasm_binary, WasiPluginRuntime};

use std::path::Path;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// PluginManager
// ---------------------------------------------------------------------------

/// Orchestrates plugin discovery, loading, sandboxing, and registration.
///
/// The `PluginManager` is the main entry point for the plugin subsystem.
/// It owns the [`PluginRegistry`] and coordinates the full lifecycle of
/// each plugin.
pub struct PluginManager {
    /// The plugin registry.
    registry: PluginRegistry,
    /// WASM runtimes keyed by plugin name.
    wasm_runtimes: std::collections::HashMap<String, WasiPluginRuntime>,
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginManager {
    /// Create a new, empty plugin manager.
    pub fn new() -> Self {
        Self {
            registry: PluginRegistry::new(),
            wasm_runtimes: std::collections::HashMap::new(),
        }
    }

    // -------------------------------------------------------------------
    // Discovery
    // -------------------------------------------------------------------

    /// Scan a directory for `plugin.toml` manifests.
    ///
    /// Returns a list of paths to discovered manifest files. This does
    /// **not** load any plugins; it only identifies candidate manifests.
    pub fn scan_directory(&self, plugin_dir: &Path) -> Result<Vec<std::path::PathBuf>, PluginError> {
        discover_manifests(plugin_dir)
    }

    // -------------------------------------------------------------------
    // Native plugin loading (stub)
    // -------------------------------------------------------------------

    /// Load a native plugin from a dynamic library.
    ///
    /// In production, this would use `libloading` to open a `.so`/`.dylib`/`.dll`
    /// and resolve the `_diccy_create_plugin` entry point. For now, this is a
    /// stub that validates the manifest and creates the sandbox.
    ///
    /// If loading fails, the system continues without the plugin (fail-closed).
    pub fn load_plugin(&mut self, manifest_path: &Path) -> Result<String, PluginError> {
        let manifest = load_manifest(manifest_path)?;
        let plugin_name = manifest.plugin.name.clone();

        // Create sandbox from declared permissions.
        let sandbox = PluginSandbox::from_permissions(&plugin_name, &manifest.plugin.permissions);

        // Register the plugin in the registry.
        self.registry
            .register_plugin(
                plugin_name.clone(),
                manifest.plugin.version.clone(),
                sandbox,
            )?;

        Ok(plugin_name)
    }

    // -------------------------------------------------------------------
    // WASM plugin loading
    // -------------------------------------------------------------------

    /// Load a WASM plugin.
    ///
    /// The manifest and WASM binary are validated separately. The WASM
    /// binary is checked for the correct magic header and version number.
    /// The sandbox is created from the manifest permissions.
    pub fn load_wasm_plugin(
        &mut self,
        manifest: PluginManifest,
        wasm_bytes: &[u8],
    ) -> Result<String, PluginError> {
        let plugin_name = manifest.plugin.name.clone();

        // Validate WASM binary.
        validate_wasm_binary(wasm_bytes)?;

        // Create sandbox.
        let sandbox = PluginSandbox::from_permissions(&plugin_name, &manifest.plugin.permissions);

        // Create WASM runtime.
        let mut runtime = WasiPluginRuntime::new();
        runtime.set_manifest(manifest);
        runtime.load_wasm(wasm_bytes)?;
        runtime.call_on_load()?;

        // Register.
        self.registry
            .register_plugin(plugin_name.clone(), String::new(), sandbox)?;

        self.wasm_runtimes.insert(plugin_name.clone(), runtime);

        Ok(plugin_name)
    }

    // -------------------------------------------------------------------
    // Unloading
    // -------------------------------------------------------------------

    /// Unload a plugin by name.
    ///
    /// If the plugin is a WASM plugin, the runtime's `on_unload` is
    /// called before removing it from the registry.
    pub fn unload_plugin(&mut self, name: &str) -> Result<(), PluginError> {
        // Unload WASM runtime if present.
        if let Some(mut runtime) = self.wasm_runtimes.remove(name) {
            runtime.call_on_unload()?;
        }
        self.registry.unregister_plugin(name)
    }

    // -------------------------------------------------------------------
    // Registry access
    // -------------------------------------------------------------------

    /// Access the plugin registry.
    pub fn registry(&self) -> &PluginRegistry {
        &self.registry
    }

    /// Access the plugin registry mutably.
    pub fn registry_mut(&mut self) -> &mut PluginRegistry {
        &mut self.registry
    }

    /// Check whether a plugin is loaded.
    pub fn is_loaded(&self, name: &str) -> bool {
        self.registry.has_plugin(name)
    }

    /// Return the number of loaded plugins.
    pub fn plugin_count(&self) -> usize {
        self.registry.plugin_count()
    }

    // -------------------------------------------------------------------
    // Convenience: register extension points for a DiccyPlugin
    // -------------------------------------------------------------------

    /// Register all extension points declared by a [`DiccyPlugin`]
    /// implementation into the plugin registry.
    ///
    /// Each extension point is checked against the plugin's sandbox
    /// before registration. Undeclared extension points are rejected.
    ///
    /// The `extension_points` are consumed (taken by value) because
    /// they contain `Box<dyn Trait>` objects that need to be converted
    /// into `Arc<dyn Trait>` for shared ownership in the registry.
    pub fn register_extension_points(
        &mut self,
        plugin_name: &str,
        extension_points: Vec<ExtensionPoint>,
    ) -> Result<(), PluginError> {
        for ep in extension_points {
            match ep {
                ExtensionPoint::ViewerTool(tool) => {
                    if let Some(sandbox) = self.registry.plugin_sandbox(plugin_name) {
                        sandbox.check_extension_point("ViewerTool")?;
                    }
                    self.registry.register_viewer_tool(Arc::from(tool))?;
                }
                ExtensionPoint::ImageProcessor(processor) => {
                    if let Some(sandbox) = self.registry.plugin_sandbox(plugin_name) {
                        sandbox.check_extension_point("ImageProcessor")?;
                    }
                    self.registry
                        .register_image_processor(Arc::from(processor))?;
                }
                ExtensionPoint::WorkflowHook(hook) => {
                    if let Some(sandbox) = self.registry.plugin_sandbox(plugin_name) {
                        sandbox.check_extension_point("WorkflowHook")?;
                    }
                    self.registry.register_workflow_hook(Arc::from(hook))?;
                }
                ExtensionPoint::StorageBackend(backend) => {
                    if let Some(sandbox) = self.registry.plugin_sandbox(plugin_name) {
                        sandbox.check_extension_point("StorageBackend")?;
                    }
                    self.registry
                        .register_storage_backend(Arc::from(backend))?;
                }
            }
        }
        Ok(())
    }
}
