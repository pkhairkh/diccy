// Auto-extracted from /home/z/diccy/crates/dicom-fhir/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_fhir::*;
    use dicom_core::{Dataset, Element, Limits, Tag, Value, Vr};
    use dicom_audit::{AuditEvent, AuditValue};
    use std::sync::Arc;

    fn limits() -> Limits {
        Limits::default()
    }

    fn patient_dataset() -> Dataset {
        let mut ds = Dataset::new();
        ds.insert(Element::new(TAG_PATIENT_ID, Vr::Lo, Value::Str("PAT001".to_string())).unwrap());
        ds.insert(
            Element::new(
                TAG_PATIENT_NAME,
                Vr::Pn,
                Value::Str("Smith^John^M".to_string()),
            )
            .unwrap(),
        );
        ds.insert(
            Element::new(
                TAG_PATIENT_BIRTH_DATE,
                Vr::Da,
                Value::Str("19800101".to_string()),
            )
            .unwrap(),
        );
        ds.insert(Element::new(TAG_PATIENT_SEX, Vr::Cs, Value::Str("M".to_string())).unwrap());
        ds
    }

    fn study_dataset() -> Dataset {
        let mut ds = Dataset::new();
        ds.insert(Element::new(TAG_PATIENT_ID, Vr::Lo, Value::Str("PAT001".to_string())).unwrap());
        ds.insert(
            Element::new(
                TAG_STUDY_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.840.113619.2.55.3".to_string()),
            )
            .unwrap(),
        );
        ds.insert(
            Element::new(TAG_STUDY_DATE, Vr::Da, Value::Str("20240115".to_string())).unwrap(),
        );
        ds.insert(
            Element::new(
                TAG_STUDY_DESCRIPTION,
                Vr::Lo,
                Value::Str("CT Chest with Contrast".to_string()),
            )
            .unwrap(),
        );
        ds.insert(
            Element::new(
                TAG_ACCESSION_NUMBER,
                Vr::Sh,
                Value::Str("ACC12345".to_string()),
            )
            .unwrap(),
        );
        ds.insert(Element::new(TAG_MODALITY, Vr::Cs, Value::Str("CT".to_string())).unwrap());
        ds
    }

    #[test]
    fn map_patient_extracts_demographics() {
        let adapter = FhirAdapter::new("https://fhir.example.com");
        let ds = patient_dataset();
        let patient = adapter.map_patient(&ds).expect("patient mapping");

        assert_eq!(patient.resource_type, "Patient");
        assert_eq!(patient.id, "PAT001");
        assert_eq!(patient.name.len(), 1);
        assert_eq!(patient.name[0].family.as_deref(), Some("Smith"));
        assert_eq!(patient.name[0].given, vec!["John", "M"]);
        assert_eq!(patient.birth_date.as_deref(), Some("1980-01-01"));
        assert_eq!(patient.gender.as_deref(), Some("male"));
        assert_eq!(patient.identifier.len(), 1);
        assert_eq!(patient.identifier[0].value, "PAT001");
    }

    #[test]
    fn map_patient_missing_id_fails() {
        let adapter = FhirAdapter::new("https://fhir.example.com");
        let ds = Dataset::new();
        let result = adapter.map_patient(&ds);
        assert!(result.is_err());
    }

    #[test]
    fn map_imaging_study_extracts_study_metadata() {
        let adapter = FhirAdapter::new("https://fhir.example.com");
        let ds = study_dataset();
        let study = adapter.map_imaging_study(&ds).expect("study mapping");

        assert_eq!(study.resource_type, "ImagingStudy");
        assert_eq!(study.study_uid, "1.2.840.113619.2.55.3");
        assert_eq!(study.started.as_deref(), Some("2024-01-15"));
        assert_eq!(study.description.as_deref(), Some("CT Chest with Contrast"));
        assert!(study.accession.is_some());
        assert_eq!(study.modality.len(), 1);
        assert_eq!(study.modality[0].code, "CT");
    }

    #[test]
    fn map_imaging_study_missing_uid_fails() {
        let adapter = FhirAdapter::new("https://fhir.example.com");
        let ds = Dataset::new();
        let result = adapter.map_imaging_study(&ds);
        assert!(result.is_err());
    }

    #[test]
    fn add_series_to_study() {
        let adapter = FhirAdapter::new("https://fhir.example.com");
        let ds = study_dataset();
        let mut study = adapter.map_imaging_study(&ds).expect("study");

        let mut series_ds = Dataset::new();
        series_ds.insert(
            Element::new(
                TAG_SERIES_INSTANCE_UID,
                Vr::Ui,
                Value::Str("1.2.840.113619.2.55.3.1".to_string()),
            )
            .unwrap(),
        );
        series_ds.insert(Element::new(TAG_MODALITY, Vr::Cs, Value::Str("CT".to_string())).unwrap());
        series_ds.insert(
            Element::new(
                TAG_SERIES_DESCRIPTION,
                Vr::Lo,
                Value::Str("Axial 5mm".to_string()),
            )
            .unwrap(),
        );

        adapter
            .add_series_to_study(&mut study, &series_ds)
            .expect("add series");
        assert_eq!(study.series.len(), 1);
        assert_eq!(study.series[0].uid, "1.2.840.113619.2.55.3.1");
        assert_eq!(study.series[0].modality.code, "CT");
    }

    #[test]
    fn add_instance_to_series() {
        let adapter = FhirAdapter::new("https://fhir.example.com");
        let ds = study_dataset();
        let mut study = adapter.map_imaging_study(&ds).expect("study");

        // Add series first
        let mut series_ds = Dataset::new();
        series_ds.insert(
            Element::new(
                TAG_SERIES_INSTANCE_UID,
                Vr::Ui,
                Value::Str("1.2.3.4.5".to_string()),
            )
            .unwrap(),
        );
        series_ds.insert(Element::new(TAG_MODALITY, Vr::Cs, Value::Str("CT".to_string())).unwrap());
        adapter
            .add_series_to_study(&mut study, &series_ds)
            .expect("add series");

        // Add instance
        let mut instance_ds = Dataset::new();
        instance_ds.insert(
            Element::new(
                TAG_SOP_INSTANCE_UID,
                Vr::Ui,
                Value::Str("1.2.3.4.5.6".to_string()),
            )
            .unwrap(),
        );
        instance_ds.insert(
            Element::new(
                TAG_SOP_CLASS_UID,
                Vr::Ui,
                Value::Str("1.2.840.10008.5.1.4.1.1.2".to_string()),
            )
            .unwrap(),
        );

        adapter
            .add_instance_to_series(&mut study, "1.2.3.4.5", &instance_ds)
            .expect("add instance");
        assert_eq!(study.series[0].instance.len(), 1);
        assert_eq!(study.series[0].instance[0].uid, "1.2.3.4.5.6");
    }

    #[test]
    fn map_sr_observation_creates_fhir_observation() {
        let adapter = FhirAdapter::new("https://fhir.example.com");
        let obs = adapter.map_sr_observation(
            "obs-001",
            "PAT001",
            "410668003",
            "Length of lesion",
            25.3,
            "mm",
            "millimeter",
            Some("2024-01-15T10:30:00Z"),
            "1.2.840.113619.2.55.3",
        );

        assert_eq!(obs.resource_type, "Observation");
        assert_eq!(obs.id, "obs-001");
        assert_eq!(obs.category.len(), 1);
        assert_eq!(obs.category[0].coding[0].code, "imaging");
        assert_eq!(obs.code.coding[0].code, "410668003");
        assert!(obs.value_quantity.is_some());
        let qty = obs.value_quantity.unwrap();
        assert!((qty.value - 25.3).abs() < 0.01);
        assert_eq!(qty.code.as_deref(), Some("mm"));
        assert_eq!(obs.derived_from.len(), 1);
    }

    #[test]
    fn serialize_patient_to_json() {
        let adapter = FhirAdapter::new("https://fhir.example.com");
        let ds = patient_dataset();
        let patient = adapter.map_patient(&ds).expect("patient");
        let json = adapter.to_json(&patient).expect("json");
        assert!(json.contains("\"resource_type\":\"Patient\""));
        assert!(json.contains("\"id\":\"PAT001\""));
    }

    #[test]
    fn serialize_imaging_study_to_json() {
        let adapter = FhirAdapter::new("https://fhir.example.com");
        let ds = study_dataset();
        let study = adapter.map_imaging_study(&ds).expect("study");
        let json = adapter.to_json(&study).expect("json");
        assert!(json.contains("\"resource_type\":\"ImagingStudy\""));
    }

    #[test]
    fn serialize_observation_to_json() {
        let adapter = FhirAdapter::new("https://fhir.example.com");
        let obs = adapter.map_sr_observation(
            "obs-001",
            "PAT001",
            "410668003",
            "Length of lesion",
            25.3,
            "mm",
            "millimeter",
            None,
            "1.2.3.4.5",
        );
        let json = adapter.to_json(&obs).expect("json");
        assert!(json.contains("\"resource_type\":\"Observation\""));
    }

    #[test]
    fn parse_person_name_with_components() {
        let (family, given) = parse_dicom_person_name(Some("Smith^John^M"));
        assert_eq!(family.as_deref(), Some("Smith"));
        assert_eq!(given, vec!["John", "M"]);
    }

    #[test]
    fn parse_person_name_family_only() {
        let (family, given) = parse_dicom_person_name(Some("Smith"));
        assert_eq!(family.as_deref(), Some("Smith"));
        assert!(given.is_empty());
    }

    #[test]
    fn parse_person_name_none() {
        let (family, given) = parse_dicom_person_name(None);
        assert!(family.is_none());
        assert!(given.is_empty());
    }

    #[test]
    fn dicom_date_conversion() {
        assert_eq!(dicom_date_to_fhir("20240115"), "2024-01-15");
        assert_eq!(dicom_date_to_fhir("19800101"), "1980-01-01");
    }

    #[test]
    fn sanitize_fhir_id_replaces_invalid_chars() {
        assert_eq!(sanitize_fhir_id("1.2.840.113619"), "1.2.840.113619");
        assert_eq!(
            sanitize_fhir_id("patient id with spaces"),
            "patient_id_with_spaces"
        );
    }

    #[test]
    fn modality_display_names() {
        assert_eq!(modality_display_name("CT"), "Computed Tomography");
        assert_eq!(modality_display_name("MR"), "Magnetic Resonance");
        assert_eq!(modality_display_name("MG"), "Mammography");
        assert_eq!(modality_display_name("US"), "Ultrasound");
        assert_eq!(modality_display_name("UNKNOWN"), "UNKNOWN");
    }

    #[test]
    fn gender_mapping() {
        let adapter = FhirAdapter::new("https://fhir.example.com");

        let mut male_ds = Dataset::new();
        male_ds.insert(
            Element::new(TAG_PATIENT_ID, Vr::Lo, Value::Str("PAT001".to_string())).unwrap(),
        );
        male_ds.insert(
            Element::new(
                TAG_PATIENT_NAME,
                Vr::Pn,
                Value::Str("Smith^John".to_string()),
            )
            .unwrap(),
        );
        male_ds.insert(Element::new(TAG_PATIENT_SEX, Vr::Cs, Value::Str("M".to_string())).unwrap());
        let male = adapter.map_patient(&male_ds).expect("male");
        assert_eq!(male.gender.as_deref(), Some("male"));

        let mut female_ds = Dataset::new();
        female_ds.insert(
            Element::new(TAG_PATIENT_ID, Vr::Lo, Value::Str("PAT002".to_string())).unwrap(),
        );
        female_ds.insert(
            Element::new(TAG_PATIENT_NAME, Vr::Pn, Value::Str("Doe^Jane".to_string())).unwrap(),
        );
        female_ds
            .insert(Element::new(TAG_PATIENT_SEX, Vr::Cs, Value::Str("F".to_string())).unwrap());
        let female = adapter.map_patient(&female_ds).expect("female");
        assert_eq!(female.gender.as_deref(), Some("female"));
    }

    #[test]
    fn audit_callback_emits_events() {
        use std::sync::Mutex;

        let events = Arc::new(Mutex::new(Vec::<AuditEvent>::new()));
        let events_handle = Arc::clone(&events);
        let audit: AuditCallback = Arc::new(move |event| {
            events_handle.lock().expect("lock").push(event);
            Ok(())
        });

        let adapter = FhirAdapter::with_audit("https://fhir.example.com", Some(audit));
        let ds = patient_dataset();
        adapter.map_patient(&ds).expect("patient");

        let events = events.lock().expect("lock");
        assert_eq!(events.len(), 1);
        assert!(events[0].fields.iter().any(|f| f.key == "operation"
            && matches!(&f.value, AuditValue::Plain(v) if v == "map_patient")));
    }

    #[test]
    fn roundtrip_json_patient() {
        let adapter = FhirAdapter::new("https://fhir.example.com");
        let ds = patient_dataset();
        let patient = adapter.map_patient(&ds).expect("patient");
        let json = adapter.to_json_pretty(&patient).expect("json pretty");

        // Verify it can be deserialized back
        let restored: FhirPatient = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.id, patient.id);
        assert_eq!(restored.name.len(), patient.name.len());
    }
