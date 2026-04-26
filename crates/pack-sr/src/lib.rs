#![deny(missing_docs)]

//! SR pack: structured report parsing and measurement provenance extraction.

use dicom_core::{parse_f64_strict, Dataset, Element, Error, ErrorKind, Result, Tag, Value, Vr};

/// Basic Text SR SOP Class UID.
pub const SOP_CLASS_BASIC_TEXT_SR: &str = "1.2.840.10008.5.1.4.1.1.88.11";
/// Comprehensive SR SOP Class UID.
pub const SOP_CLASS_COMPREHENSIVE_SR: &str = "1.2.840.10008.5.1.4.1.1.88.33";

/// SR SOP Class manifest list.
pub const SR_SOP_CLASS_UIDS: &[&str] = &[SOP_CLASS_BASIC_TEXT_SR, SOP_CLASS_COMPREHENSIVE_SR];

/// Marker type for SR pack features.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SrPack;

impl SrPack {
    /// Return true when SR pack functionality is enabled for this crate.
    pub const fn enabled() -> bool {
        cfg!(feature = "pack-sr")
    }

    /// Require the SR pack to be enabled for SR SOP classes.
    pub fn ensure_supported(sop_class_uid: &str) -> Result<()> {
        if !Self::enabled() {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                "sr requires pack-sr feature",
            )));
        }
        if !SR_SOP_CLASS_UIDS.contains(&sop_class_uid) {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                "unsupported SR SOP class",
            )));
        }
        Ok(())
    }
}

/// SR code item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Code {
    /// Code value.
    pub code_value: String,
    /// Coding scheme designator.
    pub scheme: String,
    /// Code meaning.
    pub meaning: String,
}

/// SR measurement with provenance.
#[derive(Debug, Clone, PartialEq)]
pub struct SrMeasurement {
    /// Concept name code.
    pub concept: Code,
    /// Numeric value.
    pub value: f64,
    /// Units code.
    pub units: Code,
    /// Referenced SOP Instance UID, if present.
    pub referenced_sop_instance_uid: Option<String>,
}

/// SR text observation with provenance.
#[derive(Debug, Clone, PartialEq)]
pub struct SrTextObservation {
    /// Concept name code.
    pub concept: Code,
    /// Text value.
    pub text: String,
    /// Referenced SOP Instance UID, if present.
    pub referenced_sop_instance_uid: Option<String>,
}

/// SR coded observation with provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrCodeObservation {
    /// Concept name code.
    pub concept: Code,
    /// Coded value.
    pub value: Code,
    /// Referenced SOP Instance UID, if present.
    pub referenced_sop_instance_uid: Option<String>,
}

/// Authoring content item supported by baseline SR write APIs.
#[derive(Debug, Clone, PartialEq)]
pub enum SrAuthoringContentItem {
    /// Numeric content item (`NUM`).
    Num {
        /// Concept name code.
        concept: Code,
        /// Numeric value.
        value: f64,
        /// Measurement units.
        units: Code,
        /// Optional referenced SOP Instance UID.
        referenced_sop_instance_uid: Option<String>,
    },
    /// Text content item (`TEXT`).
    Text {
        /// Concept name code.
        concept: Code,
        /// Text value.
        text: String,
        /// Optional referenced SOP Instance UID.
        referenced_sop_instance_uid: Option<String>,
    },
    /// Coded content item (`CODE`).
    Code {
        /// Concept name code.
        concept: Code,
        /// Coded value.
        value: Code,
        /// Optional referenced SOP Instance UID.
        referenced_sop_instance_uid: Option<String>,
    },
}

impl SrAuthoringContentItem {
    fn sort_key(&self) -> (&'static str, &str) {
        match self {
            SrAuthoringContentItem::Num { concept, .. } => ("NUM", concept.code_value.as_str()),
            SrAuthoringContentItem::Text { concept, .. } => ("TEXT", concept.code_value.as_str()),
            SrAuthoringContentItem::Code { concept, .. } => ("CODE", concept.code_value.as_str()),
        }
    }

    fn referenced_uid(&self) -> Option<&str> {
        match self {
            SrAuthoringContentItem::Num {
                referenced_sop_instance_uid,
                ..
            } => referenced_sop_instance_uid.as_deref(),
            SrAuthoringContentItem::Text {
                referenced_sop_instance_uid,
                ..
            } => referenced_sop_instance_uid.as_deref(),
            SrAuthoringContentItem::Code {
                referenced_sop_instance_uid,
                ..
            } => referenced_sop_instance_uid.as_deref(),
        }
    }
}

/// Immutable provenance fields for authored SR documents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrProvenance {
    /// Study Instance UID.
    pub study_instance_uid: String,
    /// Series Instance UID.
    pub series_instance_uid: String,
    /// SOP Instance UID.
    pub sop_instance_uid: String,
    /// Author/observer identifier.
    pub observer: String,
    /// Deterministic authored timestamp in milliseconds.
    pub authored_epoch_ms: u64,
}

/// Authored SR document model.
#[derive(Debug, Clone, PartialEq)]
pub struct SrAuthoredDocument {
    /// Immutable provenance block.
    pub provenance: SrProvenance,
    /// Mutable content items.
    pub items: Vec<SrAuthoringContentItem>,
    /// Monotonic version for conflict handling.
    pub version: u64,
}

/// Deterministic builder defaults for SR authoring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrBuilderDefaults {
    /// Observer value used when not explicitly provided.
    pub observer: String,
    /// Timestamp used when not explicitly provided.
    pub authored_epoch_ms: u64,
}

impl Default for SrBuilderDefaults {
    fn default() -> Self {
        Self {
            observer: "rdvf-author".to_string(),
            authored_epoch_ms: 0,
        }
    }
}

/// Deterministic SR authoring builder.
#[derive(Debug, Clone, PartialEq)]
pub struct SrAuthoringBuilder {
    study_instance_uid: String,
    series_instance_uid: String,
    sop_instance_uid: String,
    defaults: SrBuilderDefaults,
    items: Vec<SrAuthoringContentItem>,
}

impl SrAuthoringBuilder {
    /// Create a new authoring builder.
    pub fn new(
        study_instance_uid: impl Into<String>,
        series_instance_uid: impl Into<String>,
        sop_instance_uid: impl Into<String>,
    ) -> Self {
        Self {
            study_instance_uid: study_instance_uid.into(),
            series_instance_uid: series_instance_uid.into(),
            sop_instance_uid: sop_instance_uid.into(),
            defaults: SrBuilderDefaults::default(),
            items: Vec::new(),
        }
    }

    /// Override deterministic defaults.
    pub fn with_defaults(mut self, defaults: SrBuilderDefaults) -> Self {
        self.defaults = defaults;
        self
    }

    /// Add one content item.
    pub fn push_item(mut self, item: SrAuthoringContentItem) -> Self {
        self.items.push(item);
        self
    }

