//! JPEG 2000 pixel codec (feature-gated).
//!
//! Handles JPEG 2000 Lossless and Lossy transfer syntaxes.
//! Only available when the `codec-j2k` feature is enabled.

use crate::codec::{
    CodecCapabilities, CodecError, CodecInput, CodecOutput, PixelCodec,
};
use crate::PixelFormat;
use crate::{TS_JPEG2000_LOSSLESS, TS_JPEG2000_LOSSY};

/// Pixel codec for JPEG 2000 transfer syntaxes.
///
/// Supports:
/// - JPEG 2000 Lossless (`1.2.840.10008.1.2.4.90`)
/// - JPEG 2000 Lossy (`1.2.840.10008.1.2.4.91`)
///
/// # Limitations
///
/// - Only single-frame pixel data is currently supported.
/// - Only 8-bit unsigned samples are currently supported.
/// - Signed pixel representation is not supported.
/// - Alpha channels are not supported.
pub struct J2kCodec;

impl J2kCodec {
    /// Create a new `J2kCodec`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for J2kCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl PixelCodec for J2kCodec {
    fn name(&self) -> &str {
        "J2kCodec"
    }

    fn capabilities(&self) -> CodecCapabilities {
        CodecCapabilities {
            lossy: true,
            lossless: true,
            max_resolution: (65535, 65535),
            photometric_interpretations: vec![
                "MONOCHROME1".into(),
                "MONOCHROME2".into(),
                "RGB".into(),
            ],
        }
    }

    fn supported_transfer_syntaxes(&self) -> Vec<&str> {
        vec![TS_JPEG2000_LOSSLESS, TS_JPEG2000_LOSSY]
    }

    fn decode(&self, input: &CodecInput<'_>) -> Result<CodecOutput, CodecError> {
        let supported = self.supported_transfer_syntaxes();
        if !supported.contains(&input.transfer_syntax_uid) {
            return Err(CodecError::UnsupportedTransferSyntax(
                input.transfer_syntax_uid.to_string(),
            ));
        }

        if input.pixel_representation == 1 {
            return Err(CodecError::DecodingFailed(
                "JPEG 2000 signed pixels unsupported".into(),
            ));
        }
        if input.bits_allocated != 8 || input.bits_stored > 8 {
            return Err(CodecError::CapabilityExceeded(
                "JPEG 2000 codec currently requires 8-bit unsigned samples".into(),
            ));
        }

        let data = input.bytes;

        let start = find_j2k_start(data)
            .ok_or_else(|| CodecError::DecodingFailed("missing JPEG 2000 codestream".into()))?;
        let slice = &data[start..];

        let bitmap = hayro_jpeg2000::decode(slice, &hayro_jpeg2000::DecodeSettings::default())
            .map_err(|_| CodecError::DecodingFailed("JPEG 2000 decode failed".into()))?;

        if bitmap.width != input.width || bitmap.height != input.height {
            return Err(CodecError::DecodingFailed(
                "JPEG 2000 dimensions do not match input".into(),
            ));
        }
        if bitmap.has_alpha {
            return Err(CodecError::DecodingFailed(
                "JPEG 2000 alpha unsupported".into(),
            ));
        }

        let expected_spp = match bitmap.color_space {
            hayro_jpeg2000::ColorSpace::Gray => 1,
            hayro_jpeg2000::ColorSpace::RGB => 3,
            _ => {
                return Err(CodecError::DecodingFailed(
                    "JPEG 2000 color space unsupported".into(),
                ))
            }
        };
        if expected_spp as u16 != input.samples_per_pixel {
            return Err(CodecError::DecodingFailed(
                "JPEG 2000 component count mismatch".into(),
            ));
        }

        let format = if input.samples_per_pixel == 1 {
            PixelFormat::Luma8
        } else {
            PixelFormat::Rgba8
        };

        Ok(CodecOutput {
            bytes: bitmap.data,
            width: input.width,
            height: input.height,
            format,
        })
    }

    fn encode(&self, _input: &CodecInput<'_>, output_format: &str) -> Result<Vec<u8>, CodecError> {
        let supported = self.supported_transfer_syntaxes();
        if !supported.contains(&output_format) {
            return Err(CodecError::UnsupportedTransferSyntax(
                output_format.to_string(),
            ));
        }
        Err(CodecError::EncodingFailed(
            "JPEG 2000 encoding is not yet implemented".into(),
        ))
    }
}

/// Find the start of the JPEG 2000 data within a buffer.
fn find_j2k_start(data: &[u8]) -> Option<usize> {
    const JP2_MAGIC: &[u8] = b"\x00\x00\x00\x0C\x6A\x50\x20\x20";
    const CODESTREAM_MAGIC: &[u8] = b"\xFF\x4F\xFF\x51";
    let search_limit = data.len().min(512);
    let mut i = 0;
    while i < search_limit {
        let tail = &data[i..search_limit];
        if tail.starts_with(JP2_MAGIC) || tail.starts_with(CODESTREAM_MAGIC) {
            return Some(i);
        }
        i += 1;
    }
    None
}
