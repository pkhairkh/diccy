#![deny(missing_docs)]

//! DICOM Part 10 IO and input boundary handling.

use dicom_core::{Dataset, Element, Error, ErrorKind, Limits, Result, Tag, Value, Vr};
use std::fs;
use std::path::PathBuf;

const PREAMBLE_LEN: usize = 128;
const DICM_PREFIX: &[u8; 4] = b"DICM";

const TAG_ITEM: Tag = Tag(0xFFFE, 0xE000);
const TAG_ITEM_DELIM: Tag = Tag(0xFFFE, 0xE00D);
const TAG_SEQ_DELIM: Tag = Tag(0xFFFE, 0xE0DD);

const TS_IMPLICIT_VR_LE: &str = "1.2.840.10008.1.2";
const TS_EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1";
const TS_DEFLATED_EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1.99";
const TS_RLE_LOSSLESS: &str = "1.2.840.10008.1.2.5";
const TS_JPEG_BASELINE: &str = "1.2.840.10008.1.2.4.50";
const TS_JPEGLS_LOSSLESS: &str = "1.2.840.10008.1.2.4.80";
const TS_JPEGLS_NEAR_LOSSLESS: &str = "1.2.840.10008.1.2.4.81";
const TS_JPEG2000_LOSSLESS: &str = "1.2.840.10008.1.2.4.90";
const TS_JPEG2000_LOSSY: &str = "1.2.840.10008.1.2.4.91";
const TS_MPEG2_MPML: &str = "1.2.840.10008.1.2.4.100";
const TS_MPEG2_MPML_F: &str = "1.2.840.10008.1.2.4.100.1";
const TS_MPEG2_MPHL: &str = "1.2.840.10008.1.2.4.101";
const TS_MPEG2_MPHL_F: &str = "1.2.840.10008.1.2.4.101.1";
const TS_H264_HP41: &str = "1.2.840.10008.1.2.4.102";
const TS_H264_HP41_F: &str = "1.2.840.10008.1.2.4.102.1";
const TS_H264_BD_COMPAT: &str = "1.2.840.10008.1.2.4.103";
const TS_H264_BD_COMPAT_F: &str = "1.2.840.10008.1.2.4.103.1";
const TS_H264_HP42: &str = "1.2.840.10008.1.2.4.104";
const TS_H264_HP42_F: &str = "1.2.840.10008.1.2.4.104.1";
const TS_H264_HP32: &str = "1.2.840.10008.1.2.4.105";
const TS_H264_HP32_F: &str = "1.2.840.10008.1.2.4.105.1";
const TS_H264_STEREO: &str = "1.2.840.10008.1.2.4.106";
const TS_H264_STEREO_F: &str = "1.2.840.10008.1.2.4.106.1";
const TS_HEVC_MP51: &str = "1.2.840.10008.1.2.4.107";
const TS_HEVC_MP51_F: &str = "1.2.840.10008.1.2.4.107.1";
const TS_HEVC_M10P51: &str = "1.2.840.10008.1.2.4.108";
const TS_HEVC_M10P51_F: &str = "1.2.840.10008.1.2.4.108.1";

const SOP_CLASS_CT: &str = "1.2.840.10008.5.1.4.1.1.2";
const SOP_CLASS_MR: &str = "1.2.840.10008.5.1.4.1.1.4";
const SOP_CLASS_SC: &str = "1.2.840.10008.5.1.4.1.1.7";
const SOP_CLASS_SC_MF_BYTE: &str = "1.2.840.10008.5.1.4.1.1.7.2";
const SOP_CLASS_SC_MF_WORD: &str = "1.2.840.10008.5.1.4.1.1.7.3";
const SOP_CLASS_SC_MF_COLOR: &str = "1.2.840.10008.5.1.4.1.1.7.4";
const SOP_CLASS_ENHANCED_CT: &str = "1.2.840.10008.5.1.4.1.1.2.1";
const SOP_CLASS_ENHANCED_MR: &str = "1.2.840.10008.5.1.4.1.1.4.1";
const SOP_CLASS_PET: &str = "1.2.840.10008.5.1.4.1.1.128";
const SOP_CLASS_CR: &str = "1.2.840.10008.5.1.4.1.1.1";
const SOP_CLASS_DX_PRESENTATION: &str = "1.2.840.10008.5.1.4.1.1.1.1";
const SOP_CLASS_US: &str = "1.2.840.10008.5.1.4.1.1.6.1";
const SOP_CLASS_US_MF: &str = "1.2.840.10008.5.1.4.1.1.3.1";
const SOP_CLASS_NM: &str = "1.2.840.10008.5.1.4.1.1.20";
const SOP_CLASS_XA: &str = "1.2.840.10008.5.1.4.1.1.12.1";
const SOP_CLASS_XRF: &str = "1.2.840.10008.5.1.4.1.1.12.2";
const SOP_CLASS_SEG: &str = "1.2.840.10008.5.1.4.1.1.66.4";
const SOP_CLASS_GSPS: &str = "1.2.840.10008.5.1.4.1.1.11.1";
const SOP_CLASS_RT_DOSE: &str = "1.2.840.10008.5.1.4.1.1.481.2";
const SOP_CLASS_RT_STRUCTURE: &str = "1.2.840.10008.5.1.4.1.1.481.3";
const SOP_CLASS_RT_PLAN: &str = "1.2.840.10008.5.1.4.1.1.481.5";
const SOP_CLASS_SR_BASIC_TEXT: &str = "1.2.840.10008.5.1.4.1.1.88.11";
const SOP_CLASS_SR_COMPREHENSIVE: &str = "1.2.840.10008.5.1.4.1.1.88.33";

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
        Some(self.bytes.len() as u64)
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

/// Reader options for explicit raw-mode handling and diagnostics capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReaderOptions {
    /// Enable raw-dataset fallback when P10 meta parsing fails.
    pub raw_mode: bool,
    /// Capture parser warnings.
    pub capture_warnings: bool,
    /// Capture top-level element offset metadata.
    pub capture_debug_offsets: bool,
}

impl Default for ReaderOptions {
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
    options: ReaderOptions,
    warnings: Vec<ParserWarning>,
}

impl<S: DicomSource> P10Reader<S> {
    /// Create a new reader with default limits.
    pub fn new(source: S) -> Self {
        Self {
            source,
            limits: Limits::default(),
            options: ReaderOptions::default(),
            warnings: Vec::new(),
        }
    }

    /// Create a new reader with explicit limits.
    pub fn with_limits(source: S, limits: Limits) -> Self {
        Self {
            source,
            limits,
            options: ReaderOptions::default(),
            warnings: Vec::new(),
        }
    }

