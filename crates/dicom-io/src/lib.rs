#![deny(missing_docs)]
#![deny(clippy::cast_possible_truncation)]

//! DICOM Part 10 IO and input boundary handling.

use dicom_core::{Dataset, Element, Error, ErrorKind, Limits, Result, Value, Vr};
use std::fs;
use std::path::PathBuf;

// Re-export Tag for integration tests.
pub use dicom_core::Tag;

const PREAMBLE_LEN: usize = 128;
const DICM_PREFIX: &[u8; 4] = b"DICM";

/// DICOM Item tag.
pub const TAG_ITEM: Tag = Tag(0xFFFE, 0xE000);
/// DICOM Item Delimitation tag.
pub const TAG_ITEM_DELIM: Tag = Tag(0xFFFE, 0xE00D);
/// DICOM Sequence Delimitation tag.
pub const TAG_SEQ_DELIM: Tag = Tag(0xFFFE, 0xE0DD);

/// Transfer Syntax UID for Implicit VR Little Endian.
pub const TS_IMPLICIT_VR_LE: &str = "1.2.840.10008.1.2";
/// Transfer Syntax UID for Explicit VR Little Endian.
pub const TS_EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1";
/// Transfer Syntax UID for Deflated Explicit VR Little Endian.
pub const TS_DEFLATED_EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1.99";
/// Transfer Syntax UID for RLE Lossless.
pub const TS_RLE_LOSSLESS: &str = "1.2.840.10008.1.2.5";
/// Transfer Syntax UID for JPEG Baseline.
pub const TS_JPEG_BASELINE: &str = "1.2.840.10008.1.2.4.50";
/// Transfer Syntax UID for JPEG-LS Lossless.
pub const TS_JPEGLS_LOSSLESS: &str = "1.2.840.10008.1.2.4.80";
/// Transfer Syntax UID for JPEG-LS Near Lossless.
pub const TS_JPEGLS_NEAR_LOSSLESS: &str = "1.2.840.10008.1.2.4.81";
/// Transfer Syntax UID for JPEG 2000 Lossless.
pub const TS_JPEG2000_LOSSLESS: &str = "1.2.840.10008.1.2.4.90";
/// Transfer Syntax UID for JPEG 2000 Lossy.
pub const TS_JPEG2000_LOSSY: &str = "1.2.840.10008.1.2.4.91";
/// Transfer Syntax UID for MPEG2 MPML.
pub const TS_MPEG2_MPML: &str = "1.2.840.10008.1.2.4.100";
/// Transfer Syntax UID for MPEG2 MPML F.
pub const TS_MPEG2_MPML_F: &str = "1.2.840.10008.1.2.4.100.1";
/// Transfer Syntax UID for MPEG2 MPHL.
pub const TS_MPEG2_MPHL: &str = "1.2.840.10008.1.2.4.101";
/// Transfer Syntax UID for MPEG2 MPHL F.
pub const TS_MPEG2_MPHL_F: &str = "1.2.840.10008.1.2.4.101.1";
/// Transfer Syntax UID for H.264 HP41.
pub const TS_H264_HP41: &str = "1.2.840.10008.1.2.4.102";
/// Transfer Syntax UID for H.264 HP41 F.
pub const TS_H264_HP41_F: &str = "1.2.840.10008.1.2.4.102.1";
/// Transfer Syntax UID for H.264 BD Compatible.
pub const TS_H264_BD_COMPAT: &str = "1.2.840.10008.1.2.4.103";
/// Transfer Syntax UID for H.264 BD Compatible F.
pub const TS_H264_BD_COMPAT_F: &str = "1.2.840.10008.1.2.4.103.1";
/// Transfer Syntax UID for H.264 HP42.
pub const TS_H264_HP42: &str = "1.2.840.10008.1.2.4.104";
/// Transfer Syntax UID for H.264 HP42 F.
pub const TS_H264_HP42_F: &str = "1.2.840.10008.1.2.4.104.1";
/// Transfer Syntax UID for H.264 HP32.
pub const TS_H264_HP32: &str = "1.2.840.10008.1.2.4.105";
/// Transfer Syntax UID for H.264 HP32 F.
pub const TS_H264_HP32_F: &str = "1.2.840.10008.1.2.4.105.1";
/// Transfer Syntax UID for H.264 Stereo.
pub const TS_H264_STEREO: &str = "1.2.840.10008.1.2.4.106";
/// Transfer Syntax UID for H.264 Stereo F.
pub const TS_H264_STEREO_F: &str = "1.2.840.10008.1.2.4.106.1";
/// Transfer Syntax UID for HEVC MP51.
pub const TS_HEVC_MP51: &str = "1.2.840.10008.1.2.4.107";
/// Transfer Syntax UID for HEVC MP51 F.
pub const TS_HEVC_MP51_F: &str = "1.2.840.10008.1.2.4.107.1";
/// Transfer Syntax UID for HEVC M10P51.
pub const TS_HEVC_M10P51: &str = "1.2.840.10008.1.2.4.108";
/// Transfer Syntax UID for HEVC M10P51 F.
pub const TS_HEVC_M10P51_F: &str = "1.2.840.10008.1.2.4.108.1";

