//! Plugin manifest parsing from `plugin.toml` files.
//!
//! Each plugin distribution must contain a `plugin.toml` manifest that
//! declares the plugin's identity, capabilities, and permission
//! requirements. The runtime reads this manifest *before* loading the
//! plugin code so that it can enforce sandboxing rules up front.

use crate::r#trait::PluginError;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Manifest types
// ---------------------------------------------------------------------------

/// Plugin manifest loaded from a `plugin.toml` file.
///
/// # Example `plugin.toml`
///
/// ```toml
/// [plugin]
/// name = "com.example.diccy-measure-tool"
/// version = "1.0.0"
/// description = "Advanced measurement tool for DICOM viewer"
/// author = "Example Corp"
///
/// [plugin.permissions]
/// extension_points = ["ViewerTool"]
/// network_access = false
/// filesystem_read = false
/// filesystem_write = false
/// max_memory_mb = 64
/// max_cpu_time_ms = 5000
///
/// [plugin.compatibility]
/// min_diccy_version = "0.14.0"
/// ```
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct PluginManifest {
    /// The `[plugin]` section.
    pub plugin: PluginSection,
}

/// The `[plugin]` section of a manifest.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct PluginSection {
    /// Unique plugin identifier (reverse-DNS recommended).
    pub name: String,
    /// Semantic version of the plugin.
    pub version: String,
    /// Human-readable description.
    #[serde(default)]
    pub description: String,
    /// Author or organisation.
    #[serde(default)]
    pub author: String,
    /// Permission and capability declarations.
    #[serde(default)]
    pub permissions: PluginPermissions,
    /// Compatibility constraints.
    #[serde(default)]
    pub compatibility: CompatibilitySection,
}

/// Permission and capability declarations for a plugin.
///
/// These are read by the sandbox layer before the plugin code is loaded
/// to enforce the principle of least privilege.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct PluginPermissions {
    /// Extension points this plugin is allowed to register.
    ///
    /// Valid values: `"ViewerTool"`, `"ImageProcessor"`,
    /// `"WorkflowHook"`, `"StorageBackend"`.
    #[serde(default)]
    pub extension_points: Vec<String>,
    /// Whether the plugin may access the network.
    #[serde(default)]
    pub network_access: bool,
    /// Whether the plugin may read from the filesystem.
    #[serde(default)]
    pub filesystem_read: bool,
    /// Whether the plugin may write to the filesystem.
    #[serde(default)]
    pub filesystem_write: bool,
    /// Maximum memory the plugin may allocate (MiB).
    #[serde(default = "default_max_memory_mb")]
    pub max_memory_mb: u64,
    /// Maximum CPU time per operation (ms).
    #[serde(default = "default_max_cpu_time_ms")]
    pub max_cpu_time_ms: u64,
}

impl Default for PluginPermissions {
    fn default() -> Self {
        Self {
            extension_points: Vec::new(),
            network_access: false,
            filesystem_read: false,
            filesystem_write: false,
            max_memory_mb: default_max_memory_mb(),
            max_cpu_time_ms: default_max_cpu_time_ms(),
        }
    }
}

fn default_max_memory_mb() -> u64 {
    64
}

fn default_max_cpu_time_ms() -> u64 {
    5000
}

/// Compatibility constraints for the plugin.
#[derive(Debug, Clone, Deserialize, PartialEq, Default)]
pub struct CompatibilitySection {
    /// Minimum DiCCY version required by this plugin.
    #[serde(default)]
    pub min_diccy_version: Option<String>,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Known extension point names that a plugin may declare.
pub const KNOWN_EXTENSION_POINTS: &[&str] = &[
    "ViewerTool",
    "ImageProcessor",
    "WorkflowHook",
    "StorageBackend",
];

/// Parse a `plugin.toml` string into a [`PluginManifest`].
pub fn parse_manifest(toml_str: &str) -> Result<PluginManifest, PluginError> {
    let manifest: PluginManifest = toml::from_str(toml_str).map_err(|e| {
        PluginError::InvalidManifest {
            reason: format!("TOML parse error: {e}"),
        }
    })?;

    validate_manifest(&manifest)?;
    Ok(manifest)
}

/// Load a `plugin.toml` file from disk.
pub fn load_manifest(path: &std::path::Path) -> Result<PluginManifest, PluginError> {
    let contents = std::fs::read_to_string(path).map_err(|e| PluginError::InvalidManifest {
        reason: format!("cannot read {}: {e}", path.display()),
    })?;
    parse_manifest(&contents)
}

/// Validate a parsed manifest for structural correctness.
fn validate_manifest(manifest: &PluginManifest) -> Result<(), PluginError> {
    // Name must be non-empty.
    if manifest.plugin.name.is_empty() {
        return Err(PluginError::InvalidManifest {
            reason: "plugin name must not be empty".to_string(),
        });
    }

    // Version must be non-empty.
    if manifest.plugin.version.is_empty() {
        return Err(PluginError::InvalidManifest {
            reason: "plugin version must not be empty".to_string(),
        });
    }

    // All declared extension points must be known.
    for ep in &manifest.plugin.permissions.extension_points {
        if !KNOWN_EXTENSION_POINTS.contains(&ep.as_str()) {
            return Err(PluginError::InvalidManifest {
                reason: format!(
                    "unknown extension point '{ep}'; valid values: {}",
                    KNOWN_EXTENSION_POINTS.join(", ")
                ),
            });
        }
    }

    Ok(())
}

/// Search a directory recursively for `plugin.toml` files.
///
/// Returns a list of paths to discovered manifest files.
pub fn discover_manifests(
    plugin_dir: &std::path::Path,
) -> Result<Vec<std::path::PathBuf>, PluginError> {
    let mut results = Vec::new();
    if !plugin_dir.is_dir() {
        return Ok(results);
    }

    visit_dir(plugin_dir, &mut results)?;
    results.sort();
    Ok(results)
}

fn visit_dir(
    dir: &std::path::Path,
    results: &mut Vec<std::path::PathBuf>,
) -> Result<(), PluginError> {
    let entries = std::fs::read_dir(dir).map_err(|e| PluginError::Other {
        reason: format!("cannot read directory {}: {e}", dir.display()),
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| PluginError::Other {
            reason: format!("directory entry error: {e}"),
        })?;
        let path = entry.path();
        if path.is_dir() {
            visit_dir(&path, results)?;
        } else if path.file_name().is_some_and(|n| n == "plugin.toml") {
            results.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod inline_tests {
    use super::*;

    #[test]
    fn parse_minimal_manifest() {
        let toml = r#"
[plugin]
name = "com.example.test"
version = "0.1.0"
"#;
        let manifest = parse_manifest(toml).unwrap();
        assert_eq!(manifest.plugin.name, "com.example.test");
        assert_eq!(manifest.plugin.version, "0.1.0");
        assert!(manifest.plugin.permissions.extension_points.is_empty());
    }

    #[test]
    fn reject_empty_name() {
        let toml = r#"
[plugin]
name = ""
version = "0.1.0"
"#;
        assert!(parse_manifest(toml).is_err());
    }

    #[test]
    fn reject_unknown_extension_point() {
        let toml = r#"
[plugin]
name = "com.example.test"
version = "0.1.0"

[plugin.permissions]
extension_points = ["UnknownThing"]
"#;
        assert!(parse_manifest(toml).is_err());
    }
}
