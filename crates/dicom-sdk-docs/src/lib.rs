//! SDK documentation and extension guide for the DiCCY PACS workstation.
//!
//! This crate provides:
//! - **Trait guides** — Documentation for `Pack`, `FromDataset`,
//!   `OverlayRenderable`, and `PixelCodec` traits
//! - **Example extensions** — Complete, compilable examples of
//!   custom codecs, packs, and overlay renderers
//! - **Stability policy** — API stability guarantees and migration guides
//! - **Manifest schema** — Validation for `plugin.toml` manifests
//!
//! # Quick start for extension developers
//!
//! 1. Create a `plugin.toml` manifest declaring your extension points
//! 2. Implement the [`DiccyPlugin`](dicom_plugin::DiccyPlugin) trait
//! 3. Implement the specific trait(s) for your extension point(s)
//! 4. Build as a native dynamic library or WASM module
//! 5. Place in the DiCCY plugins directory
//!
//! See the `examples/` module for complete working examples.

#![deny(missing_docs)]

pub mod examples;

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// Trait guides
// ---------------------------------------------------------------------------

/// Documentation for the `Pack` trait.
///
/// A **Pack** is a bundle of configuration, overlays, and measurement
/// tools that customise the DiCCY viewer for a particular modality.
/// Built-in packs include CT, MR, MG, and enhanced multi-frame.
///
/// # Implementing a custom pack
///
/// A custom pack should:
/// 1. Implement `DiccyPlugin` with `extension_points()` returning
///    the pack's `ViewerTool` and `ImageProcessor` instances
/// 2. Declare `ViewerTool` and `ImageProcessor` in `plugin.toml`
///    permissions
/// 3. Include a modality-specific configuration that tells DiCCY
///    when to activate the pack (based on Modality tag)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackTraitGuide {
    /// Trait name.
    pub trait_name: String,
    /// Human-readable description of the trait.
    pub description: String,
    /// Required methods.
    pub required_methods: Vec<String>,
    /// Example implementations.
    pub examples: Vec<String>,
}

impl Default for PackTraitGuide {
    fn default() -> Self {
        Self {
            trait_name: "Pack".to_string(),
            description: "A modality-specific bundle of viewer tools, image processors, and configuration.".to_string(),
            required_methods: vec![
                "modality() -> &str".to_string(),
                "activate(context: &PluginContext) -> Result<(), PluginError>".to_string(),
                "deactivate() -> Result<(), PluginError>".to_string(),
                "overlay_renderers() -> Vec<Box<dyn OverlayRenderable>>".to_string(),
            ],
            examples: vec![
                "examples/custom_pack.rs — OCT modality pack".to_string(),
            ],
        }
    }
}

/// Documentation for the `FromDataset` trait.
///
/// `FromDataset` allows a plugin to construct itself from a DICOM
/// dataset. This is used when DiCCY loads a study and needs to
/// instantiate modality-specific pack objects from the dataset metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FromDatasetTraitGuide {
    /// Trait name.
    pub trait_name: String,
    /// Human-readable description.
    pub description: String,
    /// Required methods.
    pub required_methods: Vec<String>,
    /// Example implementations.
    pub examples: Vec<String>,
}

impl Default for FromDatasetTraitGuide {
    fn default() -> Self {
        Self {
            trait_name: "FromDataset".to_string(),
            description: "Construct a plugin object from a DICOM dataset. Used for modality-specific initialisation.".to_string(),
            required_methods: vec![
                "from_dataset(dataset: &Dataset) -> Result<Self, PluginError>".to_string(),
            ],
            examples: vec![
                "examples/custom_pack.rs — OCT pack initialisation from dataset".to_string(),
            ],
        }
    }
}

/// Documentation for the `OverlayRenderable` trait.
///
/// `OverlayRenderable` defines the interface for drawing visual
/// overlays on top of DICOM images. Implementors specify the
/// viewport layer, z-order, and rendering callback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayRenderableTraitGuide {
    /// Trait name.
    pub trait_name: String,
    /// Human-readable description.
    pub description: String,
    /// Required methods.
    pub required_methods: Vec<String>,
    /// Example implementations.
    pub examples: Vec<String>,
}

impl Default for OverlayRenderableTraitGuide {
    fn default() -> Self {
        Self {
            trait_name: "OverlayRenderable".to_string(),
            description: "Visual overlay renderer for DICOM viewer. Draws annotations, measurements, heatmaps, etc.".to_string(),
            required_methods: vec![
                "layer() -> OverlayLayer".to_string(),
                "z_order() -> i32".to_string(),
                "render(context: &RenderContext) -> Result<(), PluginError>".to_string(),
                "is_visible() -> bool".to_string(),
            ],
            examples: vec![
                "examples/custom_overlay.rs — AI heatmap overlay".to_string(),
            ],
        }
    }
}