/// SOP Class UID for CT.
pub const SOP_CLASS_CT: &str = "1.2.840.10008.5.1.4.1.1.2";
/// SOP Class UID for MR.
pub const SOP_CLASS_MR: &str = "1.2.840.10008.5.1.4.1.1.4";
/// SOP Class UID for Secondary Capture.
pub const SOP_CLASS_SC: &str = "1.2.840.10008.5.1.4.1.1.7";
/// SOP Class UID for Multi-frame Secondary Capture Byte.
pub const SOP_CLASS_SC_MF_BYTE: &str = "1.2.840.10008.5.1.4.1.1.7.2";
/// SOP Class UID for Multi-frame Secondary Capture Word.
pub const SOP_CLASS_SC_MF_WORD: &str = "1.2.840.10008.5.1.4.1.1.7.3";
/// SOP Class UID for Multi-frame Secondary Capture Color.
pub const SOP_CLASS_SC_MF_COLOR: &str = "1.2.840.10008.5.1.4.1.1.7.4";
/// SOP Class UID for Enhanced CT.
pub const SOP_CLASS_ENHANCED_CT: &str = "1.2.840.10008.5.1.4.1.1.2.1";
/// SOP Class UID for Enhanced MR.
pub const SOP_CLASS_ENHANCED_MR: &str = "1.2.840.10008.5.1.4.1.1.4.1";
/// SOP Class UID for PET.
pub const SOP_CLASS_PET: &str = "1.2.840.10008.5.1.4.1.1.128";
/// SOP Class UID for CR.
pub const SOP_CLASS_CR: &str = "1.2.840.10008.5.1.4.1.1.1";
/// SOP Class UID for DX Presentation.
pub const SOP_CLASS_DX_PRESENTATION: &str = "1.2.840.10008.5.1.4.1.1.1.1";
/// SOP Class UID for Ultrasound.
pub const SOP_CLASS_US: &str = "1.2.840.10008.5.1.4.1.1.6.1";
/// SOP Class UID for Multi-frame Ultrasound.
pub const SOP_CLASS_US_MF: &str = "1.2.840.10008.5.1.4.1.1.3.1";
/// SOP Class UID for Nuclear Medicine.
pub const SOP_CLASS_NM: &str = "1.2.840.10008.5.1.4.1.1.20";
/// SOP Class UID for X-Ray Angiographic.
pub const SOP_CLASS_XA: &str = "1.2.840.10008.5.1.4.1.1.12.1";
/// SOP Class UID for X-Ray Radiofluoroscopic.
pub const SOP_CLASS_XRF: &str = "1.2.840.10008.5.1.4.1.1.12.2";
/// SOP Class UID for Segmentation.
pub const SOP_CLASS_SEG: &str = "1.2.840.10008.5.1.4.1.1.66.4";
/// SOP Class UID for Grayscale Softcopy Presentation State.
pub const SOP_CLASS_GSPS: &str = "1.2.840.10008.5.1.4.1.1.11.1";
/// SOP Class UID for RT Dose.
pub const SOP_CLASS_RT_DOSE: &str = "1.2.840.10008.5.1.4.1.1.481.2";
/// SOP Class UID for RT Structure Set.
pub const SOP_CLASS_RT_STRUCTURE: &str = "1.2.840.10008.5.1.4.1.1.481.3";
/// SOP Class UID for RT Plan.
pub const SOP_CLASS_RT_PLAN: &str = "1.2.840.10008.5.1.4.1.1.481.5";
/// SOP Class UID for Basic Text SR.
pub const SOP_CLASS_SR_BASIC_TEXT: &str = "1.2.840.10008.5.1.4.1.1.88.11";
/// SOP Class UID for Comprehensive SR.
pub const SOP_CLASS_SR_COMPREHENSIVE: &str = "1.2.840.10008.5.1.4.1.1.88.33";

const TAG_SOP_CLASS_UID: Tag = Tag(0x0008, 0x0016);
const TAG_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x0018);
const TAG_STUDY_UID: Tag = Tag(0x0020, 0x000D);
const TAG_SERIES_UID: Tag = Tag(0x0020, 0x000E);
const TAG_FRAME_OF_REFERENCE_UID: Tag = Tag(0x0020, 0x0052);
const TAG_PIXEL_DATA: Tag = Tag(0x7FE0, 0x0010);
const TAG_FLOAT_PIXEL_DATA: Tag = Tag(0x7FE0, 0x0008);
const TAG_DOUBLE_FLOAT_PIXEL_DATA: Tag = Tag(0x7FE0, 0x0009);

const TAG_SAMPLES_PER_PIXEL: Tag = Tag(0x0028, 0x0002);
const TAG_PHOTOMETRIC_INTERPRETATION: Tag = Tag(0x0028, 0x0004);
const TAG_ROWS: Tag = Tag(0x0028, 0x0010);
const TAG_COLUMNS: Tag = Tag(0x0028, 0x0011);
const TAG_PLANAR_CONFIGURATION: Tag = Tag(0x0028, 0x0006);
const TAG_BITS_ALLOCATED: Tag = Tag(0x0028, 0x0100);
const TAG_BITS_STORED: Tag = Tag(0x0028, 0x0101);
const TAG_HIGH_BIT: Tag = Tag(0x0028, 0x0102);
const TAG_PIXEL_REPRESENTATION: Tag = Tag(0x0028, 0x0103);
const TAG_IMAGE_POSITION: Tag = Tag(0x0020, 0x0032);
const TAG_IMAGE_ORIENTATION: Tag = Tag(0x0020, 0x0037);
const TAG_PIXEL_SPACING: Tag = Tag(0x0028, 0x0030);
const TAG_NUMBER_OF_FRAMES: Tag = Tag(0x0028, 0x0008);
const TAG_SHARED_FUNCTIONAL_GROUPS_SEQUENCE: Tag = Tag(0x5200, 0x9229);
const TAG_PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE: Tag = Tag(0x5200, 0x9230);
const TAG_ROI_CONTOUR_SEQUENCE: Tag = Tag(0x3006, 0x0039);
const TAG_REFERENCED_STRUCTURE_SET_SEQUENCE: Tag = Tag(0x300C, 0x0060);

/// A byte source for DICOM input.
pub trait DicomSource {
    /// Return a size hint for the input, if known.
    fn len_hint(&self) -> Option<u64> {
        None
    }

    /// Read the full input into memory.
    fn read_to_end(&mut self) -> Result<Vec<u8>>;
}

/// An in-memory byte source.
#[derive(Debug, Clone)]
pub struct BytesSource {
    bytes: Vec<u8>,
}

impl BytesSource {
    /// Create a new byte source from owned bytes.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
}

impl DicomSource for BytesSource {
    fn len_hint(&self) -> Option<u64> {
        Some(u64::try_from(self.bytes.len()).unwrap_or(u64::MAX))
    }

    fn read_to_end(&mut self) -> Result<Vec<u8>> {
        Ok(std::mem::take(&mut self.bytes))
    }
}

/// A filesystem-backed DICOM source.
#[derive(Debug, Clone)]
pub struct FileSource {
    path: PathBuf,
    allow_symlinks: bool,
}

impl FileSource {
    /// Create a new file source.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            allow_symlinks: false,
        }
    }

    /// Allow symlinks for this source (default: false).
    pub fn allow_symlinks(mut self, allow: bool) -> Self {
        self.allow_symlinks = allow;
        self
    }

    fn metadata(&self) -> Result<fs::Metadata> {
        fs::symlink_metadata(&self.path).map_err(|err| {
            Box::new(Error::from_kind(
                ErrorKind::IoError {
                    detail: err.to_string(),
                },
                "failed to read file metadata",
            ))
        })
    }

    fn validate_path(&self, metadata: &fs::Metadata) -> Result<()> {
        if metadata.file_type().is_symlink() && !self.allow_symlinks {
            return Err(Box::new(Error::from_kind(
                ErrorKind::IoError {
                    detail: "symlink inputs are not allowed".to_string(),
                },
                "symlink inputs are not allowed",
            )));
        }
        if !metadata.is_file() {
            return Err(Box::new(Error::from_kind(
                ErrorKind::IoError {
                    detail: "input path is not a file".to_string(),
                },
                "input path is not a file",
            )));
        }
        Ok(())
    }
}

