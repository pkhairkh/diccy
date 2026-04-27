//! Plugin registry for managing loaded plugins and extension point lookup.
//!
//! The [`PluginRegistry`] is the central index of all active plugins and
//! their registered extension points. It supports registering and
//! looking up extensions by type and identifier.

use std::collections::HashMap;
use std::sync::Arc;

use crate::sandbox::PluginSandbox;
use crate::r#trait::{
    ImageProcessor, PluginError, StorageBackendPlugin, ViewerTool, WorkflowHook,
};

// ---------------------------------------------------------------------------
// LoadedPlugin
// ---------------------------------------------------------------------------

/// A loaded plugin together with its sandbox.
pub struct LoadedPlugin {
    /// Plugin name (matches [`crate::r#trait::DiccyPlugin::name`]).
    pub name: String,
    /// Plugin version.
    pub version: String,
    /// Sandbox enforcing this plugin's declared permissions.
    pub sandbox: PluginSandbox,
}

// ---------------------------------------------------------------------------
// PluginRegistry
// ---------------------------------------------------------------------------

/// Manages loaded plugins and provides extension point lookup.
///
/// The registry is populated by the [`crate::PluginManager`] during the
/// plugin loading phase. Each extension point is stored in a separate
/// collection keyed by its identifier, allowing fast lookup by runtime
/// surfaces that need to dispatch to a specific extension.
pub struct PluginRegistry {
    /// Loaded plugins keyed by plugin name.
    plugins: HashMap<String, LoadedPlugin>,
    /// Registered viewer tools keyed by `tool_id`.
    viewer_tools: HashMap<String, Arc<dyn ViewerTool>>,
    /// Registered image processors keyed by `processor_id`.
    image_processors: HashMap<String, Arc<dyn ImageProcessor>>,
    /// Registered workflow hooks keyed by `hook_id`.
    workflow_hooks: HashMap<String, Arc<dyn WorkflowHook>>,
    /// Registered storage backends keyed by `backend_id`.
    storage_backends: HashMap<String, Arc<dyn StorageBackendPlugin>>,
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            viewer_tools: HashMap::new(),
            image_processors: HashMap::new(),
            workflow_hooks: HashMap::new(),
            storage_backends: HashMap::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Plugin registration
    // -----------------------------------------------------------------------

    /// Register a loaded plugin.
    ///
    /// Returns an error if a plugin with the same name is already registered.
    pub fn register_plugin(
        &mut self,
        name: String,
        version: String,
        sandbox: PluginSandbox,
    ) -> Result<(), PluginError> {
        if self.plugins.contains_key(&name) {
            return Err(PluginError::Other {
                reason: format!("plugin '{name}' is already registered"),
            });
        }
        self.plugins.insert(
            name.clone(),
            LoadedPlugin {
                name,
                version,
                sandbox,
            },
        );
        Ok(())
    }

    /// Unregister a plugin by name.
    ///
    /// This removes the plugin entry but does **not** automatically remove
    /// extension points that were registered by that plugin. The caller
    /// is responsible for removing extension points before unregistering.
    pub fn unregister_plugin(&mut self, name: &str) -> Result<(), PluginError> {
        if self.plugins.remove(name).is_none() {
            return Err(PluginError::Other {
                reason: format!("plugin '{name}' is not registered"),
            });
        }
        Ok(())
    }

    /// Return whether a plugin with the given name is registered.
    pub fn has_plugin(&self, name: &str) -> bool {
        self.plugins.contains_key(name)
    }

    /// Return a reference to the sandbox for a plugin, if it exists.
    pub fn plugin_sandbox(&self, name: &str) -> Option<&PluginSandbox> {
        self.plugins.get(name).map(|p| &p.sandbox)
    }

