// Auto-extracted from /home/z/diccy/crates/dicom-net/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_net::*;
    use dicom_core::ErrorKind;

    fn build_uid_item(item_type: u8, uid: &str) -> Vec<u8> {
        let mut out = vec![item_type, 0x00];
        out.extend_from_slice(
            &u16::try_from(uid.len())
                .unwrap_or_else(|_| panic!("UID item too long"))
                .to_be_bytes(),
        );
        out.extend_from_slice(uid.as_bytes());
        out
    }

    fn build_item(item_type: u8, body: &[u8]) -> Vec<u8> {
        let mut out = vec![item_type, 0x00];
        out.extend_from_slice(
            &u16::try_from(body.len())
                .unwrap_or_else(|_| panic!("item body too long"))
                .to_be_bytes(),
        );
        out.extend_from_slice(body);
        out
    }

    fn build_presentation_context_rq(id: u8) -> Vec<u8> {
        let mut body = Vec::new();
        body.push(id);
        body.push(0x00);
        body.extend_from_slice(&0u16.to_be_bytes());
        body.extend_from_slice(&build_uid_item(0x30, "1.2.840.10008.1.1"));
        body.extend_from_slice(&build_uid_item(0x40, "1.2.840.10008.1.2"));
        build_item(0x20, &body)
    }

    fn build_user_info() -> Vec<u8> {
        let mut body = Vec::new();
        let mut max_pdu = vec![0x51, 0x00];
        max_pdu.extend_from_slice(&4u16.to_be_bytes());
        max_pdu.extend_from_slice(&16_384u32.to_be_bytes());
        body.extend_from_slice(&max_pdu);
        build_item(0x50, &body)
    }

    fn build_associate_rq() -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&1u16.to_be_bytes());
        body.extend_from_slice(&0u16.to_be_bytes());
        body.extend_from_slice(b"CALLED_AE       ");
        body.extend_from_slice(b"CALLING_AE      ");
        body.extend_from_slice(&[0u8; 32]);
        body.extend_from_slice(&build_uid_item(0x10, APPLICATION_CONTEXT_UID));
        body.extend_from_slice(&build_presentation_context_rq(0x01));
        body.extend_from_slice(&build_user_info());

        let mut pdu = vec![0x01, 0x00];
        pdu.extend_from_slice(
            &u32::try_from(body.len())
                .unwrap_or_else(|_| panic!("PDU body too long"))
                .to_be_bytes(),
        );
        pdu.extend_from_slice(&body);
        pdu
    }

    #[test]
    fn parse_associate_rq_basic() {
        // REQ-NET-300: PDU length must be validated and parsed deterministically.
        let pdu = build_associate_rq();
        let parsed = parse_pdu(&pdu, &NetworkLimits::default()).expect("parse");
        match parsed {
            Pdu::AssociateRq(req) => {
                assert_eq!(req.called_ae, "CALLED_AE");
                assert_eq!(req.calling_ae, "CALLING_AE");
                assert_eq!(req.application_context, APPLICATION_CONTEXT_UID);
                assert_eq!(req.presentation_contexts.len(), 1);
            }
            _ => panic!("expected associate rq"),
        }
    }

    #[test]
    fn encode_associate_rq_roundtrip() {
        // REQ-NET-300: encoded association PDUs must parse deterministically.
        let request = AssociationRequest {
            called_ae: "CALLED_AE".to_string(),
            calling_ae: "CALLING_AE".to_string(),
            application_context: APPLICATION_CONTEXT_UID.to_string(),
            presentation_contexts: vec![PresentationContext::new(
                0x01,
                "1.2.840.10008.1.1".to_string(),
                vec!["1.2.840.10008.1.2".to_string()],
            )
            .unwrap()],
            max_pdu_length: 16_384,
            implementation_class_uid: None,
            implementation_version_name: None,
        };
        let pdu = Pdu::AssociateRq(request.clone());
        let bytes = encode_pdu(&pdu, &NetworkLimits::default()).expect("encode");
        let parsed = parse_pdu(&bytes, &NetworkLimits::default()).expect("parse");
        match parsed {
            Pdu::AssociateRq(decoded) => {
                assert_eq!(decoded.called_ae, request.called_ae);
                assert_eq!(decoded.calling_ae, request.calling_ae);
                assert_eq!(decoded.presentation_contexts.len(), 1);
            }
            _ => panic!("expected associate rq"),
        }
    }

    #[test]
    fn encode_pdata_roundtrip() {
        // REQ-NET-303: PDV encoding must be deterministic.
        let pdv = Pdv {
            presentation_context_id: 1,
            message_control_header: 0x03,
            data: vec![1, 2, 3],
        };
        let bytes = encode_pdu(&Pdu::PDataTf(vec![pdv.clone()]), &NetworkLimits::default())
            .expect("encode");
        let parsed = parse_pdu(&bytes, &NetworkLimits::default()).expect("parse");
        match parsed {
            Pdu::PDataTf(pdvs) => {
                assert_eq!(pdvs.len(), 1);
                assert_eq!(pdvs[0], pdv);
            }
            _ => panic!("expected pdata"),
        }
    }

    #[test]
    fn accept_association_negotiates_transfer_syntax() {
        // REQ-NET-305: Association negotiation selects the first supported transfer syntax and result codes.
        let request = AssociationRequest {
            called_ae: "CALLED_AE".to_string(),
            calling_ae: "CALLING_AE".to_string(),
            application_context: APPLICATION_CONTEXT_UID.to_string(),
            presentation_contexts: vec![
                PresentationContext::new(
                    0x01,
                    "1.2.3".to_string(),
                    vec!["1.2.840.10008.1.2".to_string()],
                )
                .unwrap(),
                PresentationContext::new(
                    0x03,
                    "1.2.3".to_string(),
                    vec!["1.2.840.10008.1.2.1".to_string()],
                )
                .unwrap(),
                PresentationContext::new(
                    0x05,
                    "9.9.9".to_string(),
                    vec!["1.2.840.10008.1.2".to_string()],
                )
                .unwrap(),
            ],
            max_pdu_length: 32_768,
            implementation_class_uid: None,
            implementation_version_name: None,
        };
        let policy = AssociationPolicy {
            called_ae: None,
            supported_abstract_syntaxes: vec!["1.2.3".to_string()],
            supported_transfer_syntaxes: vec!["1.2.840.10008.1.2".to_string()],
            max_pdu_length: 16_384,
        };

        let accept =
            accept_association(&request, &policy, &NetworkLimits::default()).expect("accept");
        assert_eq!(accept.max_pdu_length, 16_384);
        assert_eq!(accept.presentation_contexts.len(), 3);
        assert_eq!(accept.presentation_contexts[0].result, 0x00);
        assert_eq!(
            accept.presentation_contexts[0].transfer_syntax.as_deref(),
            Some("1.2.840.10008.1.2")
        );
        assert_eq!(accept.presentation_contexts[1].result, 0x04);
        assert_eq!(accept.presentation_contexts[2].result, 0x03);
    }

    #[test]
    fn association_state_machine_requestor_flow() {
        // REQ-NET-306: Association state machine enforces valid UL sequencing.
        let mut fsm = AssociationStateMachine::new(AssociationRole::Requestor);
        fsm.on_event(AssociationEvent::Send(AssociationPduType::AssociateRq))
            .expect("send rq");
        fsm.on_event(AssociationEvent::Receive(AssociationPduType::AssociateAc))
            .expect("receive ac");
        fsm.on_event(AssociationEvent::Send(AssociationPduType::PDataTf))
            .expect("send data");
        fsm.on_event(AssociationEvent::Receive(AssociationPduType::ReleaseRq))
            .expect("receive release");
        fsm.on_event(AssociationEvent::Send(AssociationPduType::ReleaseRp))
            .expect("send release rp");
        assert_eq!(fsm.state(), AssociationState::Closed);
    }

    #[test]
    fn association_state_machine_rejects_out_of_order() {
        // REQ-NET-306: Out-of-order UL PDUs fail closed.
        // REQ-NET-307: UL sequencing failures surface DecodeError with dicom-net stage.
        let mut fsm = AssociationStateMachine::new(AssociationRole::Requestor);
        let err = fsm
            .on_event(AssociationEvent::Send(AssociationPduType::PDataTf))
            .expect_err("error");
        match err.kind() {
            ErrorKind::DecodeError { stage, .. } => {
                assert_eq!(stage, "dicom-net");
            }
            _ => panic!("expected decode error"),
        }
    }

    #[test]
    fn parse_rejects_invalid_uid() {
        // REQ-NET-304: Invalid UIDs must fail closed.
        let mut pdu = build_associate_rq();
        let idx = pdu
            .windows(APPLICATION_CONTEXT_UID.len())
            .position(|w| w == APPLICATION_CONTEXT_UID.as_bytes())
            .expect("uid position");
        pdu[idx] = b'x';
        let err = parse_pdu(&pdu, &NetworkLimits::default()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::InvalidTagValue { .. }));
    }

    #[test]
    fn parse_rejects_invalid_ae_title() {
        // REQ-NET-301: AE titles must be ASCII and non-empty.
        let mut pdu = build_associate_rq();
        let start = 6 + 4;
        for byte in &mut pdu[start..start + 16] {
            *byte = b' ';
        }
        let err = parse_pdu(&pdu, &NetworkLimits::default()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn parse_rejects_duplicate_presentation_context_ids() {
        // REQ-NET-307: Presentation context IDs must be odd and unique.
        let mut body = Vec::new();
        body.extend_from_slice(&1u16.to_be_bytes());
        body.extend_from_slice(&0u16.to_be_bytes());
        body.extend_from_slice(b"CALLED_AE       ");
        body.extend_from_slice(b"CALLING_AE      ");
        body.extend_from_slice(&[0u8; 32]);
        body.extend_from_slice(&build_uid_item(0x10, APPLICATION_CONTEXT_UID));
        body.extend_from_slice(&build_presentation_context_rq(0x01));
        body.extend_from_slice(&build_presentation_context_rq(0x01));
        body.extend_from_slice(&build_user_info());

        let mut pdu = vec![0x01, 0x00];
        pdu.extend_from_slice(
            &u32::try_from(body.len())
                .unwrap_or_else(|_| panic!("PDU body too long"))
                .to_be_bytes(),
        );
        pdu.extend_from_slice(&body);

        let err = parse_pdu(&pdu, &NetworkLimits::default()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn parse_rejects_even_presentation_context_id() {
        // REQ-NET-307: Presentation context IDs must be odd and unique.
        let mut body = Vec::new();
        body.extend_from_slice(&1u16.to_be_bytes());
        body.extend_from_slice(&0u16.to_be_bytes());
        body.extend_from_slice(b"CALLED_AE       ");
        body.extend_from_slice(b"CALLING_AE      ");
        body.extend_from_slice(&[0u8; 32]);
        body.extend_from_slice(&build_uid_item(0x10, APPLICATION_CONTEXT_UID));
        body.extend_from_slice(&build_presentation_context_rq(0x02));
        body.extend_from_slice(&build_user_info());

        let mut pdu = vec![0x01, 0x00];
        pdu.extend_from_slice(
            &u32::try_from(body.len())
                .unwrap_or_else(|_| panic!("PDU body too long"))
                .to_be_bytes(),
        );
        pdu.extend_from_slice(&body);

        let err = parse_pdu(&pdu, &NetworkLimits::default()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn parse_rejects_unsupported_application_context() {
        // REQ-NET-302: Application context must match the DICOM UID.
        let mut pdu = build_associate_rq();
        let idx = pdu
            .windows(APPLICATION_CONTEXT_UID.len())
            .position(|w| w == APPLICATION_CONTEXT_UID.as_bytes())
            .expect("uid position");
        pdu[idx + APPLICATION_CONTEXT_UID.len() - 1] = b'2';
        let err = parse_pdu(&pdu, &NetworkLimits::default()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn parse_enforces_pdu_limit() {
        // REQ-SEC-401: default limits are enforced for networking PDUs.
        let pdu = build_associate_rq();
        let limits = NetworkLimits {
            max_pdu_bytes: 64,
            ..NetworkLimits::default()
        };
        let err = parse_pdu(&pdu, &limits).expect_err("error");
        match err.kind() {
            ErrorKind::LimitExceeded { limit_name, .. } => {
                assert_eq!(*limit_name, "max_pdu_bytes");
            }
            _ => panic!("expected limit exceeded"),
        }
    }

    #[test]
    fn presentation_context_limit_enforced() {
        // REQ-SEC-401: presentation context limits are enforced.
        let mut body = Vec::new();
        body.extend_from_slice(&1u16.to_be_bytes());
        body.extend_from_slice(&0u16.to_be_bytes());
        body.extend_from_slice(b"CALLED_AE       ");
        body.extend_from_slice(b"CALLING_AE      ");
        body.extend_from_slice(&[0u8; 32]);
        body.extend_from_slice(&build_uid_item(0x10, APPLICATION_CONTEXT_UID));
        body.extend_from_slice(&build_presentation_context_rq(0x01));
        body.extend_from_slice(&build_presentation_context_rq(0x03));
        body.extend_from_slice(&build_user_info());

        let mut pdu = vec![0x01, 0x00];
        pdu.extend_from_slice(
            &u32::try_from(body.len())
                .unwrap_or_else(|_| panic!("PDU body too long"))
                .to_be_bytes(),
        );
        pdu.extend_from_slice(&body);

        let limits = NetworkLimits {
            max_presentation_contexts: 1,
            ..NetworkLimits::default()
        };
        let err = parse_pdu(&pdu, &limits).expect_err("error");
        match err.kind() {
            ErrorKind::LimitExceeded { limit_name, .. } => {
                assert_eq!(*limit_name, "max_presentation_contexts");
            }
            _ => panic!("expected limit exceeded"),
        }
    }

    #[test]
    fn pdv_length_limit_enforced() {
        // REQ-NET-303: PDV limits fail closed.
        let mut body = Vec::new();
        let pdv_len = 300u32;
        body.extend_from_slice(&pdv_len.to_be_bytes());
        body.push(0x01);
        body.push(0x03);
        body.extend_from_slice(&vec![0u8; usize::try_from(pdv_len - 2).unwrap_or(0)]);
        let mut pdu = vec![0x04, 0x00];
        pdu.extend_from_slice(
            &u32::try_from(body.len())
                .unwrap_or_else(|_| panic!("PDU body too long"))
                .to_be_bytes(),
        );
        pdu.extend_from_slice(&body);
        let limits = NetworkLimits {
            max_pdv_bytes: 128,
            ..NetworkLimits::default()
        };
        let err = parse_pdu(&pdu, &limits).expect_err("error");
        match err.kind() {
            ErrorKind::LimitExceeded { limit_name, .. } => {
                assert_eq!(*limit_name, "max_pdv_bytes");
            }
            _ => panic!("expected limit exceeded"),
        }
    }