    /// Build an authored SR document with deterministic ordering.
    pub fn build(mut self) -> SrAuthoredDocument {
        self.items
            .sort_by(|lhs, rhs| lhs.sort_key().cmp(&rhs.sort_key()));
        SrAuthoredDocument {
            provenance: SrProvenance {
                study_instance_uid: self.study_instance_uid,
                series_instance_uid: self.series_instance_uid,
                sop_instance_uid: self.sop_instance_uid,
                observer: self.defaults.observer,
                authored_epoch_ms: self.defaults.authored_epoch_ms,
            },
            items: self.items,
            version: 1,
        }
    }
}

/// SR update request payload.
#[derive(Debug, Clone, PartialEq)]
pub struct SrUpdateRequest {
    /// Expected current version.
    pub expected_version: u64,
    /// Items to append.
    pub append_items: Vec<SrAuthoringContentItem>,
    /// Optional observer override for this update event.
    pub observer: Option<String>,
}

/// SR authoring/update failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SrAuthoringError {
    /// Version conflict.
    VersionConflict {
        /// Expected version.
        expected: u64,
        /// Actual version.
        actual: u64,
    },
    /// Referenced SOP UID not present in known indexed set.
    UnknownReferencedSopInstanceUid(String),
}

impl std::fmt::Display for SrAuthoringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SrAuthoringError::VersionConflict { expected, actual } => {
                write!(
                    f,
                    "sr update version conflict: expected {expected}, actual {actual}"
                )
            }
            SrAuthoringError::UnknownReferencedSopInstanceUid(uid) => {
                write!(f, "unknown referenced SOP Instance UID: {uid}")
            }
        }
    }
}

impl std::error::Error for SrAuthoringError {}

/// Apply a deterministic SR update while preserving immutable provenance fields.
pub fn apply_sr_update(
    document: &mut SrAuthoredDocument,
    mut request: SrUpdateRequest,
    known_sop_instance_uids: &[String],
) -> std::result::Result<(), SrAuthoringError> {
    if request.expected_version != document.version {
        return Err(SrAuthoringError::VersionConflict {
            expected: request.expected_version,
            actual: document.version,
        });
    }
    for item in &request.append_items {
        if let Some(uid) = item.referenced_uid() {
            if !known_sop_instance_uids.iter().any(|known| known == uid) {
                return Err(SrAuthoringError::UnknownReferencedSopInstanceUid(
                    uid.to_string(),
                ));
            }
        }
    }
    request
        .append_items
        .sort_by(|lhs, rhs| lhs.sort_key().cmp(&rhs.sort_key()));
    document.items.extend(request.append_items);
    document.version = document.version.saturating_add(1);
    if let Some(observer) = request.observer {
        // Provenance observer is mutable context metadata by policy.
        document.provenance.observer = observer;
    }
    Ok(())
}

/// Serialize an authored SR document into a deterministic dataset representation.
pub fn serialize_authored_document(document: &SrAuthoredDocument) -> Dataset {
    let mut dataset = Dataset::new();
    dataset.insert(Element::new(TAG_STUDY_INSTANCE_UID, Vr::Ui, Value::Uid(document.provenance.study_instance_uid.clone()),
    ).unwrap());
    dataset.insert(Element::new(TAG_SERIES_INSTANCE_UID, Vr::Ui, Value::Uid(document.provenance.series_instance_uid.clone()),
    ).unwrap());
    dataset.insert(Element::new(TAG_SOP_INSTANCE_UID, Vr::Ui, Value::Uid(document.provenance.sop_instance_uid.clone()),
    ).unwrap());
    dataset.insert(Element::new(TAG_PRIVATE_OBSERVER, Vr::Lo, Value::Str(document.provenance.observer.clone()),
    ).unwrap());
    dataset.insert(Element::new(TAG_PRIVATE_AUTHORED_EPOCH_MS, Vr::Sl, Value::I32(document.provenance.authored_epoch_ms.min(i32::MAX as u64) as i32),
    ).unwrap());
    dataset.insert(Element::new(TAG_PRIVATE_DOCUMENT_VERSION, Vr::Sl, Value::I32(document.version.min(i32::MAX as u64) as i32),
    ).unwrap());

    let mut items = document.items.clone();
    items.sort_by(|lhs, rhs| lhs.sort_key().cmp(&rhs.sort_key()));
    let serialized_items = items
        .into_iter()
        .map(serialize_content_item)
        .collect::<Vec<_>>();
    dataset.insert(Element::new(TAG_CONTENT_SEQUENCE, Vr::Sq, Value::Sequence(serialized_items),
    ).unwrap());
    dataset
}

/// Parse an authored SR document from a deterministic dataset representation.
pub fn parse_authored_document(dataset: &Dataset) -> Result<SrAuthoredDocument> {
    let study_instance_uid = read_str(dataset, TAG_STUDY_INSTANCE_UID)?.to_string();
    let series_instance_uid = read_str(dataset, TAG_SERIES_INSTANCE_UID)?.to_string();
    let sop_instance_uid = read_str(dataset, TAG_SOP_INSTANCE_UID)?.to_string();
    let observer = read_str(dataset, TAG_PRIVATE_OBSERVER)?.to_string();
    let authored_epoch_ms = read_non_negative_i32(dataset, TAG_PRIVATE_AUTHORED_EPOCH_MS)? as u64;
    let version = read_non_negative_i32(dataset, TAG_PRIVATE_DOCUMENT_VERSION)? as u64;

    let content = sequence_items(dataset, TAG_CONTENT_SEQUENCE)?;
    let mut items = Vec::new();
    visit_content_items(content, &mut |item| {
        let value_type = read_str(item, TAG_VALUE_TYPE)?;
        let concept = read_code(item, TAG_CONCEPT_NAME_CODE_SEQUENCE)?;
        let referenced_sop_instance_uid = read_referenced_uid(item)?;
        let parsed = match value_type {
            "NUM" => {
                let measured_value = first_sequence_item(item, TAG_MEASURED_VALUE_SEQUENCE)?;
                let numeric = read_str(measured_value, TAG_NUMERIC_VALUE)?;
                let units = read_code(measured_value, TAG_MEASUREMENT_UNITS_CODE_SEQUENCE)?;
                let value = parse_f64_strict(TAG_NUMERIC_VALUE, numeric)?;
                SrAuthoringContentItem::Num {
                    concept,
                    value,
                    units,
                    referenced_sop_instance_uid,
                }
            }
            "TEXT" => {
                let text = read_str(item, TAG_TEXT_VALUE)?.to_string();
                SrAuthoringContentItem::Text {
                    concept,
                    text,
                    referenced_sop_instance_uid,
                }
            }
            "CODE" => {
                let value = read_code(item, TAG_CONCEPT_CODE_SEQUENCE)?;
                SrAuthoringContentItem::Code {
                    concept,
                    value,
                    referenced_sop_instance_uid,
                }
            }
            _ => return Ok(()),
        };
        items.push(parsed);
        Ok(())
    })?;
    items.sort_by(|lhs, rhs| lhs.sort_key().cmp(&rhs.sort_key()));

    Ok(SrAuthoredDocument {
        provenance: SrProvenance {
            study_instance_uid,
            series_instance_uid,
            sop_instance_uid,
            observer,
            authored_epoch_ms,
        },
        items,
        version,
    })
}

