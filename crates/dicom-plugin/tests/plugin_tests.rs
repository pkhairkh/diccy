//! Integration tests for the dicom-plugin crate.
//!
//! Covers:
//! - PluginManifest parsing from TOML
//! - PluginRegistry registration and lookup
//! - PluginSandbox capability enforcement
//! - PluginManager discovery
//! - WASM binary validation
//! - Mock plugin load/unload cycle

use dicom_plugin::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

// ===================================================================
// Mock plugin implementations
// ===================================================================

struct MockViewerTool;

impl ViewerTool for MockViewerTool {
    fn tool_id(&self) -> &str {
        "com.example.mock-tool"
    }
    fn display_name(&self) -> &str {
        "Mock Measurement Tool"
    }
    fn icon_name(&self) -> &str {
        "ruler"
    }
    fn on_activate(&self, _ctx: &ToolContext) -> Result<(), PluginError> {
        Ok(())
    }
    fn on_deactivate(&self) -> Result<(), PluginError> {
        Ok(())
    }
}

struct MockImageProcessor;

impl ImageProcessor for MockImageProcessor {
    fn processor_id(&self) -> &str {
        "com.example.invert-processor"
    }
    fn input_format(&self) -> PixelFormat {
        PixelFormat::Luma8
    }
    fn output_format(&self) -> PixelFormat {
        PixelFormat::Luma8
    }
    fn process(&self, input: &[u8], _w: u32, _h: u32) -> Result<Vec<u8>, PluginError> {
        Ok(input.iter().map(|b| 255 - b).collect())
    }
}

struct MockWorkflowHook;

impl WorkflowHook for MockWorkflowHook {
    fn hook_id(&self) -> &str {
        "com.example.audit-hook"
    }
    fn event_types(&self) -> Vec<WorkflowEventType> {
        vec![WorkflowEventType::StudyReceived, WorkflowEventType::MeasurementAdded]
    }
    fn on_event(&self, event: &WorkflowEvent) -> Result<HookAction, PluginError> {
        match event.event_type {
            WorkflowEventType::StudyReceived => Ok(HookAction::Continue),
            WorkflowEventType::MeasurementAdded => {
                let mut mods = HashMap::new();
                mods.insert("audited".to_string(), "true".to_string());
                Ok(HookAction::Modify(mods))
            }
            _ => Ok(HookAction::Continue),
        }
    }
}

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
        "com.example.mem-backend"
    }
    fn put(&self, key: &str, data: &[u8]) -> Result<(), PluginError> {
        self.data.lock().unwrap().insert(key.to_string(), data.to_vec());
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
        Ok(guard.keys().filter(|k| k.starts_with(prefix)).cloned().collect())
    }
}

// A full DiccyPlugin implementation for the load/unload cycle test.
struct MockDiccyPlugin {
    name: String,
    version: String,
    loaded: std::sync::Mutex<bool>,
}

impl MockDiccyPlugin {
    fn new(name: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            loaded: std::sync::Mutex::new(false),
        }
    }
}

impl DiccyPlugin for MockDiccyPlugin {
    fn name(&self) -> &str {
        &self.name
    }
    fn version(&self) -> &str {
        &self.version
    }
    fn on_load(&mut self, _context: &PluginContext) -> Result<(), PluginError> {
        *self.loaded.lock().unwrap() = true;
        Ok(())
    }
    fn on_unload(&mut self) -> Result<(), PluginError> {
        *self.loaded.lock().unwrap() = false;
        Ok(())
    }
    fn extension_points(&self) -> Vec<ExtensionPoint> {
        // Return empty for the basic lifecycle test.
        Vec::new()
    }
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            description: "A mock plugin for testing".to_string(),
            author: "DiCCY Test Suite".to_string(),
            url: "https://example.com".to_string(),
            license: "MIT".to_string(),
        }
    }
}

// ===================================================================
// Test: PluginManifest parsing from TOML
// ===================================================================

