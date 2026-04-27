#![deny(missing_docs)]
#![deny(clippy::cast_possible_truncation)]

//! Core DICOM types and shared error model.
//!
//! Note: `dicom-core` currently relies on `std` for error traits and owned
//! collections, so `no_std` support is not yet available (REQ-API-203).

// Named constants for DICOM spec magic numbers (S13-T3).
pub mod constants;

// Re-export key constants from the constants module for ergonomic access.
pub use constants::{
    DICOM_PREAMBLE_LENGTH, EXPLICIT_VR_LE, IMPLICIT_VR_LE, MAX_AE_TITLE_LENGTH, MAX_PDU_LENGTH,
    MAX_UID_LENGTH,
};

// Re-export shared value types from the dicom-types crate so that downstream
// consumers can continue to import them from dicom-core.
pub use dicom_types::{PatientPosition, WindowLevel};

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

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
    pub fn as_u32(self) -> u32 {
        (u32::from(self.0) << 16) | u32::from(self.1)
    }

    /// Create a tag from a packed `u32` (group << 16 | element).
    pub const fn from_u32(value: u32) -> Self {
        let bytes = value.to_be_bytes();
        let group = u16::from_be_bytes([bytes[0], bytes[1]]);
        let element = u16::from_be_bytes([bytes[2], bytes[3]]);
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
    tag: Tag,
    /// Value representation.
    vr: Vr,
    /// Element value.
    value: Value,
}

impl Element {
    /// Create a new element, validating VR/value consistency.
    ///
    /// Returns an error if the VR and value are obviously inconsistent
    /// (e.g. `Vr::Sq` paired with a non-sequence value).
    pub fn new(tag: Tag, vr: Vr, value: Value) -> Result<Self> {
        Self::validate_vr_value(vr, &value)?;
        Ok(Self { tag, vr, value })
    }

    /// Return a reference to the element tag.
    pub fn tag(&self) -> &Tag {
        &self.tag
    }

    /// Return a reference to the value representation.
    pub fn vr(&self) -> &Vr {
        &self.vr
    }

    /// Return a reference to the element value.
    pub fn value(&self) -> &Value {
        &self.value
    }

    /// Consume the element and return its value.
    pub fn into_value(self) -> Value {
        self.value
    }

    /// Return the UID string if the value is a Uid variant.
    ///
    /// Returns `None` for non-UID values.
    pub fn as_uid(&self) -> Option<&str> {
        match &self.value {
            Value::Uid(uid) => Some(uid.as_str()),
            _ => None,
        }
    }

    /// Return the value as `f64` if the value is an I32 or F64 variant.
    ///
    /// I32 values are converted to f64 losslessly. Returns `None` for
    /// non-numeric values.
    pub fn as_f64(&self) -> Option<f64> {
        match &self.value {
            Value::I32(v) => Some(f64::from(*v)),
            Value::F64(v) => Some(*v),
            _ => None,
        }
    }

    /// Return the value as `i64` if the value is an I32 variant.
    ///
    /// Returns `None` for non-integer values.
    pub fn as_i64(&self) -> Option<i64> {
        match &self.value {
            Value::I32(v) => Some(i64::from(*v)),
            _ => None,
        }
    }

    /// Validate that the VR and value are consistent.
    fn validate_vr_value(vr: Vr, value: &Value) -> Result<()> {
        match (vr, value) {
            // SQ must be Sequence or Empty
            (Vr::Sq, Value::Sequence(_) | Value::Empty) => Ok(()),
            (Vr::Sq, _) => Err(invalid_tag_value(
                Tag(0x0000, 0x0000),
                "SQ VR requires Sequence or Empty value",
            )),
            // UI should be Uid, Str, or Empty
            (Vr::Ui, Value::Uid(_) | Value::Str(_) | Value::Empty) => Ok(()),
            (Vr::Ui, _) => Err(invalid_tag_value(
                Tag(0x0000, 0x0000),
                "UI VR requires Uid, Str, or Empty value",
            )),
            // Binary VRs should pair with Bytes or Empty
            (Vr::Ob | Vr::Ow | Vr::Od | Vr::Of | Vr::Un, Value::Bytes(_) | Value::Empty) => Ok(()),
            (Vr::Ob | Vr::Ow | Vr::Od | Vr::Of | Vr::Un, _) => Err(invalid_tag_value(
                Tag(0x0000, 0x0000),
                "Binary VR requires Bytes or Empty value",
            )),
            // Numeric VRs should pair with appropriate numeric types or Empty
            (Vr::Ss | Vr::Us | Vr::Sl | Vr::Ul, Value::I32(_) | Value::Empty) => Ok(()),
            (Vr::Fl | Vr::Fd, Value::F64(_) | Value::Empty) => Ok(()),
            // String VRs pair with Str, Uid (for UI handled above), or Empty
            (_, Value::Str(_) | Value::Empty) => Ok(()),
            // Allow remaining combinations (e.g. I32/F64 with non-numeric VR)
            _ => Ok(()),
        }
    }
}

/// A DICOM dataset.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Dataset {
    elements: BTreeMap<Tag, Element>,
}

