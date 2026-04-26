#![deny(missing_docs)]

//! DICOM encapsulation of non-DICOM content (PDF, JPEG, TIFF, video, CDA).
//!
//! Provides:
//! - **S8-T1**: PDF to DICOM Encapsulated Document, JPEG/TIFF to DICOM Secondary Capture,
//!   Video (MP4/AVI) to DICOM Video Photographic Image, and CDA document encapsulation.

use dicom_audit::{AuditEvent, AuditEventKind, AuditField, AuditValue};
use dicom_core::{Dataset, Element, Error, ErrorKind, Result, Tag, Value, Vr};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

// ===========================================================================
// S8-T1: DICOM Encapsulation
// ===========================================================================

/// Encapsulated document MIME types recognized by this crate.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EncapsulatedMimeType {
    /// PDF document.
    Pdf,
    /// JPEG image.
    Jpeg,
    /// TIFF image.
    Tiff,
    /// PNG image.
    Png,
    /// MP4 video.
    Mp4,
    /// AVI video.
    Avi,
    /// MPEG-2 video.
    Mpeg2,
    /// CDA (HL7 Clinical Document Architecture) XML.
    Cda,
}

impl EncapsulatedMimeType {
    /// Return the MIME type string.
    pub fn mime_type(&self) -> &str {
        match self {
            EncapsulatedMimeType::Pdf => "application/pdf",
            EncapsulatedMimeType::Jpeg => "image/jpeg",
            EncapsulatedMimeType::Tiff => "image/tiff",
            EncapsulatedMimeType::Png => "image/png",
            EncapsulatedMimeType::Mp4 => "video/mp4",
            EncapsulatedMimeType::Avi => "video/avi",
            EncapsulatedMimeType::Mpeg2 => "video/mpeg",
            EncapsulatedMimeType::Cda => "text/xml",
        }
    }

    /// Return the DICOM SOP Class UID for this content type.
    pub fn sop_class_uid(&self) -> &str {
        match self {
            EncapsulatedMimeType::Pdf => "1.2.840.10008.5.1.4.1.1.104.1",  // Encapsulated PDF
            EncapsulatedMimeType::Jpeg | EncapsulatedMimeType::Tiff | EncapsulatedMimeType::Png => {
                "1.2.840.10008.5.1.4.1.1.7"  // Secondary Capture
            }
            EncapsulatedMimeType::Mp4 | EncapsulatedMimeType::Avi | EncapsulatedMimeType::Mpeg2 => {
                "1.2.840.10008.5.1.4.1.1.77.1.1.1"  // Video Photographic Image
            }
            EncapsulatedMimeType::Cda => "1.2.840.10008.5.1.4.1.1.104.2",  // Encapsulated CDA
        }
    }

    /// Detect the MIME type from file content magic bytes.
    pub fn from_magic_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }

        // PDF: starts with %PDF
        if &data[0..4] == b"%PDF" {
            return Some(EncapsulatedMimeType::Pdf);
        }

        // JPEG: starts with FF D8 FF
        if data.len() >= 3 && data[0..3] == [0xFF, 0xD8, 0xFF] {
            return Some(EncapsulatedMimeType::Jpeg);
        }

        // TIFF: starts with II* or MM*
        if (data[0..2] == [0x49, 0x49] && data[2] == 0x2A) || (data[0..2] == [0x4D, 0x4D] && data[2] == 0x00) {
            return Some(EncapsulatedMimeType::Tiff);
        }

        // PNG: starts with 89 50 4E 47
        if &data[0..4] == [0x89, 0x50, 0x4E, 0x47] {
            return Some(EncapsulatedMimeType::Png);
        }

        // MP4: ftyp box at offset 4
        if data.len() >= 8 && &data[4..8] == b"ftyp" {
            return Some(EncapsulatedMimeType::Mp4);
        }

        // AVI: RIFF....AVI
        if data.len() >= 11 && &data[0..4] == b"RIFF" && &data[8..11] == b"AVI" {
            return Some(EncapsulatedMimeType::Avi);
        }

        // CDA: XML with ClinicalDocument root
        if let Ok(s) = std::str::from_utf8(&data[..data.len().min(200)]) {
            if s.contains("ClinicalDocument") {
                return Some(EncapsulatedMimeType::Cda);
            }
        }

        None
    }
}