#[test]
fn test_parse_full_manifest() {
    let toml = r#"
[plugin]
name = "com.example.diccy-measure-tool"
version = "1.0.0"
description = "Advanced measurement tool"
author = "Example Corp"

[plugin.permissions]
extension_points = ["ViewerTool", "WorkflowHook"]
network_access = false
filesystem_read = true
filesystem_write = false
max_memory_mb = 128
max_cpu_time_ms = 3000

[plugin.compatibility]
min_diccy_version = "0.14.0"
"#;
    let manifest = parse_manifest(toml).unwrap();
    assert_eq!(manifest.plugin.name, "com.example.diccy-measure-tool");
    assert_eq!(manifest.plugin.version, "1.0.0");
    assert_eq!(manifest.plugin.description, "Advanced measurement tool");
    assert_eq!(manifest.plugin.author, "Example Corp");
    assert_eq!(
        manifest.plugin.permissions.extension_points,
        vec!["ViewerTool", "WorkflowHook"]
    );
    assert!(!manifest.plugin.permissions.network_access);
    assert!(manifest.plugin.permissions.filesystem_read);
    assert!(!manifest.plugin.permissions.filesystem_write);
    assert_eq!(manifest.plugin.permissions.max_memory_mb, 128);
    assert_eq!(manifest.plugin.permissions.max_cpu_time_ms, 3000);
    assert_eq!(
        manifest.plugin.compatibility.min_diccy_version,
        Some("0.14.0".to_string())
    );
}

#[test]
fn test_parse_minimal_manifest() {
    let toml = r#"
[plugin]
name = "com.example.minimal"
version = "0.1.0"
"#;
    let manifest = parse_manifest(toml).unwrap();
    assert_eq!(manifest.plugin.name, "com.example.minimal");
    assert!(manifest.plugin.permissions.extension_points.is_empty());
    assert!(!manifest.plugin.permissions.network_access);
    assert_eq!(manifest.plugin.compatibility.min_diccy_version, None);
}

#[test]
fn test_parse_manifest_rejects_empty_name() {
    let toml = r#"
[plugin]
name = ""
version = "0.1.0"
"#;
    assert!(parse_manifest(toml).is_err());
}

#[test]
fn test_parse_manifest_rejects_empty_version() {
    let toml = r#"
[plugin]
name = "com.example.test"
version = ""
"#;
    assert!(parse_manifest(toml).is_err());
}

#[test]
fn test_parse_manifest_rejects_unknown_extension_point() {
    let toml = r#"
[plugin]
name = "com.example.test"
version = "0.1.0"

[plugin.permissions]
extension_points = ["Telepathy"]
"#;
    let err = parse_manifest(toml).unwrap_err();
    match err {
        PluginError::InvalidManifest { reason } => {
            assert!(reason.contains("unknown extension point"));
        }
        other => panic!("expected InvalidManifest, got {other:?}"),
    }
}

#[test]
fn test_load_manifest_from_file() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/test_plugin.toml");
    let manifest = load_manifest(&path).unwrap();
    assert_eq!(manifest.plugin.name, "com.example.diccy-measure-tool");
    assert_eq!(manifest.plugin.version, "1.0.0");
}

// ===================================================================
// Test: PluginRegistry registration and lookup
// ===================================================================

#[test]
fn test_registry_viewer_tool_lifecycle() {
    let mut registry = PluginRegistry::new();

    // Register
    registry
        .register_viewer_tool(Arc::new(MockViewerTool))
        .unwrap();

    // Lookup
    let tool = registry.get_viewer_tool("com.example.mock-tool").unwrap();
    assert_eq!(tool.tool_id(), "com.example.mock-tool");
    assert_eq!(tool.display_name(), "Mock Measurement Tool");
    assert_eq!(tool.icon_name(), "ruler");

    // Activate / deactivate
    let ctx = ToolContext::default();
    assert!(tool.on_activate(&ctx).is_ok());
    assert!(tool.on_deactivate().is_ok());

    // IDs listing
    assert_eq!(registry.viewer_tool_ids(), vec!["com.example.mock-tool"]);
}

