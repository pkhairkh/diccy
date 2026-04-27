//! Integration tests for the pixel codec trait API.
//!
//! Covers:
//! - Codec capabilities
//! - Registry lookup by transfer syntax
//! - RawCodec round-trip encode/decode
//! - RleCodec decode
//! - JpegBaselineCodec decode
//! - Registry returns None for unknown syntax
//! - Custom codec registration ("how to add a codec" pattern)

use dicom_pixel::codec::{
    CodecCapabilities, CodecError, CodecInput, CodecOutput, CodecRegistry, PixelCodec,
    default_codec_registry,
};
use dicom_pixel::{
    JpegBaselineCodec, PixelFormat, RawCodec, RleCodec, TS_DEFLATED_EXPLICIT_VR_LE,
    TS_EXPLICIT_VR_LE, TS_IMPLICIT_VR_LE, TS_JPEG_BASELINE, TS_RLE_LOSSLESS,
};

// ---------------------------------------------------------------------------
// Helper: build a CodecInput for an 8-bit grayscale image
// ---------------------------------------------------------------------------

fn make_grayscale_8bit_input<'a>(
    bytes: &'a [u8],
    ts_uid: &'a str,
    width: u32,
    height: u32,
) -> CodecInput<'a> {
    CodecInput {
        bytes,
        width,
        height,
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        samples_per_pixel: 1,
        photometric_interpretation: "MONOCHROME2",
        transfer_syntax_uid: ts_uid,
        frame_index: 0,
    }
}

fn make_rgb_8bit_input<'a>(
    bytes: &'a [u8],
    ts_uid: &'a str,
    width: u32,
    height: u32,
) -> CodecInput<'a> {
    CodecInput {
        bytes,
        width,
        height,
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        samples_per_pixel: 3,
        photometric_interpretation: "RGB",
        transfer_syntax_uid: ts_uid,
        frame_index: 0,
    }
}

// ---------------------------------------------------------------------------
// RawCodec tests
// ---------------------------------------------------------------------------

#[test]
fn raw_codec_capabilities() {
    let codec = RawCodec::new();
    assert_eq!(codec.name(), "RawCodec");
    assert_eq!(codec.version(), "1.0.0");

    let caps = codec.capabilities();
    assert!(!caps.lossy);
    assert!(caps.lossless);
    assert_eq!(caps.max_resolution, (65535, 65535));
    assert!(caps
        .photometric_interpretations
        .contains(&"MONOCHROME2".to_string()));
    assert!(caps.photometric_interpretations.contains(&"RGB".to_string()));
}

#[test]
fn raw_codec_supported_syntaxes() {
    let codec = RawCodec::new();
    let syntaxes = codec.supported_transfer_syntaxes();
    assert!(syntaxes.contains(&TS_IMPLICIT_VR_LE));
    assert!(syntaxes.contains(&TS_EXPLICIT_VR_LE));
    assert!(syntaxes.contains(&TS_DEFLATED_EXPLICIT_VR_LE));
    assert_eq!(syntaxes.len(), 3);
}

#[test]
fn raw_codec_decode_8bit_grayscale() {
    let codec = RawCodec::new();
    let pixels: Vec<u8> = (0..64).collect(); // 8x8 grayscale
    let input = make_grayscale_8bit_input(&pixels, TS_EXPLICIT_VR_LE, 8, 8);

    let output = codec.decode(&input).unwrap();
    assert_eq!(output.width, 8);
    assert_eq!(output.height, 8);
    assert_eq!(output.format, PixelFormat::Luma8);
    assert_eq!(output.bytes.len(), 64);
    assert_eq!(output.bytes[0], 0);
    assert_eq!(output.bytes[63], 63);
}

#[test]
fn raw_codec_decode_rgb() {
    let codec = RawCodec::new();
    // 2x2 RGB = 12 bytes
    let pixels: Vec<u8> = vec![
        255, 0, 0, // R pixel (0,0)
        0, 255, 0, // G pixel (0,1)
        0, 0, 255, // B pixel (1,0)
        128, 128, 128, // gray pixel (1,1)
    ];
    let input = make_rgb_8bit_input(&pixels, TS_IMPLICIT_VR_LE, 2, 2);

    let output = codec.decode(&input).unwrap();
    assert_eq!(output.width, 2);
    assert_eq!(output.height, 2);
    assert_eq!(output.format, PixelFormat::Rgba8);
    assert_eq!(output.bytes.len(), 12);
}

