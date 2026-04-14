#![deny(missing_docs)]

//! Core DICOM types and shared error model.
//!
//! Note: `dicom-core` currently relies on `std` for error traits and owned
//! collections, so `no_std` support is not yet available (REQ-API-203).

use std::fmt;

const KIB: u64 = 1024;
const MIB: u64 = 1024 * KIB;
const GIB: u64 = 1024 * MIB;

/// A DICOM tag (group, element).
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tag(pub u16, pub u16);

impl Tag {
    /// Create a new tag from group and element.
    pub const fn new(group: u16, element: u16) -> Self {
        Self(group, element)
    }

    /// Return the tag as a packed `u32` (group << 16 | element).
    pub const fn as_u32(self) -> u32 {
        ((self.0 as u32) << 16) | (self.1 as u32)
    }

    /// Create a tag from a packed `u32` (group << 16 | element).
    pub const fn from_u32(value: u32) -> Self {
        let group = (value >> 16) as u16;
        let element = (value & 0xFFFF) as u16;
        Self(group, element)
    }

    /// Return true if this is a private tag (odd group).
    pub const fn is_private(self) -> bool {
        (self.0 % 2) == 1
    }

    /// Parse a tag from `gggg,eeee`, `ggggeeee`, or `(gggg,eeee)` hex formats.
    pub fn parse_str(input: &str) -> Result<Self> {
        if input.chars().any(|ch| ch.is_whitespace()) {
            return Err(invalid_tag_value(
                Tag(0x0000, 0x0000),
                "tag contains whitespace",
            ));
        }

        let mut raw = input;
        if raw.starts_with('(') && raw.ends_with(')') && raw.len() > 2 {
            raw = &raw[1..raw.len() - 1];
        }

        let (group_str, element_str) = if let Some((group, element)) = raw.split_once(',') {
            (group, element)
        } else if raw.len() == 8 {
            raw.split_at(4)
        } else {
            return Err(invalid_tag_value(
                Tag(0x0000, 0x0000),
                "tag must be gggg,eeee or ggggeeee",
            ));
        };

        if group_str.len() != 4 || element_str.len() != 4 {
            return Err(invalid_tag_value(
                Tag(0x0000, 0x0000),
                "tag group/element must be 4 hex digits",
            ));
        }

        let group = u16::from_str_radix(group_str, 16)
            .map_err(|_| invalid_tag_value(Tag(0x0000, 0x0000), "tag group is not valid hex"))?;
        let element = u16::from_str_radix(element_str, 16)
            .map_err(|_| invalid_tag_value(Tag(0x0000, 0x0000), "tag element is not valid hex"))?;

        Ok(Tag(group, element))
    }
}

/// Value representation (VR) identifiers.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Vr {
    /// Application Entity.
    Ae,
    /// Age String.
    As,
    /// Attribute Tag.
    At,
    /// Code String.
    Cs,
    /// Date.
    Da,
    /// Decimal String.
    Ds,
    /// Floating Point Double.
    Fd,
    /// Floating Point Single.
    Fl,
    /// Integer String.
    Is,
    /// Long String.
    Lo,
    /// Long Text.
    Lt,
    /// Other Byte.
    Ob,
    /// Other Double.
    Od,
    /// Other Float.
    Of,
    /// Other Word.
    Ow,
    /// Person Name.
    Pn,
    /// Short String.
    Sh,
    /// Signed Long.
    Sl,
    /// Sequence of Items.
    Sq,
    /// Signed Short.
    Ss,
    /// Short Text.
    St,
    /// Time.
    Tm,
    /// Unique Identifier.
    Ui,
    /// Unsigned Long.
    Ul,
    /// Unknown.
    Un,
    /// Unsigned Short.
    Us,
    /// Unlimited Text.
    Ut,
    /// Other / custom VR.
    Other([u8; 2]),
}