fn serialize_content_item(item: SrAuthoringContentItem) -> Dataset {
    let mut dataset = Dataset::new();
    match item {
        SrAuthoringContentItem::Num {
            concept,
            value,
            units,
            referenced_sop_instance_uid,
        } => {
            dataset.insert(Element::new(TAG_VALUE_TYPE, Vr::Cs, Value::Str("NUM".to_string()),
            ).unwrap());
            dataset.insert(code_sequence_element(
                TAG_CONCEPT_NAME_CODE_SEQUENCE,
                concept,
            ));
            let mut measured = Dataset::new();
            measured.insert(Element::new(TAG_NUMERIC_VALUE, Vr::Ds, Value::Str(value.to_string()),
            ).unwrap());
            measured.insert(code_sequence_element(
                TAG_MEASUREMENT_UNITS_CODE_SEQUENCE,
                units,
            ));
            dataset.insert(Element::new(TAG_MEASURED_VALUE_SEQUENCE, Vr::Sq, Value::Sequence(vec![measured]),
            ).unwrap());
            insert_referenced_uid(&mut dataset, referenced_sop_instance_uid);
        }
        SrAuthoringContentItem::Text {
            concept,
            text,
            referenced_sop_instance_uid,
        } => {
            dataset.insert(Element::new(TAG_VALUE_TYPE, Vr::Cs, Value::Str("TEXT".to_string()),
            ).unwrap());
            dataset.insert(code_sequence_element(
                TAG_CONCEPT_NAME_CODE_SEQUENCE,
                concept,
            ));
            dataset.insert(Element::new(TAG_TEXT_VALUE, Vr::Ut, Value::Str(text),
            ).unwrap());
            insert_referenced_uid(&mut dataset, referenced_sop_instance_uid);
        }
        SrAuthoringContentItem::Code {
            concept,
            value,
            referenced_sop_instance_uid,
        } => {
            dataset.insert(Element::new(TAG_VALUE_TYPE, Vr::Cs, Value::Str("CODE".to_string()),
            ).unwrap());
            dataset.insert(code_sequence_element(
                TAG_CONCEPT_NAME_CODE_SEQUENCE,
                concept,
            ));
            dataset.insert(code_sequence_element(TAG_CONCEPT_CODE_SEQUENCE, value));
            insert_referenced_uid(&mut dataset, referenced_sop_instance_uid);
        }
    }
    dataset
}

fn code_sequence_element(tag: Tag, code: Code) -> Element {
    let mut code_item = Dataset::new();
    code_item.insert(Element::new(TAG_CODE_VALUE, Vr::Sh, Value::Str(code.code_value),
    ).unwrap());
    code_item.insert(Element::new(TAG_CODING_SCHEME_DESIGNATOR, Vr::Sh, Value::Str(code.scheme),
    ).unwrap());
    code_item.insert(Element::new(TAG_CODE_MEANING, Vr::Lo, Value::Str(code.meaning),
    ).unwrap());
    Element::new(tag, Vr::Sq, Value::Sequence(vec![code_item]),
    ).unwrap()
}

fn insert_referenced_uid(dataset: &mut Dataset, referenced_uid: Option<String>) {
    if let Some(uid) = referenced_uid {
        let mut reference = Dataset::new();
        reference.insert(Element::new(TAG_REFERENCED_SOP_INSTANCE_UID, Vr::Ui, Value::Uid(uid),
        ).unwrap());
        dataset.insert(Element::new(TAG_REFERENCED_SOP_SEQUENCE, Vr::Sq, Value::Sequence(vec![reference]),
        ).unwrap());
    }
}

/// Extract numeric measurements from an SR dataset.
pub fn extract_measurements(dataset: &Dataset) -> Result<Vec<SrMeasurement>> {
    let content = sequence_items(dataset, TAG_CONTENT_SEQUENCE)?;
    let mut out = Vec::new();
    visit_content_items(content, &mut |item| {
        let value_type = read_str(item, TAG_VALUE_TYPE)?;
        if value_type != "NUM" {
            return Ok(());
        }
        let concept = read_code(item, TAG_CONCEPT_NAME_CODE_SEQUENCE)?;
        let measured_value = first_sequence_item(item, TAG_MEASURED_VALUE_SEQUENCE)?;
        let numeric = read_str(measured_value, TAG_NUMERIC_VALUE)?;
        let units = read_code(measured_value, TAG_MEASUREMENT_UNITS_CODE_SEQUENCE)?;
        let value = parse_f64_strict(TAG_NUMERIC_VALUE, numeric)?;
        if !value.is_finite() {
            return Err(invalid_tag_value(TAG_NUMERIC_VALUE, "non-finite value"));
        }
        let referenced_uid = read_referenced_uid(item)?;
        out.push(SrMeasurement {
            concept,
            value,
            units,
            referenced_sop_instance_uid: referenced_uid,
        });
        Ok(())
    })?;
    Ok(out)
}

/// Extract text observations from an SR dataset.
pub fn extract_text_observations(dataset: &Dataset) -> Result<Vec<SrTextObservation>> {
    let content = sequence_items(dataset, TAG_CONTENT_SEQUENCE)?;
    let mut out = Vec::new();
    visit_content_items(content, &mut |item| {
        let value_type = read_str(item, TAG_VALUE_TYPE)?;
        if value_type != "TEXT" {
            return Ok(());
        }
        let concept = read_code(item, TAG_CONCEPT_NAME_CODE_SEQUENCE)?;
        let text = read_str(item, TAG_TEXT_VALUE)?;
        if text.trim().is_empty() {
            return Err(invalid_tag_value(
                TAG_TEXT_VALUE,
                "text value must be non-empty",
            ));
        }
        let referenced_uid = read_referenced_uid(item)?;
        out.push(SrTextObservation {
            concept,
            text: text.to_string(),
            referenced_sop_instance_uid: referenced_uid,
        });
        Ok(())
    })?;
    Ok(out)
}

/// Extract coded observations from an SR dataset.
pub fn extract_code_observations(dataset: &Dataset) -> Result<Vec<SrCodeObservation>> {
    let content = sequence_items(dataset, TAG_CONTENT_SEQUENCE)?;
    let mut out = Vec::new();
    visit_content_items(content, &mut |item| {
        let value_type = read_str(item, TAG_VALUE_TYPE)?;
        if value_type != "CODE" {
            return Ok(());
        }
        let concept = read_code(item, TAG_CONCEPT_NAME_CODE_SEQUENCE)?;
        let value = read_code(item, TAG_CONCEPT_CODE_SEQUENCE)?;
        let referenced_uid = read_referenced_uid(item)?;
        out.push(SrCodeObservation {
            concept,
            value,
            referenced_sop_instance_uid: referenced_uid,
        });
        Ok(())
    })?;
    Ok(out)
}