#[test]
fn raw_codec_round_trip() {
    let codec = RawCodec::new();
    let original: Vec<u8> = (0..100).collect(); // 10x10 grayscale
    let input = make_grayscale_8bit_input(&original, TS_EXPLICIT_VR_LE, 10, 10);

    // Decode
    let decoded = codec.decode(&input).unwrap();
    assert_eq!(decoded.bytes, original);

    // Encode (passthrough for raw)
    let encode_input = make_grayscale_8bit_input(&original, TS_EXPLICIT_VR_LE, 10, 10);
    let encoded = codec.encode(&encode_input, TS_EXPLICIT_VR_LE).unwrap();
    assert_eq!(encoded, original);
}

#[test]
fn raw_codec_rejects_unsupported_syntax() {
    let codec = RawCodec::new();
    let pixels = vec![0u8; 64];
    let input = make_grayscale_8bit_input(&pixels, TS_JPEG_BASELINE, 8, 8);

    let result = codec.decode(&input);
    assert!(matches!(result, Err(CodecError::UnsupportedTransferSyntax(_))));
}

#[test]
fn raw_codec_decode_data_too_short() {
    let codec = RawCodec::new();
    let pixels = vec![0u8; 4]; // too short for 8x8
    let input = make_grayscale_8bit_input(&pixels, TS_EXPLICIT_VR_LE, 8, 8);

    let result = codec.decode(&input);
    assert!(matches!(result, Err(CodecError::DecodingFailed(_))));
}

// ---------------------------------------------------------------------------
// RleCodec tests
// ---------------------------------------------------------------------------

#[test]
fn rle_codec_capabilities() {
    let codec = RleCodec::new();
    assert_eq!(codec.name(), "RleCodec");

    let caps = codec.capabilities();
    assert!(!caps.lossy);
    assert!(caps.lossless);
    assert!(caps
        .photometric_interpretations
        .contains(&"MONOCHROME2".to_string()));
}

#[test]
fn rle_codec_supported_syntaxes() {
    let codec = RleCodec::new();
    let syntaxes = codec.supported_transfer_syntaxes();
    assert_eq!(syntaxes, vec![TS_RLE_LOSSLESS]);
}

#[test]
fn rle_codec_decode_8bit() {
    let codec = RleCodec::new();

    // Build a minimal RLE-encoded 4x4 8-bit grayscale image.
    // RLE header: 1 segment (1 spp * 1 byte_per_sample = 1)
    // Segment offsets: 1 entry pointing to offset 64
    let pixels_per_frame: usize = 4 * 4; // 16
    let segment_count: u32 = 1;
    let mut rle_data = Vec::new();

    // Header: segment count
    rle_data.extend_from_slice(&segment_count.to_le_bytes());
    // Offset table: 1 offset pointing to byte 64
    let data_offset: u32 = 64;
    rle_data.extend_from_slice(&data_offset.to_le_bytes());
    // Pad to offset 64
    while rle_data.len() < 64 {
        rle_data.push(0);
    }
    // Segment: PackBits encoding of 16 bytes, all value 42
    // Header byte: -127 means repeat next byte (1 - (-127)) = 128 times
    // But we only need 16 bytes. Use literal run of 15 + 1 literal
    // Or simpler: repeat run: header = -(16-1) as i8 = -15 => byte 0xF1
    let repeat_header: u8 = (-(pixels_per_frame as i32 - 1)) as u8; // -15 as u8 = 241
    rle_data.push(repeat_header);
    rle_data.push(42); // value to repeat

    let input = CodecInput {
        bytes: &rle_data,
        width: 4,
        height: 4,
        bits_allocated: 8,
        bits_stored: 8,
        high_bit: 7,
        pixel_representation: 0,
        samples_per_pixel: 1,
        photometric_interpretation: "MONOCHROME2",
        transfer_syntax_uid: TS_RLE_LOSSLESS,
        frame_index: 0,
    };

    let output = codec.decode(&input).unwrap();
    assert_eq!(output.width, 4);
    assert_eq!(output.height, 4);
    assert_eq!(output.format, PixelFormat::Luma8);
    assert_eq!(output.bytes.len(), 16);
    // All bytes should be 42
    for &b in &output.bytes {
        assert_eq!(b, 42);
    }
}

#[test]
fn rle_codec_encode_not_implemented() {
    let codec = RleCodec::new();
    let pixels = vec![0u8; 16];
    let input = make_grayscale_8bit_input(&pixels, TS_RLE_LOSSLESS, 4, 4);

    let result = codec.encode(&input, TS_RLE_LOSSLESS);
    assert!(matches!(result, Err(CodecError::EncodingFailed(_))));
}

// ---------------------------------------------------------------------------
// JpegBaselineCodec tests
// ---------------------------------------------------------------------------