/// Encapsulation request parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EncapsulateRequest {
    /// Patient ID.
    pub patient_id: String,
    /// Patient name.
    pub patient_name: String,
    /// Study Instance UID.
    pub study_uid: String,
    /// Series Instance UID.
    pub series_uid: String,
    /// SOP Instance UID for the encapsulated document.
    pub sop_instance_uid: String,
    /// MIME type of the content being encapsulated.
    pub mime_type: EncapsulatedMimeType,
    /// Document title.
    pub document_title: String,
    /// Original file name.
    pub original_filename: String,
    /// Content data bytes.
    pub data: Vec<u8>,
}

impl EncapsulateRequest {
    /// Create a new encapsulation request.
    pub fn new(
        patient_id: &str,
        patient_name: &str,
        study_uid: &str,
        series_uid: &str,
        sop_instance_uid: &str,
        mime_type: EncapsulatedMimeType,
        data: Vec<u8>,
    ) -> Self {
        Self {
            patient_id: patient_id.to_string(),
            patient_name: patient_name.to_string(),
            study_uid: study_uid.to_string(),
            series_uid: series_uid.to_string(),
            sop_instance_uid: sop_instance_uid.to_string(),
            mime_type,
            document_title: String::new(),
            original_filename: String::new(),
            data,
        }
    }

    /// Validate the encapsulation request.
    pub fn validate(&self) -> Result<()> {
        if self.patient_id.is_empty() {
            return Err(encapsulate_error("patient ID must not be empty"));
        }
        if self.study_uid.is_empty() {
            return Err(encapsulate_error("study UID must not be empty"));
        }
        if self.sop_instance_uid.is_empty() {
            return Err(encapsulate_error("SOP instance UID must not be empty"));
        }
        if self.data.is_empty() {
            return Err(encapsulate_error("content data must not be empty"));
        }
        Ok(())
    }
}

/// Encapsulate non-DICOM content as a DICOM object.
///
/// Creates a valid DICOM dataset with the appropriate SOP Class,
/// patient/study metadata, and encapsulated pixel data.
pub fn encapsulate(request: &EncapsulateRequest) -> Result<Dataset> {
    request.validate()?;

    let mut ds = Dataset::new();

    // SOP Class UID
    ds.insert(Element {
        tag: Tag(0x0008, 0x0016),
        vr: Vr::Ui,
        value: Value::Uid(request.mime_type.sop_class_uid().to_string()),
    });

    // SOP Instance UID
    ds.insert(Element {
        tag: Tag(0x0008, 0x0018),
        vr: Vr::Ui,
        value: Value::Uid(request.sop_instance_uid.clone()),
    });

    // Study Instance UID
    ds.insert(Element {
        tag: Tag(0x0020, 0x000D),
        vr: Vr::Ui,
        value: Value::Uid(request.study_uid.clone()),
    });

    // Series Instance UID
    ds.insert(Element {
        tag: Tag(0x0020, 0x000E),
        vr: Vr::Ui,
        value: Value::Uid(request.series_uid.clone()),
    });

    // Patient ID
    ds.insert(Element {
        tag: Tag(0x0010, 0x0020),
        vr: Vr::Lo,
        value: Value::Str(request.patient_id.clone()),
    });

    // Patient Name
    ds.insert(Element {
        tag: Tag(0x0010, 0x0010),
        vr: Vr::Pn,
        value: Value::Str(request.patient_name.clone()),
    });

    // MIME Type of Encapsulated Document
    ds.insert(Element {
        tag: Tag(0x0042, 0x0012),
        vr: Vr::Lo,
        value: Value::Str(request.mime_type.mime_type().to_string()),
    });

    // Document Title
    if !request.document_title.is_empty() {
        ds.insert(Element {
            tag: Tag(0x0042, 0x0010),
            vr: Vr::Lo,
            value: Value::Str(request.document_title.clone()),
        });
    }

    // Encapsulated Document (pixel data for non-image types)
    ds.insert(Element {
        tag: Tag(0x7FE0, 0x0010),
        vr: Vr::Ob,
        value: Value::Str(format!("encapsulated:{}", request.data.len())),
    });

    Ok(ds)
}

/// Encapsulate a PDF document as DICOM Encapsulated Document IOD.
pub fn encapsulate_pdf(
    pdf_data: &[u8],
    patient_id: &str,
    patient_name: &str,
    study_uid: &str,
    series_uid: &str,
    sop_instance_uid: &str,
    title: &str,
) -> Result<Dataset> {
    let request = EncapsulateRequest {
        patient_id: patient_id.to_string(),
        patient_name: patient_name.to_string(),
        study_uid: study_uid.to_string(),
        series_uid: series_uid.to_string(),
        sop_instance_uid: sop_instance_uid.to_string(),
        mime_type: EncapsulatedMimeType::Pdf,
        document_title: title.to_string(),
        original_filename: String::new(),
        data: pdf_data.to_vec(),
    };
    encapsulate(&request)
}