const TAG_CONTENT_SEQUENCE: Tag = Tag(0x0040, 0xA730);
const TAG_VALUE_TYPE: Tag = Tag(0x0040, 0xA040);
const TAG_CONCEPT_NAME_CODE_SEQUENCE: Tag = Tag(0x0040, 0xA043);
const TAG_CONCEPT_CODE_SEQUENCE: Tag = Tag(0x0040, 0xA168);
const TAG_MEASURED_VALUE_SEQUENCE: Tag = Tag(0x0040, 0xA300);
const TAG_NUMERIC_VALUE: Tag = Tag(0x0040, 0xA30A);
const TAG_TEXT_VALUE: Tag = Tag(0x0040, 0xA160);
const TAG_MEASUREMENT_UNITS_CODE_SEQUENCE: Tag = Tag(0x0040, 0x08EA);
const TAG_CODE_VALUE: Tag = Tag(0x0008, 0x0100);
const TAG_CODING_SCHEME_DESIGNATOR: Tag = Tag(0x0008, 0x0102);
const TAG_CODE_MEANING: Tag = Tag(0x0008, 0x0104);
const TAG_REFERENCED_SOP_SEQUENCE: Tag = Tag(0x0008, 0x1199);
const TAG_REFERENCED_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x1155);
const TAG_STUDY_INSTANCE_UID: Tag = Tag(0x0020, 0x000D);
const TAG_SERIES_INSTANCE_UID: Tag = Tag(0x0020, 0x000E);
const TAG_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x0018);
const TAG_PRIVATE_OBSERVER: Tag = Tag(0x0099, 0x1001);
const TAG_PRIVATE_AUTHORED_EPOCH_MS: Tag = Tag(0x0099, 0x1002);
const TAG_PRIVATE_DOCUMENT_VERSION: Tag = Tag(0x0099, 0x1003);

fn read_code(dataset: &Dataset, tag: Tag) -> Result<Code> {
    let item = first_sequence_item(dataset, tag)?;
    let code_value = read_str(item, TAG_CODE_VALUE)?;
    let scheme = read_str(item, TAG_CODING_SCHEME_DESIGNATOR)?;
    let meaning = read_str(item, TAG_CODE_MEANING)?;
    Ok(Code {
        code_value: code_value.to_string(),
        scheme: scheme.to_string(),
        meaning: meaning.to_string(),
    })
}

fn read_referenced_uid(dataset: &Dataset) -> Result<Option<String>> {
    if let Ok(item) = first_sequence_item(dataset, TAG_REFERENCED_SOP_SEQUENCE) {
        let uid = read_str(item, TAG_REFERENCED_SOP_INSTANCE_UID)?;
        return Ok(Some(uid.to_string()));
    }
    Ok(None)
}

fn sequence_items(dataset: &Dataset, tag: Tag) -> Result<&[Dataset]> {
    match dataset.get(tag) {
        Some(element) => match element.value() {
            Value::Sequence(items) => Ok(items.as_slice()),
            _ => Err(invalid_tag_value(tag, "expected sequence")),
        },
        None => Err(missing_required_tag(tag)),
    }
}

fn sequence_items_optional(dataset: &Dataset, tag: Tag) -> Result<Option<&[Dataset]>> {
    match dataset.get(tag) {
        Some(element) => match element.value() {
            Value::Sequence(items) => Ok(Some(items.as_slice())),
            _ => Err(invalid_tag_value(tag, "expected sequence")),
        },
        None => Ok(None),
    }
}

fn first_sequence_item(dataset: &Dataset, tag: Tag) -> Result<&Dataset> {
    let items = sequence_items(dataset, tag)?;
    items.first().ok_or_else(|| missing_required_tag(tag))
}

fn visit_content_items<F>(items: &[Dataset], visitor: &mut F) -> Result<()>
where
    F: FnMut(&Dataset) -> Result<()>,
{
    for item in items {
        visitor(item)?;
        if let Some(nested) = sequence_items_optional(item, TAG_CONTENT_SEQUENCE)? {
            visit_content_items(nested, visitor)?;
        }
    }
    Ok(())
}

fn read_str(dataset: &Dataset, tag: Tag) -> Result<&str> {
    match dataset.get(tag) {
        Some(element) => match element.value() {
            Value::Str(value) => Ok(value.as_str()),
            Value::Uid(value) => Ok(value.as_str()),
            _ => Err(invalid_tag_value(tag, "expected string")),
        },
        None => Err(missing_required_tag(tag)),
    }
}

fn read_i32(dataset: &Dataset, tag: Tag) -> Result<i32> {
    match dataset.get(tag) {
        Some(element) => match element.value() {
            Value::I32(value) => Ok(*value),
            Value::Str(value) => value
                .parse::<i32>()
                .map_err(|_| invalid_tag_value(tag, "expected integer")),
            _ => Err(invalid_tag_value(tag, "expected integer")),
        },
        None => Err(missing_required_tag(tag)),
    }
}

fn read_non_negative_i32(dataset: &Dataset, tag: Tag) -> Result<i32> {
    let value = read_i32(dataset, tag)?;
    if value < 0 {
        return Err(invalid_tag_value(tag, "expected non-negative integer"));
    }
    Ok(value)
}