#[test]
fn jpeg_baseline_codec_capabilities() {
    let codec = JpegBaselineCodec::new();
    assert_eq!(codec.name(), "JpegBaselineCodec");

    let caps = codec.capabilities();
    assert!(caps.lossy);
    assert!(!caps.lossless);
}

#[test]
fn jpeg_baseline_codec_supported_syntaxes() {
    let codec = JpegBaselineCodec::new();
    let syntaxes = codec.supported_transfer_syntaxes();
    assert_eq!(syntaxes, vec![TS_JPEG_BASELINE]);
}

#[test]
fn jpeg_baseline_codec_rejects_wrong_syntax() {
    let codec = JpegBaselineCodec::new();
    let pixels = vec![0u8; 64];
    let input = make_grayscale_8bit_input(&pixels, TS_EXPLICIT_VR_LE, 8, 8);

    let result = codec.decode(&input);
    assert!(matches!(result, Err(CodecError::UnsupportedTransferSyntax(_))));
}

#[test]
fn jpeg_baseline_codec_encode_not_implemented() {
    let codec = JpegBaselineCodec::new();
    let pixels = vec![0u8; 64];
    let input = make_grayscale_8bit_input(&pixels, TS_JPEG_BASELINE, 8, 8);

    let result = codec.encode(&input, TS_JPEG_BASELINE);
    assert!(matches!(result, Err(CodecError::EncodingFailed(_))));
}

// ---------------------------------------------------------------------------
// CodecRegistry tests
// ---------------------------------------------------------------------------

#[test]
fn registry_default_has_builtin_codecs() {
    let registry = default_codec_registry();

    // Raw transfer syntaxes
    assert!(registry.get_by_syntax(TS_IMPLICIT_VR_LE).is_some());
    assert!(registry.get_by_syntax(TS_EXPLICIT_VR_LE).is_some());
    assert!(registry.get_by_syntax(TS_DEFLATED_EXPLICIT_VR_LE).is_some());

    // RLE
    assert!(registry.get_by_syntax(TS_RLE_LOSSLESS).is_some());

    // JPEG Baseline
    assert!(registry.get_by_syntax(TS_JPEG_BASELINE).is_some());
}

#[test]
fn registry_returns_none_for_unknown_syntax() {
    let registry = default_codec_registry();
    assert!(registry.get_by_syntax("9.9.9.9.9").is_none());
    assert!(registry.get_by_syntax("").is_none());
    assert!(registry.get_by_syntax("not-a-uid").is_none());
}

#[test]
fn registry_lookup_returns_correct_codec() {
    let registry = default_codec_registry();

    let raw_codec = registry.get_by_syntax(TS_EXPLICIT_VR_LE).unwrap();
    assert_eq!(raw_codec.name(), "RawCodec");

    let rle_codec = registry.get_by_syntax(TS_RLE_LOSSLESS).unwrap();
    assert_eq!(rle_codec.name(), "RleCodec");

    let jpeg_codec = registry.get_by_syntax(TS_JPEG_BASELINE).unwrap();
    assert_eq!(jpeg_codec.name(), "JpegBaselineCodec");
}

#[test]
fn registry_list_codecs() {
    let registry = default_codec_registry();
    let codecs = registry.list_codecs();
    // At minimum: RawCodec, RleCodec, JpegBaselineCodec
    assert!(codecs.len() >= 3);

    let names: Vec<&str> = codecs.iter().map(|c| c.name()).collect();
    assert!(names.contains(&"RawCodec"));
    assert!(names.contains(&"RleCodec"));
    assert!(names.contains(&"JpegBaselineCodec"));
}

#[test]
fn registry_get_by_syntax_mut() {
    let mut registry = default_codec_registry();
    let codec = registry.get_by_syntax_mut(TS_EXPLICIT_VR_LE);
    assert!(codec.is_some());
    assert_eq!(codec.unwrap().name(), "RawCodec");
}

#[test]
fn registry_new_is_empty() {
    let registry = CodecRegistry::new();
    assert!(registry.get_by_syntax(TS_EXPLICIT_VR_LE).is_none());
    assert!(registry.list_codecs().is_empty());
}