impl Dataset {
    /// Create an empty dataset.
    pub fn new() -> Self {
        Self {
            elements: BTreeMap::new(),
        }
    }

    /// Insert an element into the dataset.
    ///
    /// If an element with the same tag already exists, it is replaced.
    pub fn insert(&mut self, element: Element) {
        self.elements.insert(element.tag, element);
    }

    /// Get a reference to an element by tag.
    pub fn get(&self, tag: Tag) -> Option<&Element> {
        self.elements.get(&tag)
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

    /// Return an iterator over the dataset elements in tag order.
    pub fn iter(&self) -> std::collections::btree_map::Values<'_, Tag, Element> {
        self.elements.values()
    }

    /// Require an element by tag, returning a reference or a `MissingRequiredTag` error.
    ///
    /// Unlike [`Dataset::get`], this method returns an error instead of `None`
    /// when the tag is absent, which is useful for mandatory DICOM attributes.
    pub fn require(&self, tag: Tag) -> Result<&Element> {
        self.get(tag).ok_or_else(|| {
            Box::new(Error::from_kind(
                ErrorKind::MissingRequiredTag { tag },
                "required DICOM tag is missing",
            ))
        })
    }

    /// Insert an element after validating VR/value consistency.
    ///
    /// This is a safer alternative to [`Dataset::insert`] that rejects elements
    /// with inconsistent VR/value pairs (e.g. `Vr::Sq` paired with a string value).
    pub fn insert_validated(&mut self, element: Element) -> Result<()> {
        Element::validate_vr_value(element.vr, element.value())?;
        self.elements.insert(element.tag, element);
        Ok(())
    }

    /// Insert an element enforcing dataset and value limits.
    pub fn insert_checked(&mut self, element: Element, limits: &Limits) -> Result<()> {
        enforce_limit(
            "max_dataset_elements",
            u64::try_from(self.elements.len())
                .ok()
                .and_then(|n| n.checked_add(1))
                .ok_or_else(|| invalid_tag_value(Tag(0x0000, 0x0000), "element count overflow"))?,
            limits.max_dataset_elements,
        )?;

        match &element.value {
            Value::Str(value) => {
                enforce_limit(
                    "max_string_bytes",
                    u64::try_from(value.len()).unwrap_or(u64::MAX),
                    limits.max_string_bytes,
                )?;
            }
            Value::Uid(value) => {
                enforce_limit(
                    "max_string_bytes",
                    u64::try_from(value.len()).unwrap_or(u64::MAX),
                    limits.max_string_bytes,
                )?;
                validate_uid_strict(element.tag, value)?;
            }
            Value::Bytes(bytes) => {
                enforce_limit(
                    "max_element_vl_bytes",
                    u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                    limits.max_element_vl_bytes,
                )?;
            }
            _ => {}
        }

        self.elements.insert(element.tag, element);
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
                    u64::try_from(value.len()).unwrap_or(u64::MAX),
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
                    u64::try_from(value.len()).unwrap_or(u64::MAX),
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
                    u64::try_from(value.len()).unwrap_or(u64::MAX),
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
                    u64::try_from(value.len()).unwrap_or(u64::MAX),
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
                    u64::try_from(value.len()).unwrap_or(u64::MAX),
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
    max_input_bytes: u64,
    /// Max number of decoded elements.
    max_dataset_elements: u64,
    /// Max sequence/item nesting depth.
    max_sequence_depth: u64,
    /// Max bytes in a single string value.
    max_string_bytes: u64,
    /// Max bytes in a single element value length.
    max_element_vl_bytes: u64,
    /// Max frames per instance.
    max_frames_per_instance: u64,
    /// Max pixels per frame.
    max_pixels_per_frame: u64,
    /// Max decompressed bytes per instance.
    max_decompressed_bytes: u64,
    /// Max total cached GPU texture bytes.
    max_gpu_texture_bytes: u64,
    /// Max total cached CPU bytes.
    max_cache_bytes: u64,
}

impl Limits {
    /// Create a new Limits with sensible defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return a builder for constructing Limits with validation.
    pub fn builder() -> LimitsBuilder {
        LimitsBuilder::default()
    }

    /// Return the max total input bytes limit.
    pub fn max_input_bytes(&self) -> u64 {
        self.max_input_bytes
    }

    /// Set the max total input bytes limit.
    pub fn set_max_input_bytes(&mut self, value: u64) {
        self.max_input_bytes = value;
    }

    /// Return the max number of decoded elements limit.
    pub fn max_dataset_elements(&self) -> u64 {
        self.max_dataset_elements
    }

    /// Set the max number of decoded elements limit.
    pub fn set_max_dataset_elements(&mut self, value: u64) {
        self.max_dataset_elements = value;
    }

    /// Return the max sequence/item nesting depth limit.
    pub fn max_sequence_depth(&self) -> u64 {
        self.max_sequence_depth
    }

    /// Set the max sequence/item nesting depth limit.
    pub fn set_max_sequence_depth(&mut self, value: u64) {
        self.max_sequence_depth = value;
    }

    /// Return the max bytes in a single string value limit.
    pub fn max_string_bytes(&self) -> u64 {
        self.max_string_bytes
    }

