//! JPEG-LS pixel codec (feature-gated).
//!
//! Handles JPEG-LS Lossless and Near-Lossless transfer syntaxes.
//! Only available when the `codec-jpegls` feature is enabled.

use crate::codec::{
    CodecCapabilities, CodecError, CodecInput, CodecOutput, PixelCodec,
};
use crate::PixelFormat;
use crate::{TS_JPEGLS_LOSSLESS, TS_JPEGLS_NEAR_LOSSLESS};

/// Pixel codec for JPEG-LS transfer syntaxes.
///
/// Supports:
/// - JPEG-LS Lossless (`1.2.840.10008.1.2.4.80`)
/// - JPEG-LS Near-Lossless (`1.2.840.10008.1.2.4.81`)
///
/// # Limitations
///
/// The current implementation only supports single-frame pixel data.
/// Multi-frame encapsulated pixel data requires offset table support
/// which is not yet implemented.
pub struct JpegLsCodec;

impl JpegLsCodec {
    /// Create a new `JpegLsCodec`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for JpegLsCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl PixelCodec for JpegLsCodec {
    fn name(&self) -> &str {
        "JpegLsCodec"
    }

    fn capabilities(&self) -> CodecCapabilities {
        CodecCapabilities {
            lossy: true, // Near-Lossless
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
        vec![TS_JPEGLS_LOSSLESS, TS_JPEGLS_NEAR_LOSSLESS]
    }

    fn decode(&self, input: &CodecInput<'_>) -> Result<CodecOutput, CodecError> {
        let supported = self.supported_transfer_syntaxes();
        if !supported.contains(&input.transfer_syntax_uid) {
            return Err(CodecError::UnsupportedTransferSyntax(
                input.transfer_syntax_uid.to_string(),
            ));
        }

        let data = input.bytes;

        // Try decoding directly; if that fails, try finding the SOI marker first.
        let (info, decoded) = match decode_jpegls_inner(data) {
            Ok(result) => result,
            Err(_) => {
                let start = find_jpeg_start(data)
                    .ok_or_else(|| CodecError::DecodingFailed("missing SOI marker".into()))?;
                decode_jpegls_inner(&data[start..])?
            }
        };

        if info.width != input.width || info.height != input.height {
            return Err(CodecError::DecodingFailed(
                "JPEG-LS dimensions do not match input".into(),
            ));
        }
        if info.component_count as u16 != input.samples_per_pixel {
            return Err(CodecError::DecodingFailed(
                "JPEG-LS component count mismatch".into(),
            ));
        }

        let format = if input.samples_per_pixel == 1 {
            if input.bits_allocated <= 8 {
                PixelFormat::Luma8
            } else {
                PixelFormat::Luma16
            }
        } else {
            PixelFormat::Rgba8
        };

        Ok(CodecOutput {
            bytes: decoded,
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
            "JPEG-LS encoding is not yet implemented".into(),
        ))
    }
}

/// Decode a JPEG-LS codestream, returning frame info and decoded bytes.
fn decode_jpegls_inner(data: &[u8]) -> Result<(charls::FrameInfo, Vec<u8>), CodecError> {
    let mut decoder = charls::CharLS::default();
    let info = decoder
        .get_frame_info(data)
        .map_err(|_| CodecError::DecodingFailed("JPEG-LS header decode failed".into()))?;
    let mut data_decoder = charls::CharLS::default();
    let decoded = data_decoder
        .decode(data)
        .map_err(|_| CodecError::DecodingFailed("JPEG-LS decode failed".into()))?;
    Ok((info, decoded))
}

/// Find the start of JPEG data (SOI marker `FF D8`).
fn find_jpeg_start(data: &[u8]) -> Option<usize> {
    data.windows(2).position(|w| w == [0xFF, 0xD8])
}
