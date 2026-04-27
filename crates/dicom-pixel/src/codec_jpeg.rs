//! JPEG Baseline pixel codec.
//!
//! Handles the JPEG Baseline transfer syntax (`1.2.840.10008.1.2.4.50`).

use crate::codec::{
    CodecCapabilities, CodecError, CodecInput, CodecOutput, PixelCodec,
};
use crate::PixelFormat;
use crate::TS_JPEG_BASELINE;

/// Pixel codec for JPEG Baseline (Process 1) transfer syntax.
///
/// Supports:
/// - JPEG Baseline (`1.2.840.10008.1.2.4.50`)
///
/// # Limitations
///
/// The current implementation only supports single-frame pixel data.
/// Multi-frame encapsulated pixel data requires offset table support
/// which is not yet implemented.
pub struct JpegBaselineCodec;

impl JpegBaselineCodec {
    /// Create a new `JpegBaselineCodec`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for JpegBaselineCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl PixelCodec for JpegBaselineCodec {
    fn name(&self) -> &str {
        "JpegBaselineCodec"
    }

    fn capabilities(&self) -> CodecCapabilities {
        CodecCapabilities {
            lossy: true,
            lossless: false,
            max_resolution: (65535, 65535),
            photometric_interpretations: vec![
                "MONOCHROME2".into(),
                "RGB".into(),
                "YBR_FULL_422".into(),
            ],
        }
    }

    fn supported_transfer_syntaxes(&self) -> Vec<&str> {
        vec![TS_JPEG_BASELINE]
    }

    fn decode(&self, input: &CodecInput<'_>) -> Result<CodecOutput, CodecError> {
        if input.transfer_syntax_uid != TS_JPEG_BASELINE {
            return Err(CodecError::UnsupportedTransferSyntax(
                input.transfer_syntax_uid.to_string(),
            ));
        }

        let data = input.bytes;

        // Find the JPEG SOI marker.
        let start = find_jpeg_start(data)
            .ok_or_else(|| CodecError::DecodingFailed("missing SOI marker".into()))?;
        let data = &data[start..];

        let mut decoder = jpeg_decoder::Decoder::new(data);
        let pixels = decoder
            .decode()
            .map_err(|_| CodecError::DecodingFailed("JPEG decode failed".into()))?;
        let metadata = decoder
            .info()
            .ok_or_else(|| CodecError::DecodingFailed("JPEG metadata missing".into()))?;

        if metadata.width != input.width as u16 || metadata.height != input.height as u16 {
            return Err(CodecError::DecodingFailed(
                "JPEG dimensions do not match input".into(),
            ));
        }

        let format = if metadata.pixel_format == jpeg_decoder::PixelFormat::RGB24 {
            PixelFormat::Rgba8
        } else {
            PixelFormat::Luma8
        };

        Ok(CodecOutput {
            bytes: pixels,
            width: input.width,
            height: input.height,
            format,
        })
    }

    fn encode(&self, _input: &CodecInput<'_>, output_format: &str) -> Result<Vec<u8>, CodecError> {
        if output_format != TS_JPEG_BASELINE {
            return Err(CodecError::UnsupportedTransferSyntax(
                output_format.to_string(),
            ));
        }
        Err(CodecError::EncodingFailed(
            "JPEG Baseline encoding is not yet implemented".into(),
        ))
    }
}

/// Find the start of the JPEG data (SOI marker `FF D8`).
fn find_jpeg_start(data: &[u8]) -> Option<usize> {
    data.windows(2).position(|w| w == [0xFF, 0xD8])
}