    /// Set the max bytes in a single string value limit.
    pub fn set_max_string_bytes(&mut self, value: u64) {
        self.max_string_bytes = value;
    }

    /// Return the max bytes in a single element value length limit.
    pub fn max_element_vl_bytes(&self) -> u64 {
        self.max_element_vl_bytes
    }

    /// Set the max bytes in a single element value length limit.
    pub fn set_max_element_vl_bytes(&mut self, value: u64) {
        self.max_element_vl_bytes = value;
    }

    /// Return the max frames per instance limit.
    pub fn max_frames_per_instance(&self) -> u64 {
        self.max_frames_per_instance
    }

    /// Set the max frames per instance limit.
    pub fn set_max_frames_per_instance(&mut self, value: u64) {
        self.max_frames_per_instance = value;
    }

    /// Return the max pixels per frame limit.
    pub fn max_pixels_per_frame(&self) -> u64 {
        self.max_pixels_per_frame
    }

    /// Set the max pixels per frame limit.
    pub fn set_max_pixels_per_frame(&mut self, value: u64) {
        self.max_pixels_per_frame = value;
    }

    /// Return the max decompressed bytes per instance limit.
    pub fn max_decompressed_bytes(&self) -> u64 {
        self.max_decompressed_bytes
    }

    /// Set the max decompressed bytes per instance limit.
    pub fn set_max_decompressed_bytes(&mut self, value: u64) {
        self.max_decompressed_bytes = value;
    }

    /// Return the max total cached GPU texture bytes limit.
    pub fn max_gpu_texture_bytes(&self) -> u64 {
        self.max_gpu_texture_bytes
    }

    /// Set the max total cached GPU texture bytes limit.
    pub fn set_max_gpu_texture_bytes(&mut self, value: u64) {
        self.max_gpu_texture_bytes = value;
    }

    /// Return the max total cached CPU bytes limit.
    pub fn max_cache_bytes(&self) -> u64 {
        self.max_cache_bytes
    }

    /// Set the max total cached CPU bytes limit.
    pub fn set_max_cache_bytes(&mut self, value: u64) {
        self.max_cache_bytes = value;
    }

    /// Validate that all limits have sensible (non-zero) values.
    ///
    /// Returns an error if any limit is zero, which would indicate a
    /// misconfiguration that could reject all valid inputs.
    pub fn validate(&self) -> Result<()> {
        if self.max_input_bytes == 0 {
            return Err(limit_exceeded("max_input_bytes", 0, 1));
        }
        if self.max_dataset_elements == 0 {
            return Err(limit_exceeded("max_dataset_elements", 0, 1));
        }
        if self.max_sequence_depth == 0 {
            return Err(limit_exceeded("max_sequence_depth", 0, 1));
        }
        if self.max_string_bytes == 0 {
            return Err(limit_exceeded("max_string_bytes", 0, 1));
        }
        if self.max_element_vl_bytes == 0 {
            return Err(limit_exceeded("max_element_vl_bytes", 0, 1));
        }
        if self.max_frames_per_instance == 0 {
            return Err(limit_exceeded("max_frames_per_instance", 0, 1));
        }
        if self.max_pixels_per_frame == 0 {
            return Err(limit_exceeded("max_pixels_per_frame", 0, 1));
        }
        if self.max_decompressed_bytes == 0 {
            return Err(limit_exceeded("max_decompressed_bytes", 0, 1));
        }
        if self.max_gpu_texture_bytes == 0 {
            return Err(limit_exceeded("max_gpu_texture_bytes", 0, 1));
        }
        if self.max_cache_bytes == 0 {
            return Err(limit_exceeded("max_cache_bytes", 0, 1));
        }
        Ok(())
    }
}

/// Builder for constructing [`Limits`] with validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LimitsBuilder {
    max_input_bytes: u64,
    max_dataset_elements: u64,
    max_sequence_depth: u64,
    max_string_bytes: u64,
    max_element_vl_bytes: u64,
    max_frames_per_instance: u64,
    max_pixels_per_frame: u64,
    max_decompressed_bytes: u64,
    max_gpu_texture_bytes: u64,
    max_cache_bytes: u64,
}

impl Default for LimitsBuilder {
    fn default() -> Self {
        let limits = Limits::default();
        Self {
            max_input_bytes: limits.max_input_bytes,
            max_dataset_elements: limits.max_dataset_elements,
            max_sequence_depth: limits.max_sequence_depth,
            max_string_bytes: limits.max_string_bytes,
            max_element_vl_bytes: limits.max_element_vl_bytes,
            max_frames_per_instance: limits.max_frames_per_instance,
            max_pixels_per_frame: limits.max_pixels_per_frame,
            max_decompressed_bytes: limits.max_decompressed_bytes,
            max_gpu_texture_bytes: limits.max_gpu_texture_bytes,
            max_cache_bytes: limits.max_cache_bytes,
        }
    }
}

impl LimitsBuilder {
    /// Set the max total input bytes limit.
    pub fn max_input_bytes(mut self, value: u64) -> Self {
        self.max_input_bytes = value;
        self
    }