#[test]
fn registry_register_custom_codec() {
    // "How to add a codec" pattern: implement PixelCodec and register.
    struct DummyCodec;

    impl PixelCodec for DummyCodec {
        fn name(&self) -> &str {
            "DummyCodec"
        }
        fn capabilities(&self) -> CodecCapabilities {
            CodecCapabilities {
                lossy: false,
                lossless: true,
                max_resolution: (1024, 1024),
                photometric_interpretations: vec!["MONOCHROME2".into()],
            }
        }
        fn supported_transfer_syntaxes(&self) -> Vec<&str> {
            vec!["1.2.3.4.5.6.7"]
        }
        fn decode(&self, _input: &CodecInput<'_>) -> Result<CodecOutput, CodecError> {
            Ok(CodecOutput {
                bytes: vec![0],
                width: 1,
                height: 1,
                format: PixelFormat::Luma8,
            })
        }
        fn encode(
            &self,
            _input: &CodecInput<'_>,
            _output_format: &str,
        ) -> Result<Vec<u8>, CodecError> {
            Err(CodecError::EncodingFailed("not implemented".into()))
        }
    }

    let mut registry = CodecRegistry::new();
    registry.register(Box::new(DummyCodec));

    // Look up the custom codec
    let codec = registry.get_by_syntax("1.2.3.4.5.6.7").unwrap();
    assert_eq!(codec.name(), "DummyCodec");
    assert!(codec.capabilities().lossless);
    assert!(!codec.capabilities().lossy);

    // Unknown syntax still returns None
    assert!(registry.get_by_syntax("9.9.9.9").is_none());
}

#[test]
fn registry_last_registered_wins_for_same_syntax() {
    struct CodecA;
    struct CodecB;

    impl PixelCodec for CodecA {
        fn name(&self) -> &str { "A" }
        fn capabilities(&self) -> CodecCapabilities {
            CodecCapabilities { lossy: false, lossless: true, max_resolution: (100, 100), photometric_interpretations: vec![] }
        }
        fn supported_transfer_syntaxes(&self) -> Vec<&str> { vec!["1.2.3.4"] }
        fn decode(&self, _input: &CodecInput<'_>) -> Result<CodecOutput, CodecError> {
            Ok(CodecOutput { bytes: vec![1], width: 1, height: 1, format: PixelFormat::Luma8 })
        }
        fn encode(&self, _input: &CodecInput<'_>, _output_format: &str) -> Result<Vec<u8>, CodecError> {
            Err(CodecError::EncodingFailed("not implemented".into()))
        }
    }

    impl PixelCodec for CodecB {
        fn name(&self) -> &str { "B" }
        fn capabilities(&self) -> CodecCapabilities {
            CodecCapabilities { lossy: false, lossless: true, max_resolution: (200, 200), photometric_interpretations: vec![] }
        }
        fn supported_transfer_syntaxes(&self) -> Vec<&str> { vec!["1.2.3.4"] }
        fn decode(&self, _input: &CodecInput<'_>) -> Result<CodecOutput, CodecError> {
            Ok(CodecOutput { bytes: vec![2], width: 1, height: 1, format: PixelFormat::Luma8 })
        }
        fn encode(&self, _input: &CodecInput<'_>, _output_format: &str) -> Result<Vec<u8>, CodecError> {
            Err(CodecError::EncodingFailed("not implemented".into()))
        }
    }

    let mut registry = CodecRegistry::new();
    registry.register(Box::new(CodecA));
    registry.register(Box::new(CodecB));

    // CodecB was registered last, so it should win for "1.2.3.4"
    let codec = registry.get_by_syntax("1.2.3.4").unwrap();
    assert_eq!(codec.name(), "B");
}

// ---------------------------------------------------------------------------
// CodecError Display test
// ---------------------------------------------------------------------------

#[test]
fn codec_error_display() {
    let err = CodecError::UnsupportedTransferSyntax("1.2.3".to_string());
    assert!(err.to_string().contains("1.2.3"));

    let err = CodecError::DecodingFailed("bad data".to_string());
    assert!(err.to_string().contains("bad data"));

    let err = CodecError::EncodingFailed("nope".to_string());
    assert!(err.to_string().contains("nope"));

    let err = CodecError::InvalidInput("wrong".to_string());
    assert!(err.to_string().contains("wrong"));

    let err = CodecError::CapabilityExceeded("too big".to_string());
    assert!(err.to_string().contains("too big"));
}

// ---------------------------------------------------------------------------
// CodecRegistry Debug test
// ---------------------------------------------------------------------------

#[test]
fn codec_registry_debug() {
    let registry = default_codec_registry();
    let debug_str = format!("{:?}", registry);
    assert!(debug_str.contains("codec_count"));
    assert!(debug_str.contains("syntax_index_len"));
}

// ---------------------------------------------------------------------------
// Feature-gated codec tests
// ---------------------------------------------------------------------------