impl Vr {
    /// Return the VR as a two-byte ASCII code.
    pub const fn as_bytes(self) -> [u8; 2] {
        match self {
            Vr::Ae => *b"AE",
            Vr::As => *b"AS",
            Vr::At => *b"AT",
            Vr::Cs => *b"CS",
            Vr::Da => *b"DA",
            Vr::Ds => *b"DS",
            Vr::Fd => *b"FD",
            Vr::Fl => *b"FL",
            Vr::Is => *b"IS",
            Vr::Lo => *b"LO",
            Vr::Lt => *b"LT",
            Vr::Ob => *b"OB",
            Vr::Od => *b"OD",
            Vr::Of => *b"OF",
            Vr::Ow => *b"OW",
            Vr::Pn => *b"PN",
            Vr::Sh => *b"SH",
            Vr::Sl => *b"SL",
            Vr::Sq => *b"SQ",
            Vr::Ss => *b"SS",
            Vr::St => *b"ST",
            Vr::Tm => *b"TM",
            Vr::Ui => *b"UI",
            Vr::Ul => *b"UL",
            Vr::Un => *b"UN",
            Vr::Us => *b"US",
            Vr::Ut => *b"UT",
            Vr::Other(bytes) => bytes,
        }
    }

    /// Parse a VR from two bytes, returning a strict error on invalid data.
    pub fn parse_for_tag(bytes: [u8; 2], tag: Tag) -> Result<Self> {
        match bytes {
            [b'A', b'E'] => Ok(Vr::Ae),
            [b'A', b'S'] => Ok(Vr::As),
            [b'A', b'T'] => Ok(Vr::At),
            [b'C', b'S'] => Ok(Vr::Cs),
            [b'D', b'A'] => Ok(Vr::Da),
            [b'D', b'S'] => Ok(Vr::Ds),
            [b'F', b'D'] => Ok(Vr::Fd),
            [b'F', b'L'] => Ok(Vr::Fl),
            [b'I', b'S'] => Ok(Vr::Is),
            [b'L', b'O'] => Ok(Vr::Lo),
            [b'L', b'T'] => Ok(Vr::Lt),
            [b'O', b'B'] => Ok(Vr::Ob),
            [b'O', b'D'] => Ok(Vr::Od),
            [b'O', b'F'] => Ok(Vr::Of),
            [b'O', b'W'] => Ok(Vr::Ow),
            [b'P', b'N'] => Ok(Vr::Pn),
            [b'S', b'H'] => Ok(Vr::Sh),
            [b'S', b'L'] => Ok(Vr::Sl),
            [b'S', b'Q'] => Ok(Vr::Sq),
            [b'S', b'S'] => Ok(Vr::Ss),
            [b'S', b'T'] => Ok(Vr::St),
            [b'T', b'M'] => Ok(Vr::Tm),
            [b'U', b'I'] => Ok(Vr::Ui),
            [b'U', b'L'] => Ok(Vr::Ul),
            [b'U', b'N'] => Ok(Vr::Un),
            [b'U', b'S'] => Ok(Vr::Us),
            [b'U', b'T'] => Ok(Vr::Ut),
            bytes if is_ascii_upper(bytes[0]) && is_ascii_upper(bytes[1]) => Ok(Vr::Other(bytes)),
            _ => Err(invalid_tag_value(
                tag,
                "VR must be two uppercase ASCII letters",
            )),
        }
    }
}

/// A DICOM element value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// No value.
    Empty,
    /// String value.
    Str(String),
    /// 32-bit integer value.
    I32(i32),
    /// 64-bit float value.
    F64(f64),
    /// UID value.
    Uid(String),
    /// Raw bytes.
    Bytes(Vec<u8>),
    /// Sequence of datasets.
    Sequence(Vec<Dataset>),
}

/// A DICOM element.
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    /// Element tag.
    pub tag: Tag,
    /// Value representation.
    pub vr: Vr,
    /// Element value.
    pub value: Value,
}

/// A DICOM dataset.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Dataset {
    elements: Vec<Element>,
}

