//! Custom overlay renderer example.
//!
//! This example demonstrates how a third-party developer can create
//! a custom overlay renderer using the DiCCY SDK's `ViewerTool`
//! extension point. Overlays are visual elements drawn on top of
//! the DICOM image (e.g. annotations, measurements, heatmaps).

use dicom_plugin::{
    DiccyPlugin, ExtensionPoint, PluginContext, PluginError,
    PluginMetadata, ViewerTool, ToolContext,
};

// ---------------------------------------------------------------------------
// Custom heatmap overlay tool
// ---------------------------------------------------------------------------

/// A heatmap overlay tool that displays AI inference results.
///
/// This tool activates a semi-transparent colour overlay on the
/// viewer that highlights regions of interest identified by an
/// AI inference model.
pub struct HeatmapOverlayTool {
    /// Opacity of the overlay (0.0–1.0).
    pub opacity: f32,
    /// Colour palette name.
    pub palette: String,
}

impl HeatmapOverlayTool {
    /// Create a new heatmap overlay tool with default settings.
    pub fn new() -> Self {
        Self {
            opacity: 0.4,
            palette: "inferno".to_string(),
        }
    }
}

impl Default for HeatmapOverlayTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewerTool for HeatmapOverlayTool {
    fn tool_id(&self) -> &str {
        "com.example.diccy-heatmap-overlay"
    }

    fn display_name(&self) -> &str {
        "AI Heatmap Overlay"
    }

    fn icon_name(&self) -> &str {
        "heatmap"
    }

    fn on_activate(&self, context: &ToolContext) -> Result<(), PluginError> {
        // In a real implementation, this would:
        // 1. Query the AI inference service for the current study
        // 2. Create a WebGL texture from the inference heatmap
        // 3. Register the overlay with the viewer's compositing pipeline
        let _ = context;
        Ok(())
    }

    fn on_deactivate(&self) -> Result<(), PluginError> {
        // Unregister the overlay from the compositing pipeline.
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Annotation overlay tool
// ---------------------------------------------------------------------------

/// A simple annotation overlay tool.
pub struct AnnotationOverlayTool;

impl ViewerTool for AnnotationOverlayTool {
    fn tool_id(&self) -> &str {
        "com.example.diccy-annotation-overlay"
    }

    fn display_name(&self) -> &str {
        "Annotation Overlay"
    }

    fn icon_name(&self) -> &str {
        "pencil"
    }

    fn on_activate(&self, _context: &ToolContext) -> Result<(), PluginError> {
        Ok(())
    }

    fn on_deactivate(&self) -> Result<(), PluginError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Plugin wrapper
// ---------------------------------------------------------------------------

/// The heatmap overlay plugin.
pub struct HeatmapOverlayPlugin {
    metadata: PluginMetadata,
}

impl HeatmapOverlayPlugin {
    /// Create a new heatmap overlay plugin.
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                description: "AI heatmap and annotation overlay renderer".to_string(),
                author: "AI Diagnostics Corp.".to_string(),
                url: "https://example.com/diccy-heatmap".to_string(),
                license: "MIT".to_string(),
            },
        }
    }
}

impl Default for HeatmapOverlayPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl DiccyPlugin for HeatmapOverlayPlugin {
    fn name(&self) -> &str {
        "com.example.diccy-heatmap-overlay"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn on_load(&mut self, _context: &PluginContext) -> Result<(), PluginError> {
        Ok(())
    }

    fn on_unload(&mut self) -> Result<(), PluginError> {
        Ok(())
    }

    fn extension_points(&self) -> Vec<ExtensionPoint> {
        vec![
            ExtensionPoint::ViewerTool(Box::new(HeatmapOverlayTool::new())),
            ExtensionPoint::ViewerTool(Box::new(AnnotationOverlayTool)),
        ]
    }

    fn metadata(&self) -> PluginMetadata {
        self.metadata.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heatmap_tool_default_opacity() {
        let tool = HeatmapOverlayTool::new();
        assert!((tool.opacity - 0.4).abs() < f32::EPSILON);
        assert_eq!(tool.palette, "inferno");
    }

    #[test]
    fn heatmap_tool_identity() {
        let tool = HeatmapOverlayTool::new();
        assert_eq!(tool.tool_id(), "com.example.diccy-heatmap-overlay");
        assert_eq!(tool.display_name(), "AI Heatmap Overlay");
        assert_eq!(tool.icon_name(), "heatmap");
    }

    #[test]
    fn annotation_tool_identity() {
        let tool = AnnotationOverlayTool;
        assert_eq!(tool.tool_id(), "com.example.diccy-annotation-overlay");
        assert_eq!(tool.display_name(), "Annotation Overlay");
    }

    #[test]
    fn heatmap_plugin_extension_points() {
        let plugin = HeatmapOverlayPlugin::new();
        let eps = plugin.extension_points();
        assert_eq!(eps.len(), 2);
    }

    #[test]
    fn heatmap_plugin_metadata() {
        let plugin = HeatmapOverlayPlugin::new();
        let meta = plugin.metadata();
        assert!(!meta.description.is_empty());
        assert_eq!(meta.license, "MIT");
    }
}
