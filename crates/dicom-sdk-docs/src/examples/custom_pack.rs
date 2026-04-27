//! Custom modality pack example.
//!
//! This example demonstrates how a third-party developer can create
//! a modality-specific pack that provides custom rendering and
//! measurement logic for a specific imaging modality.
//!
//! A "pack" in DiCCY is a bundle of configuration, overlays, and
//! measurement tools that customise the viewer for a particular
//! modality (e.g. CT, MR, MG, or a custom modality like OCT).

use dicom_plugin::{
    DiccyPlugin, ExtensionPoint, ImageProcessor, PixelFormat, PluginContext, PluginError,
    PluginMetadata, ViewerTool, ToolContext,
};

// ---------------------------------------------------------------------------
// OCT (Optical Coherence Tomography) modality pack
// ---------------------------------------------------------------------------

/// OCT-specific image processor that applies retinal layer enhancement.
pub struct OctLayerEnhancer;

impl ImageProcessor for OctLayerEnhancer {
    fn processor_id(&self) -> &str {
        "com.example.oct-pack.layer-enhancer"
    }

    fn input_format(&self) -> PixelFormat {
        PixelFormat::Luma16
    }

    fn output_format(&self) -> PixelFormat {
        PixelFormat::Luma16
    }

    fn process(
        &self,
        input: &[u8],
        _width: u32,
        _height: u32,
    ) -> Result<Vec<u8>, PluginError> {
        // In a real OCT pack, this would apply retinal layer
        // segmentation and enhancement algorithms.
        // For this example, we apply a simple contrast stretch.
        let mut output = input.to_vec();

        // Find min/max in 16-bit data
        let mut min_val = u16::MAX;
        let mut max_val = u16::MIN;
        for chunk in output.chunks_exact(2) {
            let val = u16::from_le_bytes([chunk[0], chunk[1]]);
            if val < min_val { min_val = val; }
            if val > max_val { max_val = val; }
        }

        let range = (max_val - min_val) as f64;
        if range > 0.0 {
            for chunk in output.chunks_exact_mut(2) {
                let val = u16::from_le_bytes([chunk[0], chunk[1]]);
                let stretched = ((val - min_val) as f64 / range * 65535.0) as u16;
                chunk.copy_from_slice(&stretched.to_le_bytes());
            }
        }

        Ok(output)
    }
}

/// OCT-specific viewer tool for retinal thickness measurement.
pub struct OctThicknessTool;

impl ViewerTool for OctThicknessTool {
    fn tool_id(&self) -> &str {
        "com.example.oct-pack.thickness-tool"
    }

    fn display_name(&self) -> &str {
        "Retinal Thickness"
    }

    fn icon_name(&self) -> &str {
        "ruler"
    }

    fn on_activate(&self, _context: &ToolContext) -> Result<(), PluginError> {
        // In a real implementation, this would register click/drag
        // handlers for A-scan thickness measurement.
        Ok(())
    }

    fn on_deactivate(&self) -> Result<(), PluginError> {
        Ok(())
    }
}

/// OCT pack plugin.
pub struct OctPackPlugin {
    metadata: PluginMetadata,
}

impl OctPackPlugin {
    /// Create a new OCT pack plugin.
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                description: "OCT (Optical Coherence Tomography) modality pack".to_string(),
                author: "OCT Diagnostics Inc.".to_string(),
                url: "https://example.com/diccy-oct-pack".to_string(),
                license: "Apache-2.0".to_string(),
            },
        }
    }
}

impl Default for OctPackPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl DiccyPlugin for OctPackPlugin {
    fn name(&self) -> &str {
        "com.example.diccy-oct-pack"
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
            ExtensionPoint::ImageProcessor(Box::new(OctLayerEnhancer)),
            ExtensionPoint::ViewerTool(Box::new(OctThicknessTool)),
        ]
    }

    fn metadata(&self) -> PluginMetadata {
        self.metadata.clone()
    }
}

/// Build the plugin.toml manifest content for the OCT pack.
pub fn oct_pack_manifest_toml() -> String {
    r#"[plugin]
name = "com.example.diccy-oct-pack"
version = "1.0.0"
description = "OCT (Optical Coherence Tomography) modality pack"
author = "OCT Diagnostics Inc."

[plugin.permissions]
extension_points = ["ImageProcessor", "ViewerTool"]
network_access = false
filesystem_read = false
filesystem_write = false
max_memory_mb = 128
max_cpu_time_ms = 10000

[plugin.compatibility]
min_diccy_version = "0.14.0"
"#.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oct_layer_enhancer_processes_data() {
        let enhancer = OctLayerEnhancer;
        let mut input = vec![0u8; 100 * 100 * 2]; // 100x100 Luma16
        // Set some non-zero values
        for i in 0..100 {
            let val = (i * 655) as u16;
            let offset = i * 2;
            input[offset] = val.to_le_bytes()[0];
            input[offset + 1] = val.to_le_bytes()[1];
        }
        let result = enhancer.process(&input, 100, 100);
        assert!(result.is_ok());
    }

    #[test]
    fn oct_thickness_tool_identity() {
        let tool = OctThicknessTool;
        assert_eq!(tool.tool_id(), "com.example.oct-pack.thickness-tool");
        assert_eq!(tool.display_name(), "Retinal Thickness");
    }

    #[test]
    fn oct_pack_plugin_extension_points() {
        let plugin = OctPackPlugin::new();
        let eps = plugin.extension_points();
        assert_eq!(eps.len(), 2);
    }

    #[test]
    fn oct_pack_manifest_is_valid_toml() {
        let toml_str = oct_pack_manifest_toml();
        let manifest = dicom_plugin::parse_manifest(&toml_str);
        assert!(manifest.is_ok());
        assert_eq!(manifest.unwrap().plugin.name, "com.example.diccy-oct-pack");
    }
}
