//! DICOM-to-FHIR mapping reference tables.
//!
//! Provides comprehensive mapping documentation between DICOM attributes
//! and FHIR R4 resource elements. These mappings follow the HL7 DICOM-SR
//! on FHIR Implementation Guide and the FHIR ImagingStudy resource
//! specification.

/// Modality → FHIR ImagingStudy.modality mapping.
///
/// Maps DICOM Modality (0008,0060) codes to FHIR Coding elements with
/// the DCM (DICOM Modality) code system.
pub mod modality_mapping {
    /// FHIR code system for DICOM modality codes.
    pub const MODALITY_CODE_SYSTEM: &str =
        "http://dicom.nema.org/resources/ontology/DCM";

    /// Mapping table: DICOM modality code → (FHIR code, display name).
    ///
    /// The FHIR code is typically the same as the DICOM modality code,
    /// but the display name provides a human-readable description.
    pub const MODALITY_MAP: &[(&str, &str, &str)] = &[
        // (DICOM Code, FHIR Code, Display Name)
        ("CT", "CT", "Computed Tomography"),
        ("MR", "MR", "Magnetic Resonance"),
        ("MG", "MG", "Mammography"),
        ("US", "US", "Ultrasound"),
        ("XR", "XR", "X-Ray"),
        ("NM", "NM", "Nuclear Medicine"),
        ("PT", "PT", "PET"),
        ("XA", "XA", "X-Ray Angiography"),
        ("RF", "RF", "Radio Fluoroscopy"),
        ("SR", "SR", "Structured Report"),
        ("CR", "CR", "Computed Radiography"),
        ("DX", "DX", "Digital Radiography"),
        ("IO", "IO", "Intra-oral Radiography"),
        ("PX", "PX", "Panoramic X-Ray"),
        ("PT", "PT", "PET"),
        ("IVUS", "IVUS", "Intravascular Ultrasound"),
        ("OP", "OP", "Ophthalmic Photography"),
        ("SM", "SM", "Slide Microscopy"),
        ("DOC", "DOC", "Document"),
        ("OT", "OT", "Other"),
    ];

    /// Look up the display name for a modality code.
    pub fn display_for_code(code: &str) -> Option<&'static str> {
        MODALITY_MAP
            .iter()
            .find(|(c, _, _)| *c == code)
            .map(|(_, _, display)| *display)
    }

    /// Look up the FHIR code for a DICOM modality code.
    pub fn fhir_code_for_dicom(code: &str) -> Option<&'static str> {
        MODALITY_MAP
            .iter()
            .find(|(c, _, _)| *c == code)
            .map(|(_, fhir_code, _)| *fhir_code)
    }
}

/// DICOM Patient (0010,xxxx) → FHIR Patient mapping.
///
/// Maps DICOM Patient Module attributes to FHIR R4 Patient resource
/// elements.
pub mod patient_mapping {
    /// Mapping table: DICOM tag → FHIR Patient element.
    ///
    /// Each entry describes a DICOM tag, its FHIR path, and the
    /// transformation applied during mapping.
    pub const PATIENT_MAP: &[(&str, &str, &str, &str)] = &[
        // (DICOM Tag, Tag Name, FHIR Path, Transform)
        ("(0010,0020)", "Patient ID", "Patient.identifier[0].value", "Direct copy"),
        ("(0010,0010)", "Patient Name", "Patient.name[0]", "PN: Family^Given → HumanName"),
        ("(0010,0030)", "Patient Birth Date", "Patient.birthDate", "YYYYMMDD → YYYY-MM-DD"),
        ("(0010,0040)", "Patient Sex", "Patient.gender", "M→male, F→female, O→other, U→unknown"),
        ("(0010,1002)", "Other Patient IDs", "Patient.identifier[1+]", "Direct copy with alternate system"),
        ("(0010,1005)", "Patient Birth Name", "Patient.name[1]", "PN parse for birth name"),
        ("(0010,1010)", "Patient Age", "Patient.extension(age)", "Convert to FHIR Age extension"),
        ("(0010,1020)", "Patient Size", "Patient.extension(height)", "Convert to FHIR BodyHeight"),
        ("(0010,1030)", "Patient Weight", "Patient.extension(weight)", "Convert to FHIR BodyWeight"),
        ("(0010,2154)", "Patient Phone Numbers", "Patient.telecom", "Map to ContactPoint"),
        ("(0010,1040)", "Patient Address", "Patient.address", "Map to Address"),
    ];

    /// Map DICOM sex code to FHIR AdministrativeGender.
    pub fn map_gender(dicom_sex: &str) -> &'static str {
        match dicom_sex {
            "M" => "male",
            "F" => "female",
            "O" => "other",
            "U" | _ => "unknown",
        }
    }

    /// Convert DICOM DA (Date) to FHIR date format.
    pub fn map_date(dicom_date: &str) -> String {
        if dicom_date.len() >= 8 {
            format!(
                "{}-{}-{}",
                &dicom_date[0..4],
                &dicom_date[4..6],
                &dicom_date[6..8]
            )
        } else {
            dicom_date.to_string()
        }
    }

    /// Parse DICOM Person Name (PN) into family and given name components.
    ///
    /// DICOM PN format: "Family^Given^Middle^Prefix^Suffix"
    pub fn parse_person_name(pn: &str) -> (Option<&str>, Vec<&str>) {
        let parts: Vec<&str> = pn.split('^').collect();
        let family = parts.first().and_then(|s| if s.is_empty() { None } else { Some(*s) });
        let given: Vec<&str> = parts
            .iter()
            .skip(1)
            .filter_map(|s| if s.is_empty() { None } else { Some(*s) })
            .collect();
        (family, given)
    }
}