/// Documentation for the `PixelCodec` trait.
///
/// `PixelCodec` defines the interface for transfer syntax codecs
/// that decompress/compress pixel data. DiCCY provides built-in
/// codecs for JPEG-LS, JPEG 2000, RLE, and raw. Third-party
/// codecs can be added via the plugin system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PixelCodecTraitGuide {
    /// Trait name.
    pub trait_name: String,
    /// Human-readable description.
    pub description: String,
    /// Required methods.
    pub required_methods: Vec<String>,
    /// Example implementations.
    pub examples: Vec<String>,
}

impl Default for PixelCodecTraitGuide {
    fn default() -> Self {
        Self {
            trait_name: "PixelCodec".to_string(),
            description: "Transfer syntax codec for DICOM pixel data. Decompresses/compresses pixel buffers.".to_string(),
            required_methods: vec![
                "transfer_syntax_uid() -> &str".to_string(),
                "decode(input: &[u8], width: u32, height: u32) -> Result<Vec<u8>, PluginError>".to_string(),
                "encode(input: &[u8], width: u32, height: u32) -> Result<Vec<u8>, PluginError>".to_string(),
            ],
            examples: vec![
                "examples/custom_codec.rs — NOP codec example".to_string(),
            ],
        }
    }
}

// ---------------------------------------------------------------------------
// Stability policy
// ---------------------------------------------------------------------------

/// API stability guarantees.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StabilityPolicy {
    /// Semver policy description.
    pub semver_policy: String,
    /// Deprecation schedule description.
    pub deprecation_schedule: String,
    /// Migration guides for version transitions.
    pub migration_guides: Vec<MigrationGuide>,
}

impl Default for StabilityPolicy {
    fn default() -> Self {
        Self {
            semver_policy: "DiCCY follows semantic versioning (semver). \
                Patch versions (0.x.y → 0.x.z) are backward-compatible bug fixes. \
                Minor versions (0.x → 0.y) may add new APIs but will not break existing ones. \
                Major versions (0 → 1) may introduce breaking changes with migration guides."
                .to_string(),
            deprecation_schedule: "Deprecated APIs are maintained for at least two minor \
                releases before removal. Deprecation warnings are emitted at compile time. \
                The deprecation schedule is documented in CHANGELOG.md and migration guides."
                .to_string(),
            migration_guides: vec![
                MigrationGuide {
                    from_version: "0.14.0".to_string(),
                    to_version: "0.15.0".to_string(),
                    breaking_changes: vec![],
                    migration_steps: vec![
                        "No breaking changes in this release.".to_string(),
                    ],
                },
            ],
        }
    }
}

/// A migration guide for a version transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationGuide {
    /// Source version.
    pub from_version: String,
    /// Target version.
    pub to_version: String,
    /// Breaking changes in this transition.
    pub breaking_changes: Vec<BreakingChange>,
    /// Step-by-step migration instructions.
    pub migration_steps: Vec<String>,
}

/// A single breaking change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakingChange {
    /// Human-readable description of the breaking change.
    pub description: String,
    /// The affected API surface.
    pub affected_api: String,
    /// Optional workaround if immediate migration is not possible.
    pub workaround: Option<String>,
}

// ---------------------------------------------------------------------------
// Plugin manifest schema
// ---------------------------------------------------------------------------

/// Result of validating a `plugin.toml` manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManifestValidation {
    /// Whether the manifest is valid.
    pub is_valid: bool,
    /// Validation errors (non-empty if `is_valid` is false).
    pub errors: Vec<String>,
    /// Validation warnings.
    pub warnings: Vec<String>,
}

/// Error type for manifest validation.
#[derive(Debug, Clone, PartialEq)]
pub enum SchemaError {
    /// The manifest is not valid TOML.
    InvalidToml(String),
    /// A required field is missing.
    MissingField(String),
    /// A field has an invalid value.
    InvalidValue(String),
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SchemaError::InvalidToml(reason) => write!(f, "invalid TOML: {reason}"),
            SchemaError::MissingField(field) => write!(f, "missing required field: {field}"),
            SchemaError::InvalidValue(detail) => write!(f, "invalid value: {detail}"),
        }
    }
}

