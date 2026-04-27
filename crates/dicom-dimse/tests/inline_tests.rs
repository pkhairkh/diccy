// Auto-extracted from /home/z/diccy/crates/dicom-dimse/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_dimse::*;
    use dicom_core::ErrorKind;

    #[test]
    fn parse_c_echo_request() {
        // REQ-DIMSE-300: Verification SOP C-ECHO requests must parse deterministically.
        let bytes = build_c_echo_request(7);
        let msg = parse_command_set(&bytes, &DimseLimits::default()).expect("parse");
        match msg {
            DimseMessage::CEchoRq { message_id, .. } => {
                assert_eq!(message_id, 7);
            }
            _ => panic!("expected C-ECHO request"),
        }
    }

    #[test]
    fn parse_rejects_unsupported_sop() {
        // REQ-DIMSE-301: Unsupported SOP classes fail closed.
        // REQ-DIMSE-305: Unsupported SOP class UIDs map to UnsupportedSopClass.
        let mut bytes = build_c_echo_request(1);
        let idx = bytes
            .windows(SOP_CLASS_VERIFICATION.len())
            .position(|w| w == SOP_CLASS_VERIFICATION.as_bytes())
            .expect("uid position");
        bytes[idx] = b'9';
        let err = parse_command_set(&bytes, &DimseLimits::default()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::UnsupportedSopClass { .. }));
    }

    #[test]
    fn decode_error_stage_is_dicom_dimse() {
        // REQ-DIMSE-305: DIMSE command parse failures surface DecodeError with dicom-dimse stage.
        let err = parse_command_set(&[], &DimseLimits::default()).expect_err("error");
        match err.kind() {
            ErrorKind::DecodeError { stage, .. } => {
                assert_eq!(stage, "dicom-dimse");
            }
            _ => panic!("expected decode error"),
        }
    }

    #[test]
    fn command_length_limit_enforced() {
        // REQ-SEC-401: DIMSE command limits are enforced.
        let bytes = build_c_echo_request(1);
        let limits = DimseLimits {
            max_command_bytes: 8,
        };
        let err = parse_command_set(&bytes, &limits).expect_err("error");
        match err.kind() {
            ErrorKind::LimitExceeded { limit_name, .. } => {
                assert_eq!(*limit_name, "max_command_bytes");
            }
            _ => panic!("expected limit exceeded"),
        }
    }

    #[test]
    fn parse_c_store_request() {
        // REQ-DIMSE-302: C-STORE requests must parse deterministically.
        let bytes = build_c_store_request(9, "1.2.840.10008.5.1.4.1.1.2", "1.2.3.4.5", 0);
        let msg = parse_command_set(&bytes, &DimseLimits::default()).expect("parse");
        match msg {
            DimseMessage::CStoreRq {
                message_id,
                sop_instance_uid,
                ..
            } => {
                assert_eq!(message_id, 9);
                assert_eq!(sop_instance_uid, "1.2.3.4.5");
            }
            _ => panic!("expected C-STORE request"),
        }
    }

    #[cfg(not(feature = "dimse-c-find"))]
    #[test]
    fn parse_rejects_unsupported_command_field() {
        // REQ-DIMSE-304: Unsupported DIMSE commands must fail closed.
        let mut body = Vec::new();
        write_ui(&mut body, 0x0002, "1.2.840.10008.5.1.4.1.2.1.1");
        write_us(&mut body, 0x0100, 0x0020);
        write_us(&mut body, 0x0110, 1);
        write_us(&mut body, 0x0800, 0x0000);

        let mut out = Vec::new();
        write_ul(
            &mut out,
            0x0000,
            u32::try_from(body.len())
                .map_err(|_| "command set length exceeds u32".to_string())
                .unwrap_or(u32::MAX),
        );
        out.extend_from_slice(&body);

        let err = parse_command_set(&out, &DimseLimits::default()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[cfg(not(feature = "dimse-c-move"))]
    #[test]
    fn parse_rejects_unsupported_c_move_command_field() {
        // REQ-DIMSE-304: Deferred C-MOVE commands must fail closed when feature is disabled.
        let mut body = Vec::new();
        write_ui(&mut body, 0x0002, "1.2.840.10008.5.1.4.1.2.2.2");
        write_us(&mut body, 0x0100, 0x0021);
        write_us(&mut body, 0x0110, 1);
        write_us(&mut body, 0x0800, 0x0000);

        let mut out = Vec::new();
        write_ul(
            &mut out,
            0x0000,
            u32::try_from(body.len())
                .map_err(|_| "command set length exceeds u32".to_string())
                .unwrap_or(u32::MAX),
        );
        out.extend_from_slice(&body);

        let err = parse_command_set(&out, &DimseLimits::default()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[cfg(not(feature = "dimse-c-get"))]
    #[test]
    fn parse_rejects_unsupported_c_get_command_field() {
        // REQ-DIMSE-304: Deferred C-GET commands must fail closed when feature is disabled.
        let mut body = Vec::new();
        write_ui(&mut body, 0x0002, "1.2.840.10008.5.1.4.1.2.2.3");
        write_us(&mut body, 0x0100, 0x0010);
        write_us(&mut body, 0x0110, 1);
        write_us(&mut body, 0x0800, 0x0000);

        let mut out = Vec::new();
        write_ul(
            &mut out,
            0x0000,
            u32::try_from(body.len())
                .map_err(|_| "command set length exceeds u32".to_string())
                .unwrap_or(u32::MAX),
        );
        out.extend_from_slice(&body);

        let err = parse_command_set(&out, &DimseLimits::default()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn parse_rejects_unknown_command_element() {
        // REQ-DIMSE-304: Unsupported command elements fail closed.
        let mut bytes = build_c_echo_request(1);
        let mut unknown = Vec::new();
        write_us(&mut unknown, 0x9999, 1);
        bytes.extend_from_slice(&unknown);
        let err = parse_command_set(&bytes, &DimseLimits::default()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn parse_rejects_duplicate_command_elements() {
        // REQ-DIMSE-304: Duplicate command elements fail closed.
        let mut bytes = build_c_echo_request(1);
        let mut duplicate = Vec::new();
        write_us(&mut duplicate, 0x0110, 2);
        bytes.extend_from_slice(&duplicate);
        let err = parse_command_set(&bytes, &DimseLimits::default()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn parse_rejects_out_of_envelope_command_field() {
        // REQ-DIMSE-301/304: out-of-envelope DIMSE command fields must fail closed.
        let mut body = Vec::new();
        write_ui(&mut body, 0x0002, SOP_CLASS_VERIFICATION);
        write_us(&mut body, 0x0100, 0x7FFF);
        write_us(&mut body, 0x0110, 1);

        let mut out = Vec::new();
        write_ul(
            &mut out,
            0x0000,
            u32::try_from(body.len())
                .map_err(|_| "command set length exceeds u32".to_string())
                .unwrap_or(u32::MAX),
        );
        out.extend_from_slice(&body);

        let err = parse_command_set(&out, &DimseLimits::default()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[cfg(feature = "dimse-c-find")]
    #[test]
    fn parse_c_find_request_response() {
        // REQ-DIMSE-310: C-FIND command sets must parse deterministically when enabled.
        let request = build_c_find_request(5, "1.2.840.10008.5.1.4.1.2.1.1", 0);
        let msg = parse_command_set(&request, &DimseLimits::default()).expect("parse");
        match msg {
            DimseMessage::CFindRq {
                message_id,
                sop_class_uid,
                command_data_set_type,
                ..
            } => {
                assert_eq!(message_id, 5);
                assert_eq!(sop_class_uid, "1.2.840.10008.5.1.4.1.2.1.1");
                assert_eq!(command_data_set_type, 0x0000);
            }
            _ => panic!("expected C-FIND request"),
        }

        let response = build_c_find_response(5, 0x0000, "1.2.840.10008.5.1.4.1.2.1.1", 0x0101);
        let msg = parse_command_set(&response, &DimseLimits::default()).expect("parse");
        match msg {
            DimseMessage::CFindRsp {
                message_id_responded_to,
                status,
                command_data_set_type,
                ..
            } => {
                assert_eq!(message_id_responded_to, 5);
                assert_eq!(status, 0x0000);
                assert_eq!(command_data_set_type, 0x0101);
            }
            _ => panic!("expected C-FIND response"),
        }
    }

    #[cfg(feature = "dimse-c-move")]
    #[test]
    fn parse_c_move_request_response() {
        // REQ-DIMSE-320: C-MOVE command sets must parse deterministically when enabled.
        let request = build_c_move_request(7, "1.2.840.10008.5.1.4.1.2.2.1", "DEST_AE", 1);
        let msg = parse_command_set(&request, &DimseLimits::default()).expect("parse");
        match msg {
            DimseMessage::CMoveRq {
                message_id,
                move_destination,
                command_data_set_type,
                ..
            } => {
                assert_eq!(message_id, 7);
                assert_eq!(move_destination, "DEST_AE");
                assert_eq!(command_data_set_type, 0x0000);
            }
            _ => panic!("expected C-MOVE request"),
        }

        let response = build_c_move_response(7, 0x0000, "1.2.840.10008.5.1.4.1.2.2.1", 0x0101);
        let msg = parse_command_set(&response, &DimseLimits::default()).expect("parse");
        match msg {
            DimseMessage::CMoveRsp {
                message_id_responded_to,
                status,
                command_data_set_type,
                ..
            } => {
                assert_eq!(message_id_responded_to, 7);
                assert_eq!(status, 0x0000);
                assert_eq!(command_data_set_type, 0x0101);
            }
            _ => panic!("expected C-MOVE response"),
        }
    }

    #[cfg(feature = "dimse-c-get")]
    #[test]
    fn parse_c_get_request_response() {
        // REQ-DIMSE-330: C-GET command sets must parse deterministically when enabled.
        let request = build_c_get_request(9, "1.2.840.10008.5.1.4.1.2.3.1", 2);
        let msg = parse_command_set(&request, &DimseLimits::default()).expect("parse");
        match msg {
            DimseMessage::CGetRq {
                message_id,
                command_data_set_type,
                ..
            } => {
                assert_eq!(message_id, 9);
                assert_eq!(command_data_set_type, 0x0000);
            }
            _ => panic!("expected C-GET request"),
        }

        let response = build_c_get_response(9, 0x0000, "1.2.840.10008.5.1.4.1.2.3.1", 0x0101);
        let msg = parse_command_set(&response, &DimseLimits::default()).expect("parse");
        match msg {
            DimseMessage::CGetRsp {
                message_id_responded_to,
                status,
                command_data_set_type,
                ..
            } => {
                assert_eq!(message_id_responded_to, 9);
                assert_eq!(status, 0x0000);
                assert_eq!(command_data_set_type, 0x0101);
            }
            _ => panic!("expected C-GET response"),
        }
    }
