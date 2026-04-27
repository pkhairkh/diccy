# S14-T4: Define pixel codec trait API in dicom-pixel

## Task ID
S14-T4

## Agent
Main implementation agent

## Summary
Implemented the `PixelCodec` trait API and `CodecRegistry` in the `dicom-pixel` crate, with 5 codec implementations and comprehensive test coverage.

## Files Created
1. **`crates/dicom-pixel/src/codec.rs`** — Core trait API:
   - `CodecCapabilities` struct (lossy, lossless, max_resolution, photometric_interpretations)
   - `PixelCodec` trait (name, version, capabilities, supported_transfer_syntaxes, decode, encode)
   - `CodecInput<'a>` / `CodecOutput` / `CodecError` types
   - `CodecRegistry` with `new()`, `register()`, `get_by_syntax()`, `get_by_syntax_mut()`, `list_codecs()`
   - `default_codec_registry()` factory function
   - Module-level documentation with "How to Add a Third-Party Codec" guide

2. **`crates/dicom-pixel/src/codec_raw.rs`** — `RawCodec` for uncompressed transfer syntaxes (Implicit VR LE, Explicit VR LE, Deflated Explicit VR LE)

3. **`crates/dicom-pixel/src/codec_rle.rs`** — `RleCodec` for RLE Lossless (1.2.840.10008.1.2.5), includes PackBits decoder

4. **`crates/dicom-pixel/src/codec_jpeg.rs`** — `JpegBaselineCodec` for JPEG Baseline (1.2.840.10008.1.2.4.50)

5. **`crates/dicom-pixel/src/codec_jpegls.rs`** — `JpegLsCodec` for JPEG-LS (feature-gated behind `codec-jpegls`)

6. **`crates/dicom-pixel/src/codec_j2k.rs`** — `J2kCodec` for JPEG 2000 (feature-gated behind `codec-j2k`)

7. **`crates/dicom-pixel/tests/codec_tests.rs`** — 27+ integration tests covering:
   - Each codec's capabilities, supported syntaxes, and basic decode/encode
   - Registry lookup by transfer syntax, including None for unknown
   - RawCodec round-trip encode/decode
   - RleCodec decode with synthetic PackBits data
   - Custom codec registration ("how to add" pattern)
   - Last-registered-wins semantics for duplicate syntaxes
   - CodecError Display, CodecRegistry Debug, Default trait
   - Feature-gated tests for JpegLsCodec and J2kCodec

## Files Modified
1. **`crates/dicom-pixel/src/lib.rs`** — Added module declarations for all 5 codec modules, re-exports of key types at crate root, updated crate-level documentation

## Test Results
- Default features: 27 codec tests + 30 inline tests + 4 golden corpus = **61 tests pass**
- `codec-jpegls` feature: 31 codec tests + 35 inline tests + 4 golden corpus = **70 tests pass**
- `codec-j2k` feature: 31 codec tests + 32 inline tests + 4 golden corpus = **67 tests pass**
- All existing `PixelPipeline` tests continue to pass (backward compatible)

## Acceptance Criteria
- ✅ `PixelCodec` trait defined with 5 implementations (Raw, RLE, JPEG Baseline, JPEG-LS, JPEG 2000)
- ✅ `CodecRegistry` resolves codecs by transfer syntax UID
- ✅ `cargo test -p dicom-pixel` passes (all feature configurations)
- ✅ Feature-gated codecs behind `codec-jpegls` and `codec-j2k`
- ✅ Documentation with "how to add a codec" guide in codec.rs module docs
- ✅ Backward compatible — existing `decode_pixels()` and `PixelPipeline` unchanged