impl DicomSource for FileSource {
    fn len_hint(&self) -> Option<u64> {
        if let Ok(metadata) = self.metadata() {
            return Some(metadata.len());
        }
        None
    }

    fn read_to_end(&mut self) -> Result<Vec<u8>> {
        let metadata = self.metadata()?;
        self.validate_path(&metadata)?;
        fs::read(&self.path).map_err(|err| {
            Box::new(Error::from_kind(
                ErrorKind::IoError {
                    detail: err.to_string(),
                },
                "failed to read file",
            ))
        })
    }
}

/// Parsed file meta information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMeta {
    /// Transfer syntax UID.
    pub transfer_syntax_uid: String,
    /// Media Storage SOP Class UID, if present.
    pub media_storage_sop_class_uid: Option<String>,
    /// Media Storage SOP Instance UID, if present.
    pub media_storage_sop_instance_uid: Option<String>,
}

/// Reader configuration for explicit raw-mode handling and diagnostics capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReaderConfig {
    /// Enable raw-dataset fallback when P10 meta parsing fails.
    pub raw_mode: bool,
    /// Capture parser warnings.
    pub capture_warnings: bool,
    /// Capture top-level element offset metadata.
    pub capture_debug_offsets: bool,
}

/// Backward-compatible alias for [`ReaderConfig`].
#[deprecated(since = "0.14.0", note = "Use ReaderConfig instead")]
pub type ReaderOptions = ReaderConfig;

impl Default for ReaderConfig {
    fn default() -> Self {
        Self {
            raw_mode: false,
            capture_warnings: true,
            capture_debug_offsets: false,
        }
    }
}

/// Structured parser warning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserWarning {
    /// Stable warning code.
    pub code: &'static str,
    /// Non-PHI warning detail.
    pub detail: String,
}

/// Structured debug metadata for one parsed element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementDebugMeta {
    /// Element tag.
    pub tag: Tag,
    /// Element byte offset relative to parsed dataset start.
    pub offset: usize,
    /// Element value length in bytes.
    pub value_len: usize,
}

/// Dataset parse output with warnings and debug metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct DatasetDiagnostics {
    /// Parsed dataset.
    pub dataset: Dataset,
    /// Structured parser warnings.
    pub warnings: Vec<ParserWarning>,
    /// Structured offset metadata.
    pub debug_meta: Vec<ElementDebugMeta>,
    /// Transfer syntax UID used for parsing.
    pub transfer_syntax_uid: String,
    /// True when raw-mode fallback was used.
    pub raw_mode_used: bool,
}

/// A Part 10 reader that enforces input limits.
pub struct P10Reader<S: DicomSource> {
    source: S,
    limits: Limits,
    options: ReaderConfig,
    warnings: Vec<ParserWarning>,
}

impl<S: DicomSource> P10Reader<S> {
    /// Create a new reader with default limits.
    pub fn new(source: S) -> Self {
        Self {
            source,
            limits: Limits::default(),
            options: ReaderConfig::default(),
            warnings: Vec::new(),
        }
    }

    /// Create a new reader with explicit limits.
    pub fn with_limits(source: S, limits: Limits) -> Self {
        Self {
            source,
            limits,
            options: ReaderConfig::default(),
            warnings: Vec::new(),
        }
    }

    /// Create a new reader with explicit limits and options.
    pub fn with_limits_and_options(source: S, limits: Limits, options: ReaderConfig) -> Self {
        Self {
            source,
            limits,
            options,
            warnings: Vec::new(),
        }
    }

    /// Return the active limits.
    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    /// Return active reader options.
    pub fn options(&self) -> &ReaderConfig {
        &self.options
    }

    /// Return warnings from the most recent parse operation.
    pub fn warnings(&self) -> &[ParserWarning] {
        &self.warnings
    }

    /// Borrow the underlying source.
    pub fn source(&self) -> &S {
        &self.source
    }

    /// Mutably borrow the underlying source.
    pub fn source_mut(&mut self) -> &mut S {
        &mut self.source
    }

    /// Consume the reader and return the source.
    pub fn into_source(self) -> S {
        self.source
    }

    /// Read and parse the file meta information.
    pub fn read_meta(&mut self) -> Result<FileMeta> {
        self.enforce_len_hint()?;
        let data = self.source.read_to_end()?;
        self.enforce_input_limit(
            u64::try_from(data.len())
                .map_err(|_| decode_error("p10", "input length exceeds u64"))?,
        )?;
        let (meta, _, _, warnings) = parse_meta_with_raw_mode(&data, &self.limits, &self.options)?;
        self.warnings = warnings;
        Ok(meta)
    }

    /// Read and parse the dataset.
    pub fn read_dataset(&mut self) -> Result<Dataset> {
        self.read_dataset_with_diagnostics()
            .map(|value| value.dataset)
    }

    /// Read and parse the dataset with structured warnings and debug metadata.
    pub fn read_dataset_with_diagnostics(&mut self) -> Result<DatasetDiagnostics> {
        self.enforce_len_hint()?;
        let data = self.source.read_to_end()?;
        self.enforce_input_limit(
            u64::try_from(data.len())
                .map_err(|_| decode_error("p10", "input length exceeds u64"))?,
        )?;
        let (meta, offset, raw_mode_used, mut warnings) =
            parse_meta_with_raw_mode(&data, &self.limits, &self.options)?;
        let transfer_syntax =
            transfer_syntax_from_uid(&meta.transfer_syntax_uid).or_else(|_| {
                if raw_mode_used {
                    Ok(TransferSyntax::ExplicitVrLittleEndian)
                } else {
                    Err(Box::new(Error::from_kind(
                        ErrorKind::UnsupportedTransferSyntax {
                            transfer_syntax_uid: meta.transfer_syntax_uid.clone(),
                        },
                        "unsupported transfer syntax",
                    )))
                }
            })?;
        let dataset = parse_dataset(&data, offset, &self.limits, transfer_syntax)?;
        if !raw_mode_used {
            validate_envelope(&meta, &dataset, &self.limits)?;
        }
        warnings.extend(charset_warnings(&dataset));
        warnings.sort_by(|lhs, rhs| lhs.code.cmp(rhs.code).then(lhs.detail.cmp(&rhs.detail)));
        let debug_meta = if self.options.capture_debug_offsets {
            capture_debug_offsets(&data, offset)
        } else {
            Vec::new()
        };
        self.warnings = warnings.clone();
        Ok(DatasetDiagnostics {
            dataset,
            warnings,
            debug_meta,
            transfer_syntax_uid: meta.transfer_syntax_uid,
            raw_mode_used,
        })
    }

