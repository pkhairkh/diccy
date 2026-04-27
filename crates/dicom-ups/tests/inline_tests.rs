// Auto-extracted from /home/z/diccy/crates/dicom-ups/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_ups::*;
    use dicom_core::{ErrorKind};

    #[test]
    fn create_start_complete_lifecycle() {
        let mut adapter = UpsCommandAdapter::new();
        let uid = "1.2.840.10008.5.1.4.34.1".to_string();
        let created = adapter
            .create(uid.clone(), "CT Abdomen".to_string())
            .expect("create");
        assert_eq!(created.state, UpsState::Scheduled);

        let started = adapter.start(&uid).expect("start");
        assert_eq!(started.state, UpsState::InProgress);
        let completed = adapter.complete(&uid).expect("complete");
        assert_eq!(completed.state, UpsState::Completed);
    }

    #[test]
    fn invalid_transition_fails_closed() {
        let mut adapter = UpsCommandAdapter::new();
        let uid = "1.2.840.10008.5.1.4.34.2".to_string();
        adapter
            .create(uid.clone(), "MR Brain".to_string())
            .expect("create");
        let err = adapter.complete(&uid).expect_err("invalid transition");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn replay_requires_state_consistency() {
        let snapshot = vec![UpsWorkitem {
            ups_instance_uid: "1.2.840.10008.5.1.4.34.3".to_string(),
            procedure_step_label: "PET".to_string(),
            state: UpsState::Scheduled,
            revision: 1,
        }];
        let events = vec![UpsEvent {
            ups_instance_uid: "1.2.840.10008.5.1.4.34.3".to_string(),
            from_state: UpsState::Scheduled,
            to_state: UpsState::InProgress,
            revision: 2,
        }];
        let replayed = UpsStore::replay(snapshot, &events).expect("replay");
        let item = replayed
            .get_workitem("1.2.840.10008.5.1.4.34.3")
            .expect("item");
        assert_eq!(item.state, UpsState::InProgress);
        assert_eq!(replayed.events().len(), 1);
    }
