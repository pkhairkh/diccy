// Auto-extracted from /home/z/diccy/crates/dicom-xr/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_xr::*;

    #[test]
    fn xr_render_config_validation() {
        let valid = XrRenderConfig::default();
        assert!(valid.validate().is_ok());

        let bad_ipd = XrRenderConfig {
            ipd: -0.1,
            ..Default::default()
        };
        assert!(bad_ipd.validate().is_err());

        let bad_fov = XrRenderConfig {
            fov_degrees: 200.0,
            ..Default::default()
        };
        assert!(bad_fov.validate().is_err());
    }

    #[test]
    fn xr_render_config_eye_offset() {
        let config = XrRenderConfig {
            ipd: 0.064,
            ..Default::default()
        };
        let left = config.eye_offset(Eye::Left);
        let right = config.eye_offset(Eye::Right);
        assert!(left[0] < 0.0, "left eye offset should be negative x");
        assert!(right[0] > 0.0, "right eye offset should be positive x");
        assert!((left[0].abs() - 0.032).abs() < 1e-10);
        assert!((right[0].abs() - 0.032).abs() < 1e-10);
    }

    #[test]
    fn xr_session_lifecycle() {
        let config = XrRenderConfig::default();
        let mut session = XrViewerSession::new(config, "1.2.3").expect("session");
        assert!(!session.active);

        session.start().expect("start");
        assert!(session.active);

        // Cannot start twice
        assert!(session.start().is_err());

        session.stop();
        assert!(!session.active);
    }

    #[test]
    fn xr_session_head_pose_update() {
        let config = XrRenderConfig::default();
        let mut session = XrViewerSession::new(config, "1.2.3").expect("session");
        session.start().expect("start");

        let pose = HeadPose::new([1.0, 2.0, 3.0], [0.0, 0.0, 0.0, 1.0], 100.0).expect("pose");
        session.update_head_pose(pose.clone()).expect("update");
        assert_eq!(session.head_pose, pose);
    }

    #[test]
    fn xr_session_head_pose_rejects_bad_quaternion() {
        let config = XrRenderConfig::default();
        let mut session = XrViewerSession::new(config, "1.2.3").expect("session");
        session.start().expect("start");

        let bad_pose = HeadPose::new([0.0, 0.0, 0.0], [10.0, 10.0, 10.0, 10.0], 0.0).expect("should create despite non-unit");
        assert!(session.update_head_pose(bad_pose).is_err());
    }

    #[test]
    fn hand_tracking_pinch_distance() {
        let mut hand = HandTrackingData::new(Hand::Right);
        hand.joint_positions[HandJoint::ThumbTip as usize] = [0.0, 0.0, 0.0];
        hand.joint_positions[HandJoint::IndexTip as usize] = [0.03, 0.0, 0.0];
        let dist = hand.pinch_distance();
        assert!((dist - 0.03).abs() < 1e-10, "pinch distance should be 3cm");
    }

    #[test]
    fn xr_clip_plane_interaction() {
        let config = XrRenderConfig::default();
        let mut session = XrViewerSession::new(config, "1.2.3").expect("session");
        session.start().expect("start");

        let idx = session.add_clip_plane(Hand::Right).expect("add clip");
        assert_eq!(idx, 0);
        assert_eq!(session.clip_planes.len(), 1);
        assert!(session.clip_planes[0].active);

        session.remove_clip_plane(0).expect("remove clip");
        assert!(session.clip_planes.is_empty());
    }

    #[test]
    fn playback_4d_state() {
        let mut playback = Playback4dState::new(10);
        assert_eq!(playback.current_time_point, 0);
        assert!(!playback.playing);

        playback.playing = true;
        playback.advance();
        assert_eq!(playback.current_time_point, 1);

        // Seek
        playback.seek(5).expect("seek");
        assert_eq!(playback.current_time_point, 5);
    }

    #[test]
    fn playback_4d_looping() {
        let mut playback = Playback4dState::new(3);
        playback.playing = true;
        playback.looping = true;

        playback.advance(); // 1
        playback.advance(); // 2
        playback.advance(); // wraps to 0
        assert_eq!(playback.current_time_point, 0);
    }

    #[test]
    fn playback_4d_no_loop() {
        let mut playback = Playback4dState::new(3);
        playback.playing = true;
        playback.looping = false;

        playback.advance(); // 1
        playback.advance(); // 2
        playback.advance(); // stays at 2, stops
        assert_eq!(playback.current_time_point, 2);
        assert!(!playback.playing);
    }

    #[test]
    fn playback_4d_validation() {
        let bad = Playback4dState {
            total_time_points: 0,
            ..Default::default()
        };
        assert!(bad.validate().is_err());

        let bad2 = Playback4dState {
            speed: -1.0,
            ..Default::default()
        };
        assert!(bad2.validate().is_err());
    }