    fn enforce_len_hint(&self) -> Result<()> {
        if let Some(len) = self.source.len_hint() {
            self.enforce_input_limit(len)?;
        }
        Ok(())
    }

    fn enforce_input_limit(&self, len: u64) -> Result<()> {
        if len > self.limits.max_input_bytes() {
            return Err(limit_exceeded(
                "max_input_bytes",
                len,
                self.limits.max_input_bytes(),
            ));
        }
        Ok(())
    }
}

/// Parse a dataset from raw DICOM bytes using an explicit transfer syntax UID.
pub fn parse_dataset_bytes(
    data: &[u8],
    transfer_syntax_uid: &str,
    limits: &Limits,
) -> Result<Dataset> {
    let data_len_u64 = u64::try_from(data.len())
        .map_err(|_| decode_error("dataset", "input length exceeds u64"))?;
    if data_len_u64 > limits.max_input_bytes() {
        return Err(limit_exceeded(
            "max_input_bytes",
            data_len_u64,
            limits.max_input_bytes(),
        ));
    }
    let transfer_syntax = transfer_syntax_from_uid(transfer_syntax_uid)?;
    parse_dataset(data, 0, limits, transfer_syntax)
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
enum TransferSyntax {
    ExplicitVrLittleEndian,
    ImplicitVrLittleEndian,
    DeflatedExplicitVrLittleEndian,
}

fn transfer_syntax_from_uid(uid: &str) -> Result<TransferSyntax> {
    match uid {
        TS_IMPLICIT_VR_LE => Ok(TransferSyntax::ImplicitVrLittleEndian),
        TS_EXPLICIT_VR_LE => Ok(TransferSyntax::ExplicitVrLittleEndian),
        TS_DEFLATED_EXPLICIT_VR_LE if cfg!(feature = "tier1-deflate") => {
            Ok(TransferSyntax::DeflatedExplicitVrLittleEndian)
        }
        TS_RLE_LOSSLESS | TS_JPEG_BASELINE => Ok(TransferSyntax::ExplicitVrLittleEndian),
        TS_JPEGLS_LOSSLESS | TS_JPEGLS_NEAR_LOSSLESS if cfg!(feature = "codec-jpegls") => {
            Ok(TransferSyntax::ExplicitVrLittleEndian)
        }
        TS_JPEG2000_LOSSLESS | TS_JPEG2000_LOSSY if cfg!(feature = "codec-j2k") => {
            Ok(TransferSyntax::ExplicitVrLittleEndian)
        }
        TS_MPEG2_MPML | TS_MPEG2_MPML_F | TS_MPEG2_MPHL | TS_MPEG2_MPHL_F
            if cfg!(feature = "codec-mpeg2") =>
        {
            Ok(TransferSyntax::ExplicitVrLittleEndian)
        }
        TS_H264_HP41 | TS_H264_HP41_F | TS_H264_BD_COMPAT | TS_H264_BD_COMPAT_F | TS_H264_HP42
        | TS_H264_HP42_F | TS_H264_HP32 | TS_H264_HP32_F | TS_H264_STEREO | TS_H264_STEREO_F
            if cfg!(feature = "codec-h264") =>
        {
            Ok(TransferSyntax::ExplicitVrLittleEndian)
        }
        TS_HEVC_MP51 | TS_HEVC_MP51_F | TS_HEVC_M10P51 | TS_HEVC_M10P51_F
            if cfg!(feature = "codec-hevc") =>
        {
            Ok(TransferSyntax::ExplicitVrLittleEndian)
        }
        _ => Err(Box::new(Error::from_kind(
            ErrorKind::UnsupportedTransferSyntax {
                transfer_syntax_uid: uid.to_string(),
            },
            "unsupported transfer syntax",
        ))),
    }
}

fn parse_meta_with_raw_mode(
    data: &[u8],
    limits: &Limits,
    options: &ReaderConfig,
) -> Result<(FileMeta, usize, bool, Vec<ParserWarning>)> {
    match parse_p10_meta(data, limits) {
        Ok((meta, offset)) => Ok((meta, offset, false, Vec::new())),
        Err(err) => {
            if !options.raw_mode {
                return Err(err);
            }
            let fallback = FileMeta {
                transfer_syntax_uid: TS_EXPLICIT_VR_LE.to_string(),
                media_storage_sop_class_uid: None,
                media_storage_sop_instance_uid: None,
            };
            let mut warnings = Vec::new();
            if options.capture_warnings {
                warnings.push(ParserWarning {
                    code: "DVF.IO.RAW_MODE_META_FALLBACK",
                    detail:
                        "P10 meta parse failed; parsing dataset as raw explicit VR little-endian"
                            .to_string(),
                });
            }
            Ok((fallback, 0, true, warnings))
        }
    }
}

fn charset_warnings(dataset: &Dataset) -> Vec<ParserWarning> {
    let mut warnings = Vec::new();
    let Some(element) = dataset.get(Tag(0x0008, 0x0005)) else {
        return warnings;
    };
    let Value::Str(value) = element.value() else {
        warnings.push(ParserWarning {
            code: "DVF.IO.CHARSET_INVALID",
            detail: "Specific Character Set has non-string value".to_string(),
        });
        return warnings;
    };
    let charset = value.trim();
    // Baseline support remains conservative; all other charsets are accepted with warning.
    if !charset.is_empty() && charset != "ISO_IR 6" && charset != "ISO_IR 192" {
        warnings.push(ParserWarning {
            code: "DVF.IO.CHARSET_UNSUPPORTED",
            detail: format!("unsupported Specific Character Set '{charset}', using replacement"),
        });
    }
    warnings
}

fn capture_debug_offsets(data: &[u8], offset: usize) -> Vec<ElementDebugMeta> {
    if offset >= data.len() {
        return Vec::new();
    }
    let mut cursor = offset;
    let mut out = Vec::new();
    while cursor + 8 <= data.len() {
        let group = u16::from_le_bytes([data[cursor], data[cursor + 1]]);
        let element = u16::from_le_bytes([data[cursor + 2], data[cursor + 3]]);
        let vr = [data[cursor + 4], data[cursor + 5]];
        let (header_len, value_len) = match &vr {
            b"OB" | b"OW" | b"SQ" | b"UN" | b"UT" => {
                if cursor + 12 > data.len() {
                    break;
                }
                let len_u32 = u32::from_le_bytes([
                    data[cursor + 8],
                    data[cursor + 9],
                    data[cursor + 10],
                    data[cursor + 11],
                ]);
                let len = match usize::try_from(len_u32) {
                    Ok(l) => l,
                    Err(_) => break,
                };
                (12usize, len)
            }
            _ => {
                let len = u16::from_le_bytes([data[cursor + 6], data[cursor + 7]]);
                (8usize, usize::from(len))
            }
        };
        if cursor
            .checked_add(header_len)
            .and_then(|value| value.checked_add(value_len))
            .is_none()
        {
            break;
        }
        out.push(ElementDebugMeta {
            tag: Tag(group, element),
            offset: cursor - offset,
            value_len,
        });
        let next = cursor + header_len + value_len;
        if next <= cursor || next > data.len() {
            break;
        }
        cursor = next;
    }
    out
}

fn parse_p10_meta(data: &[u8], limits: &Limits) -> Result<(FileMeta, usize)> {
    if data.len() < PREAMBLE_LEN + DICM_PREFIX.len() {
        return Err(decode_error("p10", "input too short for P10 preamble"));
    }
    let prefix = &data[PREAMBLE_LEN..PREAMBLE_LEN + DICM_PREFIX.len()];
    if prefix != DICM_PREFIX {
        return Err(decode_error("p10", "missing DICM prefix"));
    }

    let mut parser = Parser::new(data, limits, PREAMBLE_LEN + DICM_PREFIX.len());
    let mut transfer_syntax_uid: Option<String> = None;
    let mut sop_class_uid: Option<String> = None;
    let mut sop_instance_uid: Option<String> = None;

    loop {
        if parser.is_eof() {
            break;
        }
        let element_start = parser.offset();
        let tag = parser.read_tag()?;
        if tag.0 != 0x0002 {
            parser.set_offset(element_start);
            break;
        }

        let (vr, length) = parser.read_explicit_vr_and_len()?;
        parser.enforce_element_length(length)?;
        let value_len = usize::try_from(length)
            .map_err(|_| decode_error("p10", "element value length exceeds usize"))?;
        let value_bytes = parser.read_bytes(value_len)?;

        match tag {
            Tag(0x0002, 0x0010) => {
                let uid = parse_uid(value_bytes, limits)?;
                transfer_syntax_uid = Some(uid);
            }
            Tag(0x0002, 0x0002) => {
                let uid = parse_uid(value_bytes, limits)?;
                sop_class_uid = Some(uid);
            }
            Tag(0x0002, 0x0003) => {
                let uid = parse_uid(value_bytes, limits)?;
                sop_instance_uid = Some(uid);
            }
            _ => {
                let _ = vr;
            }
        }
    }

    let transfer_syntax_uid = transfer_syntax_uid.ok_or_else(|| {
        Box::new(Error::from_kind(
            ErrorKind::MissingRequiredTag {
                tag: Tag(0x0002, 0x0010),
            },
            "missing transfer syntax UID",
        ))
    })?;

    Ok((
        FileMeta {
            transfer_syntax_uid,
            media_storage_sop_class_uid: sop_class_uid,
            media_storage_sop_instance_uid: sop_instance_uid,
        },
        parser.offset(),
    ))
}

fn parse_dataset(
    data: &[u8],
    offset: usize,
    limits: &Limits,
    transfer_syntax: TransferSyntax,
) -> Result<Dataset> {
    match transfer_syntax {
        TransferSyntax::DeflatedExplicitVrLittleEndian => {
            #[cfg(feature = "tier1-deflate")]
            {
                let inflated = inflate_dataset(data, offset, limits)?;
                let mut parser = Parser::new(&inflated, limits, 0);
                parse_dataset_internal(&mut parser, None, TransferSyntax::ExplicitVrLittleEndian, 0)
            }
            #[cfg(not(feature = "tier1-deflate"))]
            {
                Err(decode_error("deflate", "deflated transfer syntax disabled"))
            }
        }
        _ => {
            let mut parser = Parser::new(data, limits, offset);
            parse_dataset_internal(&mut parser, None, transfer_syntax, 0)
        }
    }
}

#[cfg(feature = "tier1-deflate")]
fn inflate_dataset(data: &[u8], offset: usize, limits: &Limits) -> Result<Vec<u8>> {
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    if offset > data.len() {
        return Err(decode_error("deflate", "offset beyond input"));
    }
    let mut decoder = ZlibDecoder::new(&data[offset..]);
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    let mut total: u64 = 0;
    loop {
        let read = decoder
            .read(&mut buf)
            .map_err(|_| decode_error("deflate", "inflate failed"))?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
        if total > limits.max_decompressed_bytes() {
            return Err(limit_exceeded(
                "max_decompressed_bytes",
                total,
                limits.max_decompressed_bytes(),
            ));
        }
        out.extend_from_slice(&buf[..read]);
    }
    Ok(out)
}

fn validate_envelope(meta: &FileMeta, dataset: &Dataset, limits: &Limits) -> Result<()> {
    let _ = transfer_syntax_from_uid(&meta.transfer_syntax_uid)?;
    reject_float_pixel_data(dataset)?;
    let sop_class_uid = read_uid(dataset, TAG_SOP_CLASS_UID, limits)?
        .ok_or_else(|| missing_required_tag(TAG_SOP_CLASS_UID))?;
    validate_sop_class(&sop_class_uid)?;
    validate_required_tags(dataset, limits, &sop_class_uid)?;
    Ok(())
}

fn reject_float_pixel_data(dataset: &Dataset) -> Result<()> {
    if dataset.get(TAG_FLOAT_PIXEL_DATA).is_some() {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidTagValue {
                tag: TAG_FLOAT_PIXEL_DATA,
                detail: "float pixel data is out-of-envelope".to_string(),
            },
            "float pixel data is out-of-envelope",
        )));
    }
    if dataset.get(TAG_DOUBLE_FLOAT_PIXEL_DATA).is_some() {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidTagValue {
                tag: TAG_DOUBLE_FLOAT_PIXEL_DATA,
                detail: "double float pixel data is out-of-envelope".to_string(),
            },
            "double float pixel data is out-of-envelope",
        )));
    }
    Ok(())
}