#[cfg(feature = "codec-jpegls")]
mod jpegls_tests {
    use super::*;
    use dicom_pixel::JpegLsCodec;
    use dicom_pixel::{TS_JPEGLS_LOSSLESS, TS_JPEGLS_NEAR_LOSSLESS};

    #[test]
    fn jpegls_codec_capabilities() {
        let codec = JpegLsCodec::new();
        assert_eq!(codec.name(), "JpegLsCodec");

        let caps = codec.capabilities();
        assert!(caps.lossy); // Near-Lossless
        assert!(caps.lossless);
    }

    #[test]
    fn jpegls_codec_supported_syntaxes() {
        let codec = JpegLsCodec::new();
        let syntaxes = codec.supported_transfer_syntaxes();
        assert!(syntaxes.contains(&TS_JPEGLS_LOSSLESS));
        assert!(syntaxes.contains(&TS_JPEGLS_NEAR_LOSSLESS));
    }

    #[test]
    fn jpegls_codec_in_default_registry() {
        let registry = default_codec_registry();
        assert!(registry.get_by_syntax(TS_JPEGLS_LOSSLESS).is_some());
        assert!(registry.get_by_syntax(TS_JPEGLS_NEAR_LOSSLESS).is_some());
        let codec = registry.get_by_syntax(TS_JPEGLS_LOSSLESS).unwrap();
        assert_eq!(codec.name(), "JpegLsCodec");
    }

    #[test]
    fn jpegls_codec_encode_not_implemented() {
        let codec = JpegLsCodec::new();
        let pixels = vec![0u8; 16];
        let input = make_grayscale_8bit_input(&pixels, TS_JPEGLS_LOSSLESS, 4, 4);
        let result = codec.encode(&input, TS_JPEGLS_LOSSLESS);
        assert!(matches!(result, Err(CodecError::EncodingFailed(_))));
    }
}

#[cfg(feature = "codec-j2k")]
mod j2k_tests {
    use super::*;
    use dicom_pixel::J2kCodec;
    use dicom_pixel::{TS_JPEG2000_LOSSLESS, TS_JPEG2000_LOSSY};

    #[test]
    fn j2k_codec_capabilities() {
        let codec = J2kCodec::new();
        assert_eq!(codec.name(), "J2kCodec");

        let caps = codec.capabilities();
        assert!(caps.lossy);
        assert!(caps.lossless);
    }

    #[test]
    fn j2k_codec_supported_syntaxes() {
        let codec = J2kCodec::new();
        let syntaxes = codec.supported_transfer_syntaxes();
        assert!(syntaxes.contains(&TS_JPEG2000_LOSSLESS));
        assert!(syntaxes.contains(&TS_JPEG2000_LOSSY));
    }

    #[test]
    fn j2k_codec_in_default_registry() {
        let registry = default_codec_registry();
        assert!(registry.get_by_syntax(TS_JPEG2000_LOSSLESS).is_some());
        assert!(registry.get_by_syntax(TS_JPEG2000_LOSSY).is_some());
        let codec = registry.get_by_syntax(TS_JPEG2000_LOSSLESS).unwrap();
        assert_eq!(codec.name(), "J2kCodec");
    }

    #[test]
    fn j2k_codec_encode_not_implemented() {
        let codec = J2kCodec::new();
        let pixels = vec![0u8; 16];
        let input = make_grayscale_8bit_input(&pixels, TS_JPEG2000_LOSSLESS, 4, 4);
        let result = codec.encode(&input, TS_JPEG2000_LOSSLESS);
        assert!(matches!(result, Err(CodecError::EncodingFailed(_))));
    }
}

// ---------------------------------------------------------------------------
// Multi-syntax codec test with default registry
// ---------------------------------------------------------------------------

#[test]
fn default_registry_raw_codec_handles_multiple_syntaxes() {
    let registry = default_codec_registry();

    // RawCodec handles three syntaxes
    let codec_implicit = registry.get_by_syntax(TS_IMPLICIT_VR_LE).unwrap();
    let codec_explicit = registry.get_by_syntax(TS_EXPLICIT_VR_LE).unwrap();
    let codec_deflated = registry.get_by_syntax(TS_DEFLATED_EXPLICIT_VR_LE).unwrap();

    // All three should return the same codec (or at least same name)
    assert_eq!(codec_implicit.name(), "RawCodec");
    assert_eq!(codec_explicit.name(), "RawCodec");
    assert_eq!(codec_deflated.name(), "RawCodec");
}

// ---------------------------------------------------------------------------
// CodecRegistry default trait test
// ---------------------------------------------------------------------------

#[test]
fn codec_registry_default_trait() {
    let registry = CodecRegistry::default();
    assert!(registry.list_codecs().is_empty());
}