    /// Set the max number of decoded elements limit.
    pub fn max_dataset_elements(mut self, value: u64) -> Self {
        self.max_dataset_elements = value;
        self
    }

    /// Set the max sequence/item nesting depth limit.
    pub fn max_sequence_depth(mut self, value: u64) -> Self {
        self.max_sequence_depth = value;
        self
    }

    /// Set the max bytes in a single string value limit.
    pub fn max_string_bytes(mut self, value: u64) -> Self {
        self.max_string_bytes = value;
        self
    }

    /// Set the max bytes in a single element value length limit.
    pub fn max_element_vl_bytes(mut self, value: u64) -> Self {
        self.max_element_vl_bytes = value;
        self
    }

    /// Set the max frames per instance limit.
    pub fn max_frames_per_instance(mut self, value: u64) -> Self {
        self.max_frames_per_instance = value;
        self
    }

    /// Set the max pixels per frame limit.
    pub fn max_pixels_per_frame(mut self, value: u64) -> Self {
        self.max_pixels_per_frame = value;
        self
    }

    /// Set the max decompressed bytes per instance limit.
    pub fn max_decompressed_bytes(mut self, value: u64) -> Self {
        self.max_decompressed_bytes = value;
        self
    }

    /// Set the max total cached GPU texture bytes limit.
    pub fn max_gpu_texture_bytes(mut self, value: u64) -> Self {
        self.max_gpu_texture_bytes = value;
        self
    }

    /// Set the max total cached CPU bytes limit.
    pub fn max_cache_bytes(mut self, value: u64) -> Self {
        self.max_cache_bytes = value;
        self
    }

    /// Build the Limits, validating that all values are non-zero.
    pub fn build(self) -> Result<Limits> {
        if self.max_input_bytes == 0 {
            return Err(limit_exceeded("max_input_bytes", 0, 1));
        }
        if self.max_dataset_elements == 0 {
            return Err(limit_exceeded("max_dataset_elements", 0, 1));
        }
        if self.max_sequence_depth == 0 {
            return Err(limit_exceeded("max_sequence_depth", 0, 1));
        }
        if self.max_string_bytes == 0 {
            return Err(limit_exceeded("max_string_bytes", 0, 1));
        }
        if self.max_element_vl_bytes == 0 {
            return Err(limit_exceeded("max_element_vl_bytes", 0, 1));
        }
        if self.max_frames_per_instance == 0 {
            return Err(limit_exceeded("max_frames_per_instance", 0, 1));
        }
        if self.max_pixels_per_frame == 0 {
            return Err(limit_exceeded("max_pixels_per_frame", 0, 1));
        }
        if self.max_decompressed_bytes == 0 {
            return Err(limit_exceeded("max_decompressed_bytes", 0, 1));
        }
        if self.max_gpu_texture_bytes == 0 {
            return Err(limit_exceeded("max_gpu_texture_bytes", 0, 1));
        }
        if self.max_cache_bytes == 0 {
            return Err(limit_exceeded("max_cache_bytes", 0, 1));
        }
        Ok(Limits {
            max_input_bytes: self.max_input_bytes,
            max_dataset_elements: self.max_dataset_elements,
            max_sequence_depth: self.max_sequence_depth,
            max_string_bytes: self.max_string_bytes,
            max_element_vl_bytes: self.max_element_vl_bytes,
            max_frames_per_instance: self.max_frames_per_instance,
            max_pixels_per_frame: self.max_pixels_per_frame,
            max_decompressed_bytes: self.max_decompressed_bytes,
            max_gpu_texture_bytes: self.max_gpu_texture_bytes,
            max_cache_bytes: self.max_cache_bytes,
        })
    }
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
    key: &'static str,
    /// Context value.
    value: String,
}

impl ContextItem {
    /// Create a new context item.
    pub fn new(key: &'static str, value: impl Into<String>) -> Self {
        Self {
            key,
            value: value.into(),
        }
    }

    /// Return the context key.
    pub fn key(&self) -> &'static str {
        self.key
    }

    /// Return the context value.
    pub fn value(&self) -> &str {
        &self.value
    }
}

/// Feature availability flags for the active build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    /// Tier 1 Deflated Explicit VR Little Endian support.
    tier1_deflate: bool,
    /// JPEG-LS codec support.
    codec_jpegls: bool,
    /// JPEG 2000 codec support.
    codec_j2k: bool,
    /// Non-DICOM raster input support.
    raster_io: bool,
    /// Grayscale Softcopy Presentation State support.
    gsps: bool,
    /// CT modality pack support.
    modality_ct: bool,
    /// PET modality pack support.
    modality_pet: bool,
    /// MG modality pack support.
    modality_mg: bool,
    /// XR modality pack support.
    modality_xr: bool,
    /// Enhanced multi-frame pack support.
    pack_enhanced: bool,
    /// Ultrasound pack support.
    pack_us: bool,
    /// Nuclear medicine pack support.
    pack_nm: bool,
    /// XA/XRF pack support.
    pack_xa: bool,
    /// Segmentation pack support.
    pack_seg: bool,
    /// RT Dose pack support.
    pack_rt: bool,
    /// Structured Report pack support.
    pack_sr: bool,
}