fn validate_sop_class(sop_class_uid: &str) -> Result<()> {
    if is_supported_sop_class(sop_class_uid) {
        return Ok(());
    }
    Err(Box::new(Error::from_kind(
        ErrorKind::UnsupportedSopClass {
            sop_class_uid: sop_class_uid.to_string(),
        },
        "unsupported SOP Class",
    )))
}

fn is_supported_sop_class(sop_class_uid: &str) -> bool {
    if matches!(
        sop_class_uid,
        SOP_CLASS_CT
            | SOP_CLASS_MR
            | SOP_CLASS_SC
            | SOP_CLASS_SC_MF_BYTE
            | SOP_CLASS_SC_MF_WORD
            | SOP_CLASS_SC_MF_COLOR
    ) {
        return true;
    }

    let caps = dicom_core::capabilities();
    if caps.pack_enhanced()
        && matches!(sop_class_uid, SOP_CLASS_ENHANCED_CT | SOP_CLASS_ENHANCED_MR)
    {
        return true;
    }
    if caps.pack_us() && matches!(sop_class_uid, SOP_CLASS_US | SOP_CLASS_US_MF) {
        return true;
    }
    if caps.pack_nm() && sop_class_uid == SOP_CLASS_NM {
        return true;
    }
    if caps.pack_xa() && matches!(sop_class_uid, SOP_CLASS_XA | SOP_CLASS_XRF) {
        return true;
    }
    if caps.modality_pet() && sop_class_uid == SOP_CLASS_PET {
        return true;
    }
    if caps.modality_xr() && matches!(sop_class_uid, SOP_CLASS_CR | SOP_CLASS_DX_PRESENTATION) {
        return true;
    }
    if caps.pack_seg() && sop_class_uid == SOP_CLASS_SEG {
        return true;
    }
    if caps.pack_rt()
        && matches!(
            sop_class_uid,
            SOP_CLASS_RT_DOSE | SOP_CLASS_RT_STRUCTURE | SOP_CLASS_RT_PLAN
        )
    {
        return true;
    }
    if caps.pack_sr()
        && matches!(
            sop_class_uid,
            SOP_CLASS_SR_BASIC_TEXT | SOP_CLASS_SR_COMPREHENSIVE
        )
    {
        return true;
    }
    if caps.gsps() && sop_class_uid == SOP_CLASS_GSPS {
        return true;
    }

    false
}

