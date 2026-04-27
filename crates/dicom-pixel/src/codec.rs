//! Pixel codec trait API and runtime registry.
//!
//! This module defines the [`PixelCodec`] trait, which provides a unified interface
//! for encoding and decoding DICOM image frames across different transfer syntaxes.
//! It also provides a [`CodecRegistry`] for runtime codec lookup by transfer syntax UID.
//!
//! # How to Add a Third-Party Codec
//!
//! To register a custom pixel codec with the `dicom-pixel` ecosystem:
//!
//! 1. Implement the [`PixelCodec`] trait for your codec struct.
//! 2. Report the correct transfer syntax UIDs via [`PixelCodec::supported_transfer_syntaxes`].
//! 3. Advertise capabilities accurately via [`PixelCodec::capabilities`].
//! 4. Register your codec with a [`CodecRegistry`] using [`CodecRegistry::register`].
//!
//! ## Example
//!
//! ```rust,ignore
//! use dicom_pixel::codec::{PixelCodec, CodecCapabilities, CodecInput, CodecOutput, CodecError};
//!
//! struct MyCustomCodec;
//!
//! impl PixelCodec for MyCustomCodec {
//!     fn name(&self) -> &str { "my-custom-codec" }
//!     fn capabilities(&self) -> CodecCapabilities {
//!         CodecCapabilities {
//!             lossy: false,
//!             lossless: true,
//!             max_resolution: (4096, 4096),
//!             photometric_interpretations: vec!["MONOCHROME2".into(), "RGB".into()],
//!         }
//!     }
//!     fn supported_transfer_syntaxes(&self) -> Vec<&str> {
//!         vec!["1.2.840.10008.1.2.4.MY"]
//!     }
//!     fn decode(&self, input: &CodecInput) -> Result<CodecOutput, CodecError> {
//!         // ... decode logic ...
//!         Ok(CodecOutput { bytes: vec![], width: input.width, height: input.height, format: PixelFormat::Luma8 })
//!     }
//!     fn encode(&self, input: &CodecInput, output_format: &str) -> Result<Vec<u8>, CodecError> {
//!         // ... encode logic ...
//!         Err(CodecError::EncodingFailed("not implemented".into()))
//!     }
//! }
//!
//! // Register with the default registry:
//! let mut registry = dicom_pixel::codec::default_codec_registry();
//! registry.register(Box::new(MyCustomCodec));
//! ```

use std::collections::HashMap;
use std::fmt;

use crate::PixelFormat;

// ---------------------------------------------------------------------------
// CodecCapabilities
// ---------------------------------------------------------------------------

/// Capabilities advertised by a pixel codec implementation.
///
/// Callers can inspect these to determine whether a codec supports lossy or
/// lossless compression, the maximum image dimensions it can handle, and which
/// photometric interpretations it accepts.
#[derive(Debug, Clone)]
pub struct CodecCapabilities {
    /// Whether the codec supports lossy compression.
    pub lossy: bool,
    /// Whether the codec supports lossless compression.
    pub lossless: bool,
    /// Maximum supported resolution as `(width, height)`.
    pub max_resolution: (u32, u32),
    /// Photometric interpretations this codec can handle (e.g. `"MONOCHROME2"`, `"RGB"`).
    pub photometric_interpretations: Vec<String>,
}

// ---------------------------------------------------------------------------
// CodecInput / CodecOutput / CodecError
// ---------------------------------------------------------------------------

/// Input data for a pixel codec encode/decode operation.
#[derive(Debug, Clone)]
pub struct CodecInput<'a> {
    /// Raw byte buffer (compressed or uncompressed pixel data).
    pub bytes: &'a [u8],
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Bits allocated per pixel sample.
    pub bits_allocated: u16,
    /// Bits stored per pixel sample (meaningful bits).
    pub bits_stored: u16,
    /// Position of the most significant bit.
    pub high_bit: u16,
    /// Pixel representation: 0 = unsigned, 1 = signed.
    pub pixel_representation: u16,
    /// Number of samples per pixel (1 for grayscale, 3 for RGB).
    pub samples_per_pixel: u16,
    /// Photometric interpretation (e.g. `"MONOCHROME2"`, `"RGB"`).
    pub photometric_interpretation: &'a str,
    /// Transfer Syntax UID identifying the encoding.
    pub transfer_syntax_uid: &'a str,
    /// Frame index (0-based) within the pixel data.
    pub frame_index: usize,
}

