// Auto-extracted from /home/z/diccy/crates/dicom-inference/src/lib.rs
// S13-T8: Move inline tests to tests/ directories

    use dicom_inference::*;
    use std::sync::{Arc, Mutex};

    /// A notification callback that records notifications for testing.
    #[derive(Debug)]
    struct RecordingNotificationCallback {
        notifications: Mutex<Vec<(TriageFlag, String, String)>>,
    }

    impl RecordingNotificationCallback {
        fn new() -> Self {
            Self {
                notifications: Mutex::new(Vec::new()),
            }
        }

        fn notifications(&self) -> Vec<(TriageFlag, String, String)> {
            self.notifications.lock().expect("lock").clone()
        }
    }

    impl NotificationCallback for RecordingNotificationCallback {
        fn notify(&self, flag: TriageFlag, message: &str, study_uid: &str) {
            self.notifications.lock().expect("lock").push((
                flag,
                message.to_string(),
                study_uid.to_string(),
            ));
        }
    }

    fn test_study() -> StudyMetadata {
        StudyMetadata {
            study_uid: "1.2.840.113619.2.55.3".to_string(),
            modality: "CT".to_string(),
            description: "CT Chest".to_string(),
            patient_id: "PAT001".to_string(),
        }
    }

    #[test]
    fn triage_engine_creation_with_rules() {
        let engine = AiTriageEngine::new();
        assert!(
            !engine.rules().is_empty(),
            "built-in rules should be present"
        );
    }

    #[test]
    fn critical_finding_detection() {
        let engine = AiTriageEngine::new();
        let study = test_study();
        let results = vec![InferenceResult::Detection(vec![DetectionBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 10.0,
            y_max: 10.0,
            probability: 0.9,
            finding_type: "pneumothorax".to_string(),
        }])];

        let triage = engine.analyze_study(&study, &results);
        assert!(
            triage.flags.contains(&TriageFlag::Critical),
            "pneumothorax should trigger Critical flag, got {:?}",
            triage.flags
        );
        assert_eq!(triage.priority_score, 100);
    }

    #[test]
    fn urgent_finding_detection() {
        let engine = AiTriageEngine::new();
        let study = test_study();
        let results = vec![InferenceResult::Detection(vec![DetectionBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 10.0,
            y_max: 10.0,
            probability: 0.8,
            finding_type: "fracture".to_string(),
        }])];

        let triage = engine.analyze_study(&study, &results);
        assert!(
            triage.flags.contains(&TriageFlag::Urgent),
            "fracture should trigger Urgent flag, got {:?}",
            triage.flags
        );
        assert_eq!(triage.priority_score, 70);
    }

    #[test]
    fn priority_scoring() {
        assert_eq!(TriageFlag::Critical.priority_score(), 100);
        assert_eq!(TriageFlag::Urgent.priority_score(), 70);
        assert_eq!(TriageFlag::Routine.priority_score(), 40);
        assert_eq!(TriageFlag::Low.priority_score(), 10);
    }

    #[test]
    fn multiple_findings_with_different_severities() {
        let engine = AiTriageEngine::new();
        let study = test_study();
        let results = vec![InferenceResult::Detection(vec![
            DetectionBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: 10.0,
                y_max: 10.0,
                probability: 0.9,
                finding_type: "pneumothorax".to_string(),
            },
            DetectionBox {
                x_min: 20.0,
                y_min: 20.0,
                x_max: 30.0,
                y_max: 30.0,
                probability: 0.7,
                finding_type: "nodule".to_string(),
            },
        ])];

        let triage = engine.analyze_study(&study, &results);
        assert!(triage.flags.contains(&TriageFlag::Critical));
        assert!(triage.flags.contains(&TriageFlag::Routine));
        assert_eq!(triage.priority_score, 100); // Max score
    }

    #[test]
    fn notification_callback_invocation() {
        let callback = Arc::new(RecordingNotificationCallback::new());
        let callback_clone = Arc::clone(&callback);

        let mut engine = AiTriageEngine::new();
        engine.set_notification_callback(Some(callback_clone));

        let study = test_study();
        let results = vec![InferenceResult::Detection(vec![DetectionBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 10.0,
            y_max: 10.0,
            probability: 0.9,
            finding_type: "pneumothorax".to_string(),
        }])];

        engine.analyze_study(&study, &results);

        let notifications = callback.notifications();
        assert!(
            !notifications.is_empty(),
            "critical finding should trigger notification"
        );
        assert_eq!(notifications[0].0, TriageFlag::Critical);
    }

    #[test]
    fn built_in_rule_matching() {
        let rules = built_in_rules();
        assert!(rules.len() >= 3, "should have at least 3 built-in rules");

        // Pneumothorax rule
        let pneumo_rule = rules
            .iter()
            .find(|r| r.finding_pattern == "pneumothorax")
            .expect("rule");
        assert!(pneumo_rule.matches("pneumothorax", 0.9));
        assert!(!pneumo_rule.matches("pneumothorax", 0.3)); // Below threshold
        assert!(!pneumo_rule.matches("nodule", 0.9)); // Wrong pattern
    }

    #[test]
    fn routine_flag_for_no_findings() {
        let engine = AiTriageEngine::new();
        let study = test_study();
        let results: Vec<InferenceResult> = vec![];

        let triage = engine.analyze_study(&study, &results);
        assert!(
            triage.flags.contains(&TriageFlag::Routine),
            "no findings should result in Routine flag"
        );
    }

    #[test]
    fn custom_rules() {
        let rules = vec![TriageRule::new(
            "hemorrhage",
            TriageFlag::Critical,
            0.6,
            "CRITICAL: Hemorrhage detected",
        )];
        let engine = AiTriageEngine::with_rules(rules);
        let study = test_study();
        let results = vec![InferenceResult::Detection(vec![DetectionBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 10.0,
            y_max: 10.0,
            probability: 0.8,
            finding_type: "hemorrhage".to_string(),
        }])];

        let triage = engine.analyze_study(&study, &results);
        assert!(triage.flags.contains(&TriageFlag::Critical));
    }

    #[test]
    fn triage_flag_ordering() {
        // Critical has highest priority score, Low has lowest
        assert!(TriageFlag::Critical.priority_score() > TriageFlag::Urgent.priority_score());
        assert!(TriageFlag::Urgent.priority_score() > TriageFlag::Routine.priority_score());
        assert!(TriageFlag::Routine.priority_score() > TriageFlag::Low.priority_score());
    }

    #[test]
    fn classification_triage() {
        let engine = AiTriageEngine::new();
        let study = test_study();
        let results = vec![InferenceResult::Classification(vec![ClassScore {
            label: "hemorrhage".to_string(),
            probability: 0.9,
        }])];

        let triage = engine.analyze_study(&study, &results);
        assert!(
            triage.flags.contains(&TriageFlag::Critical),
            "hemorrhage classification should trigger Critical, got {:?}",
            triage.flags
        );
    }
