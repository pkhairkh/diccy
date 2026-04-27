// Auto-extracted from /home/z/diccy/crates/dicom-ihe/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_ihe::*;

    // --- SWF Tests ---

    #[test]
    fn swf_submit_order() {
        let mut engine = SwfEngine::new();
        let order = SwfOrder::new("ACC-001", "P001", "CT-CHEST");
        let acc = engine.submit_order(order).unwrap();
        assert_eq!(acc, "ACC-001");
        assert_eq!(engine.orders().len(), 1);
    }

    #[test]
    fn swf_reject_empty_accession() {
        let mut engine = SwfEngine::new();
        let order = SwfOrder::new("", "P001", "CT-CHEST");
        assert!(engine.submit_order(order).is_err());
    }

    #[test]
    fn swf_schedule_workitem() {
        let mut engine = SwfEngine::new();
        engine
            .submit_order(SwfOrder::new("ACC-002", "P002", "MR-BRAIN"))
            .unwrap();
        let uid = engine
            .schedule_workitem("ACC-002", "Dr. Smith", 1000)
            .unwrap();
        assert!(!uid.is_empty());
        let wi = engine.get_workitem(&uid).unwrap();
        assert!(matches!(wi.state, SwfWorkitemState::Scheduled));
    }

    #[test]
    fn swf_complete_workflow() {
        let mut engine = SwfEngine::new();
        engine
            .submit_order(SwfOrder::new("ACC-003", "P003", "CT-ABD"))
            .unwrap();
        let uid = engine
            .schedule_workitem("ACC-003", "Dr. Jones", 2000)
            .unwrap();

        engine.start_workitem(&uid, 2100).unwrap();
        let wi = engine.get_workitem(&uid).unwrap();
        assert!(matches!(wi.state, SwfWorkitemState::InProgress));

        engine.complete_workitem(&uid, 3000).unwrap();
        let wi = engine.get_workitem(&uid).unwrap();
        assert!(matches!(wi.state, SwfWorkitemState::Completed));
        assert_eq!(wi.actual_end, Some(3000));
    }

    #[test]
    fn swf_cancel_workitem() {
        let mut engine = SwfEngine::new();
        engine
            .submit_order(SwfOrder::new("ACC-004", "P004", "XR-CHEST"))
            .unwrap();
        let uid = engine
            .schedule_workitem("ACC-004", "Dr. Lee", 4000)
            .unwrap();

        engine.cancel_workitem(&uid).unwrap();
        let wi = engine.get_workitem(&uid).unwrap();
        assert!(matches!(wi.state, SwfWorkitemState::Canceled));
    }

    #[test]
    fn swf_cannot_start_completed_workitem() {
        let mut engine = SwfEngine::new();
        engine
            .submit_order(SwfOrder::new("ACC-005", "P005", "US-ABD"))
            .unwrap();
        let uid = engine
            .schedule_workitem("ACC-005", "Dr. Kim", 5000)
            .unwrap();
        engine.start_workitem(&uid, 5100).unwrap();
        engine.complete_workitem(&uid, 6000).unwrap();

        let result = engine.start_workitem(&uid, 6100);
        assert!(result.is_err());
    }

    // --- PIR Tests ---

    #[test]
    fn pir_register_unidentified() {
        let mut engine = PirEngine::new();
        engine
            .register_unidentified(UnidentifiedPatient {
                temporary_id: "TEMP-001".to_string(),
                study_uid: "1.2.3.4".to_string(),
                study_description: "CT Chest".to_string(),
                study_date: "2024-01-15".to_string(),
                confidence: 0.0,
            })
            .unwrap();
        assert_eq!(engine.unidentified_count(), 1);
    }

    #[test]
    fn pir_reconcile_patient() {
        let mut engine = PirEngine::new();
        engine
            .register_unidentified(UnidentifiedPatient {
                temporary_id: "TEMP-002".to_string(),
                study_uid: "1.2.3.5".to_string(),
                study_description: "MR Brain".to_string(),
                study_date: "2024-02-20".to_string(),
                confidence: 0.0,
            })
            .unwrap();

        engine
            .reconcile("TEMP-002", "P12345", "Doe^John", "19800101", "M")
            .unwrap();
        assert_eq!(engine.unidentified_count(), 0);
        assert_eq!(engine.reconciled_count(), 1);
    }

    #[test]
    fn pir_reconcile_unknown_fails() {
        let mut engine = PirEngine::new();
        let result = engine.reconcile("NONEXISTENT", "P12345", "Doe^John", "19800101", "M");
        assert!(result.is_err());
    }

    // --- XDS Tests ---

    #[test]
    fn xds_register_document() {
        let mut registry = XdsRegistry::new();
        registry
            .register(XdsDocumentEntry {
                entry_uuid: "urn:uuid:12345".to_string(),
                unique_id: "1.2.3.4.5".to_string(),
                patient_id: "P001".to_string(),
                class_code: "DICOM".to_string(),
                type_code: "Imaging".to_string(),
                facility_code: "Hospital".to_string(),
                practice_setting: "Radiology".to_string(),
                repository_uid: "1.2.3.repo".to_string(),
                available: true,
            })
            .unwrap();
        assert_eq!(registry.document_count(), 1);
    }

    #[test]
    fn xds_query_by_patient() {
        let mut registry = XdsRegistry::new();
        registry
            .register(XdsDocumentEntry {
                entry_uuid: "urn:uuid:aaa".to_string(),
                unique_id: "1.2.3.4".to_string(),
                patient_id: "P001".to_string(),
                class_code: "DICOM".to_string(),
                type_code: "Imaging".to_string(),
                facility_code: "H".to_string(),
                practice_setting: "R".to_string(),
                repository_uid: "repo".to_string(),
                available: true,
            })
            .unwrap();
        registry
            .register(XdsDocumentEntry {
                entry_uuid: "urn:uuid:bbb".to_string(),
                unique_id: "1.2.3.5".to_string(),
                patient_id: "P001".to_string(),
                class_code: "DICOM".to_string(),
                type_code: "Imaging".to_string(),
                facility_code: "H".to_string(),
                practice_setting: "R".to_string(),
                repository_uid: "repo".to_string(),
                available: true,
            })
            .unwrap();

        let results = registry.query_by_patient("P001");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn xds_remove_document() {
        let mut registry = XdsRegistry::new();
        registry
            .register(XdsDocumentEntry {
                entry_uuid: "urn:uuid:del".to_string(),
                unique_id: "1.2.3.6".to_string(),
                patient_id: "P002".to_string(),
                class_code: "DICOM".to_string(),
                type_code: "Imaging".to_string(),
                facility_code: "H".to_string(),
                practice_setting: "R".to_string(),
                repository_uid: "repo".to_string(),
                available: true,
            })
            .unwrap();

        let removed = registry.remove("urn:uuid:del").unwrap();
        assert_eq!(removed.unique_id, "1.2.3.6");
        assert_eq!(registry.document_count(), 0);
    }

    // --- AIR Tests ---

    #[test]
    fn air_submit_finding() {
        let mut exchange = AirExchange::new();
        exchange
            .submit_finding(AirFinding {
                finding_uid: "1.2.3.f1".to_string(),
                study_uid: "1.2.3.s1".to_string(),
                finding_type_code: "76581006".to_string(),
                confidence: 0.85,
                algorithm_name: "LungNoduleAI".to_string(),
                algorithm_version: "1.0.0".to_string(),
                algorithm_uid: "1.2.3.ai1".to_string(),
            })
            .unwrap();
        assert_eq!(exchange.finding_count(), 1);
    }

    #[test]
    fn air_query_by_study() {
        let mut exchange = AirExchange::new();
        exchange
            .submit_finding(AirFinding {
                finding_uid: "1.2.3.f2".to_string(),
                study_uid: "1.2.3.study1".to_string(),
                finding_type_code: "76581006".to_string(),
                confidence: 0.9,
                algorithm_name: "LungAI".to_string(),
                algorithm_version: "2.0".to_string(),
                algorithm_uid: "1.2.3.ai2".to_string(),
            })
            .unwrap();

        let results = exchange.query_by_study("1.2.3.study1");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].confidence, 0.9);
    }

    // --- Profile metadata tests ---

    #[test]
    fn ihe_profile_names() {
        assert_eq!(IheProfile::Swf.name(), "Scheduled Workflow");
        assert_eq!(IheProfile::Pir.name(), "Patient Information Reconciliation");
        assert_eq!(IheProfile::XdsIb.name(), "XDS-I.b");
        assert_eq!(IheProfile::Air.name(), "AI Results");
    }

    #[test]
    fn ihe_profile_domains() {
        assert_eq!(IheProfile::Swf.domain(), "Radiology");
        assert_eq!(IheProfile::XdsIb.domain(), "IT Infrastructure");
    }