/// Output of a successful pixel codec decode operation.
#[derive(Debug, Clone)]
pub struct CodecOutput {
    /// Decoded pixel bytes in the format indicated by [`CodecOutput::format`].
    pub bytes: Vec<u8>,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Pixel format of the decoded output.
    pub format: PixelFormat,
}

/// Errors that can occur during codec encode/decode operations.
#[derive(Debug, Clone)]
pub enum CodecError {
    /// The requested transfer syntax is not supported by this codec.
    UnsupportedTransferSyntax(String),
    /// Decoding the pixel data failed.
    DecodingFailed(String),
    /// Encoding the pixel data failed.
    EncodingFailed(String),
    /// The input data or parameters are invalid.
    InvalidInput(String),
    /// The requested operation exceeds the codec's advertised capabilities.
    CapabilityExceeded(String),
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodecError::UnsupportedTransferSyntax(uid) => {
                write!(f, "unsupported transfer syntax: {uid}")
            }
            CodecError::DecodingFailed(detail) => write!(f, "decoding failed: {detail}"),
            CodecError::EncodingFailed(detail) => write!(f, "encoding failed: {detail}"),
            CodecError::InvalidInput(detail) => write!(f, "invalid input: {detail}"),
            CodecError::CapabilityExceeded(detail) => {
                write!(f, "capability exceeded: {detail}")
            }
        }
    }
}

impl std::error::Error for CodecError {}

// ---------------------------------------------------------------------------
// PixelCodec trait
// ---------------------------------------------------------------------------

/// A pixel codec that can encode and decode DICOM image frames.
///
/// Implementors describe their capabilities, supported transfer syntaxes, and
/// provide `decode`/`encode` methods. All implementations must be `Send + Sync`
/// so they can be stored in a [`CodecRegistry`] and shared across threads.
///
/// See the [module-level documentation](self) for a guide on adding custom codecs.
pub trait PixelCodec: Send + Sync {
    /// Human-readable name of the codec (e.g. `"RawCodec"`, `"JpegLsCodec"`).
    fn name(&self) -> &str;

    /// Version of the codec implementation. Defaults to `"1.0.0"`.
    fn version(&self) -> &str {
        "1.0.0"
    }

    /// Advertised capabilities of this codec.
    fn capabilities(&self) -> CodecCapabilities;

    /// Transfer syntax UIDs this codec can handle.
    ///
    /// Each UID string must be a valid DICOM Transfer Syntax UID such as
    /// `"1.2.840.10008.1.2.1"` (Explicit VR Little Endian).
    fn supported_transfer_syntaxes(&self) -> Vec<&str>;