impl Dataset {
    /// Create an empty dataset.
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
        }
    }

    /// Insert an element into the dataset.
    pub fn insert(&mut self, element: Element) {
        self.elements.push(element);
    }

    /// Get a reference to an element by tag.
    pub fn get(&self, tag: Tag) -> Option<&Element> {
        self.elements.iter().find(|el| el.tag == tag)
    }

    /// Get a string value by tag.
    pub fn get_str(&self, tag: Tag) -> Option<&str> {
        match self.get(tag) {
            Some(Element {
                value: Value::Str(value),
                ..
            }) => Some(value.as_str()),
            _ => None,
        }
    }

    /// Get a 32-bit integer value by tag.
    pub fn get_i32(&self, tag: Tag) -> Option<i32> {
        match self.get(tag) {
            Some(Element {
                value: Value::I32(value),
                ..
            }) => Some(*value),
            _ => None,
        }
    }

    /// Get a 64-bit float value by tag.
    pub fn get_f64(&self, tag: Tag) -> Option<f64> {
        match self.get(tag) {
            Some(Element {
                value: Value::F64(value),
                ..
            }) => Some(*value),
            _ => None,
        }
    }

    /// Get a UID value by tag.
    pub fn get_uid(&self, tag: Tag) -> Option<&str> {
        match self.get(tag) {
            Some(Element {
                value: Value::Uid(value),
                ..
            }) => Some(value.as_str()),
            _ => None,
        }
    }

    /// Return the number of elements in the dataset.
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Return true if the dataset has no elements.
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Return a read-only view of the dataset elements.
    pub fn elements(&self) -> &[Element] {
        &self.elements
    }

    /// Insert an element enforcing dataset and value limits.
    pub fn insert_checked(&mut self, element: Element, limits: &Limits) -> Result<()> {
        enforce_limit(
            "max_dataset_elements",
            (self.elements.len() as u64) + 1,
            limits.max_dataset_elements,
        )?;

        match &element.value {
            Value::Str(value) => {
                enforce_limit(
                    "max_string_bytes",
                    value.len() as u64,
                    limits.max_string_bytes,
                )?;
            }
            Value::Uid(value) => {
                enforce_limit(
                    "max_string_bytes",
                    value.len() as u64,
                    limits.max_string_bytes,
                )?;
                validate_uid_strict(element.tag, value)?;
            }
            Value::Bytes(bytes) => {
                enforce_limit(
                    "max_element_vl_bytes",
                    bytes.len() as u64,
                    limits.max_element_vl_bytes,
                )?;
            }
            _ => {}
        }

        self.elements.push(element);
        Ok(())
    }

    /// Get a string value by tag, enforcing limits.
    pub fn get_str_strict(&self, tag: Tag, limits: &Limits) -> Result<Option<&str>> {
        match self.get(tag) {
            None => Ok(None),
            Some(Element {
                value: Value::Str(value),
                ..
            }) => {
                enforce_limit(
                    "max_string_bytes",
                    value.len() as u64,
                    limits.max_string_bytes,
                )?;
                Ok(Some(value.as_str()))
            }
            Some(_) => Err(invalid_tag_value(tag, "expected string value")),
        }
    }

    /// Get a 32-bit integer value by tag, parsing strings strictly.
    pub fn get_i32_strict(&self, tag: Tag, limits: &Limits) -> Result<Option<i32>> {
        match self.get(tag) {
            None => Ok(None),
            Some(Element {
                value: Value::I32(value),
                ..
            }) => Ok(Some(*value)),
            Some(Element {
                value: Value::Str(value),
                ..
            }) => {
                enforce_limit(
                    "max_string_bytes",
                    value.len() as u64,
                    limits.max_string_bytes,
                )?;
                parse_i32_strict(tag, value).map(Some)
            }
            Some(_) => Err(invalid_tag_value(tag, "expected integer value")),
        }
    }

    /// Get a 64-bit float value by tag, parsing strings strictly.
    pub fn get_f64_strict(&self, tag: Tag, limits: &Limits) -> Result<Option<f64>> {
        match self.get(tag) {
            None => Ok(None),
            Some(Element {
                value: Value::F64(value),
                ..
            }) => Ok(Some(*value)),
            Some(Element {
                value: Value::Str(value),
                ..
            }) => {
                enforce_limit(
                    "max_string_bytes",
                    value.len() as u64,
                    limits.max_string_bytes,
                )?;
                parse_f64_strict(tag, value).map(Some)
            }
            Some(_) => Err(invalid_tag_value(tag, "expected float value")),
        }
    }

    /// Get a UID value by tag, enforcing strict UID formatting.
    pub fn get_uid_strict(&self, tag: Tag, limits: &Limits) -> Result<Option<&str>> {
        match self.get(tag) {
            None => Ok(None),
            Some(Element {
                value: Value::Uid(value),
                ..
            }) => {
                enforce_limit(
                    "max_string_bytes",
                    value.len() as u64,
                    limits.max_string_bytes,
                )?;
                validate_uid_strict(tag, value)?;
                Ok(Some(value.as_str()))
            }
            Some(Element {
                value: Value::Str(value),
                ..
            }) => {
                enforce_limit(
                    "max_string_bytes",
                    value.len() as u64,
                    limits.max_string_bytes,
                )?;
                validate_uid_strict(tag, value)?;
                Ok(Some(value.as_str()))
            }
            Some(_) => Err(invalid_tag_value(tag, "expected UID value")),
        }
    }
}