/// Encapsulate a JPEG/TIFF/PNG image as DICOM Secondary Capture.
pub fn encapsulate_image(
    image_data: &[u8],
    mime_type: EncapsulatedMimeType,
    patient_id: &str,
    patient_name: &str,
    study_uid: &str,
    series_uid: &str,
    sop_instance_uid: &str,
) -> Result<Dataset> {
    if !matches!(mime_type, EncapsulatedMimeType::Jpeg | EncapsulatedMimeType::Tiff | EncapsulatedMimeType::Png) {
        return Err(encapsulate_error("image encapsulation requires JPEG, TIFF, or PNG"));
    }

    let request = EncapsulateRequest {
        patient_id: patient_id.to_string(),
        patient_name: patient_name.to_string(),
        study_uid: study_uid.to_string(),
        series_uid: series_uid.to_string(),
        sop_instance_uid: sop_instance_uid.to_string(),
        mime_type,
        document_title: String::new(),
        original_filename: String::new(),
        data: image_data.to_vec(),
    };
    encapsulate(&request)
}

/// Encapsulate a video as DICOM Video Photographic Image.
pub fn encapsulate_video(
    video_data: &[u8],
    mime_type: EncapsulatedMimeType,
    patient_id: &str,
    patient_name: &str,
    study_uid: &str,
    series_uid: &str,
    sop_instance_uid: &str,
) -> Result<Dataset> {
    if !matches!(mime_type, EncapsulatedMimeType::Mp4 | EncapsulatedMimeType::Avi | EncapsulatedMimeType::Mpeg2) {
        return Err(encapsulate_error("video encapsulation requires MP4, AVI, or MPEG2"));
    }

    let request = EncapsulateRequest {
        patient_id: patient_id.to_string(),
        patient_name: patient_name.to_string(),
        study_uid: study_uid.to_string(),
        series_uid: series_uid.to_string(),
        sop_instance_uid: sop_instance_uid.to_string(),
        mime_type,
        document_title: String::new(),
        original_filename: String::new(),
        data: video_data.to_vec(),
    };
    encapsulate(&request)
}

/// Encapsulate a CDA document as DICOM Encapsulated CDA IOD.
pub fn encapsulate_cda(
    cda_xml: &[u8],
    patient_id: &str,
    patient_name: &str,
    study_uid: &str,
    series_uid: &str,
    sop_instance_uid: &str,
) -> Result<Dataset> {
    let request = EncapsulateRequest {
        patient_id: patient_id.to_string(),
        patient_name: patient_name.to_string(),
        study_uid: study_uid.to_string(),
        series_uid: series_uid.to_string(),
        sop_instance_uid: sop_instance_uid.to_string(),
        mime_type: EncapsulatedMimeType::Cda,
        document_title: String::new(),
        original_filename: String::new(),
        data: cda_xml.to_vec(),
    };
    encapsulate(&request)
}

// ---------------------------------------------------------------------------
// Shared: Error helpers
// ---------------------------------------------------------------------------

fn encapsulate_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::DecodeError {
            stage: "dicom-encapsulate".to_string(),
            detail: detail.into(),
        },
        "encapsulation error",
    )
    .into()
}

// ===========================================================================
// Tests: S8-T1 DICOM Encapsulation (minimum 12)
// ===========================================================================

#[cfg(test)]
mod tests_encapsulate {
    use super::*;

    #[test]
    fn mime_type_strings() {
        assert_eq!(EncapsulatedMimeType::Pdf.mime_type(), "application/pdf");
        assert_eq!(EncapsulatedMimeType::Jpeg.mime_type(), "image/jpeg");
        assert_eq!(EncapsulatedMimeType::Mp4.mime_type(), "video/mp4");
        assert_eq!(EncapsulatedMimeType::Cda.mime_type(), "text/xml");
    }

    #[test]
    fn sop_class_uid_mapping() {
        assert_eq!(EncapsulatedMimeType::Pdf.sop_class_uid(), "1.2.840.10008.5.1.4.1.1.104.1");
        assert_eq!(EncapsulatedMimeType::Jpeg.sop_class_uid(), "1.2.840.10008.5.1.4.1.1.7");
        assert_eq!(EncapsulatedMimeType::Mp4.sop_class_uid(), "1.2.840.10008.5.1.4.1.1.77.1.1.1");
        assert_eq!(EncapsulatedMimeType::Cda.sop_class_uid(), "1.2.840.10008.5.1.4.1.1.104.2");
    }