fn validate_required_tags(dataset: &Dataset, limits: &Limits, sop_class_uid: &str) -> Result<()> {
    require_tag(dataset, TAG_SOP_CLASS_UID)?;
    require_tag(dataset, TAG_SOP_INSTANCE_UID)?;
    require_tag(dataset, TAG_STUDY_UID)?;
    require_tag(dataset, TAG_SERIES_UID)?;

    if matches!(
        sop_class_uid,
        SOP_CLASS_GSPS | SOP_CLASS_SR_BASIC_TEXT | SOP_CLASS_SR_COMPREHENSIVE
    ) {
        return Ok(());
    }
    if sop_class_uid == SOP_CLASS_RT_STRUCTURE {
        require_tag(dataset, TAG_FRAME_OF_REFERENCE_UID)?;
        require_tag(dataset, TAG_ROI_CONTOUR_SEQUENCE)?;
        return Ok(());
    }
    if sop_class_uid == SOP_CLASS_RT_PLAN {
        require_tag(dataset, TAG_FRAME_OF_REFERENCE_UID)?;
        require_tag(dataset, TAG_REFERENCED_STRUCTURE_SET_SEQUENCE)?;
        return Ok(());
    }

    require_tag(dataset, TAG_PIXEL_DATA)?;

    require_tag(dataset, TAG_SAMPLES_PER_PIXEL)?;
    require_tag(dataset, TAG_PHOTOMETRIC_INTERPRETATION)?;
    require_tag(dataset, TAG_ROWS)?;
    require_tag(dataset, TAG_COLUMNS)?;
    require_tag(dataset, TAG_BITS_ALLOCATED)?;
    require_tag(dataset, TAG_BITS_STORED)?;
    require_tag(dataset, TAG_HIGH_BIT)?;

    let photometric = read_string(dataset, TAG_PHOTOMETRIC_INTERPRETATION, limits)?
        .ok_or_else(|| missing_required_tag(TAG_PHOTOMETRIC_INTERPRETATION))?;
    if matches!(photometric.as_str(), "MONOCHROME1" | "MONOCHROME2") {
        require_tag(dataset, TAG_PIXEL_REPRESENTATION)?;
    }

    let samples_per_pixel = read_u16(dataset, TAG_SAMPLES_PER_PIXEL)?
        .ok_or_else(|| missing_required_tag(TAG_SAMPLES_PER_PIXEL))?;
    if samples_per_pixel > 1 {
        require_tag(dataset, TAG_PLANAR_CONFIGURATION)?;
    }

    if sop_class_uid == SOP_CLASS_CT {
        require_tag(dataset, TAG_IMAGE_POSITION)?;
        require_tag(dataset, TAG_IMAGE_ORIENTATION)?;
        require_tag(dataset, TAG_PIXEL_SPACING)?;
    }
    if sop_class_uid == SOP_CLASS_SEG {
        require_tag(dataset, TAG_FRAME_OF_REFERENCE_UID)?;
    }
    if sop_class_uid == SOP_CLASS_RT_DOSE {
        require_tag(dataset, TAG_FRAME_OF_REFERENCE_UID)?;
        require_tag(dataset, TAG_PIXEL_SPACING)?;
        require_tag(dataset, TAG_IMAGE_POSITION)?;
        require_tag(dataset, TAG_IMAGE_ORIENTATION)?;
        require_tag(dataset, TAG_NUMBER_OF_FRAMES)?;
    }
    if matches!(sop_class_uid, SOP_CLASS_ENHANCED_CT | SOP_CLASS_ENHANCED_MR) {
        require_tag(dataset, TAG_NUMBER_OF_FRAMES)?;
        require_tag(dataset, TAG_SHARED_FUNCTIONAL_GROUPS_SEQUENCE)?;
        require_tag(dataset, TAG_PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE)?;
    }

    Ok(())
}

fn require_tag(dataset: &Dataset, tag: Tag) -> Result<&Element> {
    dataset.get(tag).ok_or_else(|| missing_required_tag(tag))
}

fn missing_required_tag(tag: Tag) -> Box<Error> {
    dicom_util::missing_required_tag(tag)
}

fn read_uid(dataset: &Dataset, tag: Tag, limits: &Limits) -> Result<Option<String>> {
    match dataset.get(tag) {
        None => Ok(None),
        Some(element) => match element.value() {
            Value::Uid(value) => Ok(Some(value.clone())),
            Value::Str(value) => Ok(Some(value.clone())),
            Value::Bytes(bytes) => Ok(Some(parse_uid(bytes, limits)?)),
            _ => Err(invalid_tag_value(tag, "expected UID value")),
        },
    }
}

