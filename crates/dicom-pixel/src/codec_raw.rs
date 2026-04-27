//! Raw (uncompressed) pixel codec.
//!
//! Handles Implicit VR Little Endian, Explicit VR Little Endian, and
//! Deflated Explicit VR Little Endian transfer syntaxes.

use crate::codec::{
    CodecCapabilities, CodecError, CodecInput, CodecOutput, PixelCodec,
};
use crate::PixelFormat;
use crate::{TS_DEFLATED_EXPLICIT_VR_LE, TS_EXPLICIT_VR_LE, TS_IMPLICIT_VR_LE};

/// Pixel codec for uncompressed DICOM transfer syntaxes.
///
/// Supports:
/// - Implicit VR Little Endian (`1.2.840.10008.1.2`)
/// - Explicit VR Little Endian (`1.2.840.10008.1.2.1`)
/// - Deflated Explicit VR Little Endian (`1.2.840.10008.1.2.1.99`)
pub struct RawCodec;

impl RawCodec {
    /// Create a new `RawCodec`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for RawCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl PixelCodec for RawCodec {
    fn name(&self) -> &str {
        "RawCodec"
    }

    fn capabilities(&self) -> CodecCapabilities {
        CodecCapabilities {
            lossy: false,
            lossless: true,
            max_resolution: (65535, 65535),
            photometric_interpretations: vec![
                "MONOCHROME1".into(),
                "MONOCHROME2".into(),
                "RGB".into(),
                "YBR_FULL".into(),
                "YBR_FULL_422".into(),
            ],
        }
    }

    fn supported_transfer_syntaxes(&self) -> Vec<&str> {
        vec![
            TS_IMPLICIT_VR_LE,
            TS_EXPLICIT_VR_LE,
            TS_DEFLATED_EXPLICIT_VR_LE,
        ]
    }

    fn decode(&self, input: &CodecInput<'_>) -> Result<CodecOutput, CodecError> {
        // Verify this is a supported transfer syntax.
        let supported = self.supported_transfer_syntaxes();
        if !supported.contains(&input.transfer_syntax_uid) {
            return Err(CodecError::UnsupportedTransferSyntax(
                input.transfer_syntax_uid.to_string(),
            ));
        }

        let width = input.width;
        let height = input.height;
        let pixels_per_frame = width as usize * height as usize;
        let bytes_per_sample = (input.bits_allocated / 8) as usize;
        let spp = input.samples_per_pixel as usize;

        // Compute expected byte size for one frame.
        let bytes_per_frame = pixels_per_frame
            .checked_mul(spp)
            .and_then(|v| v.checked_mul(bytes_per_sample))
            .ok_or_else(|| {
                CodecError::DecodingFailed("pixel frame size overflow".into())
            })?;

        let offset = input.frame_index * bytes_per_frame;
        let end = offset + bytes_per_frame;
        if end > input.bytes.len() {
            return Err(CodecError::DecodingFailed(
                "pixel data shorter than declared dimensions".into(),
            ));
        }

        let slice = &input.bytes[offset..end];

        // Determine the output pixel format based on the input parameters.
        let format = if spp == 1 {
            if input.bits_allocated <= 8 {
                PixelFormat::Luma8
            } else {
                PixelFormat::Luma16
            }
        } else {
            PixelFormat::Rgba8
        };

        // For raw codec, the decoded bytes are the raw pixel data themselves.
        // We copy them directly — sign extension and masking are handled by the
        // pixel pipeline, not the codec layer.
        Ok(CodecOutput {
            bytes: slice.to_vec(),
            width,
            height,
            format,
        })
    }

    fn encode(&self, input: &CodecInput<'_>, output_format: &str) -> Result<Vec<u8>, CodecError> {
        let supported = self.supported_transfer_syntaxes();
        if !supported.contains(&output_format) {
            return Err(CodecError::UnsupportedTransferSyntax(
                output_format.to_string(),
            ));
        }

        // For raw/uncompressed transfer syntaxes, encoding is a passthrough:
        // the input bytes are already in the correct format.
        let width = input.width;
        let height = input.height;
        let spp = input.samples_per_pixel as usize;
        let bytes_per_sample = (input.bits_allocated / 8) as usize;
        let pixels_per_frame = width as usize * height as usize;
        let expected = pixels_per_frame * spp * bytes_per_sample;

        if input.bytes.len() < expected {
            return Err(CodecError::InvalidInput(
                "input pixel data shorter than declared dimensions".into(),
            ));
        }

        Ok(input.bytes[..expected].to_vec())
    }
}