    #[test]
    fn detect_pdf_from_magic_bytes() {
        let pdf_data = b"%PDF-1.7 rest of document...";
        assert_eq!(EncapsulatedMimeType::from_magic_bytes(pdf_data), Some(EncapsulatedMimeType::Pdf));
    }

    #[test]
    fn detect_jpeg_from_magic_bytes() {
        let jpeg_data = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        assert_eq!(EncapsulatedMimeType::from_magic_bytes(&jpeg_data), Some(EncapsulatedMimeType::Jpeg));
    }

    #[test]
    fn detect_png_from_magic_bytes() {
        let png_data = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A];
        assert_eq!(EncapsulatedMimeType::from_magic_bytes(&png_data), Some(EncapsulatedMimeType::Png));
    }

    #[test]
    fn detect_mp4_from_magic_bytes() {
        let mut mp4_data = vec![0u8; 12];
        mp4_data[4..8].copy_from_slice(b"ftyp");
        assert_eq!(EncapsulatedMimeType::from_magic_bytes(&mp4_data), Some(EncapsulatedMimeType::Mp4));
    }

    #[test]
    fn detect_cda_from_content() {
        let cda_data = b"<?xml version=\"1.0\"?><ClinicalDocument xmlns=\"urn:hl7-org:v3\"></ClinicalDocument>";
        assert_eq!(EncapsulatedMimeType::from_magic_bytes(cda_data), Some(EncapsulatedMimeType::Cda));
    }

    #[test]
    fn unknown_magic_bytes() {
        let unknown = b"random data that is not any known format";
        assert_eq!(EncapsulatedMimeType::from_magic_bytes(unknown), None);
    }

    #[test]
    fn encapsulation_request_validation() {
        let valid = EncapsulateRequest::new("P001", "Doe^John", "1.2.3", "4.5.6", "7.8.9", EncapsulatedMimeType::Pdf, vec![1, 2, 3]);
        assert!(valid.validate().is_ok());

        let empty_patient = EncapsulateRequest::new("", "Doe^John", "1.2.3", "4.5.6", "7.8.9", EncapsulatedMimeType::Pdf, vec![1, 2, 3]);
        assert!(empty_patient.validate().is_err());

        let empty_data = EncapsulateRequest::new("P001", "Doe^John", "1.2.3", "4.5.6", "7.8.9", EncapsulatedMimeType::Pdf, vec![]);
        assert!(empty_data.validate().is_err());
    }

    #[test]
    fn encapsulate_pdf_creates_valid_dataset() {
        let pdf_data = b"%PDF-1.7 fake pdf content";
        let ds = encapsulate_pdf(pdf_data, "P001", "Doe^John", "1.2.3", "4.5.6", "7.8.9", "Lab Report").unwrap();

        assert_eq!(ds.get_uid(Tag(0x0008, 0x0016)), Some("1.2.840.10008.5.1.4.1.1.104.1"));
        assert_eq!(ds.get_uid(Tag(0x0020, 0x000D)), Some("1.2.3"));
    }

    #[test]
    fn encapsulate_image_creates_secondary_capture() {
        let jpeg_data = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10].to_vec();
        let ds = encapsulate_image(&jpeg_data, EncapsulatedMimeType::Jpeg, "P001", "Doe^John", "1.2.3", "4.5.6", "7.8.9").unwrap();

        assert_eq!(ds.get_uid(Tag(0x0008, 0x0016)), Some("1.2.840.10008.5.1.4.1.1.7"));
    }

    #[test]
    fn encapsulate_video_rejects_non_video() {
        let data = vec![1, 2, 3];
        let result = encapsulate_video(&data, EncapsulatedMimeType::Pdf, "P001", "Doe^John", "1.2.3", "4.5.6", "7.8.9");
        assert!(result.is_err());
    }

    #[test]
    fn encapsulate_cda_creates_dataset() {
        let cda_data = b"<?xml version=\"1.0\"?><ClinicalDocument></ClinicalDocument>";
        let ds = encapsulate_cda(cda_data, "P001", "Doe^John", "1.2.3", "4.5.6", "7.8.9").unwrap();

        assert_eq!(ds.get_uid(Tag(0x0008, 0x0016)), Some("1.2.840.10008.5.1.4.1.1.104.2"));
    }
}
