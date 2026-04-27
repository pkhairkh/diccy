// Auto-extracted from /home/z/diccy/crates/dicom-collab/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_collab::*;

    /// Helper to create a SessionId from a &str.
    fn sid(s: &str) -> SessionId {
        SessionId::from(s.to_string())
    }

    /// Helper to create a UserId from a &str.
    fn uid(s: &str) -> UserId {
        UserId::from(s.to_string())
    }

    /// Helper to create a Tick from a u64.
    fn tk(v: u64) -> Tick {
        Tick::new(v)
    }

    #[test]
    fn user_color_hex_format() {
        // REQ-COLLAB-100: user colors must produce valid CSS hex
        let color = UserColor::new(255, 128, 0);
        assert_eq!(color.to_hex(), "#ff8000");
    }

    #[test]
    fn user_color_palette_has_8_entries() {
        assert_eq!(user_colors().len(), 8);
    }

    #[test]
    fn user_presence_creation() {
        let presence = UserPresence::new(uid("user1"), "Dr. Smith".to_string(), 0);
        assert_eq!(presence.user_id, uid("user1"));
        assert_eq!(presence.display_name, "Dr. Smith");
        assert!(presence.cursor_position.is_none());
    }

    #[test]
    fn session_join_and_leave() {
        let mut session = CollabSession::new(sid("session-1"));
        session.join(uid("user1"), "Dr. Smith".to_string()).unwrap();
        assert_eq!(session.user_count(), 1);

        session.join(uid("user2"), "Dr. Jones".to_string()).unwrap();
        assert_eq!(session.user_count(), 2);

        session.leave(&uid("user1")).unwrap();
        assert_eq!(session.user_count(), 1);
    }

    #[test]
    fn session_rejects_duplicate_join() {
        let mut session = CollabSession::new(sid("session-1"));
        session.join(uid("user1"), "Dr. Smith".to_string()).unwrap();
        assert!(session.join(uid("user1"), "Dr. Smith".to_string()).is_err());
    }

    #[test]
    fn session_rejects_unknown_leave() {
        let mut session = CollabSession::new(sid("session-1"));
        assert!(session.leave(&uid("ghost")).is_err());
    }

    #[test]
    fn viewport_change_syncs() {
        let mut session = CollabSession::new(sid("session-1"));
        session.join(uid("user1"), "Dr. Smith".to_string()).unwrap();

        let state = SharedViewportState {
            pan_x: 10.0,
            pan_y: 20.0,
            zoom: 2.0,
            window_center: 40.0,
            window_width: 400.0,
            frame_index: 0,
            viewport_index: 0,
        };

        session
            .apply_operation(CollabOperation::ViewportChange {
                state: state.clone(),
                tick: tk(1),
                user_id: uid("user1"),
            })
            .unwrap();

        let synced = session.viewport_state(0).unwrap();
        assert_eq!(synced.pan_x, 10.0);
        assert_eq!(synced.zoom, 2.0);
    }

    #[test]
    fn cursor_move_updates_presence() {
        let mut session = CollabSession::new(sid("session-1"));
        session.join(uid("user1"), "Dr. Smith".to_string()).unwrap();

        session
            .apply_operation(CollabOperation::CursorMove {
                position: (100.0, 200.0),
                viewport_index: 0,
                tick: tk(1),
                user_id: uid("user1"),
            })
            .unwrap();

        let cursors = session.viewport_cursors(0);
        assert_eq!(cursors.len(), 1);
        assert_eq!(cursors[0].0, &uid("user1"));
        assert_eq!(cursors[0].1, (100.0, 200.0));
    }

    #[test]
    fn operation_from_unknown_user_rejected() {
        let mut session = CollabSession::new(sid("session-1"));
        let result = session.apply_operation(CollabOperation::ViewportChange {
            state: SharedViewportState::default(),
            tick: tk(1),
            user_id: uid("unknown"),
        });
        assert!(result.is_err());
    }

    #[test]
    fn lww_register_last_writer_wins() {
        let mut reg = LwwRegister::new(0, tk(0), uid("user1"));
        reg.write(10, tk(1), uid("user1"));
        reg.write(20, tk(2), uid("user2"));
        assert_eq!(*reg.value(), 20);
    }

    #[test]
    fn lww_register_deterministic_tie_breaking() {
        let mut reg = LwwRegister::new(0, tk(5), uid("user_b"));
        let other = LwwRegister::new(42, tk(5), uid("user_a"));
        reg.merge(&other);
        // "user_a" < "user_b" lexicographically, so user_a wins
        assert_eq!(*reg.value(), 42);
    }

    #[test]
    fn g_set_add_and_merge() {
        let mut set1 = GSet::new();
        set1.add("annotation-1".to_string());
        set1.add("annotation-2".to_string());

        let mut set2 = GSet::new();
        set2.add("annotation-2".to_string());
        set2.add("annotation-3".to_string());

        set1.merge(&set2);
        assert_eq!(set1.len(), 3);
        assert!(set1.contains(&"annotation-1".to_string()));
        assert!(set1.contains(&"annotation-3".to_string()));
    }

    #[test]
    fn or_set_add_remove_and_merge() {
        let mut set = OrSet::new();
        set.add("ann-1".to_string(), "tag-1".to_string());
        set.add("ann-2".to_string(), "tag-2".to_string());
        assert_eq!(set.len(), 2);

        set.remove(&"ann-1".to_string());
        assert_eq!(set.len(), 1);
        assert!(!set.contains(&"ann-1".to_string()));
    }

    #[test]
    fn or_set_concurrent_add_remove_merge() {
        let mut set1 = OrSet::new();
        set1.add("ann-1".to_string(), "tag-1".to_string());

        let mut set2 = OrSet::new();
        set2.add("ann-1".to_string(), "tag-1".to_string());
        set2.remove(&"ann-1".to_string());

        set1.merge(&set2);
        // After merge, ann-1 should be removed because tag-1 is tombstoned
        assert!(!set1.contains(&"ann-1".to_string()));
    }

    #[test]
    fn annotation_add_and_remove_operations() {
        let mut session = CollabSession::new(sid("session-1"));
        session.join(uid("user1"), "Dr. Smith".to_string()).unwrap();

        session
            .apply_operation(CollabOperation::AnnotationAdd {
                annotation_id: "ann-1".to_string(),
                annotation_data: "{}".to_string(),
                tick: tk(1),
                user_id: uid("user1"),
            })
            .unwrap();

        assert_eq!(session.annotation_count(), 1);

        session
            .apply_operation(CollabOperation::AnnotationRemove {
                annotation_id: "ann-1".to_string(),
                tick: tk(2),
                user_id: uid("user1"),
            })
            .unwrap();

        assert_eq!(session.annotation_count(), 0);
    }

    #[test]
    fn operation_log_trim() {
        let mut session = CollabSession::with_max_log_size(sid("session-1"), 5);
        session.join(uid("user1"), "Dr. Smith".to_string()).unwrap();

        for i in 0..10 {
            session
                .apply_operation(CollabOperation::CursorMove {
                    position: (i as f64, i as f64),
                    viewport_index: 0,
                    tick: tk(i),
                    user_id: uid("user1"),
                })
                .unwrap();
        }

        assert!(session.operation_log().len() <= 5);
    }

    #[test]
    fn session_serialization() {
        let mut session = CollabSession::new(sid("session-1"));
        session.join(uid("user1"), "Dr. Smith".to_string()).unwrap();

        let json = session.to_json().unwrap();
        assert!(json.contains("session-1"));
        assert!(json.contains("user1"));
    }

    #[test]
    fn merge_from_another_session() {
        let mut session1 = CollabSession::new(sid("session-1"));
        session1
            .join(uid("user1"), "Dr. Smith".to_string())
            .unwrap();

        let mut session2 = CollabSession::new(sid("session-1"));
        session2
            .join(uid("user1"), "Dr. Smith".to_string())
            .unwrap();

        session2
            .apply_operation(CollabOperation::ViewportChange {
                state: SharedViewportState {
                    zoom: 3.0,
                    ..SharedViewportState::default()
                },
                tick: tk(10),
                user_id: uid("user1"),
            })
            .unwrap();

        session1.merge_from(session2.operation_log());
        // After merge, session1 should have the viewport change
        assert!(session1.viewport_state(0).is_some());
    }

    #[test]
    fn multiple_users_cursors_in_viewport() {
        let mut session = CollabSession::new(sid("session-1"));
        session.join(uid("user1"), "Dr. Smith".to_string()).unwrap();
        session.join(uid("user2"), "Dr. Jones".to_string()).unwrap();

        session
            .apply_operation(CollabOperation::CursorMove {
                position: (10.0, 20.0),
                viewport_index: 0,
                tick: tk(1),
                user_id: uid("user1"),
            })
            .unwrap();

        session
            .apply_operation(CollabOperation::CursorMove {
                position: (30.0, 40.0),
                viewport_index: 0,
                tick: tk(2),
                user_id: uid("user2"),
            })
            .unwrap();

        let cursors = session.viewport_cursors(0);
        assert_eq!(cursors.len(), 2);
    }

    #[test]
    fn shared_viewport_default_values() {
        let state = SharedViewportState::default();
        assert_eq!(state.pan_x, 0.0);
        assert_eq!(state.zoom, 1.0);
        assert_eq!(state.frame_index, 0);
    }

    // --- S12-T5: Newtype-specific tests ---

    #[test]
    fn tick_zero_next_and_value() {
        let t0 = Tick::zero();
        assert_eq!(t0.value(), 0);
        let t1 = t0.next();
        assert_eq!(t1.value(), 1);
        let t2 = t1.next();
        assert_eq!(t2.value(), 2);
    }

    #[test]
    fn tick_ordering() {
        assert!(Tick::new(1) < Tick::new(2));
        assert!(Tick::new(2) > Tick::new(1));
        assert_eq!(Tick::new(3), Tick::new(3));
    }

    #[test]
    fn tick_display() {
        assert_eq!(format!("{}", Tick::new(42)), "42");
    }

    #[test]
    fn session_id_display_and_as_ref() {
        let sid = SessionId::from("abc".to_string());
        assert_eq!(sid.as_ref(), "abc");
        assert_eq!(format!("{}", sid), "abc");
    }

    #[test]
    fn user_id_display_and_as_ref() {
        let uid = UserId::from("dr-smith".to_string());
        assert_eq!(uid.as_ref(), "dr-smith");
        assert_eq!(format!("{}", uid), "dr-smith");
    }

    #[test]
    fn session_id_from_str_roundtrip() {
        let sid: SessionId = "hello".parse().unwrap();
        assert_eq!(sid.as_ref(), "hello");
    }

    #[test]
    fn user_id_from_str_roundtrip() {
        let uid: UserId = "world".parse().unwrap();
        assert_eq!(uid.as_ref(), "world");
    }