/// Configurable security and resource limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// Max total input bytes.
    pub max_input_bytes: u64,
    /// Max number of decoded elements.
    pub max_dataset_elements: u64,
    /// Max sequence/item nesting depth.
    pub max_sequence_depth: u64,
    /// Max bytes in a single string value.
    pub max_string_bytes: u64,
    /// Max bytes in a single element value length.
    pub max_element_vl_bytes: u64,
    /// Max frames per instance.
    pub max_frames_per_instance: u64,
    /// Max pixels per frame.
    pub max_pixels_per_frame: u64,
    /// Max decompressed bytes per instance.
    pub max_decompressed_bytes: u64,
    /// Max total cached GPU texture bytes.
    pub max_gpu_texture_bytes: u64,
    /// Max total cached CPU bytes.
    pub max_cache_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_input_bytes: 512 * MIB,
            max_dataset_elements: 250_000,
            max_sequence_depth: 64,
            max_string_bytes: MIB,
            max_element_vl_bytes: 64 * MIB,
            max_frames_per_instance: 4_096,
            max_pixels_per_frame: 16_777_216,
            max_decompressed_bytes: GIB,
            max_gpu_texture_bytes: 512 * MIB,
            max_cache_bytes: GIB,
        }
    }
}

/// Enforce a numeric limit, returning `LimitExceeded` on violation.
pub fn enforce_limit(limit_name: &'static str, observed: u64, allowed: u64) -> Result<()> {
    if observed > allowed {
        return Err(limit_exceeded(limit_name, observed, allowed));
    }
    Ok(())
}

/// Validate a dataset against core limits (elements, strings, bytes, and depth).
pub fn validate_dataset(dataset: &Dataset, limits: &Limits) -> Result<()> {
    let mut count = 0u64;
    validate_dataset_inner(dataset, limits, 0, &mut count)
}

/// A structured error context item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextItem {
    /// Context key.
    pub key: &'static str,
    /// Context value.
    pub value: String,
}

/// Feature availability flags for the active build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    /// Tier 1 Deflated Explicit VR Little Endian support.
    pub tier1_deflate: bool,
    /// JPEG-LS codec support.
    pub codec_jpegls: bool,
    /// JPEG 2000 codec support.
    pub codec_j2k: bool,
    /// Non-DICOM raster input support.
    pub raster_io: bool,
    /// Grayscale Softcopy Presentation State support.
    pub gsps: bool,
    /// CT modality pack support.
    pub modality_ct: bool,
    /// PET modality pack support.
    pub modality_pet: bool,
    /// MG modality pack support.
    pub modality_mg: bool,
    /// XR modality pack support.
    pub modality_xr: bool,
    /// Enhanced multi-frame pack support.
    pub pack_enhanced: bool,
    /// Ultrasound pack support.
    pub pack_us: bool,
    /// Nuclear medicine pack support.
    pub pack_nm: bool,
    /// XA/XRF pack support.
    pub pack_xa: bool,
    /// Segmentation pack support.
    pub pack_seg: bool,
    /// RT Dose pack support.
    pub pack_rt: bool,
    /// Structured Report pack support.
    pub pack_sr: bool,
}

impl Capabilities {
    /// Detect feature availability for this build.
    pub fn detect() -> Self {
        Self {
            tier1_deflate: cfg!(feature = "tier1-deflate"),
            codec_jpegls: cfg!(feature = "codec-jpegls"),
            codec_j2k: cfg!(feature = "codec-j2k"),
            raster_io: cfg!(feature = "raster-io"),
            gsps: cfg!(feature = "gsps"),
            modality_ct: cfg!(feature = "modality-ct"),
            modality_pet: cfg!(feature = "modality-pet"),
            modality_mg: cfg!(feature = "modality-mg"),
            modality_xr: cfg!(feature = "modality-xr"),
            pack_enhanced: cfg!(feature = "pack-enhanced"),
            pack_us: cfg!(feature = "pack-us"),
            pack_nm: cfg!(feature = "pack-nm"),
            pack_xa: cfg!(feature = "pack-xa"),
            pack_seg: cfg!(feature = "pack-seg"),
            pack_rt: cfg!(feature = "pack-rt"),
            pack_sr: cfg!(feature = "pack-sr"),
        }
    }

