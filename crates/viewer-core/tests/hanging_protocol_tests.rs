// Auto-extracted from /home/z/diccy/crates/viewer-core/src/hanging_protocol.rs
// S13-T8: Move inline tests to tests/ directories


    use viewer_core::*;

    #[test]
    fn engine_starts_empty() {
        let engine = HangingProtocolEngine::new();
        assert!(engine.is_empty());
        assert_eq!(engine.len(), 0);
    }

    #[test]
    fn register_and_unregister_protocol() {
        let mut engine = HangingProtocolEngine::new();
        let protocol = default_fallback_protocol();
        let id = protocol.protocol_id.clone();
        engine.register(protocol).expect("register");
        assert_eq!(engine.len(), 1);
        assert!(engine.unregister(&id));
        assert!(engine.is_empty());
    }

    #[test]
    fn reject_protocol_with_zero_rows() {
        let mut engine = HangingProtocolEngine::new();
        let protocol = HangingProtocol {
            protocol_id: "bad".to_string(),
            name: "Bad".to_string(),
            priority: 1,
            match_criteria: vec![],
            image_sets: vec![],
            display_sets: vec![],
            rows: 0,
            columns: 1,
            is_fallback: false,
        };
        let err = engine.register(protocol).expect_err("expected error");
        assert!(
            matches!(err, HangingProtocolError::InvalidProtocol { .. }),
            "expected InvalidProtocol, got {:?}",
            err
        );
    }

    #[test]
    fn reject_display_set_with_invalid_image_set() {
        let mut engine = HangingProtocolEngine::new();
        let protocol = HangingProtocol {
            protocol_id: "bad_ds".to_string(),
            name: "Bad Display Set".to_string(),
            priority: 1,
            match_criteria: vec![],
            image_sets: vec![],
            display_sets: vec![DisplaySetAssignment {
                set_id: "ds1".to_string(),
                image_set_id: "nonexistent".to_string(),
                row: 0,
                column: 0,
                window_center: None,
                window_width: None,
                rotation_degrees: None,
                flip_horizontal: false,
                flip_vertical: false,
            }],
            rows: 1,
            columns: 1,
            is_fallback: false,
        };
        let err = engine.register(protocol).expect_err("expected error");
        assert!(
            matches!(err, HangingProtocolError::MissingImageSet { .. }),
            "expected MissingImageSet, got {:?}",
            err
        );
    }

    #[test]
    fn reject_display_set_row_out_of_bounds() {
        let mut engine = HangingProtocolEngine::new();
        let protocol = HangingProtocol {
            protocol_id: "row_oob".to_string(),
            name: "Row Out of Bounds".to_string(),
            priority: 1,
            match_criteria: vec![],
            image_sets: vec![ImageSetDefinition {
                set_id: "is1".to_string(),
                criteria: vec![],
                time_perspective: TimePerspective::Current,
            }],
            display_sets: vec![DisplaySetAssignment {
                set_id: "ds1".to_string(),
                image_set_id: "is1".to_string(),
                row: 5,
                column: 0,
                window_center: None,
                window_width: None,
                rotation_degrees: None,
                flip_horizontal: false,
                flip_vertical: false,
            }],
            rows: 2,
            columns: 2,
            is_fallback: false,
        };
        let err = engine.register(protocol).expect_err("expected error");
        assert!(
            matches!(err, HangingProtocolError::InvalidProtocol { .. }),
            "expected InvalidProtocol, got {:?}",
            err
        );
    }

    #[test]
    fn mammography_protocol_matches_mg_modality() {
        let mut engine = HangingProtocolEngine::new();
        engine.register(mammography_protocol()).expect("register");
        engine
            .register(default_fallback_protocol())
            .expect("register");

        let context = StudyMatchContext {
            modalities: vec!["MG".to_string()],
            body_part: Some("BREAST".to_string()),
            laterality: Some("L".to_string()),
            study_description: Some("Screening Mammography CC MLO".to_string()),
            sop_classes: vec![],
            series_count: 4,
            prior_count: 0,
        };
        let result = engine.match_protocol(&context).expect("match");
        assert_eq!(result.protocol_id, "builtin.mammography_4view");
        assert!(!result.used_fallback);
        assert_eq!(result.rows, 2);
        assert_eq!(result.columns, 2);
        assert_eq!(result.display_sets.len(), 4);
    }

    #[test]
    fn ct_chest_protocol_matches_ct_chest() {
        let mut engine = HangingProtocolEngine::new();
        engine
            .register(ct_chest_abdomen_protocol())
            .expect("register");
        engine
            .register(default_fallback_protocol())
            .expect("register");

        let context = StudyMatchContext {
            modalities: vec!["CT".to_string()],
            body_part: Some("CHEST".to_string()),
            laterality: None,
            study_description: Some("CT Chest with lung and soft tissue windows".to_string()),
            sop_classes: vec![],
            series_count: 2,
            prior_count: 0,
        };
        let result = engine.match_protocol(&context).expect("match");
        assert_eq!(result.protocol_id, "builtin.ct_chest_abdomen");
        assert!(!result.used_fallback);
    }

    #[test]
    fn fallback_used_when_no_primary_matches() {
        let mut engine = HangingProtocolEngine::new();
        engine.register(mammography_protocol()).expect("register");
        engine
            .register(default_fallback_protocol())
            .expect("register");

        let context = StudyMatchContext {
            modalities: vec!["US".to_string()],
            body_part: Some("ABDOMEN".to_string()),
            laterality: None,
            study_description: Some("Ultrasound abdomen".to_string()),
            sop_classes: vec![],
            series_count: 1,
            prior_count: 0,
        };
        let result = engine.match_protocol(&context).expect("match");
        assert_eq!(result.protocol_id, "builtin.default_fallback");
        assert!(result.used_fallback);
    }

    #[test]
    fn no_match_when_no_protocols_registered() {
        let engine = HangingProtocolEngine::new();
        let context = StudyMatchContext {
            modalities: vec!["CT".to_string()],
            body_part: None,
            laterality: None,
            study_description: None,
            sop_classes: vec![],
            series_count: 1,
            prior_count: 0,
        };
        let err = engine.match_protocol(&context).expect_err("expected error");
        assert!(matches!(err, HangingProtocolError::NoMatch { .. }));
    }

    #[test]
    fn no_match_when_no_fallback_registered() {
        let mut engine = HangingProtocolEngine::new();
        engine.register(mammography_protocol()).expect("register");

        let context = StudyMatchContext {
            modalities: vec!["NM".to_string()],
            body_part: None,
            laterality: None,
            study_description: None,
            sop_classes: vec![],
            series_count: 1,
            prior_count: 0,
        };
        let err = engine.match_protocol(&context).expect_err("expected error");
        assert!(matches!(err, HangingProtocolError::NoMatch { .. }));
    }

    #[test]
    fn criteria_matching_is_case_insensitive() {
        let mut engine = HangingProtocolEngine::new();
        engine.register(mammography_protocol()).expect("register");

        let context = StudyMatchContext {
            modalities: vec!["mg".to_string()],
            body_part: Some("breast".to_string()),
            laterality: Some("l".to_string()),
            study_description: Some("cc and mlo views".to_string()),
            sop_classes: vec![],
            series_count: 4,
            prior_count: 0,
        };
        let result = engine.match_protocol(&context).expect("match");
        assert_eq!(result.protocol_id, "builtin.mammography_4view");
    }

    #[test]
    fn study_description_pattern_is_substring_match() {
        let mut engine = HangingProtocolEngine::new();
        engine
            .register(ct_chest_abdomen_protocol())
            .expect("register");

        let context = StudyMatchContext {
            modalities: vec!["CT".to_string()],
            body_part: Some("CHEST".to_string()),
            laterality: None,
            study_description: Some("CT CHEST W CONTRAST LUNG WINDOWS".to_string()),
            sop_classes: vec![],
            series_count: 2,
            prior_count: 0,
        };
        let result = engine.match_protocol(&context).expect("match");
        assert_eq!(result.protocol_id, "builtin.ct_chest_abdomen");
    }

    #[test]
    fn empty_protocol_id_rejected() {
        let mut engine = HangingProtocolEngine::new();
        let protocol = HangingProtocol {
            protocol_id: String::new(),
            name: "Empty ID".to_string(),
            priority: 1,
            match_criteria: vec![],
            image_sets: vec![],
            display_sets: vec![],
            rows: 1,
            columns: 1,
            is_fallback: false,
        };
        let err = engine.register(protocol).expect_err("expected error");
        assert!(matches!(err, HangingProtocolError::InvalidProtocol { .. }));
    }

    #[test]
    fn match_score_increases_with_criteria_count() {
        let mut engine = HangingProtocolEngine::new();
        engine.register(mammography_protocol()).expect("register");
        engine
            .register(ct_chest_abdomen_protocol())
            .expect("register");

        // MG modality only matches mammography protocol
        let context = StudyMatchContext {
            modalities: vec!["MG".to_string()],
            body_part: Some("BREAST".to_string()),
            laterality: Some("L".to_string()),
            study_description: Some("Screening CC MLO".to_string()),
            sop_classes: vec![],
            series_count: 4,
            prior_count: 0,
        };
        let result = engine.match_protocol(&context).expect("match");
        assert_eq!(result.protocol_id, "builtin.mammography_4view");
        // Mammography has 1 match criterion + 4 image sets => score > 0
        assert!(result.match_score > 0);
    }

    #[test]
    fn laterality_criterion_matches_when_present() {
        let mut engine = HangingProtocolEngine::new();
        let protocol = HangingProtocol {
            protocol_id: "test.laterality".to_string(),
            name: "Test Laterality".to_string(),
            priority: 1,
            match_criteria: vec![
                MatchCriterion::Modality {
                    code: "MR".to_string(),
                },
                MatchCriterion::Laterality {
                    code: "L".to_string(),
                },
            ],
            image_sets: vec![ImageSetDefinition {
                set_id: "is1".to_string(),
                criteria: vec![],
                time_perspective: TimePerspective::Current,
            }],
            display_sets: vec![DisplaySetAssignment {
                set_id: "ds1".to_string(),
                image_set_id: "is1".to_string(),
                row: 0,
                column: 0,
                window_center: None,
                window_width: None,
                rotation_degrees: None,
                flip_horizontal: false,
                flip_vertical: false,
            }],
            rows: 1,
            columns: 1,
            is_fallback: false,
        };
        engine.register(protocol).expect("register");

        let matching = StudyMatchContext {
            modalities: vec!["MR".to_string()],
            body_part: None,
            laterality: Some("L".to_string()),
            study_description: None,
            sop_classes: vec![],
            series_count: 1,
            prior_count: 0,
        };
        let result = engine.match_protocol(&matching).expect("match");
        assert_eq!(result.protocol_id, "test.laterality");

        let non_matching = StudyMatchContext {
            modalities: vec!["MR".to_string()],
            body_part: None,
            laterality: Some("R".to_string()),
            study_description: None,
            sop_classes: vec![],
            series_count: 1,
            prior_count: 0,
        };
        // Should not match since laterality is R, not L
        assert!(engine.match_protocol(&non_matching).is_err());
    }

    #[test]
    fn sop_class_criterion_matches() {
        let mut engine = HangingProtocolEngine::new();
        let protocol = HangingProtocol {
            protocol_id: "test.sop_class".to_string(),
            name: "Test SOP Class".to_string(),
            priority: 1,
            match_criteria: vec![MatchCriterion::SopClass {
                uid: "1.2.840.10008.5.1.4.1.1.2".to_string(),
            }],
            image_sets: vec![ImageSetDefinition {
                set_id: "is1".to_string(),
                criteria: vec![],
                time_perspective: TimePerspective::Current,
            }],
            display_sets: vec![DisplaySetAssignment {
                set_id: "ds1".to_string(),
                image_set_id: "is1".to_string(),
                row: 0,
                column: 0,
                window_center: None,
                window_width: None,
                rotation_degrees: None,
                flip_horizontal: false,
                flip_vertical: false,
            }],
            rows: 1,
            columns: 1,
            is_fallback: false,
        };
        engine.register(protocol).expect("register");

        let context = StudyMatchContext {
            modalities: vec!["CT".to_string()],
            body_part: None,
            laterality: None,
            study_description: None,
            sop_classes: vec!["1.2.840.10008.5.1.4.1.1.2".to_string()],
            series_count: 1,
            prior_count: 0,
        };
        let result = engine.match_protocol(&context).expect("match");
        assert_eq!(result.protocol_id, "test.sop_class");
    }

    #[test]
    fn body_part_criterion_fails_when_missing_in_context() {
        let mut engine = HangingProtocolEngine::new();
        engine
            .register(ct_chest_abdomen_protocol())
            .expect("register");

        let context = StudyMatchContext {
            modalities: vec!["CT".to_string()],
            body_part: None, // Missing body part
            laterality: None,
            study_description: None,
            sop_classes: vec![],
            series_count: 1,
            prior_count: 0,
        };
        // CT chest protocol requires body_part=CHEST, so this should not match
        assert!(engine.match_protocol(&context).is_err());
    }

    #[test]
    fn multiple_protocols_highest_score_wins() {
        let mut engine = HangingProtocolEngine::new();

        // Generic CT protocol (fewer criteria = lower score)
        let generic = HangingProtocol {
            protocol_id: "generic_ct".to_string(),
            name: "Generic CT".to_string(),
            priority: 5, // Higher priority number (lower precedence in tie)
            match_criteria: vec![MatchCriterion::Modality {
                code: "CT".to_string(),
            }],
            image_sets: vec![ImageSetDefinition {
                set_id: "is1".to_string(),
                criteria: vec![],
                time_perspective: TimePerspective::Current,
            }],
            display_sets: vec![DisplaySetAssignment {
                set_id: "ds1".to_string(),
                image_set_id: "is1".to_string(),
                row: 0,
                column: 0,
                window_center: None,
                window_width: None,
                rotation_degrees: None,
                flip_horizontal: false,
                flip_vertical: false,
            }],
            rows: 1,
            columns: 1,
            is_fallback: false,
        };

        // Specific CT chest protocol (more criteria = higher score)
        let specific = ct_chest_abdomen_protocol();

        engine.register(generic).expect("register");
        engine.register(specific).expect("register");

        let context = StudyMatchContext {
            modalities: vec!["CT".to_string()],
            body_part: Some("CHEST".to_string()),
            laterality: None,
            study_description: Some("lung soft".to_string()),
            sop_classes: vec![],
            series_count: 2,
            prior_count: 0,
        };
        let result = engine.match_protocol(&context).expect("match");
        // CT chest/abdomen has more criteria so should score higher
        assert_eq!(result.protocol_id, "builtin.ct_chest_abdomen");
    }

    // =======================================================================
    // Sprint 3 Extended Tests: Hanging Protocol Engine
    // =======================================================================

    #[test]
    fn protocol_with_multiple_image_sets() {
        let mut engine = HangingProtocolEngine::new();
        let protocol = HangingProtocol {
            protocol_id: "multi_image_set".to_string(),
            name: "Multi Image Set".to_string(),
            priority: 10,
            match_criteria: vec![MatchCriterion::Modality {
                code: "MG".to_string(),
            }],
            image_sets: vec![
                ImageSetDefinition {
                    set_id: "current_cc".to_string(),
                    criteria: vec![
                        MatchCriterion::Modality {
                            code: "MG".to_string(),
                        },
                        MatchCriterion::Laterality {
                            code: "L".to_string(),
                        },
                    ],
                    time_perspective: TimePerspective::Current,
                },
                ImageSetDefinition {
                    set_id: "prior_cc".to_string(),
                    criteria: vec![
                        MatchCriterion::Modality {
                            code: "MG".to_string(),
                        },
                        MatchCriterion::Laterality {
                            code: "L".to_string(),
                        },
                    ],
                    time_perspective: TimePerspective::Prior,
                },
            ],
            display_sets: vec![DisplaySetAssignment {
                set_id: "ds1".to_string(),
                image_set_id: "current_cc".to_string(),
                row: 0,
                column: 0,
                window_center: None,
                window_width: None,
                rotation_degrees: None,
                flip_horizontal: false,
                flip_vertical: false,
            }],
            rows: 1,
            columns: 1,
            is_fallback: false,
        };
        engine.register(protocol).expect("register");
        let context = StudyMatchContext {
            modalities: vec!["MG".to_string()],
            body_part: None,
            laterality: Some("L".to_string()),
            study_description: None,
            sop_classes: vec![],
            series_count: 1,
            prior_count: 1,
        };
        let result = engine.match_protocol(&context).expect("match");
        assert_eq!(result.protocol_id, "multi_image_set");
    }

    #[test]
    fn protocol_unregister_removes_from_matching() {
        let mut engine = HangingProtocolEngine::new();
        let protocol = HangingProtocol {
            protocol_id: "temp_protocol".to_string(),
            name: "Temp".to_string(),
            priority: 10,
            match_criteria: vec![MatchCriterion::Modality {
                code: "CT".to_string(),
            }],
            image_sets: vec![ImageSetDefinition {
                set_id: "is1".to_string(),
                criteria: vec![MatchCriterion::Modality {
                    code: "CT".to_string(),
                }],
                time_perspective: TimePerspective::Current,
            }],
            display_sets: vec![DisplaySetAssignment {
                set_id: "ds1".to_string(),
                image_set_id: "is1".to_string(),
                row: 0,
                column: 0,
                window_center: None,
                window_width: None,
                rotation_degrees: None,
                flip_horizontal: false,
                flip_vertical: false,
            }],
            rows: 1,
            columns: 1,
            is_fallback: false,
        };
        engine.register(protocol).expect("register");
        engine.unregister("temp_protocol");
        let context = StudyMatchContext {
            modalities: vec!["CT".to_string()],
            body_part: None,
            laterality: None,
            study_description: None,
            sop_classes: vec![],
            series_count: 1,
            prior_count: 0,
        };
        assert!(engine.match_protocol(&context).is_err());
    }

    #[test]
    fn display_set_with_window_level_presets() {
        let mut engine = HangingProtocolEngine::new();
        let protocol = HangingProtocol {
            protocol_id: "wl_preset".to_string(),
            name: "W/L Preset".to_string(),
            priority: 10,
            match_criteria: vec![MatchCriterion::Modality {
                code: "CT".to_string(),
            }],
            image_sets: vec![ImageSetDefinition {
                set_id: "ct_lung".to_string(),
                criteria: vec![MatchCriterion::Modality {
                    code: "CT".to_string(),
                }],
                time_perspective: TimePerspective::Current,
            }],
            display_sets: vec![
                DisplaySetAssignment {
                    set_id: "lung_wnd".to_string(),
                    image_set_id: "ct_lung".to_string(),
                    row: 0,
                    column: 0,
                    window_center: Some(-600.0),
                    window_width: Some(1500.0),
                    rotation_degrees: None,
                    flip_horizontal: false,
                    flip_vertical: false,
                },
                DisplaySetAssignment {
                    set_id: "bone_wnd".to_string(),
                    image_set_id: "ct_lung".to_string(),
                    row: 0,
                    column: 1,
                    window_center: Some(300.0),
                    window_width: Some(2000.0),
                    rotation_degrees: None,
                    flip_horizontal: false,
                    flip_vertical: false,
                },
            ],
            rows: 1,
            columns: 2,
            is_fallback: false,
        };
        engine.register(protocol).expect("register");
        let context = StudyMatchContext {
            modalities: vec!["CT".to_string()],
            body_part: None,
            laterality: None,
            study_description: None,
            sop_classes: vec![],
            series_count: 1,
            prior_count: 0,
        };
        let result = engine.match_protocol(&context).expect("match");
        assert_eq!(result.display_sets.len(), 2);
        assert_eq!(result.display_sets[0].window_center, Some(-600.0));
        assert_eq!(result.display_sets[1].window_center, Some(300.0));
    }

    #[test]
    fn match_with_no_body_part_in_context() {
        let mut engine = HangingProtocolEngine::new();
        let protocol = HangingProtocol {
            protocol_id: "body_part_req".to_string(),
            name: "Body Part Required".to_string(),
            priority: 10,
            match_criteria: vec![
                MatchCriterion::Modality {
                    code: "CT".to_string(),
                },
                MatchCriterion::BodyPart {
                    code: "CHEST".to_string(),
                },
            ],
            image_sets: vec![ImageSetDefinition {
                set_id: "is1".to_string(),
                criteria: vec![MatchCriterion::Modality {
                    code: "CT".to_string(),
                }],
                time_perspective: TimePerspective::Current,
            }],
            display_sets: vec![DisplaySetAssignment {
                set_id: "ds1".to_string(),
                image_set_id: "is1".to_string(),
                row: 0,
                column: 0,
                window_center: None,
                window_width: None,
                rotation_degrees: None,
                flip_horizontal: false,
                flip_vertical: false,
            }],
            rows: 1,
            columns: 1,
            is_fallback: false,
        };
        engine.register(protocol).expect("register");
        // Context without body part should not match the body part criterion
        let context = StudyMatchContext {
            modalities: vec!["CT".to_string()],
            body_part: None,
            laterality: None,
            study_description: None,
            sop_classes: vec![],
            series_count: 1,
            prior_count: 0,
        };
        // Should not match because body part criterion fails when missing
        assert!(engine.match_protocol(&context).is_err());
    }

    #[test]
    fn study_description_pattern_partial_match() {
        let mut engine = HangingProtocolEngine::new();
        let protocol = HangingProtocol {
            protocol_id: "desc_match".to_string(),
            name: "Description Match".to_string(),
            priority: 10,
            match_criteria: vec![MatchCriterion::StudyDescription {
                pattern: "BRAIN".to_string(),
            }],
            image_sets: vec![ImageSetDefinition {
                set_id: "is1".to_string(),
                criteria: vec![MatchCriterion::StudyDescription {
                    pattern: "BRAIN".to_string(),
                }],
                time_perspective: TimePerspective::Current,
            }],
            display_sets: vec![DisplaySetAssignment {
                set_id: "ds1".to_string(),
                image_set_id: "is1".to_string(),
                row: 0,
                column: 0,
                window_center: None,
                window_width: None,
                rotation_degrees: None,
                flip_horizontal: false,
                flip_vertical: false,
            }],
            rows: 1,
            columns: 1,
            is_fallback: false,
        };
        engine.register(protocol).expect("register");
        // "BRAIN" is a substring of "MRI BRAIN WITH CONTRAST"
        let context = StudyMatchContext {
            modalities: vec!["MR".to_string()],
            body_part: None,
            laterality: None,
            study_description: Some("MRI BRAIN WITH CONTRAST".to_string()),
            sop_classes: vec![],
            series_count: 1,
            prior_count: 0,
        };
        let result = engine.match_protocol(&context).expect("match");
        assert_eq!(result.protocol_id, "desc_match");
    }