#[test]
fn test_registry_image_processor_lifecycle() {
    let mut registry = PluginRegistry::new();
    registry
        .register_image_processor(Arc::new(MockImageProcessor))
        .unwrap();

    let proc = registry.get_image_processor("com.example.invert-processor").unwrap();
    assert_eq!(proc.processor_id(), "com.example.invert-processor");
    assert_eq!(proc.input_format(), PixelFormat::Luma8);
    assert_eq!(proc.output_format(), PixelFormat::Luma8);

    // Process pixels
    let input = vec![0u8, 128, 255];
    let output = proc.process(&input, 3, 1).unwrap();
    assert_eq!(output, vec![255, 127, 0]);
}

#[test]
fn test_registry_workflow_hook_lifecycle() {
    let mut registry = PluginRegistry::new();
    registry
        .register_workflow_hook(Arc::new(MockWorkflowHook))
        .unwrap();

    let hook = registry.get_workflow_hook("com.example.audit-hook").unwrap();
    assert_eq!(hook.hook_id(), "com.example.audit-hook");
    assert_eq!(
        hook.event_types(),
        vec![WorkflowEventType::StudyReceived, WorkflowEventType::MeasurementAdded]
    );

    // StudyReceived → Continue
    let event = WorkflowEvent::new(WorkflowEventType::StudyReceived);
    let action = hook.on_event(&event).unwrap();
    assert_eq!(action, HookAction::Continue);

    // MeasurementAdded → Modify
    let event = WorkflowEvent::new(WorkflowEventType::MeasurementAdded);
    let action = hook.on_event(&event).unwrap();
    match action {
        HookAction::Modify(mods) => {
            assert_eq!(mods.get("audited"), Some(&"true".to_string()));
        }
        other => panic!("expected Modify, got {other:?}"),
    }

    // Hooks by event type
    let study_hooks = registry.workflow_hooks_for_event(WorkflowEventType::StudyReceived);
    assert_eq!(study_hooks.len(), 1);
    let report_hooks = registry.workflow_hooks_for_event(WorkflowEventType::ReportCreated);
    assert!(report_hooks.is_empty());
}

#[test]
fn test_registry_storage_backend_lifecycle() {
    let mut registry = PluginRegistry::new();
    let backend = Arc::new(MockStorageBackend::new());
    registry.register_storage_backend(backend.clone()).unwrap();

    let b = registry.get_storage_backend("com.example.mem-backend").unwrap();

    // Put + Get
    b.put("studies/123", b"dcm-data").unwrap();
    let data = b.get("studies/123").unwrap();
    assert_eq!(data, Some(b"dcm-data".to_vec()));

    // List
    b.put("studies/456", b"other").unwrap();
    let keys = b.list("studies/").unwrap();
    assert_eq!(keys.len(), 2);

    // Delete
    assert!(b.delete("studies/123").unwrap());
    assert!(!b.delete("studies/123").unwrap()); // already deleted
    assert_eq!(b.get("studies/123").unwrap(), None);
}

#[test]
fn test_registry_rejects_duplicate_plugin() {
    let mut registry = PluginRegistry::new();
    let sandbox = PluginSandbox::permissive("test.plugin");
    registry
        .register_plugin("test.plugin".to_string(), "1.0.0".to_string(), sandbox)
        .unwrap();
    let sandbox2 = PluginSandbox::permissive("test.plugin");
    assert!(registry
        .register_plugin("test.plugin".to_string(), "2.0.0".to_string(), sandbox2)
        .is_err());
}

// ===================================================================
// Test: PluginSandbox capability enforcement
// ===================================================================

#[test]
fn test_sandbox_allows_declared_extension_points() {
    let permissions = PluginPermissions {
        extension_points: vec!["ViewerTool".to_string(), "WorkflowHook".to_string()],
        ..Default::default()
    };
    let sandbox = PluginSandbox::from_permissions("test.plugin", &permissions);

    assert!(sandbox.check_extension_point("ViewerTool").is_ok());
    assert!(sandbox.check_extension_point("WorkflowHook").is_ok());
}

#[test]
fn test_sandbox_blocks_undeclared_extension_points() {
    let permissions = PluginPermissions {
        extension_points: vec!["ViewerTool".to_string()],
        ..Default::default()
    };
    let sandbox = PluginSandbox::from_permissions("test.plugin", &permissions);

    assert!(sandbox.check_extension_point("ImageProcessor").is_err());
    assert!(sandbox.check_extension_point("WorkflowHook").is_err());
    assert!(sandbox.check_extension_point("StorageBackend").is_err());
}