    /// Return deterministic key/value rows for capability-reporting surfaces.
    pub fn report_rows(&self) -> [(&'static str, bool); 16] {
        [
            ("capabilities.tier1_deflate", self.tier1_deflate),
            ("capabilities.codec_jpegls", self.codec_jpegls),
            ("capabilities.codec_j2k", self.codec_j2k),
            ("capabilities.raster_io", self.raster_io),
            ("capabilities.gsps", self.gsps),
            ("capabilities.modality_ct", self.modality_ct),
            ("capabilities.modality_pet", self.modality_pet),
            ("capabilities.modality_mg", self.modality_mg),
            ("capabilities.modality_xr", self.modality_xr),
            ("capabilities.pack_enhanced", self.pack_enhanced),
            ("capabilities.pack_us", self.pack_us),
            ("capabilities.pack_nm", self.pack_nm),
            ("capabilities.pack_xa", self.pack_xa),
            ("capabilities.pack_seg", self.pack_seg),
            ("capabilities.pack_rt", self.pack_rt),
            ("capabilities.pack_sr", self.pack_sr),
        ]
    }
}

/// Return the detected feature capabilities for this build.
pub fn capabilities() -> Capabilities {
    Capabilities::detect()
}

/// A shared error kind taxonomy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    /// Unsupported SOP Class UID.
    UnsupportedSopClass {
        /// SOP Class UID value.
        sop_class_uid: String,
    },
    /// Unsupported Transfer Syntax UID.
    UnsupportedTransferSyntax {
        /// Transfer Syntax UID value.
        transfer_syntax_uid: String,
    },
    /// Missing required tag.
    MissingRequiredTag {
        /// Missing tag identifier.
        tag: Tag,
    },
    /// Invalid tag value.
    InvalidTagValue {
        /// Tag with invalid value.
        tag: Tag,
        /// Human-readable detail.
        detail: String,
    },
    /// Invalid geometry or spatial relationships.
    InvalidGeometry {
        /// Human-readable detail.
        detail: String,
    },
    /// Invalid pixel transform state.
    InvalidPixelTransform {
        /// Stage name or identifier.
        stage: String,
        /// Human-readable detail.
        detail: String,
    },
    /// Decoder failure.
    DecodeError {
        /// Stage name or identifier.
        stage: String,
        /// Human-readable detail.
        detail: String,
    },
    /// Limit exceeded.
    LimitExceeded {
        /// Limit name.
        limit_name: &'static str,
        /// Observed value.
        observed: u64,
        /// Allowed value.
        allowed: u64,
    },
    /// IO error (native only).
    IoError {
        /// Human-readable detail.
        detail: String,
    },
    /// Integrity error (hash mismatch, UID conflict).
    IntegrityError {
        /// Human-readable detail.
        detail: String,
    },
    /// Internal error indicating a bug.
    InternalError {
        /// Human-readable detail.
        detail: String,
    },
}

impl ErrorKind {
    /// Return the canonical error code for this kind.
    pub fn code(&self) -> &'static str {
        match self {
            ErrorKind::UnsupportedSopClass { .. } => "DVF.DICOM.UNSUPPORTED_SOP",
            ErrorKind::UnsupportedTransferSyntax { .. } => "DVF.DICOM.UNSUPPORTED_TS",
            ErrorKind::MissingRequiredTag { .. } => "DVF.DICOM.MISSING_TAG",
            ErrorKind::InvalidTagValue { .. } => "DVF.DICOM.INVALID_TAG_VALUE",
            ErrorKind::InvalidGeometry { .. } => "DVF.GEOM.INVALID",
            ErrorKind::InvalidPixelTransform { .. } => "DVF.PIXEL.INVALID_TRANSFORM",
            ErrorKind::DecodeError { .. } => "DVF.DICOM.DECODE_ERROR",
            ErrorKind::LimitExceeded { .. } => "DVF.SECURITY.LIMIT_EXCEEDED",
            ErrorKind::IoError { .. } => "DVF.IO.ERROR",
            ErrorKind::IntegrityError { .. } => "DVF.INTEGRITY.ERROR",
            ErrorKind::InternalError { .. } => "DVF.INTERNAL.ERROR",
        }
    }
}