fn read_string(dataset: &Dataset, tag: Tag, limits: &Limits) -> Result<Option<String>> {
    match dataset.get(tag) {
        None => Ok(None),
        Some(element) => match element.value() {
            Value::Str(value) => Ok(Some(value.clone())),
            Value::Uid(value) => Ok(Some(value.clone())),
            Value::Bytes(bytes) => Ok(Some(parse_string(bytes, limits)?)),
            _ => Err(invalid_tag_value(tag, "expected string value")),
        },
    }
}

fn read_u16(dataset: &Dataset, tag: Tag) -> Result<Option<u16>> {
    match dataset.get(tag) {
        None => Ok(None),
        Some(element) => match element.value() {
            Value::I32(value) if *value >= 0 && *value <= i32::from(u16::MAX) => {
                Ok(Some(u16::try_from(*value).map_err(|_| {
                    invalid_tag_value(tag, "i32 value out of u16 range")
                })?))
            }
            Value::Bytes(bytes) => Ok(Some(parse_u16_le(bytes, tag)?)),
            _ => Err(invalid_tag_value(tag, "expected u16 value")),
        },
    }
}

fn parse_u16_le(bytes: &[u8], tag: Tag) -> Result<u16> {
    if bytes.len() < 2 {
        return Err(invalid_tag_value(tag, "u16 value must be at least 2 bytes"));
    }
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn parse_dataset_internal(
    parser: &mut Parser<'_>,
    end_offset: Option<usize>,
    transfer_syntax: TransferSyntax,
    depth: u64,
) -> Result<Dataset> {
    if depth > parser.limits.max_sequence_depth() {
        return Err(limit_exceeded(
            "max_sequence_depth",
            depth,
            parser.limits.max_sequence_depth(),
        ));
    }

    let mut dataset = Dataset::new();

    loop {
        if let Some(end) = end_offset {
            if parser.offset() >= end {
                break;
            }
        }
        if parser.is_eof() {
            break;
        }

        let tag = parser.read_tag()?;

        if tag == TAG_ITEM_DELIM || tag == TAG_SEQ_DELIM {
            let _ = parser.read_u32()?;
            break;
        }

        parser.increment_element_count()?;

        let (vr, length) = match transfer_syntax {
            TransferSyntax::ExplicitVrLittleEndian
            | TransferSyntax::DeflatedExplicitVrLittleEndian => {
                parser.read_explicit_vr_and_len()?
            }
            TransferSyntax::ImplicitVrLittleEndian => {
                let length = parser.read_u32()?;
                (Vr::Un, length)
            }
        };

        if length == u32::MAX {
            if tag == Tag(0x7FE0, 0x0010) {
                let bytes = parse_fragments(parser)?;
                dataset.insert(Element::new(tag, vr, Value::Bytes(bytes))?);
                continue;
            }
            if is_undefined_length_sequence_container(parser, vr, transfer_syntax) {
                let items = parse_sequence(parser, transfer_syntax, depth + 1, None)?;
                let effective_vr = if vr == Vr::Un { Vr::Sq } else { vr };
                dataset.insert(Element::new(tag, effective_vr, Value::Sequence(items))?);
                continue;
            }

            return Err(decode_error(
                "dataset",
                "undefined length not supported for this VR",
            ));
        }

        parser.enforce_element_length(length)?;
        let value_len_usize = usize::try_from(length)
            .map_err(|_| decode_error("dataset", "element value length exceeds usize"))?;
        let value_bytes = parser.read_bytes(value_len_usize)?;

        if vr == Vr::Sq {
            let items = parse_sequence(parser, transfer_syntax, depth + 1, Some(length))?;
            dataset.insert(Element::new(tag, vr, Value::Sequence(items))?);
            continue;
        }

        let value = value_from_bytes(vr, value_bytes, &parser.limits)?;
        dataset.insert(Element::new(tag, vr, value)?);
    }

    Ok(dataset)
}

fn is_undefined_length_sequence_container(
    parser: &Parser<'_>,
    vr: Vr,
    transfer_syntax: TransferSyntax,
) -> bool {
    if vr == Vr::Sq {
        return true;
    }
    if transfer_syntax != TransferSyntax::ImplicitVrLittleEndian && vr != Vr::Un {
        return false;
    }
    matches!(parser.peek_tag(), Some(TAG_ITEM | TAG_SEQ_DELIM))
}

fn parse_sequence(
    parser: &mut Parser<'_>,
    transfer_syntax: TransferSyntax,
    depth: u64,
    length: Option<u32>,
) -> Result<Vec<Dataset>> {
    let mut items = Vec::new();
    let end_offset = length.map(|len| {
        usize::try_from(len)
            .ok()
            .and_then(|l| parser.offset().checked_add(l))
            .unwrap_or(usize::MAX)
    });

    loop {
        if let Some(end) = end_offset {
            if parser.offset() >= end {
                break;
            }
        }
        if parser.is_eof() {
            break;
        }

        let tag = parser.read_tag()?;
        if tag == TAG_SEQ_DELIM {
            let _ = parser.read_u32()?;
            break;
        }
        if tag != TAG_ITEM {
            return Err(decode_error("sequence", "expected item tag"));
        }
        let item_len = parser.read_u32()?;
        let item_end = if item_len == u32::MAX {
            None
        } else {
            let item_len_usize = usize::try_from(item_len)
                .map_err(|_| decode_error("sequence", "item length exceeds usize"))?;
            Some(
                parser
                    .offset()
                    .checked_add(item_len_usize)
                    .ok_or_else(|| decode_error("sequence", "offset overflow"))?,
            )
        };
        let item_dataset = parse_dataset_internal(parser, item_end, transfer_syntax, depth)?;
        items.push(item_dataset);
    }

    Ok(items)
}

fn parse_fragments(parser: &mut Parser<'_>) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();

    loop {
        if parser.is_eof() {
            return Err(decode_error("fragments", "unexpected EOF"));
        }
        let tag = parser.read_tag()?;
        if tag == TAG_SEQ_DELIM {
            let _ = parser.read_u32()?;
            break;
        }
        if tag != TAG_ITEM {
            return Err(decode_error("fragments", "expected item tag"));
        }
        let item_len = parser.read_u32()?;
        if item_len == u32::MAX {
            return Err(decode_error("fragments", "undefined fragment length"));
        }
        let item_len_usize = usize::try_from(item_len)
            .map_err(|_| decode_error("fragments", "fragment length exceeds usize"))?;
        let new_len = u64::try_from(bytes.len())
            .map_err(|_| decode_error("fragments", "accumulated length exceeds u64"))?
            + u64::from(item_len);
        if new_len > parser.limits.max_element_vl_bytes() {
            return Err(limit_exceeded(
                "max_element_vl_bytes",
                new_len,
                parser.limits.max_element_vl_bytes(),
            ));
        }
        let frag = parser.read_bytes(item_len_usize)?;
        bytes.extend_from_slice(frag);
    }

    Ok(bytes)
}

fn value_from_bytes(vr: Vr, bytes: &[u8], limits: &Limits) -> Result<Value> {
    match vr {
        Vr::Ui => Ok(Value::Uid(parse_uid(bytes, limits)?)),
        Vr::Ae
        | Vr::As
        | Vr::Cs
        | Vr::Da
        | Vr::Ds
        | Vr::Is
        | Vr::Lo
        | Vr::Lt
        | Vr::Pn
        | Vr::Sh
        | Vr::St
        | Vr::Tm
        | Vr::Ut => Ok(Value::Str(parse_string(bytes, limits)?)),
        _ => Ok(Value::Bytes(bytes.to_vec())),
    }
}

fn parse_uid(bytes: &[u8], limits: &Limits) -> Result<String> {
    let raw = parse_string(bytes, limits)?;
    Ok(raw.trim_end_matches(['\0', ' ']).to_string())
}

fn parse_string(bytes: &[u8], limits: &Limits) -> Result<String> {
    let bytes_len_u64 = u64::try_from(bytes.len())
        .map_err(|_| decode_error("string", "string length exceeds u64"))?;
    if bytes_len_u64 > limits.max_string_bytes() {
        return Err(limit_exceeded(
            "max_string_bytes",
            bytes_len_u64,
            limits.max_string_bytes(),
        ));
    }
    Ok(String::from_utf8_lossy(bytes).into_owned())
}

struct Parser<'a> {
    data: &'a [u8],
    limits: Limits,
    offset: usize,
    element_count: u64,
}