    /// Create a new reader with explicit limits and options.
    pub fn with_limits_and_options(source: S, limits: Limits, options: ReaderOptions) -> Self {
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
    pub fn options(&self) -> &ReaderOptions {
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
        self.enforce_input_limit(data.len() as u64)?;
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
        self.enforce_input_limit(data.len() as u64)?;
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
        if len > self.limits.max_input_bytes {
            return Err(limit_exceeded(
                "max_input_bytes",
                len,
                self.limits.max_input_bytes,
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
    if data.len() as u64 > limits.max_input_bytes {
        return Err(limit_exceeded(
            "max_input_bytes",
            data.len() as u64,
            limits.max_input_bytes,
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
    options: &ReaderOptions,
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
    let Value::Str(value) = &element.value else {
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
                let len = u32::from_le_bytes([
                    data[cursor + 8],
                    data[cursor + 9],
                    data[cursor + 10],
                    data[cursor + 11],
                ]) as usize;
                (12usize, len)
            }
            _ => {
                let len = u16::from_le_bytes([data[cursor + 6], data[cursor + 7]]) as usize;
                (8usize, len)
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
        let value_bytes = parser.read_bytes(length as usize)?;

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
        total = total.saturating_add(read as u64);
        if total > limits.max_decompressed_bytes {
            return Err(limit_exceeded(
                "max_decompressed_bytes",
                total,
                limits.max_decompressed_bytes,
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
    if caps.pack_enhanced && matches!(sop_class_uid, SOP_CLASS_ENHANCED_CT | SOP_CLASS_ENHANCED_MR)
    {
        return true;
    }
    if caps.pack_us && matches!(sop_class_uid, SOP_CLASS_US | SOP_CLASS_US_MF) {
        return true;
    }
    if caps.pack_nm && sop_class_uid == SOP_CLASS_NM {
        return true;
    }
    if caps.pack_xa && matches!(sop_class_uid, SOP_CLASS_XA | SOP_CLASS_XRF) {
        return true;
    }
    if caps.modality_pet && sop_class_uid == SOP_CLASS_PET {
        return true;
    }
    if caps.modality_xr && matches!(sop_class_uid, SOP_CLASS_CR | SOP_CLASS_DX_PRESENTATION) {
        return true;
    }
    if caps.pack_seg && sop_class_uid == SOP_CLASS_SEG {
        return true;
    }
    if caps.pack_rt
        && matches!(
            sop_class_uid,
            SOP_CLASS_RT_DOSE | SOP_CLASS_RT_STRUCTURE | SOP_CLASS_RT_PLAN
        )
    {
        return true;
    }
    if caps.pack_sr
        && matches!(
            sop_class_uid,
            SOP_CLASS_SR_BASIC_TEXT | SOP_CLASS_SR_COMPREHENSIVE
        )
    {
        return true;
    }
    if caps.gsps && sop_class_uid == SOP_CLASS_GSPS {
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
    Box::new(Error::from_kind(
        ErrorKind::MissingRequiredTag { tag },
        "missing required tag",
    ))
}

fn read_uid(dataset: &Dataset, tag: Tag, limits: &Limits) -> Result<Option<String>> {
    match dataset.get(tag) {
        None => Ok(None),
        Some(element) => match &element.value {
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
        Some(element) => match &element.value {
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
        Some(element) => match &element.value {
            Value::I32(value) if *value >= 0 && *value <= u16::MAX as i32 => {
                Ok(Some(*value as u16))
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
    if depth > parser.limits.max_sequence_depth {
        return Err(limit_exceeded(
            "max_sequence_depth",
            depth,
            parser.limits.max_sequence_depth,
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
                dataset.insert(Element {
                    tag,
                    vr,
                    value: Value::Bytes(bytes),
                });
                continue;
            }
            if is_undefined_length_sequence_container(parser, vr, transfer_syntax) {
                let items = parse_sequence(parser, transfer_syntax, depth + 1, None)?;
                let effective_vr = if vr == Vr::Un { Vr::Sq } else { vr };
                dataset.insert(Element {
                    tag,
                    vr: effective_vr,
                    value: Value::Sequence(items),
                });
                continue;
            }

            return Err(decode_error(
                "dataset",
                "undefined length not supported for this VR",
            ));
        }

        parser.enforce_element_length(length)?;
        let value_bytes = parser.read_bytes(length as usize)?;

        if vr == Vr::Sq {
            let items = parse_sequence(parser, transfer_syntax, depth + 1, Some(length))?;
            dataset.insert(Element {
                tag,
                vr,
                value: Value::Sequence(items),
            });
            continue;
        }

        let value = value_from_bytes(vr, value_bytes, &parser.limits)?;
        dataset.insert(Element { tag, vr, value });
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
    let end_offset = length.map(|len| parser.offset().saturating_add(len as usize));

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
            Some(parser.offset().saturating_add(item_len as usize))
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
        let new_len = bytes.len() as u64 + item_len as u64;
        if new_len > parser.limits.max_element_vl_bytes {
            return Err(limit_exceeded(
                "max_element_vl_bytes",
                new_len,
                parser.limits.max_element_vl_bytes,
            ));
        }
        let frag = parser.read_bytes(item_len as usize)?;
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
    if bytes.len() as u64 > limits.max_string_bytes {
        return Err(limit_exceeded(
            "max_string_bytes",
            bytes.len() as u64,
            limits.max_string_bytes,
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
            _ => self.read_u16()? as u32,
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
        if length as u64 > self.limits.max_element_vl_bytes {
            return Err(limit_exceeded(
                "max_element_vl_bytes",
                length as u64,
                self.limits.max_element_vl_bytes,
            ));
        }
        Ok(())
    }

    fn increment_element_count(&mut self) -> Result<()> {
        self.element_count += 1;
        if self.element_count > self.limits.max_dataset_elements {
            return Err(limit_exceeded(
                "max_dataset_elements",
                self.element_count,
                self.limits.max_dataset_elements,
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

#[cfg(test)]
mod tests {
    #[cfg(feature = "codec-j2k")]
    use super::TS_JPEG2000_LOSSLESS;
    #[cfg(feature = "codec-jpegls")]
    use super::TS_JPEGLS_LOSSLESS;
    use super::{
        BytesSource, DicomSource, FileSource, P10Reader, ReaderOptions, Tag, TS_EXPLICIT_VR_LE,
        TS_IMPLICIT_VR_LE, TS_JPEG_BASELINE, TS_RLE_LOSSLESS,
    };
    use dicom_core::{ErrorKind, Limits};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct EmptySource;

    impl DicomSource for EmptySource {
        fn read_to_end(&mut self) -> dicom_core::Result<Vec<u8>> {
            Ok(Vec::new())
        }
    }

    fn build_p10(meta_ts: &str, dataset: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0u8; 128];
        bytes.extend_from_slice(b"DICM");
        bytes.extend_from_slice(&meta_element_ui(Tag(0x0002, 0x0010), meta_ts));
        bytes.extend_from_slice(dataset);
        bytes
    }

    fn meta_element_ui(tag: Tag, value: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&tag.0.to_le_bytes());
        buf.extend_from_slice(&tag.1.to_le_bytes());
        buf.extend_from_slice(b"UI");
        let mut bytes = value.as_bytes().to_vec();
        if bytes.len() % 2 == 1 {
            bytes.push(0);
        }
        buf.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(&bytes);
        buf
    }

    fn dataset_element_implicit(tag: Tag, value: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&tag.0.to_le_bytes());
        buf.extend_from_slice(&tag.1.to_le_bytes());
        buf.extend_from_slice(&(value.len() as u32).to_le_bytes());
        buf.extend_from_slice(value);
        buf
    }

    fn dataset_element_implicit_undefined_length(tag: Tag) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&tag.0.to_le_bytes());
        buf.extend_from_slice(&tag.1.to_le_bytes());
        buf.extend_from_slice(&u32::MAX.to_le_bytes());
        buf
    }

    fn item_tag_with_length(len: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&super::TAG_ITEM.0.to_le_bytes());
        buf.extend_from_slice(&super::TAG_ITEM.1.to_le_bytes());
        buf.extend_from_slice(&len.to_le_bytes());
        buf
    }

    fn sequence_delim_tag() -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&super::TAG_SEQ_DELIM.0.to_le_bytes());
        buf.extend_from_slice(&super::TAG_SEQ_DELIM.1.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf
    }

    fn dataset_element_explicit(tag: Tag, vr: [u8; 2], value: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&tag.0.to_le_bytes());
        buf.extend_from_slice(&tag.1.to_le_bytes());
        buf.extend_from_slice(&vr);
        let mut bytes = value.to_vec();
        if bytes.len() % 2 == 1 {
            bytes.push(0);
        }
        match &vr {
            b"OB" | b"OW" | b"SQ" | b"UN" | b"UT" => {
                buf.extend_from_slice(&0u16.to_le_bytes());
                buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            }
            _ => {
                buf.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
            }
        }
        buf.extend_from_slice(&bytes);
        buf
    }

    fn u16_bytes(value: u16) -> [u8; 2] {
        value.to_le_bytes()
    }

    fn minimal_sc_dataset_explicit(sop_class_uid: &str) -> Vec<u8> {
        let mut dataset = Vec::new();
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0008, 0x0016),
            *b"UI",
            sop_class_uid.as_bytes(),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0008, 0x0018),
            *b"UI",
            b"1.2.3.4.5",
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0020, 0x000D),
            *b"UI",
            b"1.2.3",
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0020, 0x000E),
            *b"UI",
            b"2.3.4",
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0002),
            *b"US",
            &u16_bytes(1),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0004),
            *b"CS",
            b"MONOCHROME2",
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0010),
            *b"US",
            &u16_bytes(1),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0011),
            *b"US",
            &u16_bytes(1),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0100),
            *b"US",
            &u16_bytes(16),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0101),
            *b"US",
            &u16_bytes(12),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0102),
            *b"US",
            &u16_bytes(11),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0103),
            *b"US",
            &u16_bytes(0),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x7FE0, 0x0010),
            *b"OB",
            &[0u8],
        ));
        dataset
    }

    fn minimal_enhanced_dataset_explicit(sop_class_uid: &str) -> Vec<u8> {
        let mut dataset = minimal_sc_dataset_explicit(sop_class_uid);
        dataset.extend_from_slice(&dataset_element_explicit(Tag(0x0028, 0x0008), *b"IS", b"1"));
        dataset.extend_from_slice(&dataset_element_explicit(Tag(0x5200, 0x9229), *b"SQ", &[]));
        dataset.extend_from_slice(&dataset_element_explicit(Tag(0x5200, 0x9230), *b"SQ", &[]));
        dataset
    }

    fn assert_dataset_accepted(dataset: Vec<u8>) {
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let parsed = reader.read_dataset().expect("dataset");
        assert!(parsed.get(Tag(0x0008, 0x0016)).is_some());
    }

    fn minimal_rt_dose_dataset_explicit(missing: Option<Tag>) -> Vec<u8> {
        let mut dataset = Vec::new();
        let mut push = |tag: Tag, vr: [u8; 2], value: &[u8]| {
            if Some(tag) != missing {
                dataset.extend_from_slice(&dataset_element_explicit(tag, vr, value));
            }
        };

        push(
            Tag(0x0008, 0x0016),
            *b"UI",
            super::SOP_CLASS_RT_DOSE.as_bytes(),
        );
        push(Tag(0x0008, 0x0018), *b"UI", b"1.2.3.4.5");
        push(Tag(0x0020, 0x000D), *b"UI", b"1.2.3");
        push(Tag(0x0020, 0x000E), *b"UI", b"2.3.4");
        push(Tag(0x0020, 0x0052), *b"UI", b"9.8.7");
        push(Tag(0x0028, 0x0002), *b"US", &u16_bytes(1));
        push(Tag(0x0028, 0x0004), *b"CS", b"MONOCHROME2");
        push(Tag(0x0028, 0x0010), *b"US", &u16_bytes(1));
        push(Tag(0x0028, 0x0011), *b"US", &u16_bytes(1));
        push(Tag(0x0028, 0x0100), *b"US", &u16_bytes(16));
        push(Tag(0x0028, 0x0101), *b"US", &u16_bytes(16));
        push(Tag(0x0028, 0x0102), *b"US", &u16_bytes(15));
        push(Tag(0x0028, 0x0103), *b"US", &u16_bytes(0));
        push(Tag(0x0028, 0x0008), *b"IS", b"1");
        push(Tag(0x0028, 0x0030), *b"DS", b"1\\1");
        push(Tag(0x0020, 0x0032), *b"DS", b"0\\0\\0");
        push(Tag(0x0020, 0x0037), *b"DS", b"1\\0\\0\\0\\1\\0");
        push(Tag(0x7FE0, 0x0010), *b"OW", &[0, 0]);

        dataset
    }

    fn minimal_rt_structure_dataset_explicit(missing: Option<Tag>) -> Vec<u8> {
        let mut dataset = Vec::new();
        let mut push = |tag: Tag, vr: [u8; 2], value: &[u8]| {
            if Some(tag) != missing {
                dataset.extend_from_slice(&dataset_element_explicit(tag, vr, value));
            }
        };

        push(
            Tag(0x0008, 0x0016),
            *b"UI",
            super::SOP_CLASS_RT_STRUCTURE.as_bytes(),
        );
        push(Tag(0x0008, 0x0018), *b"UI", b"1.2.3.4.5");
        push(Tag(0x0020, 0x000D), *b"UI", b"1.2.3");
        push(Tag(0x0020, 0x000E), *b"UI", b"2.3.4");
        push(Tag(0x0020, 0x0052), *b"UI", b"9.8.7");
        push(Tag(0x3006, 0x0039), *b"SQ", &[]);

        dataset
    }

    fn minimal_rt_plan_dataset_explicit(missing: Option<Tag>) -> Vec<u8> {
        let mut dataset = Vec::new();
        let mut push = |tag: Tag, vr: [u8; 2], value: &[u8]| {
            if Some(tag) != missing {
                dataset.extend_from_slice(&dataset_element_explicit(tag, vr, value));
            }
        };

        push(
            Tag(0x0008, 0x0016),
            *b"UI",
            super::SOP_CLASS_RT_PLAN.as_bytes(),
        );
        push(Tag(0x0008, 0x0018), *b"UI", b"1.2.3.4.5");
        push(Tag(0x0020, 0x000D), *b"UI", b"1.2.3");
        push(Tag(0x0020, 0x000E), *b"UI", b"2.3.4");
        push(Tag(0x0020, 0x0052), *b"UI", b"9.8.7");
        push(Tag(0x300C, 0x0060), *b"SQ", &[]);

        dataset
    }

    fn minimal_seg_dataset_explicit(missing: Option<Tag>) -> Vec<u8> {
        let mut dataset = Vec::new();
        let mut push = |tag: Tag, vr: [u8; 2], value: &[u8]| {
            if Some(tag) != missing {
                dataset.extend_from_slice(&dataset_element_explicit(tag, vr, value));
            }
        };

        push(Tag(0x0008, 0x0016), *b"UI", super::SOP_CLASS_SEG.as_bytes());
        push(Tag(0x0008, 0x0018), *b"UI", b"1.2.3.4.5");
        push(Tag(0x0020, 0x000D), *b"UI", b"1.2.3");
        push(Tag(0x0020, 0x000E), *b"UI", b"2.3.4");
        push(Tag(0x0020, 0x0052), *b"UI", b"9.8.7");
        push(Tag(0x0028, 0x0002), *b"US", &u16_bytes(1));
        push(Tag(0x0028, 0x0004), *b"CS", b"MONOCHROME2");
        push(Tag(0x0028, 0x0010), *b"US", &u16_bytes(1));
        push(Tag(0x0028, 0x0011), *b"US", &u16_bytes(1));
        push(Tag(0x0028, 0x0100), *b"US", &u16_bytes(1));
        push(Tag(0x0028, 0x0101), *b"US", &u16_bytes(1));
        push(Tag(0x0028, 0x0102), *b"US", &u16_bytes(0));
        push(Tag(0x0028, 0x0103), *b"US", &u16_bytes(0));
        push(Tag(0x0062, 0x0001), *b"CS", b"BINARY");
        push(Tag(0x0062, 0x0004), *b"US", &u16_bytes(1));
        push(Tag(0x7FE0, 0x0010), *b"OB", &[0x01]);

        dataset
    }

    fn minimal_sc_dataset_explicit_missing_study_uid(sop_class_uid: &str) -> Vec<u8> {
        let dataset = minimal_sc_dataset_explicit(sop_class_uid);
        let mut stripped = Vec::new();
        let study_tag = Tag(0x0020, 0x000D);
        let mut offset = 0usize;
        while offset + 8 <= dataset.len() {
            let group = u16::from_le_bytes([dataset[offset], dataset[offset + 1]]);
            let element = u16::from_le_bytes([dataset[offset + 2], dataset[offset + 3]]);
            let tag = Tag(group, element);
            let vr = [dataset[offset + 4], dataset[offset + 5]];
            let (len, header_len) = match &vr {
                b"OB" | b"OW" | b"SQ" | b"UN" | b"UT" => {
                    let len_bytes = &dataset[offset + 8..offset + 12];
                    (
                        u32::from_le_bytes([
                            len_bytes[0],
                            len_bytes[1],
                            len_bytes[2],
                            len_bytes[3],
                        ]),
                        12,
                    )
                }
                _ => {
                    let len_bytes = &dataset[offset + 6..offset + 8];
                    (u16::from_le_bytes([len_bytes[0], len_bytes[1]]) as u32, 8)
                }
            };
            let element_end = offset + header_len + (len as usize);
            if tag != study_tag {
                stripped.extend_from_slice(&dataset[offset..element_end]);
            }
            offset = element_end;
        }
        stripped
    }

    fn minimal_sc_dataset_implicit(sop_class_uid: &str) -> Vec<u8> {
        let mut dataset = Vec::new();
        dataset.extend_from_slice(&dataset_element_implicit(
            Tag(0x0008, 0x0016),
            sop_class_uid.as_bytes(),
        ));
        dataset.extend_from_slice(&dataset_element_implicit(Tag(0x0008, 0x0018), b"1.2.3.4.5"));
        dataset.extend_from_slice(&dataset_element_implicit(Tag(0x0020, 0x000D), b"1.2.3"));
        dataset.extend_from_slice(&dataset_element_implicit(Tag(0x0020, 0x000E), b"2.3.4"));
        dataset.extend_from_slice(&dataset_element_implicit(
            Tag(0x0028, 0x0002),
            &u16_bytes(1),
        ));
        dataset.extend_from_slice(&dataset_element_implicit(
            Tag(0x0028, 0x0004),
            b"MONOCHROME2",
        ));
        dataset.extend_from_slice(&dataset_element_implicit(
            Tag(0x0028, 0x0010),
            &u16_bytes(1),
        ));
        dataset.extend_from_slice(&dataset_element_implicit(
            Tag(0x0028, 0x0011),
            &u16_bytes(1),
        ));
        dataset.extend_from_slice(&dataset_element_implicit(
            Tag(0x0028, 0x0100),
            &u16_bytes(16),
        ));
        dataset.extend_from_slice(&dataset_element_implicit(
            Tag(0x0028, 0x0101),
            &u16_bytes(12),
        ));
        dataset.extend_from_slice(&dataset_element_implicit(
            Tag(0x0028, 0x0102),
            &u16_bytes(11),
        ));
        dataset.extend_from_slice(&dataset_element_implicit(
            Tag(0x0028, 0x0103),
            &u16_bytes(0),
        ));
        dataset.extend_from_slice(&dataset_element_implicit(Tag(0x7FE0, 0x0010), &[0u8]));
        dataset
    }

    fn tmp_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut path = std::env::temp_dir();
        path.push(format!("rdvf-{name}-{nonce}"));
        path
    }

    #[test]
    fn p10reader_defaults_to_limits() {
        // REQ-SEC-402: defaults are enforced unless explicitly overridden.
        let reader = P10Reader::new(EmptySource);
        assert_eq!(reader.limits(), &Limits::default());
    }

    #[test]
    fn p10reader_uses_explicit_limits() {
        // REQ-SEC-402: explicit limits override defaults.
        let limits = Limits {
            max_input_bytes: 1,
            ..Limits::default()
        };
        let reader = P10Reader::with_limits(EmptySource, limits.clone());
        assert_eq!(reader.limits(), &limits);
    }

    #[test]
    fn raw_mode_fallback_is_explicit_and_fails_closed_by_default() {
        let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");

        let mut strict = P10Reader::new(BytesSource::new(dataset.clone()));
        let err = strict
            .read_dataset()
            .expect_err("strict mode must reject raw bytes");
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));

        let mut raw_reader = P10Reader::with_limits_and_options(
            BytesSource::new(dataset),
            Limits::default(),
            ReaderOptions {
                raw_mode: true,
                capture_warnings: true,
                capture_debug_offsets: true,
            },
        );
        let diagnostics = raw_reader
            .read_dataset_with_diagnostics()
            .expect("raw-mode parse");
        assert!(diagnostics.raw_mode_used);
        assert!(diagnostics
            .warnings
            .iter()
            .any(|warning| warning.code == "DVF.IO.RAW_MODE_META_FALLBACK"));
        assert!(!diagnostics.debug_meta.is_empty());
        assert!(diagnostics.dataset.get(Tag(0x0008, 0x0016)).is_some());
    }