/// Shared error type.
#[derive(Debug)]
pub struct Error {
    /// Stable error code.
    pub code: &'static str,
    /// Error kind.
    pub kind: ErrorKind,
    /// Short, non-PII message.
    pub message: String,
    /// Structured context items.
    pub context: Vec<ContextItem>,
    /// Optional source error.
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl Error {
    /// Create a new error.
    pub fn new(code: &'static str, kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            code,
            kind,
            message: message.into(),
            context: Vec::new(),
            source: None,
        }
    }

    /// Create a new error using the canonical code for the kind.
    pub fn from_kind(kind: ErrorKind, message: impl Into<String>) -> Self {
        let code = kind.code();
        Self::new(code, kind, message)
    }

    /// Attach a context item to the error.
    pub fn with_context(mut self, key: &'static str, value: impl Into<String>) -> Self {
        self.context.push(ContextItem {
            key,
            value: value.into(),
        });
        self
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_deref()
            .map(|err| err as &(dyn std::error::Error + 'static))
    }
}

/// Result alias for shared error type.
pub type Result<T> = std::result::Result<T, Box<Error>>;

fn validate_dataset_inner(
    dataset: &Dataset,
    limits: &Limits,
    depth: u64,
    count: &mut u64,
) -> Result<()> {
    enforce_limit("max_sequence_depth", depth, limits.max_sequence_depth)?;

    for element in &dataset.elements {
        *count += 1;
        enforce_limit("max_dataset_elements", *count, limits.max_dataset_elements)?;

        match &element.value {
            Value::Str(value) => {
                enforce_limit(
                    "max_string_bytes",
                    value.len() as u64,
                    limits.max_string_bytes,
                )?;
            }
            Value::Uid(value) => {
                enforce_limit(
                    "max_string_bytes",
                    value.len() as u64,
                    limits.max_string_bytes,
                )?;
                validate_uid_strict(element.tag, value)?;
            }
            Value::Bytes(bytes) => {
                enforce_limit(
                    "max_element_vl_bytes",
                    bytes.len() as u64,
                    limits.max_element_vl_bytes,
                )?;
            }
            Value::Sequence(items) => {
                for item in items {
                    validate_dataset_inner(item, limits, depth + 1, count)?;
                }
            }
            _ => {}
        }
    }

    Ok(())
}

/// Parse a strict 32-bit integer for a specific tag.
pub fn parse_i32_strict(tag: Tag, input: &str) -> Result<i32> {
    if has_whitespace(input) {
        return Err(invalid_tag_value(tag, "integer contains whitespace"));
    }
    if input.starts_with('+') || input.is_empty() {
        return Err(invalid_tag_value(tag, "integer format is invalid"));
    }
    if !input.chars().all(|ch| ch.is_ascii_digit() || ch == '-')
        || input.matches('-').count() > 1
        || input.ends_with('-')
        || (input.starts_with('-') && input.len() == 1)
    {
        return Err(invalid_tag_value(tag, "integer format is invalid"));
    }
    input
        .parse::<i32>()
        .map_err(|_| invalid_tag_value(tag, "integer out of range"))
}

/// Parse a strict 64-bit float for a specific tag.
pub fn parse_f64_strict(tag: Tag, input: &str) -> Result<f64> {
    if has_whitespace(input) {
        return Err(invalid_tag_value(tag, "float contains whitespace"));
    }
    if input.starts_with('+') || input.is_empty() {
        return Err(invalid_tag_value(tag, "float format is invalid"));
    }
    if !input
        .chars()
        .all(|ch| ch.is_ascii_digit() || matches!(ch, '-' | '+' | '.' | 'e' | 'E'))
    {
        return Err(invalid_tag_value(tag, "float format is invalid"));
    }

    let value = input
        .parse::<f64>()
        .map_err(|_| invalid_tag_value(tag, "float format is invalid"))?;
    if !value.is_finite() {
        return Err(invalid_tag_value(tag, "float must be finite"));
    }
    Ok(value)
}

/// Validate a UID string for a specific tag.
pub fn validate_uid_strict(tag: Tag, input: &str) -> Result<()> {
    if has_whitespace(input) {
        return Err(invalid_tag_value(tag, "UID contains whitespace"));
    }
    if input.is_empty() || input.len() > 64 {
        return Err(invalid_tag_value(tag, "UID length is invalid"));
    }
    if input.starts_with('.') || input.ends_with('.') {
        return Err(invalid_tag_value(tag, "UID must not start or end with '.'"));
    }
    let mut prev_dot = false;
    for ch in input.chars() {
        match ch {
            '0'..='9' => prev_dot = false,
            '.' => {
                if prev_dot {
                    return Err(invalid_tag_value(tag, "UID contains empty component"));
                }
                prev_dot = true;
            }
            _ => {
                return Err(invalid_tag_value(tag, "UID contains invalid characters"));
            }
        }
    }
    Ok(())
}

fn invalid_tag_value(tag: Tag, detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::InvalidTagValue {
            tag,
            detail: detail.into(),
        },
        "invalid tag value",
    )
    .into()
}

fn limit_exceeded(limit_name: &'static str, observed: u64, allowed: u64) -> Box<Error> {
    Error::from_kind(
        ErrorKind::LimitExceeded {
            limit_name,
            observed,
            allowed,
        },
        "limit exceeded",
    )
    .into()
}

fn has_whitespace(input: &str) -> bool {
    input.chars().any(|ch| ch.is_whitespace())
}

fn is_ascii_upper(byte: u8) -> bool {
    byte.is_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use super::{
        enforce_limit, parse_f64_strict, parse_i32_strict, validate_dataset, validate_uid_strict,
        Dataset, Element, Error, ErrorKind, Limits, Tag, Value, Vr,
    };

    #[test]
    fn default_limits_match_spec() {
        // REQ-SEC-401: default limits are explicitly defined.
        let limits = Limits::default();

        assert_eq!(limits.max_input_bytes, 512 * 1024 * 1024);
        assert_eq!(limits.max_dataset_elements, 250_000);
        assert_eq!(limits.max_sequence_depth, 64);
        assert_eq!(limits.max_string_bytes, super::MIB);
        assert_eq!(limits.max_element_vl_bytes, 64 * super::MIB);
        assert_eq!(limits.max_frames_per_instance, 4_096);
        assert_eq!(limits.max_pixels_per_frame, 16_777_216);
        assert_eq!(limits.max_decompressed_bytes, super::GIB);
        assert_eq!(limits.max_gpu_texture_bytes, 512 * super::MIB);
        assert_eq!(limits.max_cache_bytes, super::GIB);
    }

    #[test]
    fn error_codes_match_required_kinds() {
        // REQ-ERR-030: required error kinds map to stable codes.
        let unsupported_sop = ErrorKind::UnsupportedSopClass {
            sop_class_uid: "1.2.3".to_string(),
        };
        let unsupported_ts = ErrorKind::UnsupportedTransferSyntax {
            transfer_syntax_uid: "1.2.840.10008.1.2".to_string(),
        };
        let missing_tag = ErrorKind::MissingRequiredTag {
            tag: Tag(0x0028, 0x0010),
        };
        let limit_exceeded = ErrorKind::LimitExceeded {
            limit_name: "max_input_bytes",
            observed: 1,
            allowed: 0,
        };
        let invalid_geometry = ErrorKind::InvalidGeometry {
            detail: "bad spacing".to_string(),
        };

        assert_eq!(unsupported_sop.code(), "DVF.DICOM.UNSUPPORTED_SOP");
        assert_eq!(unsupported_ts.code(), "DVF.DICOM.UNSUPPORTED_TS");
        assert_eq!(missing_tag.code(), "DVF.DICOM.MISSING_TAG");
        assert_eq!(limit_exceeded.code(), "DVF.SECURITY.LIMIT_EXCEEDED");
        assert_eq!(invalid_geometry.code(), "DVF.GEOM.INVALID");

        let err = Error::from_kind(missing_tag, "missing tag");
        assert_eq!(err.code, "DVF.DICOM.MISSING_TAG");
    }

    #[test]
    fn tag_parsing_accepts_known_formats() {
        // REQ-API-202: parsing helpers are portable and deterministic.
        let tag = Tag::parse_str("0010,0010").expect("parse tag");
        assert_eq!(tag, Tag(0x0010, 0x0010));

        let packed = Tag::parse_str("7FE00010").expect("parse packed tag");
        assert_eq!(packed, Tag(0x7FE0, 0x0010));

        let wrapped = Tag::parse_str("(0028,0010)").expect("parse wrapped tag");
        assert_eq!(wrapped, Tag(0x0028, 0x0010));
    }

    #[test]
    fn tag_parsing_rejects_invalid_formats() {
        // REQ-ERR-008: invalid tag strings must yield structured errors.
        assert!(Tag::parse_str("0010,010").is_err());
        assert!(Tag::parse_str("0010 0010").is_err());
        assert!(Tag::parse_str("ZZZZ,0010").is_err());
    }

    #[test]
    fn vr_parse_accepts_known_and_custom() {
        // REQ-ERR-002: VR enum must cover required variants and parsing.
        let tag = Tag(0x0010, 0x0010);
        assert_eq!(Vr::parse_for_tag(*b"PN", tag).unwrap(), Vr::Pn);
        assert_eq!(Vr::parse_for_tag(*b"ZZ", tag).unwrap(), Vr::Other(*b"ZZ"));
        assert!(Vr::parse_for_tag(*b"pN", tag).is_err());
    }

    #[test]
    fn parse_numbers_strictly() {
        // REQ-ERR-008: invalid numeric formats must be rejected.
        let tag = Tag(0x0028, 0x1050);
        assert_eq!(parse_i32_strict(tag, "42").unwrap(), 42);
        assert_eq!(parse_i32_strict(tag, "-7").unwrap(), -7);
        assert!(parse_i32_strict(tag, " 1").is_err());
        assert!(parse_i32_strict(tag, "+1").is_err());

        assert_eq!(parse_f64_strict(tag, "1.25").unwrap(), 1.25);
        assert_eq!(parse_f64_strict(tag, "-1e-3").unwrap(), -1e-3);
        assert!(parse_f64_strict(tag, "NaN").is_err());
        assert!(parse_f64_strict(tag, " 1.0").is_err());
    }

    #[test]
    fn uid_validation_is_strict() {
        // REQ-ERR-008: invalid UID formats must be rejected.
        let tag = Tag(0x0008, 0x0018);
        assert!(validate_uid_strict(tag, "1.2.840.10008.1.2").is_ok());
        assert!(validate_uid_strict(tag, "").is_err());
        assert!(validate_uid_strict(tag, "1..2").is_err());
        assert!(validate_uid_strict(tag, "1.2.3.").is_err());
        assert!(validate_uid_strict(tag, "1.2.3a").is_err());
    }

    #[test]
    fn enforce_limits_returns_limit_exceeded() {
        // REQ-SEC-404: enforce limits before allocations.
        let err = enforce_limit("max_input_bytes", 10, 5).unwrap_err();
        assert_eq!(err.code, "DVF.SECURITY.LIMIT_EXCEEDED");
        assert!(matches!(
            err.kind,
            ErrorKind::LimitExceeded {
                limit_name: "max_input_bytes",
                observed: 10,
                allowed: 5
            }
        ));
    }

    #[test]
    fn dataset_validation_enforces_limits() {
        // REQ-SEC-401..404: limit enforcement covers datasets, strings, and bytes.
        let mut dataset = Dataset::new();
        let tag = Tag(0x0010, 0x0010);
        dataset.insert(Element {
            tag,
            vr: Vr::Pn,
            value: Value::Str("abc".to_string()),
        });

        let limits = Limits {
            max_dataset_elements: 1,
            max_string_bytes: 2,
            ..Limits::default()
        };
        assert!(validate_dataset(&dataset, &limits).is_err());
    }

    #[test]
    fn dataset_strict_getters_parse_and_validate() {
        // REQ-ERR-008: typed getters enforce strict parsing.
        let mut dataset = Dataset::new();
        let tag_int = Tag(0x0028, 0x0002);
        let tag_uid = Tag(0x0008, 0x0018);

        dataset.insert(Element {
            tag: tag_int,
            vr: Vr::Is,
            value: Value::Str("12".to_string()),
        });
        dataset.insert(Element {
            tag: tag_uid,
            vr: Vr::Ui,
            value: Value::Uid("1.2.840.10008.1.2".to_string()),
        });

        let limits = Limits::default();
        assert_eq!(dataset.get_i32_strict(tag_int, &limits).unwrap(), Some(12));
        assert_eq!(
            dataset.get_uid_strict(tag_uid, &limits).unwrap(),
            Some("1.2.840.10008.1.2")
        );
    }
}