impl<'a> Parser<'a> {
    fn new(data: &'a [u8], limits: &Limits, offset: usize) -> Self {
        Self {
            data,
            limits: limits.clone(),
            offset,
            element_count: 0,
        }
    }

    fn offset(&self) -> usize {
        self.offset
    }

    fn set_offset(&mut self, offset: usize) {
        self.offset = offset;
    }

    fn is_eof(&self) -> bool {
        self.offset >= self.data.len()
    }

    fn peek_tag(&self) -> Option<Tag> {
        if self.offset.checked_add(4)? > self.data.len() {
            return None;
        }
        let group = u16::from_le_bytes([self.data[self.offset], self.data[self.offset + 1]]);
        let element = u16::from_le_bytes([self.data[self.offset + 2], self.data[self.offset + 3]]);
        Some(Tag(group, element))
    }

    fn read_u16(&mut self) -> Result<u16> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32(&mut self) -> Result<u32> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_tag(&mut self) -> Result<Tag> {
        let group = self.read_u16()?;
        let element = self.read_u16()?;
        Ok(Tag(group, element))
    }

    fn read_explicit_vr_and_len(&mut self) -> Result<(Vr, u32)> {
        let vr_bytes = self.read_bytes(2)?;
        let vr = parse_vr(vr_bytes);
        let length = match vr {
            Vr::Ob | Vr::Ow | Vr::Sq | Vr::Un | Vr::Ut => {
                let _ = self.read_u16()?;
                self.read_u32()?
            }
            _ => u32::from(self.read_u16()?),
        };
        Ok((vr, length))
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| decode_error("parser", "offset overflow while reading bytes"))?;
        if end > self.data.len() {
            return Err(decode_error("parser", "unexpected EOF"));
        }
        let bytes = &self.data[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }

    fn enforce_element_length(&self, length: u32) -> Result<()> {
        if u64::from(length) > self.limits.max_element_vl_bytes() {
            return Err(limit_exceeded(
                "max_element_vl_bytes",
                u64::from(length),
                self.limits.max_element_vl_bytes(),
            ));
        }
        Ok(())
    }

    fn increment_element_count(&mut self) -> Result<()> {
        self.element_count += 1;
        if self.element_count > self.limits.max_dataset_elements() {
            return Err(limit_exceeded(
                "max_dataset_elements",
                self.element_count,
                self.limits.max_dataset_elements(),
            ));
        }
        Ok(())
    }
}

fn parse_vr(bytes: &[u8]) -> Vr {
    match bytes {
        b"AE" => Vr::Ae,
        b"AS" => Vr::As,
        b"AT" => Vr::At,
        b"CS" => Vr::Cs,
        b"DA" => Vr::Da,
        b"DS" => Vr::Ds,
        b"FD" => Vr::Fd,
        b"FL" => Vr::Fl,
        b"IS" => Vr::Is,
        b"LO" => Vr::Lo,
        b"LT" => Vr::Lt,
        b"OB" => Vr::Ob,
        b"OD" => Vr::Od,
        b"OF" => Vr::Of,
        b"OW" => Vr::Ow,
        b"PN" => Vr::Pn,
        b"SH" => Vr::Sh,
        b"SL" => Vr::Sl,
        b"SQ" => Vr::Sq,
        b"SS" => Vr::Ss,
        b"ST" => Vr::St,
        b"TM" => Vr::Tm,
        b"UI" => Vr::Ui,
        b"UL" => Vr::Ul,
        b"UN" => Vr::Un,
        b"US" => Vr::Us,
        b"UT" => Vr::Ut,
        _ => Vr::Other([bytes[0], bytes[1]]),
    }
}

fn decode_error(stage: &'static str, detail: &str) -> Box<Error> {
    Box::new(Error::from_kind(
        ErrorKind::DecodeError {
            stage: stage.to_string(),
            detail: detail.to_string(),
        },
        "decode error",
    ))
}

fn invalid_tag_value(tag: Tag, detail: impl Into<String>) -> Box<Error> {
    Box::new(Error::from_kind(
        ErrorKind::InvalidTagValue {
            tag,
            detail: detail.into(),
        },
        "invalid tag value",
    ))
}

fn limit_exceeded(limit_name: &'static str, observed: u64, allowed: u64) -> Box<Error> {
    Box::new(Error::from_kind(
        ErrorKind::LimitExceeded {
            limit_name,
            observed,
            allowed,
        },
        "limit exceeded",
    ))
}