impl Capabilities {
    /// Create a new Capabilities with the given feature flags.
    pub fn new(
        tier1_deflate: bool,
        codec_jpegls: bool,
        codec_j2k: bool,
        raster_io: bool,
        gsps: bool,
        modality_ct: bool,
        modality_pet: bool,
        modality_mg: bool,
        modality_xr: bool,
        pack_enhanced: bool,
        pack_us: bool,
        pack_nm: bool,
        pack_xa: bool,
        pack_seg: bool,
        pack_rt: bool,
        pack_sr: bool,
    ) -> Self {
        Self {
            tier1_deflate,
            codec_jpegls,
            codec_j2k,
            raster_io,
            gsps,
            modality_ct,
            modality_pet,
            modality_mg,
            modality_xr,
            pack_enhanced,
            pack_us,
            pack_nm,
            pack_xa,
            pack_seg,
            pack_rt,
            pack_sr,
        }
    }

    /// Return whether Tier 1 Deflate support is enabled.
    pub fn tier1_deflate(&self) -> bool {
        self.tier1_deflate
    }

    /// Set whether Tier 1 Deflate support is enabled.
    pub fn set_tier1_deflate(&mut self, value: bool) {
        self.tier1_deflate = value;
    }

    /// Return whether JPEG-LS codec support is enabled.
    pub fn codec_jpegls(&self) -> bool {
        self.codec_jpegls
    }

    /// Set whether JPEG-LS codec support is enabled.
    pub fn set_codec_jpegls(&mut self, value: bool) {
        self.codec_jpegls = value;
    }

    /// Return whether JPEG 2000 codec support is enabled.
    pub fn codec_j2k(&self) -> bool {
        self.codec_j2k
    }

    /// Set whether JPEG 2000 codec support is enabled.
    pub fn set_codec_j2k(&mut self, value: bool) {
        self.codec_j2k = value;
    }

    /// Return whether non-DICOM raster input support is enabled.
    pub fn raster_io(&self) -> bool {
        self.raster_io
    }

    /// Set whether non-DICOM raster input support is enabled.
    pub fn set_raster_io(&mut self, value: bool) {
        self.raster_io = value;
    }

    /// Return whether GSPS support is enabled.
    pub fn gsps(&self) -> bool {
        self.gsps
    }

    /// Set whether GSPS support is enabled.
    pub fn set_gsps(&mut self, value: bool) {
        self.gsps = value;
    }

    /// Return whether CT modality pack support is enabled.
    pub fn modality_ct(&self) -> bool {
        self.modality_ct
    }

    /// Set whether CT modality pack support is enabled.
    pub fn set_modality_ct(&mut self, value: bool) {
        self.modality_ct = value;
    }

    /// Return whether PET modality pack support is enabled.
    pub fn modality_pet(&self) -> bool {
        self.modality_pet
    }

    /// Set whether PET modality pack support is enabled.
    pub fn set_modality_pet(&mut self, value: bool) {
        self.modality_pet = value;
    }

    /// Return whether MG modality pack support is enabled.
    pub fn modality_mg(&self) -> bool {
        self.modality_mg
    }

    /// Set whether MG modality pack support is enabled.
    pub fn set_modality_mg(&mut self, value: bool) {
        self.modality_mg = value;
    }

    /// Return whether XR modality pack support is enabled.
    pub fn modality_xr(&self) -> bool {
        self.modality_xr
    }

    /// Set whether XR modality pack support is enabled.
    pub fn set_modality_xr(&mut self, value: bool) {
        self.modality_xr = value;
    }

    /// Return whether enhanced multi-frame pack support is enabled.
    pub fn pack_enhanced(&self) -> bool {
        self.pack_enhanced
    }

    /// Set whether enhanced multi-frame pack support is enabled.
    pub fn set_pack_enhanced(&mut self, value: bool) {
        self.pack_enhanced = value;
    }

    /// Return whether ultrasound pack support is enabled.
    pub fn pack_us(&self) -> bool {
        self.pack_us
    }

    /// Set whether ultrasound pack support is enabled.
    pub fn set_pack_us(&mut self, value: bool) {
        self.pack_us = value;
    }

    /// Return whether nuclear medicine pack support is enabled.
    pub fn pack_nm(&self) -> bool {
        self.pack_nm
    }

    /// Set whether nuclear medicine pack support is enabled.
    pub fn set_pack_nm(&mut self, value: bool) {
        self.pack_nm = value;
    }

    /// Return whether XA/XRF pack support is enabled.
    pub fn pack_xa(&self) -> bool {
        self.pack_xa
    }

    /// Set whether XA/XRF pack support is enabled.
    pub fn set_pack_xa(&mut self, value: bool) {
        self.pack_xa = value;
    }

    /// Return whether segmentation pack support is enabled.
    pub fn pack_seg(&self) -> bool {
        self.pack_seg
    }

    /// Set whether segmentation pack support is enabled.
    pub fn set_pack_seg(&mut self, value: bool) {
        self.pack_seg = value;
    }

    /// Return whether RT Dose pack support is enabled.
    pub fn pack_rt(&self) -> bool {
        self.pack_rt
    }

