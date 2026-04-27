// Auto-extracted from /home/z/diccy/crates/pack-sr/src/lib.rs
// S13-T8: Move inline tests to tests/ directories

    use pack_sr::*;
    use dicom_core::{Dataset, Element, Error, ErrorKind, Tag, Value, Vr};

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
        item.insert(Element::new(TAG_VALUE_TYPE, Vr::Cs, Value::Str("NUM".to_string())).unwrap());
        let mut concept_item = Dataset::new();
        concept_item
            .insert(Element::new(TAG_CODE_VALUE, Vr::Sh, Value::Str("123".to_string())).unwrap());
        concept_item.insert(
            Element::new(
                TAG_CODING_SCHEME_DESIGNATOR,
                Vr::Sh,
                Value::Str("99TEST".to_string()),
            )
            .unwrap(),
        );
        concept_item.insert(
            Element::new(TAG_CODE_MEANING, Vr::Lo, Value::Str("Length".to_string())).unwrap(),
        );
        item.insert(
            Element::new(
                TAG_CONCEPT_NAME_CODE_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![concept_item]),
            )
            .unwrap(),
        );
        let mut units_item = Dataset::new();
        units_item
            .insert(Element::new(TAG_CODE_VALUE, Vr::Sh, Value::Str("mm".to_string())).unwrap());
        units_item.insert(
            Element::new(
                TAG_CODING_SCHEME_DESIGNATOR,
                Vr::Sh,
                Value::Str("UCUM".to_string()),
            )
            .unwrap(),
        );
        units_item.insert(
            Element::new(
                TAG_CODE_MEANING,
                Vr::Lo,
                Value::Str("millimeter".to_string()),
            )
            .unwrap(),
        );
        let mut measured = Dataset::new();
        measured.insert(
            Element::new(TAG_NUMERIC_VALUE, Vr::Ds, Value::Str("12.5".to_string())).unwrap(),
        );
        measured.insert(
            Element::new(
                TAG_MEASUREMENT_UNITS_CODE_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![units_item]),
            )
            .unwrap(),
        );
        item.insert(
            Element::new(
                TAG_MEASURED_VALUE_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![measured]),
            )
            .unwrap(),
        );
        item
    }

    fn build_text_item(text: &str) -> Dataset {
        let mut item = Dataset::new();
        item.insert(Element::new(TAG_VALUE_TYPE, Vr::Cs, Value::Str("TEXT".to_string())).unwrap());
        let mut concept_item = Dataset::new();
        concept_item
            .insert(Element::new(TAG_CODE_VALUE, Vr::Sh, Value::Str("TXT".to_string())).unwrap());
        concept_item.insert(
            Element::new(
                TAG_CODING_SCHEME_DESIGNATOR,
                Vr::Sh,
                Value::Str("99TEST".to_string()),
            )
            .unwrap(),
        );
        concept_item.insert(
            Element::new(TAG_CODE_MEANING, Vr::Lo, Value::Str("Comment".to_string())).unwrap(),
        );
        item.insert(
            Element::new(
                TAG_CONCEPT_NAME_CODE_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![concept_item]),
            )
            .unwrap(),
        );
        item.insert(Element::new(TAG_TEXT_VALUE, Vr::Ut, Value::Str(text.to_string())).unwrap());
        item
    }

    fn build_code_item() -> Dataset {
        let mut item = Dataset::new();
        item.insert(Element::new(TAG_VALUE_TYPE, Vr::Cs, Value::Str("CODE".to_string())).unwrap());
        let mut concept_item = Dataset::new();
        concept_item
            .insert(Element::new(TAG_CODE_VALUE, Vr::Sh, Value::Str("OBS".to_string())).unwrap());
        concept_item.insert(
            Element::new(
                TAG_CODING_SCHEME_DESIGNATOR,
                Vr::Sh,
                Value::Str("99TEST".to_string()),
            )
            .unwrap(),
        );
        concept_item.insert(
            Element::new(
                TAG_CODE_MEANING,
                Vr::Lo,
                Value::Str("Observation".to_string()),
            )
            .unwrap(),
        );
        item.insert(
            Element::new(
                TAG_CONCEPT_NAME_CODE_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![concept_item]),
            )
            .unwrap(),
        );
        let mut value_item = Dataset::new();
        value_item.insert(
            Element::new(TAG_CODE_VALUE, Vr::Sh, Value::Str("R-404FB".to_string())).unwrap(),
        );
        value_item.insert(
            Element::new(
                TAG_CODING_SCHEME_DESIGNATOR,
                Vr::Sh,
                Value::Str("SRT".to_string()),
            )
            .unwrap(),
        );
        value_item.insert(
            Element::new(TAG_CODE_MEANING, Vr::Lo, Value::Str("Normal".to_string())).unwrap(),
        );
        item.insert(
            Element::new(
                TAG_CONCEPT_CODE_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![value_item]),
            )
            .unwrap(),
        );
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
        dataset.insert(
            Element::new(
                TAG_CONTENT_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![build_num_item()]),
            )
            .unwrap(),
        );
        let measurements = extract_measurements(&dataset).expect("extract");
        assert_eq!(measurements.len(), 1);
        assert_eq!(measurements[0].value, 12.5);
    }

    #[test]
    fn sr_referenced_uid_is_captured() {
        // REQ-MEAS-082, REQ-SR-300
        let mut ref_item = Dataset::new();
        ref_item.insert(
            Element::new(
                TAG_REFERENCED_SOP_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3.4".to_string()),
            )
            .unwrap(),
        );
        let mut dataset = Dataset::new();
        let mut num_item = build_num_item();
        num_item.insert(
            Element::new(
                TAG_REFERENCED_SOP_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![ref_item]),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_CONTENT_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![num_item]),
            )
            .unwrap(),
        );
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
        dataset.insert(
            Element::new(
                TAG_CONTENT_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![build_text_item("Finding present")]),
            )
            .unwrap(),
        );
        let observations = extract_text_observations(&dataset).expect("extract");
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].text, "Finding present");
    }

    #[test]
    fn extract_nested_num_and_text_items() {
        // REQ-MEAS-081, REQ-MEAS-082, REQ-SR-300
        let mut container = Dataset::new();
        container.insert(
            Element::new(TAG_VALUE_TYPE, Vr::Cs, Value::Str("CONTAINER".to_string())).unwrap(),
        );
        container.insert(
            Element::new(
                TAG_CONTENT_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![build_num_item(), build_text_item("Nested note")]),
            )
            .unwrap(),
        );

        let mut dataset = Dataset::new();
        dataset.insert(
            Element::new(
                TAG_CONTENT_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![container]),
            )
            .unwrap(),
        );

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
        dataset.insert(
            Element::new(
                TAG_CONTENT_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![build_code_item()]),
            )
            .unwrap(),
        );
        let observations = extract_code_observations(&dataset).expect("extract");
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].value.code_value, "R-404FB");
    }

    #[test]
    fn extract_nested_code_item() {
        // REQ-MEAS-082, REQ-SR-300
        let mut container = Dataset::new();
        container.insert(
            Element::new(TAG_VALUE_TYPE, Vr::Cs, Value::Str("CONTAINER".to_string())).unwrap(),
        );
        container.insert(
            Element::new(
                TAG_CONTENT_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![build_code_item()]),
            )
            .unwrap(),
        );
        let mut dataset = Dataset::new();
        dataset.insert(
            Element::new(
                TAG_CONTENT_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![container]),
            )
            .unwrap(),
        );
        let observations = extract_code_observations(&dataset).expect("extract");
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].value.meaning, "Normal");
    }

    #[test]
    fn empty_text_observation_fails() {
        // REQ-UI-065, REQ-SR-300
        let mut dataset = Dataset::new();
        dataset.insert(
            Element::new(
                TAG_CONTENT_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![build_text_item("   ")]),
            )
            .unwrap(),
        );
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
        let doc = build_measurement_report("1.2.3", "1.2.3.4", "1.2.3.4.5", "test-observer", &[]);
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
        let doc =
            build_measurement_report("1.2.3", "1.2.3.4", "1.2.3.4.5", "observer", &measurements);
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
        let measurements = vec![SrMeasurement {
            concept: coded_concepts::distance(),
            value: 50.0,
            units: coded_concepts::millimeter(),
            referenced_sop_instance_uid: None,
        }];
        let doc =
            build_measurement_report("1.2.3", "1.2.3.4", "1.2.3.4.5", "observer", &measurements);
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
                assert_eq!(referenced_sop_instance_uid.as_deref(), Some("1.2.3.4.5"));
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
        let doc =
            build_measurement_report("1.2.3", "1.2.3.4", "1.2.3.4.5", "observer", &measurements);
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