    #[test]
    fn charset_warnings_are_structured_and_deterministic() {
        let mut dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0008, 0x0005),
            *b"CS",
            b"ISO_IR 100",
        ));
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::with_limits_and_options(
            BytesSource::new(bytes),
            Limits::default(),
            ReaderOptions {
                raw_mode: false,
                capture_warnings: true,
                capture_debug_offsets: false,
            },
        );
        let diagnostics = reader
            .read_dataset_with_diagnostics()
            .expect("dataset with charset warning");
        let codes: Vec<&str> = diagnostics
            .warnings
            .iter()
            .map(|warning| warning.code)
            .collect();
        let mut sorted = codes.clone();
        sorted.sort_unstable();
        assert_eq!(codes, sorted);
        assert!(codes.contains(&"DVF.IO.CHARSET_UNSUPPORTED"));
    }

    #[test]
    fn read_meta_rejects_missing_prefix() {
        // REQ-IO-010: malformed P10 prefix is rejected.
        let mut reader = P10Reader::new(BytesSource::new(vec![0u8; 10]));
        let err = reader.read_meta().expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
        assert_eq!(err.code, "DVF.DICOM.DECODE_ERROR");
    }

    #[test]
    fn read_meta_reads_transfer_syntax() {
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &[]);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let meta = reader.read_meta().expect("meta");
        assert_eq!(meta.transfer_syntax_uid, TS_EXPLICIT_VR_LE);
    }

    #[test]
    fn read_dataset_respects_max_input_bytes() {
        // REQ-API-210: limit violations return LimitExceeded.
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &[]);
        let limits = Limits {
            max_input_bytes: 4,
            ..Limits::default()
        };
        let mut reader = P10Reader::with_limits(BytesSource::new(bytes), limits);
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::LimitExceeded { .. }));
    }

    #[test]
    fn read_dataset_supports_implicit_vr() {
        // REQ-CONF-002/003/020: implicit VR path still enforces envelope validation.
        let dataset = minimal_sc_dataset_implicit("1.2.840.10008.5.1.4.1.1.7");
        let bytes = build_p10(TS_IMPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let dataset = reader.read_dataset().expect("dataset");
        assert!(dataset.get(Tag(0x0008, 0x0016)).is_some());
    }

    #[test]
    fn read_dataset_accepts_implicit_undefined_length_sequence() {
        // REQ-IO-013: implicit-VR undefined-length sequences with item/delimiter tokens decode.
        let mut dataset = minimal_sc_dataset_implicit("1.2.840.10008.5.1.4.1.1.7");
        dataset.extend_from_slice(&dataset_element_implicit_undefined_length(Tag(
            0x0008, 0x1115,
        )));
        let item_payload = dataset_element_implicit(Tag(0x0008, 0x1155), b"1.2.3");
        dataset.extend_from_slice(&item_tag_with_length(item_payload.len() as u32));
        dataset.extend_from_slice(&item_payload);
        dataset.extend_from_slice(&sequence_delim_tag());

        let bytes = build_p10(TS_IMPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let parsed = reader.read_dataset().expect("dataset");
        let seq = parsed.get(Tag(0x0008, 0x1115)).expect("sequence element");
        match &seq.value {
            dicom_core::Value::Sequence(items) => assert_eq!(items.len(), 1),
            _ => panic!("expected sequence value"),
        }
    }

    #[test]
    fn read_dataset_rejects_implicit_undefined_length_non_sequence_container() {
        // REQ-IO-013: undefined-length non-sequence containers fail closed.
        let mut dataset = minimal_sc_dataset_implicit("1.2.840.10008.5.1.4.1.1.7");
        dataset.extend_from_slice(&dataset_element_implicit_undefined_length(Tag(
            0x0010, 0x0010,
        )));
        dataset.extend_from_slice(&[0u8; 8]);
        let bytes = build_p10(TS_IMPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected decode error");
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn read_dataset_enforces_element_limit() {
        // REQ-SEC-404: limits are enforced before allocating element values.
        let value = vec![0u8; 8];
        let mut dataset = Vec::new();
        dataset.extend_from_slice(&Tag(0x0010, 0x0010).0.to_le_bytes());
        dataset.extend_from_slice(&Tag(0x0010, 0x0010).1.to_le_bytes());
        dataset.extend_from_slice(b"LO");
        dataset.extend_from_slice(&(value.len() as u16).to_le_bytes());
        dataset.extend_from_slice(&value);

        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let limits = Limits {
            max_element_vl_bytes: 4,
            ..Limits::default()
        };
        let mut reader = P10Reader::with_limits(BytesSource::new(bytes), limits);
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::LimitExceeded { .. }));
    }

    #[test]
    fn read_dataset_rejects_unsupported_transfer_syntax() {
        // REQ-CONF-002: unsupported Transfer Syntax is rejected early.
        let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
        let bytes = build_p10("1.2.3.4.5.6.7.8", &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(
            err.kind,
            ErrorKind::UnsupportedTransferSyntax { .. }
        ));
    }

    #[test]
    fn read_dataset_rejects_explicit_vr_big_endian_transfer_syntax() {
        // REQ-CONF-002: out-of-envelope Big Endian transfer syntax must fail closed.
        let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
        let bytes = build_p10("1.2.840.10008.1.2.2", &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(
            err.kind,
            ErrorKind::UnsupportedTransferSyntax { .. }
        ));
    }

    #[test]
    #[cfg(not(feature = "tier1-deflate"))]
    fn read_dataset_rejects_deflated_transfer_syntax_without_feature() {
        // REQ-TS-204: deflated transfer syntax is deferred unless tier1-deflate is enabled.
        let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
        let bytes = build_p10(super::TS_DEFLATED_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(
            err.kind,
            ErrorKind::UnsupportedTransferSyntax { .. }
        ));
    }

    #[test]
    fn read_dataset_accepts_rle_transfer_syntax() {
        // REQ-TS-202: RLE transfer syntax parsed as explicit VR LE.
        let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
        let bytes = build_p10(TS_RLE_LOSSLESS, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let dataset = reader.read_dataset().expect("dataset");
        assert!(dataset.get(Tag(0x7FE0, 0x0010)).is_some());
    }

    #[test]
    fn read_dataset_accepts_jpeg_baseline_transfer_syntax() {
        // REQ-TS-201: JPEG Baseline transfer syntax parsed as explicit VR LE.
        let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
        let bytes = build_p10(TS_JPEG_BASELINE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let dataset = reader.read_dataset().expect("dataset");
        assert!(dataset.get(Tag(0x7FE0, 0x0010)).is_some());
    }

    #[test]
    #[cfg(feature = "tier1-deflate")]
    fn read_dataset_accepts_deflated_transfer_syntax() {
        // REQ-TS-204: deflated transfer syntax is accepted when feature enabled.
        use flate2::{write::ZlibEncoder, Compression};
        use std::io::Write;

        let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(&dataset).expect("deflate write");
        let compressed = encoder.finish().expect("deflate finish");
        let bytes = build_p10(super::TS_DEFLATED_EXPLICIT_VR_LE, &compressed);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let dataset = reader.read_dataset().expect("dataset");
        assert!(dataset.get(Tag(0x7FE0, 0x0010)).is_some());
    }

    #[test]
    #[cfg(feature = "codec-jpegls")]
    fn read_dataset_accepts_jpegls_transfer_syntax() {
        // REQ-TS-203: codec pack transfer syntax parsed as explicit VR LE.
        let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
        let bytes = build_p10(TS_JPEGLS_LOSSLESS, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let dataset = reader.read_dataset().expect("dataset");
        assert!(dataset.get(Tag(0x7FE0, 0x0010)).is_some());
    }

    #[test]
    #[cfg(feature = "codec-jpegls")]
    fn read_dataset_accepts_jpegls_near_lossless_transfer_syntax() {
        // REQ-TS-203: near-lossless JPEG-LS transfer syntax parsed as explicit VR LE.
        let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
        let bytes = build_p10(super::TS_JPEGLS_NEAR_LOSSLESS, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let dataset = reader.read_dataset().expect("dataset");
        assert!(dataset.get(Tag(0x7FE0, 0x0010)).is_some());
    }

    #[test]
    #[cfg(not(feature = "codec-jpegls"))]
    fn read_dataset_rejects_jpegls_transfer_syntax_without_feature() {
        // REQ-TS-203: JPEG-LS transfer syntax is deferred unless codec-jpegls is enabled.
        let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
        let bytes = build_p10(super::TS_JPEGLS_LOSSLESS, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(
            err.kind,
            ErrorKind::UnsupportedTransferSyntax { .. }
        ));
    }

    #[test]
    #[cfg(feature = "codec-j2k")]
    fn read_dataset_accepts_jpeg2000_transfer_syntax() {
        // REQ-TS-203: codec pack transfer syntax parsed as explicit VR LE.
        let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
        let bytes = build_p10(TS_JPEG2000_LOSSLESS, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let dataset = reader.read_dataset().expect("dataset");
        assert!(dataset.get(Tag(0x7FE0, 0x0010)).is_some());
    }

    #[test]
    #[cfg(feature = "codec-j2k")]
    fn read_dataset_accepts_jpeg2000_lossy_transfer_syntax() {
        // REQ-TS-203: lossy JPEG 2000 transfer syntax parsed as explicit VR LE.
        let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
        let bytes = build_p10(super::TS_JPEG2000_LOSSY, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let dataset = reader.read_dataset().expect("dataset");
        assert!(dataset.get(Tag(0x7FE0, 0x0010)).is_some());
    }

    #[test]
    #[cfg(not(feature = "codec-j2k"))]
    fn read_dataset_rejects_jpeg2000_transfer_syntax_without_feature() {
        // REQ-TS-203: JPEG 2000 transfer syntax is deferred unless codec-j2k is enabled.
        let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
        let bytes = build_p10(super::TS_JPEG2000_LOSSLESS, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(
            err.kind,
            ErrorKind::UnsupportedTransferSyntax { .. }
        ));
    }

    #[test]
    #[cfg(not(feature = "codec-mpeg2"))]
    fn read_dataset_rejects_mpeg2_transfer_syntax_without_feature() {
        // REQ-TS-205: MPEG-2 transfer syntax requires feature gate.
        let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
        let bytes = build_p10(super::TS_MPEG2_MPML, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(
            err.kind,
            ErrorKind::UnsupportedTransferSyntax { .. }
        ));
    }

    #[test]
    #[cfg(feature = "codec-mpeg2")]
    fn read_dataset_accepts_mpeg2_transfer_syntax_when_enabled() {
        // REQ-TS-205: MPEG-2 transfer syntax accepted when feature enabled.
        let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
        let bytes = build_p10(super::TS_MPEG2_MPML, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let dataset = reader.read_dataset().expect("dataset");
        assert!(dataset.get(Tag(0x7FE0, 0x0010)).is_some());
    }

    #[test]
    #[cfg(not(feature = "codec-h264"))]
    fn read_dataset_rejects_h264_transfer_syntax_without_feature() {
        // REQ-TS-205: H.264 transfer syntax requires feature gate.
        let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
        let bytes = build_p10(super::TS_H264_HP41, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(
            err.kind,
            ErrorKind::UnsupportedTransferSyntax { .. }
        ));
    }

    #[test]
    #[cfg(feature = "codec-h264")]
    fn read_dataset_accepts_h264_transfer_syntax_when_enabled() {
        // REQ-TS-205: H.264 transfer syntax accepted when feature enabled.
        let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
        let bytes = build_p10(super::TS_H264_HP41, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let dataset = reader.read_dataset().expect("dataset");
        assert!(dataset.get(Tag(0x7FE0, 0x0010)).is_some());
    }

    #[test]
    #[cfg(not(feature = "codec-hevc"))]
    fn read_dataset_rejects_hevc_transfer_syntax_without_feature() {
        // REQ-TS-205: HEVC transfer syntax requires feature gate.
        let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
        let bytes = build_p10(super::TS_HEVC_MP51, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(
            err.kind,
            ErrorKind::UnsupportedTransferSyntax { .. }
        ));
    }

    #[test]
    #[cfg(feature = "codec-hevc")]
    fn read_dataset_accepts_hevc_transfer_syntax_when_enabled() {
        // REQ-TS-205: HEVC transfer syntax accepted when feature enabled.
        let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
        let bytes = build_p10(super::TS_HEVC_MP51, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let dataset = reader.read_dataset().expect("dataset");
        assert!(dataset.get(Tag(0x7FE0, 0x0010)).is_some());
    }

    #[test]
    fn read_dataset_rejects_unsupported_sop_class() {
        // REQ-CONF-002: unsupported SOP Class is rejected.
        let dataset = minimal_sc_dataset_explicit("1.2.3.4.5.6.7");
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::UnsupportedSopClass { .. }));
    }

    #[test]
    fn read_dataset_rejects_deferred_enhanced_ct_sop_when_pack_disabled() {
        // REQ-SOP-301: deferred pack SOP classes must fail closed when pack is not enabled.
        if dicom_core::capabilities().pack_enhanced {
            return;
        }
        let dataset = minimal_sc_dataset_explicit(super::SOP_CLASS_ENHANCED_CT);
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::UnsupportedSopClass { .. }));
    }

    #[test]
    fn read_dataset_rejects_enhanced_mr_sop_when_pack_disabled() {
        // REQ-SOP-301: pack SOP classes must fail closed when pack is not enabled.
        if dicom_core::capabilities().pack_enhanced {
            return;
        }
        let dataset = minimal_sc_dataset_explicit(super::SOP_CLASS_ENHANCED_MR);
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::UnsupportedSopClass { .. }));
    }

    #[test]
    fn read_dataset_accepts_enhanced_ct_mr_sops_when_pack_enabled() {
        // REQ-CONF-084/REQ-SOP-301: enhanced SOP classes are accepted when pack-enhanced is enabled.
        if !dicom_core::capabilities().pack_enhanced {
            return;
        }
        assert_dataset_accepted(minimal_enhanced_dataset_explicit(
            super::SOP_CLASS_ENHANCED_CT,
        ));
        assert_dataset_accepted(minimal_enhanced_dataset_explicit(
            super::SOP_CLASS_ENHANCED_MR,
        ));
    }

    #[test]
    fn read_dataset_rejects_us_sops_when_pack_disabled() {
        // REQ-SOP-301: US SOP classes fail closed when pack-us is disabled.
        if dicom_core::capabilities().pack_us {
            return;
        }
        for sop in [super::SOP_CLASS_US, super::SOP_CLASS_US_MF] {
            let dataset = minimal_sc_dataset_explicit(sop);
            let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
            let mut reader = P10Reader::new(BytesSource::new(bytes));
            let err = reader.read_dataset().expect_err("expected error");
            assert!(matches!(err.kind, ErrorKind::UnsupportedSopClass { .. }));
        }
    }

    #[test]
    fn read_dataset_accepts_us_sops_when_pack_enabled() {
        // REQ-CONF-085/REQ-SOP-301: US SOP classes are accepted when pack-us is enabled.
        if !dicom_core::capabilities().pack_us {
            return;
        }
        assert_dataset_accepted(minimal_sc_dataset_explicit(super::SOP_CLASS_US));
        assert_dataset_accepted(minimal_sc_dataset_explicit(super::SOP_CLASS_US_MF));
    }

    #[test]
    fn read_dataset_rejects_nm_sop_when_pack_disabled() {
        // REQ-SOP-301: NM SOP class fails closed when pack-nm is disabled.
        if dicom_core::capabilities().pack_nm {
            return;
        }
        let dataset = minimal_sc_dataset_explicit(super::SOP_CLASS_NM);
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::UnsupportedSopClass { .. }));
    }

    #[test]
    fn read_dataset_accepts_nm_sop_when_pack_enabled() {
        // REQ-CONF-085/REQ-SOP-301: NM SOP class is accepted when pack-nm is enabled.
        if !dicom_core::capabilities().pack_nm {
            return;
        }
        assert_dataset_accepted(minimal_sc_dataset_explicit(super::SOP_CLASS_NM));
    }

    #[test]
    fn read_dataset_rejects_xa_xrf_sops_when_pack_disabled() {
        // REQ-SOP-301: XA/XRF SOP classes fail closed when pack-xa is disabled.
        if dicom_core::capabilities().pack_xa {
            return;
        }
        for sop in [super::SOP_CLASS_XA, super::SOP_CLASS_XRF] {
            let dataset = minimal_sc_dataset_explicit(sop);
            let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
            let mut reader = P10Reader::new(BytesSource::new(bytes));
            let err = reader.read_dataset().expect_err("expected error");
            assert!(matches!(err.kind, ErrorKind::UnsupportedSopClass { .. }));
        }
    }

    #[test]
    fn read_dataset_accepts_xa_xrf_sops_when_pack_enabled() {
        // REQ-CONF-085/REQ-SOP-301: XA/XRF SOP classes are accepted when pack-xa is enabled.
        if !dicom_core::capabilities().pack_xa {
            return;
        }
        assert_dataset_accepted(minimal_sc_dataset_explicit(super::SOP_CLASS_XA));
        assert_dataset_accepted(minimal_sc_dataset_explicit(super::SOP_CLASS_XRF));
    }

    #[test]
    fn read_dataset_rejects_deferred_pet_sop_when_feature_not_enabled() {
        // REQ-SOP-301: deferred modality SOP classes must fail closed when feature is not enabled.
        if dicom_core::capabilities().modality_pet {
            return;
        }
        let dataset = minimal_sc_dataset_explicit(super::SOP_CLASS_PET);
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::UnsupportedSopClass { .. }));
    }

    #[test]
    fn read_dataset_accepts_pet_sop_when_feature_enabled() {
        // REQ-CONF-002/REQ-SOP-301: promoted modality SOP classes are accepted when feature is enabled.
        if !dicom_core::capabilities().modality_pet {
            return;
        }
        let dataset = minimal_sc_dataset_explicit(super::SOP_CLASS_PET);
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let dataset = reader.read_dataset().expect("dataset");
        assert!(dataset.get(Tag(0x0008, 0x0016)).is_some());
    }

    #[test]
    fn read_dataset_rejects_cr_sop_when_feature_not_enabled() {
        // REQ-SOP-301: CR SOP class fails closed when modality-xr is disabled.
        if dicom_core::capabilities().modality_xr {
            return;
        }
        let dataset = minimal_sc_dataset_explicit(super::SOP_CLASS_CR);
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::UnsupportedSopClass { .. }));
    }

    #[test]
    fn read_dataset_rejects_dx_presentation_sop_when_feature_not_enabled() {
        // REQ-SOP-301: DX SOP class fails closed when modality-xr is disabled.
        if dicom_core::capabilities().modality_xr {
            return;
        }
        let dataset = minimal_sc_dataset_explicit(super::SOP_CLASS_DX_PRESENTATION);
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::UnsupportedSopClass { .. }));
    }

    #[test]
    fn read_dataset_accepts_cr_sop_when_feature_enabled() {
        // REQ-CONF-002/REQ-SOP-301: promoted CR SOP class is accepted when modality-xr is enabled.
        if !dicom_core::capabilities().modality_xr {
            return;
        }
        let dataset = minimal_sc_dataset_explicit(super::SOP_CLASS_CR);
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let dataset = reader.read_dataset().expect("dataset");
        assert!(dataset.get(Tag(0x0008, 0x0016)).is_some());
    }

    #[test]
    fn read_dataset_accepts_dx_presentation_sop_when_feature_enabled() {
        // REQ-CONF-002/REQ-SOP-301: promoted DX SOP class is accepted when modality-xr is enabled.
        if !dicom_core::capabilities().modality_xr {
            return;
        }
        let dataset = minimal_sc_dataset_explicit(super::SOP_CLASS_DX_PRESENTATION);
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let dataset = reader.read_dataset().expect("dataset");
        assert!(dataset.get(Tag(0x0008, 0x0016)).is_some());
    }

    #[test]
    fn read_dataset_rejects_seg_sop_when_pack_disabled() {
        // REQ-SOP-301: SEG SOP class fails closed when pack-seg is disabled.
        if dicom_core::capabilities().pack_seg {
            return;
        }
        let dataset = minimal_sc_dataset_explicit(super::SOP_CLASS_SEG);
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::UnsupportedSopClass { .. }));
    }

    #[test]
    fn read_dataset_accepts_seg_sop_when_pack_enabled() {
        // REQ-CONF-086/REQ-SOP-301: SEG SOP class is accepted when pack-seg is enabled.
        if !dicom_core::capabilities().pack_seg {
            return;
        }
        assert_dataset_accepted(minimal_seg_dataset_explicit(None));
    }

    #[test]
    fn read_dataset_rejects_rt_sops_when_pack_disabled() {
        // REQ-SOP-301: RT SOP classes fail closed when pack-rt is disabled.
        if dicom_core::capabilities().pack_rt {
            return;
        }
        for sop in [
            super::SOP_CLASS_RT_DOSE,
            super::SOP_CLASS_RT_STRUCTURE,
            super::SOP_CLASS_RT_PLAN,
        ] {
            let dataset = minimal_sc_dataset_explicit(sop);
            let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
            let mut reader = P10Reader::new(BytesSource::new(bytes));
            let err = reader.read_dataset().expect_err("expected error");
            assert!(matches!(err.kind, ErrorKind::UnsupportedSopClass { .. }));
        }
    }

    #[test]
    fn read_dataset_accepts_rt_sops_when_pack_enabled() {
        // REQ-CONF-087/REQ-SOP-301: RT SOP classes are accepted when pack-rt is enabled.
        if !dicom_core::capabilities().pack_rt {
            return;
        }
        assert_dataset_accepted(minimal_rt_dose_dataset_explicit(None));
        assert_dataset_accepted(minimal_rt_structure_dataset_explicit(None));
        assert_dataset_accepted(minimal_rt_plan_dataset_explicit(None));
    }

    #[test]
    fn read_dataset_rejects_sr_sops_when_pack_disabled() {
        // REQ-SOP-301: SR SOP classes fail closed when pack-sr is disabled.
        if dicom_core::capabilities().pack_sr {
            return;
        }
        for sop in [
            super::SOP_CLASS_SR_BASIC_TEXT,
            super::SOP_CLASS_SR_COMPREHENSIVE,
        ] {
            let dataset = minimal_sc_dataset_explicit(sop);
            let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
            let mut reader = P10Reader::new(BytesSource::new(bytes));
            let err = reader.read_dataset().expect_err("expected error");
            assert!(matches!(err.kind, ErrorKind::UnsupportedSopClass { .. }));
        }
    }

    #[test]
    fn read_dataset_accepts_sr_sops_when_pack_enabled() {
        // REQ-CONF-088/REQ-SOP-301: SR SOP classes are accepted when pack-sr is enabled.
        if !dicom_core::capabilities().pack_sr {
            return;
        }
        assert_dataset_accepted(minimal_sc_dataset_explicit(super::SOP_CLASS_SR_BASIC_TEXT));
        assert_dataset_accepted(minimal_sc_dataset_explicit(
            super::SOP_CLASS_SR_COMPREHENSIVE,
        ));
    }

    #[test]
    fn read_dataset_rejects_gsps_sop_when_feature_disabled() {
        // REQ-SOP-301: GSPS SOP class fails closed when gsps feature is disabled.
        if dicom_core::capabilities().gsps {
            return;
        }
        let dataset = minimal_sc_dataset_explicit(super::SOP_CLASS_GSPS);
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::UnsupportedSopClass { .. }));
    }

    #[test]
    fn read_dataset_accepts_gsps_sop_when_feature_enabled() {
        // REQ-CONF-089/REQ-SOP-301: GSPS SOP class is accepted when gsps is enabled.
        if !dicom_core::capabilities().gsps {
            return;
        }
        assert_dataset_accepted(minimal_sc_dataset_explicit(super::SOP_CLASS_GSPS));
    }

    #[test]
    fn read_dataset_rejects_mammography_sops_even_when_modality_mg_enabled() {
        // REQ-CONF-002: MG SOP classes remain out-of-envelope in the IO conformance matrix and fail closed.
        if !dicom_core::capabilities().modality_mg {
            return;
        }
        for sop in [
            "1.2.840.10008.5.1.4.1.1.1.2",
            "1.2.840.10008.5.1.4.1.1.1.2.1",
        ] {
            let dataset = minimal_sc_dataset_explicit(sop);
            let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
            let mut reader = P10Reader::new(BytesSource::new(bytes));
            let err = reader.read_dataset().expect_err("expected error");
            assert!(matches!(err.kind, ErrorKind::UnsupportedSopClass { .. }));
        }
    }

    #[test]
    fn read_dataset_rejects_missing_required_tag() {
        // REQ-CONF-020: missing required tags fail closed.
        let dataset = minimal_sc_dataset_explicit_missing_study_uid("1.2.840.10008.5.1.4.1.1.7");
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::MissingRequiredTag { .. }));
    }

    #[test]
    fn read_dataset_rejects_rt_dose_missing_geometry() {
        // REQ-CONF-087
        if !dicom_core::capabilities().pack_rt {
            return;
        }
        let dataset = minimal_rt_dose_dataset_explicit(Some(Tag(0x0020, 0x0037)));
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::MissingRequiredTag { .. }));
    }

    #[test]
    fn read_dataset_rejects_rt_structure_missing_contours() {
        // REQ-CONF-003
        if !dicom_core::capabilities().pack_rt {
            return;
        }
        let dataset = minimal_rt_structure_dataset_explicit(Some(Tag(0x3006, 0x0039)));
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::MissingRequiredTag { .. }));
    }

    #[test]
    fn read_dataset_rejects_rt_plan_missing_reference() {
        // REQ-CONF-003
        if !dicom_core::capabilities().pack_rt {
            return;
        }
        let dataset = minimal_rt_plan_dataset_explicit(Some(Tag(0x300C, 0x0060)));
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::MissingRequiredTag { .. }));
    }

    #[test]
    fn read_dataset_rejects_seg_missing_frame_of_reference() {
        // REQ-CONF-086
        if !dicom_core::capabilities().pack_seg {
            return;
        }
        let dataset = minimal_seg_dataset_explicit(Some(Tag(0x0020, 0x0052)));
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::MissingRequiredTag { .. }));
    }

    #[test]
    fn read_dataset_rejects_float_pixel_data() {
        // REQ-CONF-070: Float Pixel Data is out-of-envelope by default.
        let mut dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x7FE0, 0x0008),
            *b"OF",
            &[0u8, 1u8, 2u8, 3u8],
        ));
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::InvalidTagValue { .. }));
    }

    #[test]
    fn file_source_rejects_symlink() {
        // REQ-SEC-426..428: symlinks are rejected by default.
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let target_path = tmp_path("dicom-io-target");
            let link_path = tmp_path("dicom-io-link");
            fs::write(&target_path, b"test").expect("write target");
            symlink(&target_path, &link_path).expect("symlink");
            let mut reader = P10Reader::new(FileSource::new(&link_path));
            let err = reader.read_meta().expect_err("expected error");
            assert!(matches!(err.kind, ErrorKind::IoError { .. }));
            let _ = fs::remove_file(&link_path);
            let _ = fs::remove_file(&target_path);
        }
    }
}