    /// Set whether RT Dose pack support is enabled.
    pub fn set_pack_rt(&mut self, value: bool) {
        self.pack_rt = value;
    }

    /// Return whether structured report pack support is enabled.
    pub fn pack_sr(&self) -> bool {
        self.pack_sr
    }

    /// Set whether structured report pack support is enabled.
    pub fn set_pack_sr(&mut self, value: bool) {
        self.pack_sr = value;
    }
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
    /// Resource not found.
    NotFound {
        /// Human-readable detail.
        detail: String,
    },
    /// Authorization denied (auth/collab/session failures).
    AuthorizationDenied {
        /// Resource that was denied access.
        resource: String,
        /// Reason for denial.
        reason: String,
    },
    /// Policy violation.
    PolicyViolation {
        /// Policy identifier that was violated.
        policy: String,
        /// Human-readable detail.
        detail: String,
    },
    /// Session error (timeout, lockout, invalid state).
    SessionError {
        /// Session identifier.
        session_id: String,
        /// Human-readable detail.
        detail: String,
    },
    /// Collaboration error (sync, CRDT, presence failures).
    CollaborationError {
        /// Session identifier.
        session_id: String,
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
            ErrorKind::NotFound { .. } => "DVF.WEB.NOT_FOUND",
            ErrorKind::AuthorizationDenied { .. } => "DVF.AUTH.DENIED",
            ErrorKind::PolicyViolation { .. } => "DVF.AUTH.POLICY",
            ErrorKind::SessionError { .. } => "DVF.AUTH.SESSION",
            ErrorKind::CollaborationError { .. } => "DVF.COLLAB.ERROR",
        }
    }
}