/// DICOM Study (0008,xxxx / 0020,xxxx) → FHIR ImagingStudy mapping.
///
/// Maps DICOM General Study Module attributes to FHIR R4 ImagingStudy
/// resource elements.
pub mod study_mapping {
    /// Mapping table: DICOM tag → FHIR ImagingStudy element.
    pub const STUDY_MAP: &[(&str, &str, &str, &str)] = &[
        // (DICOM Tag, Tag Name, FHIR Path, Transform)
        ("(0020,000D)", "Study Instance UID", "ImagingStudy.identifier[0].value", "Direct copy; also ImagingStudy.studyUid"),
        ("(0008,0020)", "Study Date", "ImagingStudy.started", "YYYYMMDD → YYYY-MM-DD"),
        ("(0008,1030)", "Study Description", "ImagingStudy.description", "Direct copy"),
        ("(0008,0050)", "Accession Number", "ImagingStudy.accession.value", "Direct copy"),
        ("(0008,0060)", "Modality", "ImagingStudy.modality[0].code", "Map via DCM code system"),
        ("(0008,0021)", "Series Date", "ImagingStudy.series[n].started", "YYYYMMDD → YYYY-MM-DD"),
        ("(0008,103E)", "Series Description", "ImagingStudy.series[n].description", "Direct copy"),
        ("(0020,000E)", "Series Instance UID", "ImagingStudy.series[n].uid", "Direct copy"),
        ("(0008,0018)", "SOP Instance UID", "ImagingStudy.series[n].instance[m].uid", "Direct copy"),
        ("(0008,0016)", "SOP Class UID", "ImagingStudy.series[n].instance[m].sopClass.code", "Wrap as urn:oid:UID"),
        ("(0020,1209)", "Number of Series Related Instances", "ImagingStudy.series[n].numberOfInstances", "Cast to integer"),
        ("(0010,0020)", "Patient ID", "ImagingStudy.subject.reference", "Build Patient/PAT_ID reference"),
    ];

    /// Map Study Instance UID to FHIR ImagingStudy.id.
    ///
    /// FHIR ids must be ASCII alphanumeric/hyphen/dot, so UID dots
    /// are preserved but other characters are replaced.
    pub fn study_uid_to_fhir_id(uid: &str) -> String {
        uid.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    }
}

/// DICOM SR → FHIR Observation mapping.
///
/// Maps DICOM Structured Report measurement data to FHIR R4 Observation
/// resources per the HL7 DICOM-SR on FHIR Implementation Guide.
pub mod sr_observation_mapping {
    /// SNOMED CT codes for common SR measurement types.
    pub const MEASUREMENT_CODES: &[(&str, &str, &str)] = &[
        // (SNOMED Code, Display, DICOM SR Concept)
        ("410668003", "Length of lesion", "Distance measurement"),
        ("103355000", "Area of region of interest", "Area measurement"),
        ("103356004", "Volume of region of interest", "Volume measurement"),
        ("42798000", "Area of structure", "Calculated area"),
        ("76853006", "Angle", "Angle measurement"),
        ("250857004", "Velocity", "Doppler velocity"),
        ("368499004", "Signal intensity", "Signal measurement"),
        ("442557002", "Attenuation coefficient", "HU measurement"),
    ];

    /// UCUM unit codes for common measurement units.
    pub const UNIT_CODES: &[(&str, &str, &str)] = &[
        // (UCUM Code, Display, DICOM Unit)
        ("mm", "millimeter", "mm"),
        ("cm", "centimeter", "cm"),
        ("m", "meter", "m"),
        ("mm2", "square millimeter", "mm2"),
        ("cm2", "square centimeter", "cm2"),
        ("mm3", "cubic millimeter", "mm3"),
        ("cm3", "cubic centimeter", "cm3"),
        ("deg", "degree", "deg"),
        ("{pixels}", "pixels", "pixel"),
        ("[hnsf'U]", "Hounsfield unit", "HU"),
        ("cm/s", "centimeter per second", "cm/s"),
    ];

    /// Observation category coding for imaging measurements.
    pub const IMAGING_CATEGORY: (&str, &str, &str) = (
        "http://terminology.hl7.org/CodeSystem/observation-category",
        "imaging",
        "Imaging",
    );

