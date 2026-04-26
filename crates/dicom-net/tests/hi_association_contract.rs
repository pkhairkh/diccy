use dicom_core::ErrorKind;
use dicom_net::{
    accept_association, encode_pdu, parse_pdu, AssociationEvent, AssociationPduType,
    AssociationPolicy, AssociationRequest, AssociationRole, AssociationState,
    AssociationStateMachine, NetworkLimits, Pdu, Pdv, PresentationContext,
};

const APP_CONTEXT_UID: &str = "1.2.840.10008.3.1.1.1";
const SOP_VERIFICATION: &str = "1.2.840.10008.1.1";
const TS_IMPLICIT_LE: &str = "1.2.840.10008.1.2";
const TS_EXPLICIT_LE: &str = "1.2.840.10008.1.2.1";

fn request_with_contexts(contexts: Vec<PresentationContext>) -> AssociationRequest {
    AssociationRequest {
        called_ae: "CALLED_AE".to_string(),
        calling_ae: "CALLING_AE".to_string(),
        application_context: APP_CONTEXT_UID.to_string(),
        presentation_contexts: contexts,
        max_pdu_length: 16_384,
        implementation_class_uid: None,
        implementation_version_name: None,
    }
}

#[test]
fn association_parsing_fails_closed_for_invalid_context_and_presentation_rules() {
    // REQ-HI-342, REQ-HI-343, REQ-HI-344
    let limits = NetworkLimits::default();

    let mut invalid_app_context = request_with_contexts(vec![PresentationContext::new(
        1,
        SOP_VERIFICATION.to_string(),
        vec![TS_IMPLICIT_LE.to_string()],
    ).unwrap()]);
    invalid_app_context.application_context = "1.2.3".to_string();
    let encoded = encode_pdu(&Pdu::AssociateRq(invalid_app_context), &limits).expect("encode");
    let err = parse_pdu(&encoded, &limits).expect_err("invalid app-context must fail");
    match &err.kind() {
        ErrorKind::DecodeError { detail, .. } => {
            assert!(detail.contains("application context"));
        }
        _ => panic!("expected decode error"),
    }

    // PresentationContext::new now validates that the ID must be odd,
    // so even IDs are rejected at construction time.
    let err = PresentationContext::new(
        2,
        SOP_VERIFICATION.to_string(),
        vec![TS_IMPLICIT_LE.to_string()],
    ).expect_err("even context IDs must fail");
    match &err.kind() {
        ErrorKind::DecodeError { detail, .. } => {
            assert!(detail.contains("presentation context ID must be odd"));
        }
        _ => panic!("expected decode error, got {:?}", err.kind()),
    }
}

#[test]
fn association_negotiation_results_are_deterministic_and_explicit() {
    // REQ-HI-347, REQ-HI-390
    let limits = NetworkLimits::default();
    let request = request_with_contexts(vec![
        PresentationContext::new(1, SOP_VERIFICATION.to_string(), vec![TS_EXPLICIT_LE.to_string(), TS_IMPLICIT_LE.to_string()]).unwrap(),
        PresentationContext::new(3, "1.2.840.10008.5.1.4.1.2.1.1".to_string(), vec![TS_IMPLICIT_LE.to_string()]).unwrap(),
        PresentationContext::new(5, SOP_VERIFICATION.to_string(), vec!["1.2.840.10008.1.2.4.70".to_string()]).unwrap(),
    ]);

    let policy = AssociationPolicy {
        called_ae: Some("CALLED_AE".to_string()),
        supported_abstract_syntaxes: vec![SOP_VERIFICATION.to_string()],
        supported_transfer_syntaxes: vec![TS_IMPLICIT_LE.to_string(), TS_EXPLICIT_LE.to_string()],
        max_pdu_length: 8_192,
    };

    let first = accept_association(&request, &policy, &limits).expect("accept");
    let second = accept_association(&request, &policy, &limits).expect("accept repeat");
    assert_eq!(first, second);
    assert_eq!(first.max_pdu_length, 8_192);
    assert_eq!(first.presentation_contexts.len(), 3);
    assert_eq!(first.presentation_contexts[0].result, 0x00);
    assert_eq!(
        first.presentation_contexts[0].transfer_syntax.as_deref(),
        Some(TS_IMPLICIT_LE)
    );
    assert_eq!(first.presentation_contexts[1].result, 0x03);
    assert!(first.presentation_contexts[1].transfer_syntax.is_none());
    assert_eq!(first.presentation_contexts[2].result, 0x04);
    assert!(first.presentation_contexts[2].transfer_syntax.is_none());
}

#[test]
fn state_machine_and_pdv_limits_expose_fail_closed_contracts() {
    // REQ-HI-345, REQ-HI-346, REQ-HI-391, REQ-HI-392
    let mut machine = AssociationStateMachine::new(AssociationRole::Requestor);
    let err = machine
        .on_event(AssociationEvent::Receive(AssociationPduType::AssociateAc))
        .expect_err("out-of-order transitions must fail");
    match &err.kind() {
        ErrorKind::DecodeError { detail, .. } => {
            assert!(detail.contains("invalid association transition"));
        }
        _ => panic!("expected decode error"),
    }
    assert_eq!(machine.state(), AssociationState::Idle);

    machine
        .on_event(AssociationEvent::Receive(AssociationPduType::Abort))
        .expect("abort closes immediately");
    assert_eq!(machine.state(), AssociationState::Closed);

    let pdu = Pdu::PDataTf(vec![Pdv {
        presentation_context_id: 1,
        message_control_header: 0x03,
        data: vec![1, 2, 3, 4],
    }]);
    let limits = NetworkLimits {
        max_pdv_bytes: 5,
        ..NetworkLimits::default()
    };
    let err = encode_pdu(&pdu, &limits).expect_err("pdv limit must be enforced");
    assert!(matches!(
        err.kind(),
        ErrorKind::LimitExceeded {
            limit_name: "max_pdv_bytes",
            ..
        }
    ));
}