fn missing_required_tag(tag: Tag) -> Box<Error> {
    Error::from_kind(
        ErrorKind::MissingRequiredTag { tag },
        "missing required tag",
    )
    .into()
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

// TID Template Constants and Structured SR Templates

/// TID 1500 Measurement Report template identifier.
pub const TID_MEASUREMENT_REPORT: &str = "1.2.840.10008.1.1.20.1.1";

/// TID 300 Measurement template identifier.
pub const TID_MEASUREMENT: &str = "1.2.840.10008.1.1.20.2.1";

/// TID 1204 Language of Content Item template identifier.
pub const TID_LANGUAGE: &str = "1.2.840.10008.1.1.20.3.1";

/// Standard coded concepts for SR measurement reports.
pub mod coded_concepts {
    //! Standard DICOM/SNOMED CT coded concepts for SR templates.

    /// Measurement Report concept.
    pub fn measurement_report() -> super::Code {
        super::Code {
            code_value: "126000".to_string(),
            scheme: "DCM".to_string(),
            meaning: "Measurement Report".to_string(),
        }
    }

    /// Distance measurement concept.
    pub fn distance() -> super::Code {
        super::Code {
            code_value: "121206".to_string(),
            scheme: "DCM".to_string(),
            meaning: "Distance".to_string(),
        }
    }

    /// Angle measurement concept.
    pub fn angle() -> super::Code {
        super::Code {
            code_value: "121207".to_string(),
            scheme: "DCM".to_string(),
            meaning: "Angle".to_string(),
        }
    }

    /// Probe (pixel value) measurement concept.
    pub fn probe() -> super::Code {
        super::Code {
            code_value: "121208".to_string(),
            scheme: "DCM".to_string(),
            meaning: "Pixel Value".to_string(),
        }
    }

    /// Millimeter unit code.
    pub fn millimeter() -> super::Code {
        super::Code {
            code_value: "mm".to_string(),
            scheme: "UCUM".to_string(),
            meaning: "millimeter".to_string(),
        }
    }

    /// Degree unit code.
    pub fn degree() -> super::Code {
        super::Code {
            code_value: "deg".to_string(),
            scheme: "UCUM".to_string(),
            meaning: "degree".to_string(),
        }
    }

    /// Pixel unit code (no unit).
    pub fn pixel() -> super::Code {
        super::Code {
            code_value: "pixel".to_string(),
            scheme: "UCUM".to_string(),
            meaning: "pixel".to_string(),
        }
    }

    /// Observation context container.
    pub fn observation_context() -> super::Code {
        super::Code {
            code_value: "121005".to_string(),
            scheme: "DCM".to_string(),
            meaning: "Observation Context".to_string(),
        }
    }

    /// Container content item.
    pub fn container() -> super::Code {
        super::Code {
            code_value: "111028".to_string(),
            scheme: "DCM".to_string(),
            meaning: "Container".to_string(),
        }
    }
}

/// Build a TID 1500 Measurement Report from measurement records.
///
/// Creates a CONTAINER content item with the Measurement Report concept,
/// wrapping individual TID 300 measurement items derived from the provided
/// measurement data. Each measurement includes its concept code, numeric
/// value, units, and optional referenced SOP Instance UID.
pub fn build_measurement_report(
    study_uid: impl Into<String>,
    series_uid: impl Into<String>,
    sop_uid: impl Into<String>,
    observer: impl Into<String>,
    measurements: &[SrMeasurement],
) -> SrAuthoredDocument {
    let mut builder = SrAuthoringBuilder::new(study_uid, series_uid, sop_uid)
        .with_defaults(SrBuilderDefaults {
            observer: observer.into(),
            authored_epoch_ms: 0,
        })
        .push_item(SrAuthoringContentItem::Code {
            concept: coded_concepts::measurement_report(),
            value: coded_concepts::container(),
            referenced_sop_instance_uid: None,
        });
    for m in measurements {
        builder = builder.push_item(SrAuthoringContentItem::Num {
            concept: m.concept.clone(),
            value: m.value,
            units: m.units.clone(),
            referenced_sop_instance_uid: m.referenced_sop_instance_uid.clone(),
        });
    }
    builder.build()
}

/// Build a single TID 300 Measurement content item from measurement data.
pub fn build_tid300_measurement(
    concept: Code,
    value: f64,
    units: Code,
    referenced_sop_instance_uid: Option<String>,
) -> SrAuthoringContentItem {
    SrAuthoringContentItem::Num {
        concept,
        value,
        units,
        referenced_sop_instance_uid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_core::{Element, Vr};

    const SR_MANIFEST: &str = include_str!("../manifest.toml");

    fn parse_manifest_uids() -> Vec<String> {
        SR_MANIFEST
            .split('"')
            .enumerate()
            .filter_map(|(idx, part)| {
                if idx % 2 == 1 {
                    Some(part.to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    fn build_num_item() -> Dataset {
        let mut item = Dataset::new();
        item.insert(Element::new(TAG_VALUE_TYPE, Vr::Cs, Value::Str("NUM".to_string()),
        ).unwrap());
        let mut concept_item = Dataset::new();
        concept_item.insert(Element::new(TAG_CODE_VALUE, Vr::Sh, Value::Str("123".to_string()),
        ).unwrap());
        concept_item.insert(Element::new(TAG_CODING_SCHEME_DESIGNATOR, Vr::Sh, Value::Str("99TEST".to_string()),
        ).unwrap());
        concept_item.insert(Element::new(TAG_CODE_MEANING, Vr::Lo, Value::Str("Length".to_string()),
        ).unwrap());
        item.insert(Element::new(TAG_CONCEPT_NAME_CODE_SEQUENCE, Vr::Sq, Value::Sequence(vec![concept_item]),
        ).unwrap());
        let mut units_item = Dataset::new();
        units_item.insert(Element::new(TAG_CODE_VALUE, Vr::Sh, Value::Str("mm".to_string()),
        ).unwrap());
        units_item.insert(Element::new(TAG_CODING_SCHEME_DESIGNATOR, Vr::Sh, Value::Str("UCUM".to_string()),
        ).unwrap());
        units_item.insert(Element::new(TAG_CODE_MEANING, Vr::Lo, Value::Str("millimeter".to_string()),
        ).unwrap());
        let mut measured = Dataset::new();
        measured.insert(Element::new(TAG_NUMERIC_VALUE, Vr::Ds, Value::Str("12.5".to_string()),
        ).unwrap());
        measured.insert(Element::new(TAG_MEASUREMENT_UNITS_CODE_SEQUENCE, Vr::Sq, Value::Sequence(vec![units_item]),
        ).unwrap());
        item.insert(Element::new(TAG_MEASURED_VALUE_SEQUENCE, Vr::Sq, Value::Sequence(vec![measured]),
        ).unwrap());
        item
    }

    fn build_text_item(text: &str) -> Dataset {
        let mut item = Dataset::new();
        item.insert(Element::new(TAG_VALUE_TYPE, Vr::Cs, Value::Str("TEXT".to_string()),
        ).unwrap());
        let mut concept_item = Dataset::new();
        concept_item.insert(Element::new(TAG_CODE_VALUE, Vr::Sh, Value::Str("TXT".to_string()),
        ).unwrap());
        concept_item.insert(Element::new(TAG_CODING_SCHEME_DESIGNATOR, Vr::Sh, Value::Str("99TEST".to_string()),
        ).unwrap());
        concept_item.insert(Element::new(TAG_CODE_MEANING, Vr::Lo, Value::Str("Comment".to_string()),
        ).unwrap());
        item.insert(Element::new(TAG_CONCEPT_NAME_CODE_SEQUENCE, Vr::Sq, Value::Sequence(vec![concept_item]),
        ).unwrap());
        item.insert(Element::new(TAG_TEXT_VALUE, Vr::Ut, Value::Str(text.to_string()),
        ).unwrap());
        item
    }

    fn build_code_item() -> Dataset {
        let mut item = Dataset::new();
        item.insert(Element::new(TAG_VALUE_TYPE, Vr::Cs, Value::Str("CODE".to_string()),
        ).unwrap());
        let mut concept_item = Dataset::new();
        concept_item.insert(Element::new(TAG_CODE_VALUE, Vr::Sh, Value::Str("OBS".to_string()),
        ).unwrap());
        concept_item.insert(Element::new(TAG_CODING_SCHEME_DESIGNATOR, Vr::Sh, Value::Str("99TEST".to_string()),
        ).unwrap());
        concept_item.insert(Element::new(TAG_CODE_MEANING, Vr::Lo, Value::Str("Observation".to_string()),
        ).unwrap());
        item.insert(Element::new(TAG_CONCEPT_NAME_CODE_SEQUENCE, Vr::Sq, Value::Sequence(vec![concept_item]),
        ).unwrap());
        let mut value_item = Dataset::new();
        value_item.insert(Element::new(TAG_CODE_VALUE, Vr::Sh, Value::Str("R-404FB".to_string()),
        ).unwrap());
        value_item.insert(Element::new(TAG_CODING_SCHEME_DESIGNATOR, Vr::Sh, Value::Str("SRT".to_string()),
        ).unwrap());
        value_item.insert(Element::new(TAG_CODE_MEANING, Vr::Lo, Value::Str("Normal".to_string()),
        ).unwrap());
        item.insert(Element::new(TAG_CONCEPT_CODE_SEQUENCE, Vr::Sq, Value::Sequence(vec![value_item]),
        ).unwrap());
        item
    }

    #[test]
    #[cfg(not(feature = "pack-sr"))]
    fn sr_pack_disabled_rejects() {
        // REQ-FEAT-302
        assert!(!SrPack::enabled());
        let err = SrPack::ensure_supported(SOP_CLASS_BASIC_TEXT_SR).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.UNSUPPORTED_SOP");
    }

    #[test]
    #[cfg(feature = "pack-sr")]
    fn sr_pack_enabled_allows() {
        // REQ-FEAT-302
        assert!(SrPack::enabled());
        SrPack::ensure_supported(SOP_CLASS_BASIC_TEXT_SR).expect("sr pack enabled");
    }

    #[test]
    fn manifest_matches_constants() {
        // REQ-CONF-083
        let parsed = parse_manifest_uids();
        assert!(!parsed.is_empty());
        for uid in SR_SOP_CLASS_UIDS {
            assert!(parsed.contains(&uid.to_string()));
        }
    }

    #[test]
    fn extract_numeric_measurement() {
        // REQ-MEAS-081, REQ-SR-300
        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_CONTENT_SEQUENCE, Vr::Sq, Value::Sequence(vec![build_num_item()]),
        ).unwrap());
        let measurements = extract_measurements(&dataset).expect("extract");
        assert_eq!(measurements.len(), 1);
        assert_eq!(measurements[0].value, 12.5);
    }

    #[test]
    fn sr_referenced_uid_is_captured() {
        // REQ-MEAS-082, REQ-SR-300
        let mut ref_item = Dataset::new();
        ref_item.insert(Element::new(TAG_REFERENCED_SOP_INSTANCE_UID, Vr::Ui, Value::Uid("1.2.3.4".to_string()),
        ).unwrap());
        let mut dataset = Dataset::new();
        let mut num_item = build_num_item();
        num_item.insert(Element::new(TAG_REFERENCED_SOP_SEQUENCE, Vr::Sq, Value::Sequence(vec![ref_item]),
        ).unwrap());
        dataset.insert(Element::new(TAG_CONTENT_SEQUENCE, Vr::Sq, Value::Sequence(vec![num_item]),
        ).unwrap());
        let measurements = extract_measurements(&dataset).expect("extract");
        assert_eq!(
            measurements[0].referenced_sop_instance_uid.as_deref(),
            Some("1.2.3.4")
        );
    }

    #[test]
    fn extract_text_observation() {
        // REQ-MEAS-082, REQ-SR-300
        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_CONTENT_SEQUENCE, Vr::Sq, Value::Sequence(vec![build_text_item("Finding present")]),
        ).unwrap());
        let observations = extract_text_observations(&dataset).expect("extract");
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].text, "Finding present");
    }

    #[test]
    fn extract_nested_num_and_text_items() {
        // REQ-MEAS-081, REQ-MEAS-082, REQ-SR-300
        let mut container = Dataset::new();
        container.insert(Element::new(TAG_VALUE_TYPE, Vr::Cs, Value::Str("CONTAINER".to_string()),
        ).unwrap());
        container.insert(Element::new(TAG_CONTENT_SEQUENCE, Vr::Sq, Value::Sequence(vec![build_num_item(), build_text_item("Nested note")]),
        ).unwrap());

        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_CONTENT_SEQUENCE, Vr::Sq, Value::Sequence(vec![container]),
        ).unwrap());

        let measurements = extract_measurements(&dataset).expect("extract measurements");
        let observations = extract_text_observations(&dataset).expect("extract text");
        assert_eq!(measurements.len(), 1);
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].text, "Nested note");
    }

    #[test]
    fn extract_code_observation() {
        // REQ-MEAS-082, REQ-SR-300
        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_CONTENT_SEQUENCE, Vr::Sq, Value::Sequence(vec![build_code_item()]),
        ).unwrap());
        let observations = extract_code_observations(&dataset).expect("extract");
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].value.code_value, "R-404FB");
    }

    #[test]
    fn extract_nested_code_item() {
        // REQ-MEAS-082, REQ-SR-300
        let mut container = Dataset::new();
        container.insert(Element::new(TAG_VALUE_TYPE, Vr::Cs, Value::Str("CONTAINER".to_string()),
        ).unwrap());
        container.insert(Element::new(TAG_CONTENT_SEQUENCE, Vr::Sq, Value::Sequence(vec![build_code_item()]),
        ).unwrap());
        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_CONTENT_SEQUENCE, Vr::Sq, Value::Sequence(vec![container]),
        ).unwrap());
        let observations = extract_code_observations(&dataset).expect("extract");
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].value.meaning, "Normal");
    }

    #[test]
    fn empty_text_observation_fails() {
        // REQ-UI-065, REQ-SR-300
        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_CONTENT_SEQUENCE, Vr::Sq, Value::Sequence(vec![build_text_item("   ")]),
        ).unwrap());
        let err = extract_text_observations(&dataset).expect_err("expected error");
        assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");
    }

    #[test]
    fn sr_builder_uses_deterministic_defaults_and_ordering() {
        let concept_b = Code {
            code_value: "B".to_string(),
            scheme: "99TEST".to_string(),
            meaning: "Second".to_string(),
        };
        let concept_a = Code {
            code_value: "A".to_string(),
            scheme: "99TEST".to_string(),
            meaning: "First".to_string(),
        };
        let units = Code {
            code_value: "mm".to_string(),
            scheme: "UCUM".to_string(),
            meaning: "millimeter".to_string(),
        };
        let authored = SrAuthoringBuilder::new("1.2.3", "1.2.3.4", "1.2.3.4.5")
            .with_defaults(SrBuilderDefaults {
                observer: "observer-1".to_string(),
                authored_epoch_ms: 1234,
            })
            .push_item(SrAuthoringContentItem::Text {
                concept: concept_b,
                text: "text".to_string(),
                referenced_sop_instance_uid: None,
            })
            .push_item(SrAuthoringContentItem::Num {
                concept: concept_a,
                value: 42.0,
                units,
                referenced_sop_instance_uid: None,
            })
            .build();
        assert_eq!(authored.provenance.observer, "observer-1");
        assert_eq!(authored.provenance.authored_epoch_ms, 1234);
        assert_eq!(authored.version, 1);
        assert!(matches!(
            authored.items[0],
            SrAuthoringContentItem::Num { .. }
        ));
    }

    #[test]
    fn sr_update_preserves_immutable_provenance_and_checks_references() {
        let concept = Code {
            code_value: "A".to_string(),
            scheme: "99TEST".to_string(),
            meaning: "Alpha".to_string(),
        };
        let mut document = SrAuthoringBuilder::new("1", "2", "3").build();
        let request = SrUpdateRequest {
            expected_version: 1,
            append_items: vec![SrAuthoringContentItem::Text {
                concept,
                text: "ok".to_string(),
                referenced_sop_instance_uid: Some("1.2.9".to_string()),
            }],
            observer: Some("observer-2".to_string()),
        };
        let known = vec!["1.2.9".to_string()];
        apply_sr_update(&mut document, request, &known).expect("update");
        assert_eq!(document.version, 2);
        assert_eq!(document.provenance.study_instance_uid, "1");
        assert_eq!(document.provenance.series_instance_uid, "2");
        assert_eq!(document.provenance.sop_instance_uid, "3");
        assert_eq!(document.provenance.observer, "observer-2");

        let bad = SrUpdateRequest {
            expected_version: 2,
            append_items: vec![SrAuthoringContentItem::Text {
                concept: Code {
                    code_value: "X".to_string(),
                    scheme: "99TEST".to_string(),
                    meaning: "X".to_string(),
                },
                text: "bad".to_string(),
                referenced_sop_instance_uid: Some("missing".to_string()),
            }],
            observer: None,
        };
        let err = apply_sr_update(&mut document, bad, &known).expect_err("must fail");
        assert!(matches!(
            err,
            SrAuthoringError::UnknownReferencedSopInstanceUid(_)
        ));
    }

    #[test]
    fn authored_document_serialize_parse_roundtrip_is_deterministic() {
        let concept_num = Code {
            code_value: "N1".to_string(),
            scheme: "99TEST".to_string(),
            meaning: "Length".to_string(),
        };
        let units = Code {
            code_value: "mm".to_string(),
            scheme: "UCUM".to_string(),
            meaning: "millimeter".to_string(),
        };
        let concept_text = Code {
            code_value: "T1".to_string(),
            scheme: "99TEST".to_string(),
            meaning: "Comment".to_string(),
        };
        let concept_code = Code {
            code_value: "C1".to_string(),
            scheme: "99TEST".to_string(),
            meaning: "Classification".to_string(),
        };
        let coded_value = Code {
            code_value: "R-404FB".to_string(),
            scheme: "SRT".to_string(),
            meaning: "Normal".to_string(),
        };

        let authored = SrAuthoringBuilder::new("1.2.3", "1.2.3.4", "1.2.3.4.5")
            .with_defaults(SrBuilderDefaults {
                observer: "observer-alpha".to_string(),
                authored_epoch_ms: 42,
            })
            .push_item(SrAuthoringContentItem::Text {
                concept: concept_text,
                text: "stable text".to_string(),
                referenced_sop_instance_uid: Some("1.2.9".to_string()),
            })
            .push_item(SrAuthoringContentItem::Num {
                concept: concept_num,
                value: 12.25,
                units,
                referenced_sop_instance_uid: None,
            })
            .push_item(SrAuthoringContentItem::Code {
                concept: concept_code,
                value: coded_value,
                referenced_sop_instance_uid: Some("1.2.8".to_string()),
            })
            .build();

        let serialized = serialize_authored_document(&authored);
        let parsed = parse_authored_document(&serialized).expect("parse authored");
        assert_eq!(parsed, authored);

        let extracted = extract_measurements(&serialized).expect("extract num");
        assert_eq!(extracted.len(), 1);
        assert_eq!(extracted[0].value, 12.25);
    }

    #[test]
    fn tid_template_constants_are_valid_oids() {
        // REQ-SR-1500
        assert!(TID_MEASUREMENT_REPORT.starts_with("1.2.840"));
        assert!(TID_MEASUREMENT.starts_with("1.2.840"));
        assert!(TID_LANGUAGE.starts_with("1.2.840"));
        assert!(!TID_MEASUREMENT_REPORT.is_empty());
        assert!(!TID_MEASUREMENT.is_empty());
        assert!(!TID_LANGUAGE.is_empty());
        assert_ne!(TID_MEASUREMENT_REPORT, TID_MEASUREMENT);
        assert_ne!(TID_MEASUREMENT_REPORT, TID_LANGUAGE);
        assert_ne!(TID_MEASUREMENT, TID_LANGUAGE);
    }

    #[test]
    fn coded_concepts_measurement_report_has_dcm_scheme() {
        // REQ-SR-1500
        let code = coded_concepts::measurement_report();
        assert_eq!(code.scheme, "DCM");
        assert_eq!(code.code_value, "126000");
        assert!(!code.meaning.is_empty());
    }

    #[test]
    fn coded_concepts_distance_uses_dcm_scheme() {
        // REQ-SR-300
        let code = coded_concepts::distance();
        assert_eq!(code.scheme, "DCM");
        assert_eq!(code.code_value, "121206");
        assert_eq!(code.meaning, "Distance");
    }

    #[test]
    fn coded_concepts_angle_uses_dcm_scheme() {
        // REQ-SR-300
        let code = coded_concepts::angle();
        assert_eq!(code.scheme, "DCM");
        assert_eq!(code.code_value, "121207");
        assert_eq!(code.meaning, "Angle");
    }

    #[test]
    fn coded_concepts_probe_uses_dcm_scheme() {
        // REQ-SR-300
        let code = coded_concepts::probe();
        assert_eq!(code.scheme, "DCM");
        assert_eq!(code.code_value, "121208");
        assert_eq!(code.meaning, "Pixel Value");
    }

    #[test]
    fn coded_concepts_units_use_ucum_scheme() {
        // REQ-SR-300
        assert_eq!(coded_concepts::millimeter().scheme, "UCUM");
        assert_eq!(coded_concepts::millimeter().code_value, "mm");
        assert_eq!(coded_concepts::degree().scheme, "UCUM");
        assert_eq!(coded_concepts::degree().code_value, "deg");
        assert_eq!(coded_concepts::pixel().scheme, "UCUM");
        assert_eq!(coded_concepts::pixel().code_value, "pixel");
    }

    #[test]
    fn coded_concepts_observation_context_and_container() {
        // REQ-SR-1500
        let obs = coded_concepts::observation_context();
        assert_eq!(obs.scheme, "DCM");
        assert_eq!(obs.code_value, "121005");
        let cont = coded_concepts::container();
        assert_eq!(cont.scheme, "DCM");
        assert_eq!(cont.code_value, "111028");
    }

    #[test]
    fn build_measurement_report_with_empty_measurements() {
        // REQ-SR-1500
        let doc = build_measurement_report(
            "1.2.3",
            "1.2.3.4",
            "1.2.3.4.5",
            "test-observer",
            &[],
        );
        assert_eq!(doc.provenance.study_instance_uid, "1.2.3");
        assert_eq!(doc.provenance.series_instance_uid, "1.2.3.4");
        assert_eq!(doc.provenance.sop_instance_uid, "1.2.3.4.5");
        assert_eq!(doc.provenance.observer, "test-observer");
        assert_eq!(doc.version, 1);
        // Should contain only the measurement report container code item
        assert_eq!(doc.items.len(), 1);
        assert!(matches!(
            &doc.items[0],
            SrAuthoringContentItem::Code { concept, value, .. }
            if concept.code_value == "126000" && value.code_value == "111028"
        ));
    }

    #[test]
    fn build_measurement_report_with_multiple_measurements() {
        // REQ-SR-1500, REQ-SR-300
        let measurements = vec![
            SrMeasurement {
                concept: coded_concepts::distance(),
                value: 42.5,
                units: coded_concepts::millimeter(),
                referenced_sop_instance_uid: Some("1.2.3.4.5.6".to_string()),
            },
            SrMeasurement {
                concept: coded_concepts::angle(),
                value: 90.0,
                units: coded_concepts::degree(),
                referenced_sop_instance_uid: None,
            },
            SrMeasurement {
                concept: coded_concepts::probe(),
                value: 128.0,
                units: coded_concepts::pixel(),
                referenced_sop_instance_uid: None,
            },
        ];
        let doc = build_measurement_report(
            "1.2.3",
            "1.2.3.4",
            "1.2.3.4.5",
            "radiologist-1",
            &measurements,
        );
        // 1 container code + 3 measurement items = 4 total
        assert_eq!(doc.items.len(), 4);
        // Items are sorted: CODE before NUM
        let code_count = doc
            .items
            .iter()
            .filter(|i| matches!(i, SrAuthoringContentItem::Code { .. }))
            .count();
        let num_count = doc
            .items
            .iter()
            .filter(|i| matches!(i, SrAuthoringContentItem::Num { .. }))
            .count();
        assert_eq!(code_count, 1);
        assert_eq!(num_count, 3);
        assert_eq!(doc.provenance.observer, "radiologist-1");
    }

    #[test]
    fn build_measurement_report_preserves_measurement_values() {
        // REQ-SR-1500, REQ-SR-300
        let measurements = vec![SrMeasurement {
            concept: coded_concepts::distance(),
            value: 123.456,
            units: coded_concepts::millimeter(),
            referenced_sop_instance_uid: Some("1.2.999".to_string()),
        }];
        let doc = build_measurement_report(
            "1.2.3",
            "1.2.3.4",
            "1.2.3.4.5",
            "observer",
            &measurements,
        );
        let num_items: Vec<_> = doc
            .items
            .iter()
            .filter_map(|i| match i {
                SrAuthoringContentItem::Num { value, .. } => Some(*value),
                _ => None,
            })
            .collect();
        assert_eq!(num_items.len(), 1);
        assert!((num_items[0] - 123.456).abs() < f64::EPSILON);
    }

    #[test]
    fn build_measurement_report_serializable_and_parseable() {
        // REQ-SR-1500
        let measurements = vec![
            SrMeasurement {
                concept: coded_concepts::distance(),
                value: 50.0,
                units: coded_concepts::millimeter(),
                referenced_sop_instance_uid: None,
            },
        ];
        let doc = build_measurement_report(
            "1.2.3",
            "1.2.3.4",
            "1.2.3.4.5",
            "observer",
            &measurements,
        );
        let serialized = serialize_authored_document(&doc);
        let parsed = parse_authored_document(&serialized).expect("parse");
        assert_eq!(parsed, doc);
    }

    #[test]
    fn build_tid300_measurement_creates_num_item() {
        // REQ-SR-300
        let item = build_tid300_measurement(
            coded_concepts::distance(),
            99.9,
            coded_concepts::millimeter(),
            Some("1.2.3.4.5".to_string()),
        );
        match &item {
            SrAuthoringContentItem::Num {
                concept,
                value,
                units,
                referenced_sop_instance_uid,
            } => {
                assert_eq!(concept.code_value, "121206");
                assert!((*value - 99.9).abs() < f64::EPSILON);
                assert_eq!(units.code_value, "mm");
                assert_eq!(
                    referenced_sop_instance_uid.as_deref(),
                    Some("1.2.3.4.5")
                );
            }
            _ => panic!("expected NUM item"),
        }
    }

    #[test]
    fn build_tid300_measurement_without_reference() {
        // REQ-SR-300
        let item = build_tid300_measurement(
            coded_concepts::angle(),
            45.0,
            coded_concepts::degree(),
            None,
        );
        match &item {
            SrAuthoringContentItem::Num {
                concept,
                value,
                units,
                referenced_sop_instance_uid,
            } => {
                assert_eq!(concept.code_value, "121207");
                assert_eq!(*value, 45.0);
                assert_eq!(units.code_value, "deg");
                assert!(referenced_sop_instance_uid.is_none());
            }
            _ => panic!("expected NUM item"),
        }
    }

    #[test]
    fn build_tid300_measurement_can_be_pushed_to_builder() {
        // REQ-SR-300
        let item = build_tid300_measurement(
            coded_concepts::probe(),
            255.0,
            coded_concepts::pixel(),
            None,
        );
        let doc = SrAuthoringBuilder::new("1.2.3", "1.2.3.4", "1.2.3.4.5")
            .push_item(item)
            .build();
        assert_eq!(doc.items.len(), 1);
        assert!(matches!(
            &doc.items[0],
            SrAuthoringContentItem::Num { concept, .. }
            if concept.code_value == "121208"
        ));
    }

    #[test]
    fn measurement_report_extract_measurements_roundtrip() {
        // REQ-SR-1500, REQ-SR-300
        let measurements = vec![
            SrMeasurement {
                concept: coded_concepts::distance(),
                value: 10.5,
                units: coded_concepts::millimeter(),
                referenced_sop_instance_uid: Some("1.2.3.4.5.6".to_string()),
            },
            SrMeasurement {
                concept: coded_concepts::angle(),
                value: 30.0,
                units: coded_concepts::degree(),
                referenced_sop_instance_uid: None,
            },
        ];
        let doc = build_measurement_report(
            "1.2.3",
            "1.2.3.4",
            "1.2.3.4.5",
            "observer",
            &measurements,
        );
        let serialized = serialize_authored_document(&doc);
        let extracted = extract_measurements(&serialized).expect("extract");
        assert_eq!(extracted.len(), 2);
        // Check distance measurement
        let dist = extracted
            .iter()
            .find(|m| m.concept.code_value == "121206")
            .expect("distance measurement");
        assert!((dist.value - 10.5).abs() < f64::EPSILON);
        assert_eq!(dist.units.code_value, "mm");
        assert_eq!(
            dist.referenced_sop_instance_uid.as_deref(),
            Some("1.2.3.4.5.6")
        );
        // Check angle measurement
        let ang = extracted
            .iter()
            .find(|m| m.concept.code_value == "121207")
            .expect("angle measurement");
        assert!((ang.value - 30.0).abs() < f64::EPSILON);
        assert_eq!(ang.units.code_value, "deg");
        assert!(ang.referenced_sop_instance_uid.is_none());
    }
}