    /// Look up a measurement code by SNOMED code.
    pub fn find_measurement(code: &str) -> Option<&'static (&str, &'static str, &'static str)> {
        MEASUREMENT_CODES.iter().find(|(c, _, _)| *c == code)
    }

    /// Look up a UCUM unit code by DICOM unit string.
    pub fn find_unit(dicom_unit: &str) -> Option<&'static (&str, &'static str, &'static str)> {
        UNIT_CODES.iter().find(|(_, _, du)| *du == dicom_unit)
    }
}

/// Summary of all mapping tables for documentation generation.
pub mod mapping_summary {
    /// Total number of modality mappings.
    pub const MODALITY_MAPPINGS: usize = 20;
    /// Total number of patient attribute mappings.
    pub const PATIENT_MAPPINGS: usize = 11;
    /// Total number of study attribute mappings.
    pub const STUDY_MAPPINGS: usize = 12;
    /// Total number of SR measurement codes.
    pub const SR_MEASUREMENT_CODES: usize = 8;
    /// Total number of UCUM unit codes.
    pub const SR_UNIT_CODES: usize = 11;

    /// FHIR R4 code systems used in DICOM-to-FHIR mapping.
    pub const CODE_SYSTEMS: &[(&str, &str)] = &[
        ("DCM", "http://dicom.nema.org/resources/ontology/DCM"),
        ("SNOMED CT", "http://snomed.info/sct"),
        ("UCUM", "http://unitsofmeasure.org"),
        ("LOINC", "http://loinc.org"),
        ("observation-category", "http://terminology.hl7.org/CodeSystem/observation-category"),
        ("DICOM UID", "urn:dicom:study_uid"),
        ("DICOM Patient", "urn:dicom:patient_id"),
        ("DICOM Accession", "urn:dicom:accession"),
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modality_lookup() {
        assert_eq!(
            modality_mapping::display_for_code("CT"),
            Some("Computed Tomography")
        );
        assert_eq!(
            modality_mapping::display_for_code("MR"),
            Some("Magnetic Resonance")
        );
        assert_eq!(modality_mapping::display_for_code("UNKNOWN"), None);
    }

    #[test]
    fn modality_fhir_code() {
        assert_eq!(modality_mapping::fhir_code_for_dicom("CT"), Some("CT"));
        assert_eq!(modality_mapping::fhir_code_for_dicom("SR"), Some("SR"));
    }

    #[test]
    fn gender_mapping() {
        assert_eq!(patient_mapping::map_gender("M"), "male");
        assert_eq!(patient_mapping::map_gender("F"), "female");
        assert_eq!(patient_mapping::map_gender("O"), "other");
        assert_eq!(patient_mapping::map_gender("U"), "unknown");
        assert_eq!(patient_mapping::map_gender("X"), "unknown");
    }

    #[test]
    fn date_mapping() {
        assert_eq!(patient_mapping::map_date("20240115"), "2024-01-15");
        assert_eq!(patient_mapping::map_date("19800101"), "1980-01-01");
        assert_eq!(patient_mapping::map_date("short"), "short");
    }

    #[test]
    fn person_name_parsing() {
        let (family, given) = patient_mapping::parse_person_name("Smith^John^M");
        assert_eq!(family, Some("Smith"));
        assert_eq!(given, vec!["John", "M"]);

        let (family, given) = patient_mapping::parse_person_name("Doe^Jane");
        assert_eq!(family, Some("Doe"));
        assert_eq!(given, vec!["Jane"]);

        let (family, given) = patient_mapping::parse_person_name("Single");
        assert_eq!(family, Some("Single"));
        assert!(given.is_empty());

        let (family, given) = patient_mapping::parse_person_name("^OnlyGiven");
        assert!(family.is_none());
        assert_eq!(given, vec!["OnlyGiven"]);
    }

    #[test]
    fn study_uid_to_fhir_id() {
        assert_eq!(
            study_mapping::study_uid_to_fhir_id("1.2.840.113619.2.55.3"),
            "1.2.840.113619.2.55.3"
        );
        assert_eq!(
            study_mapping::study_uid_to_fhir_id("uid with spaces"),
            "uid_with_spaces"
        );
    }

    #[test]
    fn measurement_codes() {
        assert!(sr_observation_mapping::find_measurement("410668003").is_some());
        assert!(sr_observation_mapping::find_measurement("999999").is_none());
    }

    #[test]
    fn unit_codes() {
        assert!(sr_observation_mapping::find_unit("mm").is_some());
        assert!(sr_observation_mapping::find_unit("unknown").is_none());
    }

    #[test]
    fn mapping_summary_counts() {
        assert_eq!(
            mapping_summary::MODALITY_MAPPINGS,
            modality_mapping::MODALITY_MAP.len()
        );
        assert_eq!(
            mapping_summary::PATIENT_MAPPINGS,
            patient_mapping::PATIENT_MAP.len()
        );
        assert_eq!(
            mapping_summary::STUDY_MAPPINGS,
            study_mapping::STUDY_MAP.len()
        );
        assert_eq!(
            mapping_summary::SR_MEASUREMENT_CODES,
            sr_observation_mapping::MEASUREMENT_CODES.len()
        );
        assert_eq!(
            mapping_summary::SR_UNIT_CODES,
            sr_observation_mapping::UNIT_CODES.len()
        );
    }
}