#[test]
fn test_sandbox_network_access() {
    let perms_off = PluginPermissions {
        network_access: false,
        ..Default::default()
    };
    let sandbox_off = PluginSandbox::from_permissions("test.plugin", &perms_off);
    assert!(sandbox_off.check_network_access().is_err());

    let perms_on = PluginPermissions {
        network_access: true,
        ..Default::default()
    };
    let sandbox_on = PluginSandbox::from_permissions("test.plugin", &perms_on);
    assert!(sandbox_on.check_network_access().is_ok());
}

#[test]
fn test_sandbox_filesystem_permissions() {
    let perms = PluginPermissions {
        filesystem_read: true,
        filesystem_write: false,
        ..Default::default()
    };
    let sandbox = PluginSandbox::from_permissions("test.plugin", &perms);

    assert!(sandbox.check_filesystem_read().is_ok());
    assert!(sandbox.check_filesystem_write().is_err());
}

#[test]
fn test_sandbox_resource_limits() {
    let perms = PluginPermissions {
        max_memory_mb: 256,
        max_cpu_time_ms: 10000,
        ..Default::default()
    };
    let sandbox = PluginSandbox::from_permissions("test.plugin", &perms);
    let limits = sandbox.resource_limits();
    assert_eq!(limits.max_memory_bytes, 256 * 1024 * 1024);
    assert_eq!(limits.max_cpu_time_ms, 10000);
}

#[test]
fn test_permissive_sandbox() {
    let sandbox = PluginSandbox::permissive("test.plugin");
    assert!(sandbox.check_extension_point("ViewerTool").is_ok());
    assert!(sandbox.check_extension_point("ImageProcessor").is_ok());
    assert!(sandbox.check_extension_point("WorkflowHook").is_ok());
    assert!(sandbox.check_extension_point("StorageBackend").is_ok());
    assert!(sandbox.check_network_access().is_ok());
    assert!(sandbox.check_filesystem_read().is_ok());
    assert!(sandbox.check_filesystem_write().is_ok());
}

// ===================================================================
// Test: PluginManager discovery
// ===================================================================

#[test]
fn test_discover_manifests_finds_test_file() {
    // Create a temporary directory with a properly-named plugin.toml
    // (discover_manifests only looks for files named "plugin.toml").
    let dir = tempfile::tempdir().unwrap();
    let plugin_dir = dir.path().join("sample-plugin");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    std::fs::write(
        plugin_dir.join("plugin.toml"),
        r#"
[plugin]
name = "com.example.discoverable"
version = "1.0.0"
"#,
    )
    .unwrap();

    let manager = PluginManager::new();
    let manifests = manager.scan_directory(dir.path()).unwrap();
    assert_eq!(manifests.len(), 1, "expected 1 manifest, got {manifests:?}");
    assert!(manifests[0].ends_with("plugin.toml"));
}

#[test]
fn test_discover_manifests_empty_dir() {
    let dir = tempfile::tempdir().unwrap();
    let manager = PluginManager::new();
    let manifests = manager.scan_directory(dir.path()).unwrap();
    assert!(manifests.is_empty());
}

#[test]
fn test_discover_manifests_nonexistent_dir() {
    let manager = PluginManager::new();
    let manifests = manager.scan_directory(Path::new("/nonexistent/path")).unwrap();
    assert!(manifests.is_empty());
}

// ===================================================================
// Test: WASM binary validation
// ===================================================================