/// Shared error type.
#[derive(Debug)]
pub struct Error {
    /// Stable error code.
    code: &'static str,
    /// Error kind.
    kind: ErrorKind,
    /// Short, non-PII message.
    message: String,
    /// Structured context items.
    context: Vec<ContextItem>,
    /// Optional source error.
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl Error {
    /// Create a new error, validating code/kind consistency.
    ///
    /// In debug builds, panics if `code` does not match `kind.code()`.
    ///
    /// # Convention (S13-T4)
    ///
    /// Prefer `Error::from_kind(kind, message)` which auto-derives the code.
    /// Use `Error::new(code, kind, message)` only when overriding the code.
    /// Chain with `.with_context()` and `.with_source()` for rich errors.
    pub fn new(code: &'static str, kind: ErrorKind, message: impl Into<String>) -> Self {
        debug_assert_eq!(
            code,
            kind.code(),
            "Error::new code/kind mismatch: code={code}, expected={}",
            kind.code()
        );
        Self {
            code,
            kind,
            message: message.into(),
            context: Vec::new(),
            source: None,
        }
    }

    /// Create a new error using the canonical code for the kind.
    ///
    /// # Convention (S13-T4)
    ///
    /// This is the **primary** error factory. All error construction should use
    /// `Error::from_kind(kind, message)`, optionally chaining
    /// `.with_context(key, val)` and `.with_source(err)`.
    pub fn from_kind(kind: ErrorKind, message: impl Into<String>) -> Self {
        let code = kind.code();
        Self::new(code, kind, message)
    }

    /// Attach a context item to the error.
    ///
    /// # Convention (S13-T4)
    ///
    /// Use the builder pattern: `Error::from_kind(kind, msg).with_context(key, val)`.
    /// Context items carry structured, non-PII diagnostic metadata.
    pub fn with_context(mut self, key: &'static str, value: impl Into<String>) -> Self {
        self.context.push(ContextItem {
            key,
            value: value.into(),
        });
        self
    }

    /// Attach a source (causal) error.
    ///
    /// # Convention (S13-T4)
    ///
    /// Use the builder pattern: `Error::from_kind(kind, msg).with_source(source_err)`.
    /// The source error is the underlying cause and is surfaced via
    /// `std::error::Error::source()`.
    pub fn with_source(mut self, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    /// Return the stable error code.
    pub fn code(&self) -> &'static str {
        self.code
    }

    /// Return the error kind.
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    /// Return the error message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Return the structured context items.
    pub fn context(&self) -> &[ContextItem] {
        &self.context
    }

    /// Return the optional source error.
    pub fn source(&self) -> Option<&(dyn std::error::Error + Send + Sync)> {
        self.source.as_deref()
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

    for element in dataset.elements.values() {
        *count += 1;
        enforce_limit("max_dataset_elements", *count, limits.max_dataset_elements)?;

        match &element.value {
            Value::Str(value) => {
                enforce_limit(
                    "max_string_bytes",
                    u64::try_from(value.len()).unwrap_or(u64::MAX),
                    limits.max_string_bytes,
                )?;
            }
            Value::Uid(value) => {
                enforce_limit(
                    "max_string_bytes",
                    u64::try_from(value.len()).unwrap_or(u64::MAX),
                    limits.max_string_bytes,
                )?;
                validate_uid_strict(element.tag, value)?;
            }
            Value::Bytes(bytes) => {
                enforce_limit(
                    "max_element_vl_bytes",
                    u64::try_from(bytes.len()).unwrap_or(u64::MAX),
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

// ===========================================================================
// S10-T1: Domain Newtypes
// ===========================================================================

/// A validated DICOM UID (Unique Identifier).
///
/// UIDs must contain only digits ('0'-'9') and '.' characters, be at most
/// 64 characters long, and must not start or end with '.'.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Uid(String);

impl Uid {
    /// Create a new validated UID from a string.
    ///
    /// Returns an error if the string is not a valid DICOM UID.
    pub fn new(s: &str) -> Result<Self> {
        validate_uid_standalone(s)?;
        Ok(Self(s.to_string()))
    }

    /// Return the UID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Return the length of the UID string.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Return true if the UID is empty (should not happen after validation).
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for Uid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for Uid {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl FromStr for Uid {
    type Err = Box<Error>;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Uid::new(s)
    }
}

fn validate_uid_standalone(input: &str) -> Result<()> {
    if input.is_empty() || input.len() > 64 {
        return Err(Error::from_kind(
            ErrorKind::InvalidTagValue {
                tag: Tag(0x0000, 0x0000),
                detail: "UID length is invalid".to_string(),
            },
            "invalid UID",
        )
        .into());
    }
    if input.starts_with('.') || input.ends_with('.') {
        return Err(Error::from_kind(
            ErrorKind::InvalidTagValue {
                tag: Tag(0x0000, 0x0000),
                detail: "UID must not start or end with '.'".to_string(),
            },
            "invalid UID",
        )
        .into());
    }
    let mut prev_dot = false;
    for ch in input.chars() {
        match ch {
            '0'..='9' => prev_dot = false,
            '.' => {
                if prev_dot {
                    return Err(Error::from_kind(
                        ErrorKind::InvalidTagValue {
                            tag: Tag(0x0000, 0x0000),
                            detail: "UID contains empty component".to_string(),
                        },
                        "invalid UID",
                    )
                    .into());
                }
                prev_dot = true;
            }
            _ => {
                return Err(Error::from_kind(
                    ErrorKind::InvalidTagValue {
                        tag: Tag(0x0000, 0x0000),
                        detail: "UID contains invalid characters".to_string(),
                    },
                    "invalid UID",
                )
                .into());
            }
        }
    }
    Ok(())
}

/// A validated DICOM AE Title (Application Entity Title).
///
/// AE Titles must be at most 16 bytes and contain only ASCII printable characters
/// (0x20-0x7E). They must not be empty.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AeTitle(String);

impl AeTitle {
    /// Create a new validated AE Title from a string.
    ///
    /// Returns an error if the string is not a valid AE Title.
    pub fn new(s: &str) -> Result<Self> {
        if s.is_empty() {
            return Err(Error::from_kind(
                ErrorKind::InvalidTagValue {
                    tag: Tag(0x0000, 0x0000),
                    detail: "AE title must not be empty".to_string(),
                },
                "invalid AE title",
            )
            .into());
        }
        if s.len() > 16 {
            return Err(Error::from_kind(
                ErrorKind::InvalidTagValue {
                    tag: Tag(0x0000, 0x0000),
                    detail: "AE title exceeds 16 bytes".to_string(),
                },
                "invalid AE title",
            )
            .into());
        }
        if !s.bytes().all(|b| (0x20..=0x7e).contains(&b)) {
            return Err(Error::from_kind(
                ErrorKind::InvalidTagValue {
                    tag: Tag(0x0000, 0x0000),
                    detail: "AE title contains non-ASCII characters".to_string(),
                },
                "invalid AE title",
            )
            .into());
        }
        Ok(Self(s.to_string()))
    }

    /// Return the AE title as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Return the length of the AE title in bytes.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Return true if the AE title is empty (should not happen after validation).
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for AeTitle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for AeTitle {
    type Err = Box<Error>;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        AeTitle::new(s)
    }
}

/// A validated SOP Class UID.
///
/// Wraps a [`Uid`] with well-known DICOM SOP Class UID constants.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SopClassUid(Uid);

impl SopClassUid {
    /// Create a new validated SOP Class UID.
    pub fn new(s: &str) -> Result<Self> {
        Ok(Self(Uid::new(s)?))
    }

    /// Verification SOP Class UID.
    pub const VERIFICATION: &'static str = "1.2.840.10008.1.1";
    /// CT Image Storage SOP Class UID.
    pub const CT_IMAGE_STORAGE: &'static str = "1.2.840.10008.5.1.4.1.1.2";
    /// MR Image Storage SOP Class UID.
    pub const MR_IMAGE_STORAGE: &'static str = "1.2.840.10008.5.1.4.1.1.4";
    /// Ultrasound Multiframe Image Storage SOP Class UID.
    pub const US_MULTIFRAME: &'static str = "1.2.840.10008.5.1.4.1.1.3.1";
    /// Secondary Capture SOP Class UID.
    pub const SECONDARY_CAPTURE: &'static str = "1.2.840.10008.5.1.4.1.1.7";
    /// Encapsulated 3D Model SOP Class UID.
    pub const ENCAPSULATED_3D_MODEL: &'static str = "1.2.840.10008.5.1.4.1.1.104.1";

    /// Return the inner UID.
    pub fn as_uid(&self) -> &Uid {
        &self.0
    }

    /// Return the SOP Class UID as a string slice.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for SopClassUid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for SopClassUid {
    type Err = Box<Error>;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        SopClassUid::new(s)
    }
}

/// A validated Transfer Syntax UID.
///
/// Wraps a [`Uid`] with well-known DICOM Transfer Syntax UID constants.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TransferSyntaxUid(Uid);

impl TransferSyntaxUid {
    /// Create a new validated Transfer Syntax UID.
    pub fn new(s: &str) -> Result<Self> {
        Ok(Self(Uid::new(s)?))
    }

    /// Implicit VR Little Endian Transfer Syntax UID.
    pub const IMPLICIT_VR_LE: &'static str = "1.2.840.10008.1.2";
    /// Explicit VR Little Endian Transfer Syntax UID.
    pub const EXPLICIT_VR_LE: &'static str = "1.2.840.10008.1.2.1";
    /// Explicit VR Big Endian Transfer Syntax UID.
    pub const EXPLICIT_VR_BE: &'static str = "1.2.840.10008.1.2.2";
    /// Deflated Explicit VR Little Endian Transfer Syntax UID.
    pub const DEFLATED_EXPLICIT_VR_LE: &'static str = "1.2.840.10008.1.2.1.99";
    /// JPEG Baseline (Process 1) Transfer Syntax UID.
    pub const JPEG_BASELINE: &'static str = "1.2.840.10008.1.2.4.50";
    /// JPEG Lossless Transfer Syntax UID.
    pub const JPEG_LOSSLESS: &'static str = "1.2.840.10008.1.2.4.70";
    /// JPEG 2000 Lossless Transfer Syntax UID.
    pub const JPEG_2000_LOSSLESS: &'static str = "1.2.840.10008.1.2.4.90";
    /// JPEG 2000 Transfer Syntax UID.
    pub const JPEG_2000: &'static str = "1.2.840.10008.1.2.4.91";
    /// JPEG-LS Lossless Transfer Syntax UID.
    pub const JPEG_LS_LOSSLESS: &'static str = "1.2.840.10008.1.2.4.80";

    /// Return the inner UID.
    pub fn as_uid(&self) -> &Uid {
        &self.0
    }

    /// Return the Transfer Syntax UID as a string slice.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for TransferSyntaxUid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for TransferSyntaxUid {
    type Err = Box<Error>;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        TransferSyntaxUid::new(s)
    }
}

/// A validated Measurement ID with format enforcement.
///
/// Measurement IDs must be non-empty, at most 128 characters, and contain
/// only alphanumeric characters, hyphens, underscores, and dots.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MeasurementId(String);

impl MeasurementId {
    /// Create a new validated Measurement ID.
    pub fn new(s: &str) -> Result<Self> {
        if s.is_empty() {
            return Err(Error::from_kind(
                ErrorKind::InvalidTagValue {
                    tag: Tag(0x0000, 0x0000),
                    detail: "measurement ID must not be empty".to_string(),
                },
                "invalid measurement ID",
            )
            .into());
        }
        if s.len() > 128 {
            return Err(Error::from_kind(
                ErrorKind::InvalidTagValue {
                    tag: Tag(0x0000, 0x0000),
                    detail: "measurement ID exceeds 128 characters".to_string(),
                },
                "invalid measurement ID",
            )
            .into());
        }
        if !s
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        {
            return Err(Error::from_kind(
                ErrorKind::InvalidTagValue {
                    tag: Tag(0x0000, 0x0000),
                    detail: "measurement ID contains invalid characters".to_string(),
                },
                "invalid measurement ID",
            )
            .into());
        }
        Ok(Self(s.to_string()))
    }

    /// Return the measurement ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MeasurementId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for MeasurementId {
    type Err = Box<Error>;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        MeasurementId::new(s)
    }
}

/// A validated Timestamp wrapping u64 epoch seconds.
///
/// The value must be non-zero (epoch 0 is not a valid timestamp for
/// clinical use).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Timestamp(u64);

impl Timestamp {
    /// Create a new validated Timestamp.
    ///
    /// Returns an error if the value is zero.
    pub fn new(epoch_secs: u64) -> Result<Self> {
        if epoch_secs == 0 {
            return Err(Error::from_kind(
                ErrorKind::InvalidTagValue {
                    tag: Tag(0x0000, 0x0000),
                    detail: "timestamp must be non-zero".to_string(),
                },
                "invalid timestamp",
            )
            .into());
        }
        Ok(Self(epoch_secs))
    }

    /// Return the epoch seconds value.
    pub fn epoch_secs(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Timestamp {
    type Err = Box<Error>;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let epoch_secs: u64 = s.parse().map_err(|_| {
            Error::from_kind(
                ErrorKind::InvalidTagValue {
                    tag: Tag(0x0000, 0x0000),
                    detail: "timestamp must be a valid u64".to_string(),
                },
                "invalid timestamp",
            )
        })?;
        Timestamp::new(epoch_secs)
    }
}