/// Validate a `plugin.toml` manifest string.
///
/// Checks that the manifest:
/// - Is valid TOML
/// - Contains a `[plugin]` section with `name` and `version`
/// - All declared extension points are known
/// - Permission values are within allowed ranges
pub fn validate_plugin_toml(content: &str) -> ManifestValidation {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Parse as TOML first.
    let parsed: Result<toml::Value, _> = toml::from_str(content);
    let Ok(parsed) = parsed else {
        return ManifestValidation {
            is_valid: false,
            errors: vec![format!("invalid TOML syntax")],
            warnings,
        };
    };

    // Check [plugin] section exists.
    let plugin_section = parsed.get("plugin");
    if plugin_section.is_none() {
        errors.push("missing [plugin] section".to_string());
        return ManifestValidation {
            is_valid: false,
            errors,
            warnings,
        };
    }
    let plugin = plugin_section.unwrap();

    // Check name.
    if plugin.get("name").and_then(|v| v.as_str()).is_none_or(|s| s.is_empty()) {
        errors.push("plugin.name must be a non-empty string".to_string());
    }

    // Check version.
    if plugin.get("version").and_then(|v| v.as_str()).is_none_or(|s| s.is_empty()) {
        errors.push("plugin.version must be a non-empty string".to_string());
    }

    // Check extension points.
    if let Some(permissions) = plugin.get("permissions").and_then(|v| v.get("extension_points")) {
        if let Some(eps) = permissions.as_array() {
            let known = dicom_plugin::KNOWN_EXTENSION_POINTS;
            for ep in eps {
                if let Some(name) = ep.as_str() {
                    if !known.contains(&name) {
                        errors.push(format!(
                            "unknown extension point '{name}'; valid: {}",
                            known.join(", ")
                        ));
                    }
                } else {
                    warnings.push("extension_points contains non-string value".to_string());
                }
            }
        }
    }

    // Check memory limit.
    if let Some(max_mem) = plugin
        .get("permissions")
        .and_then(|p| p.get("max_memory_mb"))
        .and_then(|v| v.as_integer())
    {
        if max_mem <= 0 {
            errors.push("max_memory_mb must be positive".to_string());
        }
        if max_mem > 4096 {
            warnings.push(format!(
                "max_memory_mb={max_mem} is very high; consider requesting only what is needed"
            ));
        }
    }

    // Check CPU time limit.
    if let Some(max_cpu) = plugin
        .get("permissions")
        .and_then(|p| p.get("max_cpu_time_ms"))
        .and_then(|v| v.as_integer())
    {
        if max_cpu <= 0 {
            errors.push("max_cpu_time_ms must be positive".to_string());
        }
    }

    // Use dicom_plugin::parse_manifest for deeper validation.
    match dicom_plugin::parse_manifest(content) {
        Ok(_) => {}
        Err(e) => {
            errors.push(format!("manifest validation: {e}"));
        }
    }

    ManifestValidation {
        is_valid: errors.is_empty(),
        errors,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_trait_guide_defaults() {
        let guide = PackTraitGuide::default();
        assert_eq!(guide.trait_name, "Pack");
        assert!(!guide.required_methods.is_empty());
    }

    #[test]
    fn from_dataset_trait_guide_defaults() {
        let guide = FromDatasetTraitGuide::default();
        assert_eq!(guide.trait_name, "FromDataset");
        assert!(!guide.required_methods.is_empty());
    }

    #[test]
    fn overlay_renderable_trait_guide_defaults() {
        let guide = OverlayRenderableTraitGuide::default();
        assert_eq!(guide.trait_name, "OverlayRenderable");
    }

    #[test]
    fn pixel_codec_trait_guide_defaults() {
        let guide = PixelCodecTraitGuide::default();
        assert_eq!(guide.trait_name, "PixelCodec");
    }

    #[test]
    fn stability_policy_defaults() {
        let policy = StabilityPolicy::default();
        assert!(!policy.semver_policy.is_empty());
        assert!(!policy.deprecation_schedule.is_empty());
        assert!(!policy.migration_guides.is_empty());
    }

    #[test]
    fn validate_valid_manifest() {
        let toml = r#"
[plugin]
name = "com.example.test"
version = "1.0.0"

[plugin.permissions]
extension_points = ["ViewerTool"]
max_memory_mb = 64
"#;
        let result = validate_plugin_toml(toml);
        assert!(result.is_valid, "errors: {:?}", result.errors);
    }

    #[test]
    fn validate_rejects_empty_name() {
        let toml = r#"
[plugin]
name = ""
version = "1.0.0"
"#;
        let result = validate_plugin_toml(toml);
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("name")));
    }

    #[test]
    fn validate_rejects_unknown_extension_point() {
        let toml = r#"
[plugin]
name = "com.example.test"
version = "1.0.0"

[plugin.permissions]
extension_points = ["UnknownThing"]
"#;
        let result = validate_plugin_toml(toml);
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("unknown extension point")));
    }

    #[test]
    fn validate_rejects_invalid_toml() {
        let toml = "this is not [[ valid toml";
        let result = validate_plugin_toml(toml);
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("TOML")));
    }

    #[test]
    fn validate_warns_on_high_memory() {
        let toml = r#"
[plugin]
name = "com.example.test"
version = "1.0.0"

[plugin.permissions]
max_memory_mb = 8192
"#;
        let result = validate_plugin_toml(toml);
        assert!(result.warnings.iter().any(|w| w.contains("very high")));
    }

    #[test]
    fn validate_rejects_missing_plugin_section() {
        let toml = "[other]\nkey = \"value\"";
        let result = validate_plugin_toml(toml);
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("plugin")));
    }

    #[test]
    fn migration_guide_serialization() {
        let guide = MigrationGuide {
            from_version: "0.14.0".to_string(),
            to_version: "0.15.0".to_string(),
            breaking_changes: vec![BreakingChange {
                description: "Removed old trait method".to_string(),
                affected_api: "OldTrait::old_method()".to_string(),
                workaround: Some("Use NewTrait::new_method() instead".to_string()),
            }],
            migration_steps: vec!["Replace old_method with new_method".to_string()],
        };
        let json = serde_json::to_string(&guide).expect("serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");
        assert_eq!(parsed["from_version"], "0.14.0");
    }
}
