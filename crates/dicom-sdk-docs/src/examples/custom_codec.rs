//! Custom transfer syntax codec example.
//!
//! This example demonstrates how a third-party developer can implement
//! a custom transfer syntax codec using the DiCCY SDK. The codec
//! registers itself as an `ImageProcessor` extension point.

use dicom_plugin::{
    DiccyPlugin, ExtensionPoint, ImageProcessor, PixelFormat, PluginContext, PluginError,
    PluginMetadata,
};

// ---------------------------------------------------------------------------
// Custom codec: "NOP" (no-operation) codec
// ---------------------------------------------------------------------------

/// A no-op image processor that passes data through unchanged.
///
/// This demonstrates the minimum viable `ImageProcessor` implementation.
/// A real codec would perform compression/decompression here.
pub struct NopCodec;

impl ImageProcessor for NopCodec {
    fn processor_id(&self) -> &str {
        "com.example.diccy-nop-codec"
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
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, PluginError> {
        // In a real codec, this would decode the compressed pixel data.
        // For this example, we just pass the data through.
        let expected_bytes = width as usize * height as usize * 2;
        if input.len() < expected_bytes {
            return Err(PluginError::ProcessingError {
                processor_id: self.processor_id().to_string(),
                reason: format!(
                    "input too short: got {} bytes, expected {}",
                    input.len(),
                    expected_bytes
                ),
            });
        }
        Ok(input.to_vec())
    }
}

// ---------------------------------------------------------------------------
// Plugin wrapper
// ---------------------------------------------------------------------------

/// The NOP codec plugin.
pub struct NopCodecPlugin {
    metadata: PluginMetadata,
}

impl NopCodecPlugin {
    /// Create a new NOP codec plugin.
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                description: "No-operation transfer syntax codec example".to_string(),
                author: "Example Corp".to_string(),
                url: "https://example.com/diccy-nop-codec".to_string(),
                license: "MIT".to_string(),
            },
        }
    }
}

impl Default for NopCodecPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl DiccyPlugin for NopCodecPlugin {
    fn name(&self) -> &str {
        "com.example.diccy-nop-codec"
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
        vec![ExtensionPoint::ImageProcessor(Box::new(NopCodec))]
    }

    fn metadata(&self) -> PluginMetadata {
        self.metadata.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nop_codec_processes_data() {
        let codec = NopCodec;
        let input = vec![0u8; 512 * 512 * 2]; // 512x512 Luma16
        let result = codec.process(&input, 512, 512);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), input.len());
    }

    #[test]
    fn nop_codec_rejects_short_input() {
        let codec = NopCodec;
        let input = vec![0u8; 10]; // too short
        let result = codec.process(&input, 512, 512);
        assert!(result.is_err());
    }

    #[test]
    fn nop_codec_plugin_implements_trait() {
        let plugin = NopCodecPlugin::new();
        assert_eq!(plugin.name(), "com.example.diccy-nop-codec");
        assert_eq!(plugin.version(), "1.0.0");
        let eps = plugin.extension_points();
        assert_eq!(eps.len(), 1);
    }

    #[test]
    fn nop_codec_plugin_metadata() {
        let plugin = NopCodecPlugin::new();
        let meta = plugin.metadata();
        assert!(!meta.description.is_empty());
        assert!(!meta.author.is_empty());
        assert!(!meta.license.is_empty());
    }
}
