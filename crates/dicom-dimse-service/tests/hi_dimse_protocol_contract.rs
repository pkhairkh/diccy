use dicom_core::ErrorKind;
use dicom_dimse_service::{
    dimse_association_contract, validate_query_retrieve_status_sequence, DimseCommandAvailability,
    DimseStatus,
};

#[test]
fn association_contract_exposes_ae_context_and_presentation_baseline() {
    // REQ-HI-290, REQ-HI-344, REQ-HI-390
    let contract = dimse_association_contract();
    assert!(!contract.called_ae_required);
    assert_eq!(contract.max_pdu_length, 16_384);
    assert_eq!(
        contract.transfer_syntax_uids,
        vec!["1.2.840.10008.1.2", "1.2.840.10008.1.2.1"]
    );
    assert!(contract.abstract_syntax_uids.contains(&"1.2.840.10008.1.1"));
}

#[test]
fn command_contract_visibility_tracks_feature_gates() {
    // REQ-HI-320, REQ-HI-393, REQ-HI-397
    let contract = dimse_association_contract();
    let c_find = contract
        .command_contracts
        .iter()
        .find(|row| row.command == "C-FIND")
        .expect("C-FIND row");
    let c_move = contract
        .command_contracts
        .iter()
        .find(|row| row.command == "C-MOVE")
        .expect("C-MOVE row");
    let c_get = contract
        .command_contracts
        .iter()
        .find(|row| row.command == "C-GET")
        .expect("C-GET row");

    if cfg!(feature = "dimse-c-find") {
        assert_eq!(c_find.availability, DimseCommandAvailability::Active);
    } else {
        assert_eq!(
            c_find.availability,
            DimseCommandAvailability::DisabledByFeature
        );
    }

    if cfg!(feature = "dimse-c-move") {
        assert_eq!(c_move.availability, DimseCommandAvailability::Active);
    } else {
        assert_eq!(
            c_move.availability,
            DimseCommandAvailability::DisabledByFeature
        );
    }

    if cfg!(feature = "dimse-c-get") {
        assert_eq!(c_get.availability, DimseCommandAvailability::Active);
    } else {
        assert_eq!(
            c_get.availability,
            DimseCommandAvailability::DisabledByFeature
        );
    }
}

#[test]
fn query_retrieve_status_progression_is_pending_then_final() {
    // REQ-HI-294, REQ-HI-397, REQ-HI-398
    validate_query_retrieve_status_sequence(&[DimseStatus::pending(), DimseStatus::success()])
        .expect("valid sequence");

    let err =
        validate_query_retrieve_status_sequence(&[DimseStatus::success(), DimseStatus::pending()])
            .expect_err("invalid sequence must fail");
    assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
}
