//! RLE Lossless pixel codec.
//!
//! Handles the RLE Lossless transfer syntax (`1.2.840.10008.1.2.5`).

use crate::codec::{
    CodecCapabilities, CodecError, CodecInput, CodecOutput, PixelCodec,
};
use crate::PixelFormat;
use crate::TS_RLE_LOSSLESS;

/// Pixel codec for RLE Lossless transfer syntax.
///
/// Supports:
/// - RLE Lossless (`1.2.840.10008.1.2.5`)
///
/// # Limitations
///
/// The current implementation only supports single-frame pixel data.
/// Multi-frame encapsulated pixel data requires offset table support
/// which is not yet implemented.
pub struct RleCodec;

impl RleCodec {
    /// Create a new `RleCodec`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for RleCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl PixelCodec for RleCodec {
    fn name(&self) -> &str {
        "RleCodec"
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
            ],
        }
    }

    fn supported_transfer_syntaxes(&self) -> Vec<&str> {
        vec![TS_RLE_LOSSLESS]
    }

    fn decode(&self, input: &CodecInput<'_>) -> Result<CodecOutput, CodecError> {
        if input.transfer_syntax_uid != TS_RLE_LOSSLESS {
            return Err(CodecError::UnsupportedTransferSyntax(
                input.transfer_syntax_uid.to_string(),
            ));
        }

        let data = input.bytes;
        let width = input.width;
        let height = input.height;

        // Find the RLE header start.
        let start = find_rle_header_offset(data);
        let data = &data[start..];

        if data.len() < 64 {
            return Err(CodecError::DecodingFailed("RLE header too short".into()));
        }

        let segment_count =
            u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if !(1..=15).contains(&segment_count) {
            return Err(CodecError::DecodingFailed("invalid RLE segment count".into()));
        }

        let bytes_per_sample = (input.bits_allocated / 8) as usize;
        let expected_segments = input.samples_per_pixel as usize * bytes_per_sample;
        if segment_count != expected_segments {
            return Err(CodecError::DecodingFailed(
                "RLE segment count mismatch".into(),
            ));
        }

        let mut offsets = Vec::with_capacity(segment_count + 1);
        for i in 0..segment_count {
            let base = 4 + i * 4;
            if base + 4 > data.len() {
                return Err(CodecError::DecodingFailed(
                    "RLE segment offset header truncated".into(),
                ));
            }
            let offset = u32::from_le_bytes([
                data[base],
                data[base + 1],
                data[base + 2],
                data[base + 3],
            ]) as usize;
            if offset >= data.len() {
                return Err(CodecError::DecodingFailed(
                    "RLE segment offset out of range".into(),
                ));
            }
            offsets.push(offset);
        }
        offsets.push(data.len());

        let pixels_per_frame = width as usize * height as usize;
        let mut decoded_segments: Vec<Vec<u8>> = Vec::with_capacity(segment_count);
        for i in 0..segment_count {
            let seg_start = offsets[i];
            let seg_end = offsets[i + 1];
            if seg_end < seg_start {
                return Err(CodecError::DecodingFailed(
                    "RLE segment offsets not monotonic".into(),
                ));
            }
            let segment_data = &data[seg_start..seg_end];
            let decoded = decode_packbits(segment_data, pixels_per_frame)?;
            if decoded.len() != pixels_per_frame {
                return Err(CodecError::DecodingFailed(
                    "RLE segment decoded size mismatch".into(),
                ));
            }
            decoded_segments.push(decoded);
        }

        // Reassemble segments into interleaved pixel bytes.
        let spp = input.samples_per_pixel as usize;
        let bps = bytes_per_sample;
        let mut out = vec![0u8; pixels_per_frame * spp * bps];

        for sample_idx in 0..spp {
            if input.bits_allocated == 8 {
                let seg = &decoded_segments[sample_idx];
                for pixel in 0..pixels_per_frame {
                    out[pixel * spp * bps + sample_idx * bps] = seg[pixel];
                }
            } else {
                // 16-bit: low byte segment then high byte segment.
                let low_seg = &decoded_segments[sample_idx * 2];
                let high_seg = &decoded_segments[sample_idx * 2 + 1];
                for pixel in 0..pixels_per_frame {
                    let base = pixel * spp * bps + sample_idx * bps;
                    out[base] = low_seg[pixel];
                    out[base + 1] = high_seg[pixel];
                }
            }
        }

        let format = if spp == 1 {
            if input.bits_allocated <= 8 {
                PixelFormat::Luma8
            } else {
                PixelFormat::Luma16
            }
        } else {
            PixelFormat::Rgba8
        };

        Ok(CodecOutput {
            bytes: out,
            width,
            height,
            format,
        })
    }

    fn encode(&self, _input: &CodecInput<'_>, output_format: &str) -> Result<Vec<u8>, CodecError> {
        if output_format != TS_RLE_LOSSLESS {
            return Err(CodecError::UnsupportedTransferSyntax(
                output_format.to_string(),
            ));
        }
        Err(CodecError::EncodingFailed(
            "RLE Lossless encoding is not yet implemented".into(),
        ))
    }
}

/// Find the offset of the RLE header within the pixel data.
fn find_rle_header_offset(data: &[u8]) -> usize {
    let search_limit = data.len().min(256);
    let mut offset = 0usize;
    while offset + 64 <= search_limit {
        let segment_count = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        if (1..=15).contains(&segment_count) {
            let mut ok = true;
            for i in 0..segment_count {
                let base = offset + 4 + i * 4;
                if base + 4 > data.len() {
                    ok = false;
                    break;
                }
                let off = u32::from_le_bytes([
                    data[base],
                    data[base + 1],
                    data[base + 2],
                    data[base + 3],
                ]) as usize;
                if off < 64 || off >= data.len() {
                    ok = false;
                    break;
                }
            }
            if ok {
                return offset;
            }
        }
        offset += 4;
    }
    0
}

/// Decode a PackBits-encoded RLE segment.
fn decode_packbits(data: &[u8], expected: usize) -> Result<Vec<u8>, CodecError> {
    let mut out = Vec::with_capacity(expected);
    let mut i = 0;
    while i < data.len() {
        let header = data[i] as i8;
        i += 1;
        if header >= 0 {
            let count = header as usize + 1;
            if i + count > data.len() {
                return Err(CodecError::DecodingFailed(
                    "PackBits literal run exceeds input".into(),
                ));
            }
            out.extend_from_slice(&data[i..i + count]);
            i += count;
        } else if header >= -127 {
            let count = (1 - header as i16) as usize;
            if i >= data.len() {
                return Err(CodecError::DecodingFailed(
                    "PackBits repeat run missing byte".into(),
                ));
            }
            let value = data[i];
            i += 1;
            out.extend(std::iter::repeat_n(value, count));
        }
        // -128 is a no-op per PackBits spec.
    }
    Ok(out)
}