#[test]
fn test_validate_valid_wasm() {
    let wasm: Vec<u8> = vec![0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
    assert!(validate_wasm_binary(&wasm).is_ok());
}

#[test]
fn test_validate_wasm_too_short() {
    assert!(validate_wasm_binary(&[0x00, 0x61, 0x73]).is_err());
}

#[test]
fn test_validate_wasm_bad_magic() {
    let wasm: Vec<u8> = vec![0x89, 0x50, 0x4E, 0x47, 0x01, 0x00, 0x00, 0x00]; // PNG magic
    assert!(validate_wasm_binary(&wasm).is_err());
}

#[test]
fn test_validate_wasm_bad_version() {
    let wasm: Vec<u8> = vec![0x00, 0x61, 0x73, 0x6D, 0x02, 0x00, 0x00, 0x00];
    assert!(validate_wasm_binary(&wasm).is_err());
}

#[test]
fn test_is_wasm_binary() {
    let good: Vec<u8> = vec![0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
    assert!(is_wasm_binary(&good));
    let bad: Vec<u8> = vec![0x89, 0x50, 0x4E, 0x47];
    assert!(!is_wasm_binary(&bad));
}

#[test]
fn test_wasi_runtime_full_lifecycle() {
    let mut rt = WasiPluginRuntime::new();
    let wasm: Vec<u8> = vec![0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00];

    // Load WASM
    rt.load_wasm(&wasm).unwrap();
    assert!(rt.has_wasm());
    assert!(!rt.is_loaded());

    // on_load
    rt.call_on_load().unwrap();
    assert!(rt.is_loaded());

    // on_unload
    rt.call_on_unload().unwrap();
    assert!(!rt.is_loaded());
}

// ===================================================================
// Test: PluginManager load/unload cycle
// ===================================================================

#[test]
fn test_load_native_plugin_via_manifest() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/test_plugin.toml");
    let mut manager = PluginManager::new();

    let name = manager.load_plugin(&path).unwrap();
    assert_eq!(name, "com.example.diccy-measure-tool");
    assert!(manager.is_loaded("com.example.diccy-measure-tool"));
    assert_eq!(manager.plugin_count(), 1);

    // Unload
    manager.unload_plugin("com.example.diccy-measure-tool").unwrap();
    assert!(!manager.is_loaded("com.example.diccy-measure-tool"));
    assert_eq!(manager.plugin_count(), 0);
}

#[test]
fn test_load_wasm_plugin() {
    let manifest_str = r#"
[plugin]
name = "com.example.wasm-plugin"
version = "2.0.0"

[plugin.permissions]
extension_points = ["ImageProcessor"]
max_memory_mb = 64
"#;
    let manifest = parse_manifest(manifest_str).unwrap();
    let wasm: Vec<u8> = vec![0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];

    let mut manager = PluginManager::new();
    let name = manager.load_wasm_plugin(manifest, &wasm).unwrap();
    assert_eq!(name, "com.example.wasm-plugin");
    assert!(manager.is_loaded("com.example.wasm-plugin"));
}

#[test]
fn test_wasm_plugin_rejects_invalid_binary() {
    let manifest_str = r#"
[plugin]
name = "com.example.bad-wasm"
version = "1.0.0"
"#;
    let manifest = parse_manifest(manifest_str).unwrap();
    let bad_wasm: Vec<u8> = vec![0xDE, 0xAD, 0xBE, 0xEF]; // Not WASM

    let mut manager = PluginManager::new();
    assert!(manager.load_wasm_plugin(manifest, &bad_wasm).is_err());
}

#[test]
fn test_mock_diccy_plugin_lifecycle() {
    let mut plugin = MockDiccyPlugin::new("com.example.lifecycle", "3.0.0");

    // Metadata
    assert_eq!(plugin.name(), "com.example.lifecycle");
    assert_eq!(plugin.version(), "3.0.0");
    let meta = plugin.metadata();
    assert_eq!(meta.author, "DiCCY Test Suite");

    // on_load
    let ctx = PluginContext::default();
    assert!(plugin.on_load(&ctx).is_ok());
    assert!(*plugin.loaded.lock().unwrap());

    // on_unload
    assert!(plugin.on_unload().is_ok());
    assert!(!*plugin.loaded.lock().unwrap());
}

#[test]
fn test_plugin_manager_with_direct_extension_registration() {
    let mut manager = PluginManager::new();

    // Register a plugin in the registry with a permissive sandbox.
    let sandbox = PluginSandbox::permissive("com.example.direct");
    manager
        .registry_mut()
        .register_plugin("com.example.direct".to_string(), "1.0.0".to_string(), sandbox)
        .unwrap();

    // Register extensions directly on the registry.
    manager
        .registry_mut()
        .register_viewer_tool(Arc::new(MockViewerTool))
        .unwrap();
    manager
        .registry_mut()
        .register_image_processor(Arc::new(MockImageProcessor))
        .unwrap();
    manager
        .registry_mut()
        .register_workflow_hook(Arc::new(MockWorkflowHook))
        .unwrap();
    manager
        .registry_mut()
        .register_storage_backend(Arc::new(MockStorageBackend::new()))
        .unwrap();

    // Verify all extensions are accessible.
    let reg = manager.registry();
    assert!(reg.get_viewer_tool("com.example.mock-tool").is_some());
    assert!(reg.get_image_processor("com.example.invert-processor").is_some());
    assert!(reg.get_workflow_hook("com.example.audit-hook").is_some());
    assert!(reg.get_storage_backend("com.example.mem-backend").is_some());

    // Verify functionality
    let proc = reg.get_image_processor("com.example.invert-processor").unwrap();
    let result = proc.process(&[100], 1, 1).unwrap();
    assert_eq!(result, vec![155]); // 255 - 100

    let backend = reg.get_storage_backend("com.example.mem-backend").unwrap();
    backend.put("key", b"value").unwrap();
    assert_eq!(backend.get("key").unwrap(), Some(b"value".to_vec()));
}

#[test]
fn test_sandbox_enforced_during_extension_registration() {
    let mut manager = PluginManager::new();

    // Register a plugin with a sandbox that only allows ViewerTool.
    let permissions = PluginPermissions {
        extension_points: vec!["ViewerTool".to_string()],
        ..Default::default()
    };
    let sandbox = PluginSandbox::from_permissions("com.example.restricted", &permissions);
    manager
        .registry_mut()
        .register_plugin(
            "com.example.restricted".to_string(),
            "1.0.0".to_string(),
            sandbox,
        )
        .unwrap();

    // Verify the sandbox blocks ImageProcessor.
    let sandbox_ref = manager.registry().plugin_sandbox("com.example.restricted").unwrap();
    assert!(sandbox_ref.check_extension_point("ViewerTool").is_ok());
    assert!(sandbox_ref.check_extension_point("ImageProcessor").is_err());
}

#[test]
fn test_discover_and_load_workflow() {
    let dir = tempfile::tempdir().unwrap();

    // Create a plugin.toml in the temp directory.
    let manifest_content = r#"
[plugin]
name = "com.example.discovered"
version = "0.5.0"
description = "Discovered plugin"

[plugin.permissions]
extension_points = ["StorageBackend"]
"#;
    let plugin_dir = dir.path().join("my-plugin");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    std::fs::write(plugin_dir.join("plugin.toml"), manifest_content).unwrap();

    // Discover
    let manager = PluginManager::new();
    let manifests = manager.scan_directory(dir.path()).unwrap();
    assert_eq!(manifests.len(), 1);

    // Load
    let mut manager = manager;
    let name = manager.load_plugin(&manifests[0]).unwrap();
    assert_eq!(name, "com.example.discovered");
    assert!(manager.is_loaded("com.example.discovered"));
}

#[test]
fn test_workflow_event_context() {
    let mut event = WorkflowEvent::new(WorkflowEventType::StoreCompleted);
    event.study_instance_uid = Some("1.2.3.4.5".to_string());
    event.payload.insert("status".to_string(), "success".to_string());

    assert_eq!(event.event_type, WorkflowEventType::StoreCompleted);
    assert_eq!(event.study_instance_uid, Some("1.2.3.4.5".to_string()));
    assert_eq!(event.payload.get("status"), Some(&"success".to_string()));
}

#[test]
fn test_known_extension_points_constant() {
    assert!(KNOWN_EXTENSION_POINTS.contains(&"ViewerTool"));
    assert!(KNOWN_EXTENSION_POINTS.contains(&"ImageProcessor"));
    assert!(KNOWN_EXTENSION_POINTS.contains(&"WorkflowHook"));
    assert!(KNOWN_EXTENSION_POINTS.contains(&"StorageBackend"));
    assert_eq!(KNOWN_EXTENSION_POINTS.len(), 4);
}