    /// Return the number of registered plugins.
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }

    /// Return the names of all registered plugins.
    pub fn plugin_names(&self) -> Vec<&str> {
        self.plugins.keys().map(|s| s.as_str()).collect()
    }

    // -----------------------------------------------------------------------
    // ViewerTool
    // -----------------------------------------------------------------------

    /// Register a viewer tool.
    pub fn register_viewer_tool(&mut self, tool: Arc<dyn ViewerTool>) -> Result<(), PluginError> {
        let id = tool.tool_id().to_string();
        if self.viewer_tools.contains_key(&id) {
            return Err(PluginError::Other {
                reason: format!("viewer tool '{id}' is already registered"),
            });
        }
        self.viewer_tools.insert(id, tool);
        Ok(())
    }

    /// Look up a viewer tool by identifier.
    pub fn get_viewer_tool(&self, tool_id: &str) -> Option<&Arc<dyn ViewerTool>> {
        self.viewer_tools.get(tool_id)
    }

    /// Return all registered viewer tool identifiers.
    pub fn viewer_tool_ids(&self) -> Vec<&str> {
        self.viewer_tools.keys().map(|s| s.as_str()).collect()
    }

    // -----------------------------------------------------------------------
    // ImageProcessor
    // -----------------------------------------------------------------------

    /// Register an image processor.
    pub fn register_image_processor(
        &mut self,
        processor: Arc<dyn ImageProcessor>,
    ) -> Result<(), PluginError> {
        let id = processor.processor_id().to_string();
        if self.image_processors.contains_key(&id) {
            return Err(PluginError::Other {
                reason: format!("image processor '{id}' is already registered"),
            });
        }
        self.image_processors.insert(id, processor);
        Ok(())
    }

    /// Look up an image processor by identifier.
    pub fn get_image_processor(&self, processor_id: &str) -> Option<&Arc<dyn ImageProcessor>> {
        self.image_processors.get(processor_id)
    }

    /// Return all registered image processor identifiers.
    pub fn image_processor_ids(&self) -> Vec<&str> {
        self.image_processors.keys().map(|s| s.as_str()).collect()
    }

    // -----------------------------------------------------------------------
    // WorkflowHook
    // -----------------------------------------------------------------------

    /// Register a workflow hook.
    pub fn register_workflow_hook(&mut self, hook: Arc<dyn WorkflowHook>) -> Result<(), PluginError> {
        let id = hook.hook_id().to_string();
        if self.workflow_hooks.contains_key(&id) {
            return Err(PluginError::Other {
                reason: format!("workflow hook '{id}' is already registered"),
            });
        }
        self.workflow_hooks.insert(id, hook);
        Ok(())
    }

    /// Look up a workflow hook by identifier.
    pub fn get_workflow_hook(&self, hook_id: &str) -> Option<&Arc<dyn WorkflowHook>> {
        self.workflow_hooks.get(hook_id)
    }

    /// Return all workflow hooks that subscribe to the given event type.
    pub fn workflow_hooks_for_event(
        &self,
        event_type: crate::r#trait::WorkflowEventType,
    ) -> Vec<&Arc<dyn WorkflowHook>> {
        self.workflow_hooks
            .values()
            .filter(|hook| hook.event_types().contains(&event_type))
            .collect()
    }

    /// Return all registered workflow hook identifiers.
    pub fn workflow_hook_ids(&self) -> Vec<&str> {
        self.workflow_hooks.keys().map(|s| s.as_str()).collect()
    }

    // -----------------------------------------------------------------------
    // StorageBackend
    // -----------------------------------------------------------------------

    /// Register a storage backend.
    pub fn register_storage_backend(
        &mut self,
        backend: Arc<dyn StorageBackendPlugin>,
    ) -> Result<(), PluginError> {
        let id = backend.backend_id().to_string();
        if self.storage_backends.contains_key(&id) {
            return Err(PluginError::Other {
                reason: format!("storage backend '{id}' is already registered"),
            });
        }
        self.storage_backends.insert(id, backend);
        Ok(())
    }

    /// Look up a storage backend by identifier.
    pub fn get_storage_backend(&self, backend_id: &str) -> Option<&Arc<dyn StorageBackendPlugin>> {
        self.storage_backends.get(backend_id)
    }

    /// Return all registered storage backend identifiers.
    pub fn storage_backend_ids(&self) -> Vec<&str> {
        self.storage_backends.keys().map(|s| s.as_str()).collect()
    }
}

#[cfg(test)]
mod inline_tests {
    use super::*;
    use crate::r#trait::{HookAction, PixelFormat, ToolContext, WorkflowEvent};

    // -- Test ViewerTool ----------------------------------------------------

    struct MockViewerTool;

    impl ViewerTool for MockViewerTool {
        fn tool_id(&self) -> &str {
            "mock.tool"
        }
        fn display_name(&self) -> &str {
            "Mock Tool"
        }
        fn on_activate(&self, _ctx: &ToolContext) -> Result<(), PluginError> {
            Ok(())
        }
        fn on_deactivate(&self) -> Result<(), PluginError> {
            Ok(())
        }
    }

    // -- Test ImageProcessor ------------------------------------------------

    struct MockImageProcessor;

    impl ImageProcessor for MockImageProcessor {
        fn processor_id(&self) -> &str {
            "mock.processor"
        }
        fn input_format(&self) -> PixelFormat {
            PixelFormat::Luma8
        }
        fn output_format(&self) -> PixelFormat {
            PixelFormat::Luma8
        }
        fn process(&self, input: &[u8], _w: u32, _h: u32) -> Result<Vec<u8>, PluginError> {
            Ok(input.to_vec())
        }
    }