    /// Decode a single frame from raw bytes.
    ///
    /// The `input` parameter contains the compressed (or uncompressed) pixel
    /// data bytes together with frame metadata. The codec must return a
    /// [`CodecOutput`] containing the decoded pixel bytes in the format
    /// indicated by [`CodecOutput::format`].
    fn decode(&self, input: &CodecInput<'_>) -> Result<CodecOutput, CodecError>;

    /// Encode a frame to the codec's transfer syntax.
    ///
    /// The `input` contains the raw pixel data and metadata. The
    /// `output_format` parameter identifies the target transfer syntax UID.
    /// Returns the encoded byte buffer on success.
    fn encode(&self, input: &CodecInput<'_>, output_format: &str) -> Result<Vec<u8>, CodecError>;
}

// ---------------------------------------------------------------------------
// CodecRegistry
// ---------------------------------------------------------------------------

/// Runtime registry for pixel codecs.
///
/// Maintains a collection of [`PixelCodec`] implementations and provides
/// lookup by transfer syntax UID. When multiple codecs support the same
/// transfer syntax, the last one registered wins.
///
/// # Example
///
/// ```
/// use dicom_pixel::codec::{CodecRegistry, default_codec_registry};
///
/// let registry = default_codec_registry();
/// assert!(registry.get_by_syntax("1.2.840.10008.1.2.1").is_some());
/// assert!(registry.get_by_syntax("9.9.9.9.9").is_none());
/// ```
pub struct CodecRegistry {
    codecs: Vec<Box<dyn PixelCodec>>,
    /// Maps transfer syntax UID → index into `codecs`.
    syntax_index: HashMap<String, usize>,
}

impl CodecRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            codecs: Vec::new(),
            syntax_index: HashMap::new(),
        }
    }

    /// Register a codec. The codec's declared transfer syntaxes are indexed
    /// for fast lookup. If a syntax was previously registered, the new codec
    /// replaces it for that syntax.
    pub fn register(&mut self, codec: Box<dyn PixelCodec>) {
        let idx = self.codecs.len();
        let syntaxes = codec.supported_transfer_syntaxes();
        let syntaxes_owned: Vec<String> = syntaxes.iter().map(|s| s.to_string()).collect();
        self.codecs.push(codec);
        for uid in syntaxes_owned {
            self.syntax_index.insert(uid, idx);
        }
    }

    /// Look up a codec by transfer syntax UID.
    pub fn get_by_syntax(&self, uid: &str) -> Option<&dyn PixelCodec> {
        self.syntax_index
            .get(uid)
            .map(|&idx| self.codecs[idx].as_ref())
    }

    /// Look up a codec by transfer syntax UID, returning a mutable reference.
    pub fn get_by_syntax_mut(&mut self, uid: &str) -> Option<&mut dyn PixelCodec> {
        if let Some(&idx) = self.syntax_index.get(uid) {
            Some(self.codecs[idx].as_mut())
        } else {
            None
        }
    }

    /// List all registered codecs.
    pub fn list_codecs(&self) -> Vec<&dyn PixelCodec> {
        self.codecs.iter().map(|c| c.as_ref()).collect()
    }
}

impl Default for CodecRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for CodecRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CodecRegistry")
            .field("codec_count", &self.codecs.len())
            .field("syntax_index_len", &self.syntax_index.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Default registry factory
// ---------------------------------------------------------------------------

/// Construct a [`CodecRegistry`] pre-loaded with all built-in codecs.
///
/// The following codecs are always registered:
/// - [`RawCodec`](crate::codec_raw::RawCodec) (uncompressed transfer syntaxes)
/// - [`RleCodec`](crate::codec_rle::RleCodec) (RLE Lossless)
/// - [`JpegBaselineCodec`](crate::codec_jpeg::JpegBaselineCodec) (JPEG Baseline)
///
/// With the `codec-jpegls` feature:
/// - [`JpegLsCodec`](crate::codec_jpegls::JpegLsCodec) (JPEG-LS Lossless / Near-Lossless)
///
/// With the `codec-j2k` feature:
/// - [`J2kCodec`](crate::codec_j2k::J2kCodec) (JPEG 2000 Lossless / Lossy)
pub fn default_codec_registry() -> CodecRegistry {
    let mut reg = CodecRegistry::new();
    reg.register(Box::new(crate::codec_raw::RawCodec::new()));
    reg.register(Box::new(crate::codec_rle::RleCodec::new()));
    reg.register(Box::new(crate::codec_jpeg::JpegBaselineCodec::new()));
    #[cfg(feature = "codec-jpegls")]
    reg.register(Box::new(crate::codec_jpegls::JpegLsCodec::new()));
    #[cfg(feature = "codec-j2k")]
    reg.register(Box::new(crate::codec_j2k::J2kCodec::new()));
    reg
}