    // -- Test WorkflowHook --------------------------------------------------

    struct MockWorkflowHook;

    impl WorkflowHook for MockWorkflowHook {
        fn hook_id(&self) -> &str {
            "mock.hook"
        }
        fn event_types(&self) -> Vec<crate::r#trait::WorkflowEventType> {
            vec![crate::r#trait::WorkflowEventType::StudyReceived]
        }
        fn on_event(&self, _event: &WorkflowEvent) -> Result<HookAction, PluginError> {
            Ok(HookAction::Continue)
        }
    }

    // -- Test StorageBackend -----------------------------------------------

    struct MockStorageBackend {
        data: std::sync::Mutex<HashMap<String, Vec<u8>>>,
    }

    impl MockStorageBackend {
        fn new() -> Self {
            Self {
                data: std::sync::Mutex::new(HashMap::new()),
            }
        }
    }

    impl StorageBackendPlugin for MockStorageBackend {
        fn backend_id(&self) -> &str {
            "mock.storage"
        }
        fn put(&self, key: &str, data: &[u8]) -> Result<(), PluginError> {
            self.data
                .lock()
                .unwrap()
                .insert(key.to_string(), data.to_vec());
            Ok(())
        }
        fn get(&self, key: &str) -> Result<Option<Vec<u8>>, PluginError> {
            Ok(self.data.lock().unwrap().get(key).cloned())
        }
        fn delete(&self, key: &str) -> Result<bool, PluginError> {
            Ok(self.data.lock().unwrap().remove(key).is_some())
        }
        fn list(&self, prefix: &str) -> Result<Vec<String>, PluginError> {
            let guard = self.data.lock().unwrap();
            let keys: Vec<String> = guard
                .keys()
                .filter(|k| k.starts_with(prefix))
                .cloned()
                .collect();
            Ok(keys)
        }
    }

    #[test]
    fn register_and_lookup_viewer_tool() {
        let mut registry = PluginRegistry::new();
        registry
            .register_viewer_tool(Arc::new(MockViewerTool))
            .unwrap();
        assert!(registry.get_viewer_tool("mock.tool").is_some());
        assert!(registry.get_viewer_tool("nonexistent").is_none());
        assert_eq!(registry.viewer_tool_ids(), vec!["mock.tool"]);
    }

    #[test]
    fn register_and_lookup_image_processor() {
        let mut registry = PluginRegistry::new();
        registry
            .register_image_processor(Arc::new(MockImageProcessor))
            .unwrap();
        assert!(registry.get_image_processor("mock.processor").is_some());
        assert_eq!(registry.image_processor_ids(), vec!["mock.processor"]);
    }

    #[test]
    fn register_and_lookup_workflow_hook() {
        let mut registry = PluginRegistry::new();
        registry
            .register_workflow_hook(Arc::new(MockWorkflowHook))
            .unwrap();
        assert!(registry.get_workflow_hook("mock.hook").is_some());

        let hooks = registry.workflow_hooks_for_event(crate::r#trait::WorkflowEventType::StudyReceived);
        assert_eq!(hooks.len(), 1);

        let no_hooks = registry.workflow_hooks_for_event(crate::r#trait::WorkflowEventType::ReportCreated);
        assert!(no_hooks.is_empty());
    }

    #[test]
    fn register_and_lookup_storage_backend() {
        let mut registry = PluginRegistry::new();
        let backend = Arc::new(MockStorageBackend::new());
        backend.put("key1", b"value1").unwrap();
        registry.register_storage_backend(backend).unwrap();
        assert!(registry.get_storage_backend("mock.storage").is_some());

        let b = registry.get_storage_backend("mock.storage").unwrap();
        let result = b.get("key1").unwrap();
        assert_eq!(result, Some(b"value1".to_vec()));
    }

    #[test]
    fn reject_duplicate_plugin() {
        let mut registry = PluginRegistry::new();
        let sandbox = PluginSandbox::permissive("test.plugin");
        registry
            .register_plugin("test.plugin".to_string(), "1.0.0".to_string(), sandbox)
            .unwrap();
        let sandbox2 = PluginSandbox::permissive("test.plugin");
        assert!(registry
            .register_plugin("test.plugin".to_string(), "1.0.0".to_string(), sandbox2)
            .is_err());
    }

    #[test]
    fn unregister_plugin() {
        let mut registry = PluginRegistry::new();
        let sandbox = PluginSandbox::permissive("test.plugin");
        registry
            .register_plugin("test.plugin".to_string(), "1.0.0".to_string(), sandbox)
            .unwrap();
        assert!(registry.has_plugin("test.plugin"));
        registry.unregister_plugin("test.plugin").unwrap();
        assert!(!registry.has_plugin("test.plugin"));
    }
}
