// Auto-extracted from /home/z/diccy/crates/dicom-workflow-server/src/sr_workflow.rs
// S13-T8: Move inline tests to tests/ directories


use dicom_workflow_server::*;
use dicom_core::Limits;
use pack_sr::{Code, SrAuthoringContentItem};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_file_path(name: &str, ext: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("diccy_{name}_{nonce}.{ext}"))
}

fn auth() -> SrAuthContext {
    SrAuthContext {
        principal: Some("sr-writer".to_string()),
        can_write: true,
    }
}

fn num_item(reference: Option<&str>) -> SrAuthoringContentItem {
    SrAuthoringContentItem::Num {
        concept: Code {
            code_value: "G-D7FE".to_string(),
            scheme: "SRT".to_string(),
            meaning: "Length".to_string(),
        },
        value: 12.5,
        units: Code {
            code_value: "mm".to_string(),
            scheme: "UCUM".to_string(),
            meaning: "millimeter".to_string(),
        },
        referenced_sop_instance_uid: reference.map(|value| value.to_string()),
    }
}

#[test]
fn endpoint_contract_is_deterministic_and_fail_closed() {
    let first = sr_endpoint_contract();
    let second = sr_endpoint_contract();
    assert_eq!(first, second);
    assert!(first
        .iter()
        .filter(|row| row.operation == "create" || row.operation == "update")
        .all(|row| row.requires_write_auth && row.requires_idempotency_key));
}

#[test]
fn create_update_retrieve_roundtrip_with_persistence_and_idempotency() {
    let snapshot = temp_file_path("sr_store", "snapshot");
    let audit = temp_file_path("sr_store", "audit");
    let mut store = SrWorkflowStore::open(
        Limits::default(),
        &snapshot.to_string_lossy(),
        &audit.to_string_lossy(),
    )
    .expect("open");

    let create = SrCreateRequest {
        study_instance_uid: "1.2.3".to_string(),
        series_instance_uid: "1.2.3.4".to_string(),
        sop_instance_uid: "1.2.3.4.5".to_string(),
        observer: "observer-1".to_string(),
        authored_epoch_ms: 42,
        item: num_item(Some("9.8.7")),
        known_referenced_sop_instance_uids: vec!["9.8.7".to_string()],
        idempotency_key: "k-create-1".to_string(),
        request_id: Some("trace-create-1".to_string()),
    };
    let created = store.create(create.clone(), &auth()).expect("create");
    assert_eq!(created.kind, SrWriteOutcomeKind::Created);
    assert_eq!(created.version, 1);

    let replayed = store.create(create, &auth()).expect("replay");
    assert_eq!(replayed.kind, SrWriteOutcomeKind::Duplicate);
    assert!(replayed.idempotency_replay);

    let update = SrUpdateEnvelope {
        sop_instance_uid: "1.2.3.4.5".to_string(),
        expected_version: 1,
        item: SrAuthoringContentItem::Text {
            concept: Code {
                code_value: "121071".to_string(),
                scheme: "DCM".to_string(),
                meaning: "Finding".to_string(),
            },
            text: "stable finding".to_string(),
            referenced_sop_instance_uid: Some("9.8.7".to_string()),
        },
        observer: Some("observer-2".to_string()),
        known_referenced_sop_instance_uids: vec!["9.8.7".to_string()],
        idempotency_key: "k-update-1".to_string(),
        request_id: Some("trace-update-1".to_string()),
    };
    let updated = store.update(update, &auth()).expect("update");
    assert_eq!(updated.kind, SrWriteOutcomeKind::Updated);
    assert_eq!(updated.version, 2);

    let reopened = SrWorkflowStore::open(
        Limits::default(),
        &snapshot.to_string_lossy(),
        &audit.to_string_lossy(),
    )
    .expect("reopen");
    let doc = reopened.get("1.2.3.4.5").expect("document");
    assert_eq!(doc.version, 2);
    assert_eq!(doc.items.len(), 2);
}

#[test]
fn version_conflict_and_auth_fail_closed_are_typed() {
    let snapshot = temp_file_path("sr_store_conflict", "snapshot");
    let audit = temp_file_path("sr_store_conflict", "audit");
    let mut store = SrWorkflowStore::open(
        Limits::default(),
        &snapshot.to_string_lossy(),
        &audit.to_string_lossy(),
    )
    .expect("open");
    store
        .create(
            SrCreateRequest {
                study_instance_uid: "1.2.3".to_string(),
                series_instance_uid: "1.2.3.4".to_string(),
                sop_instance_uid: "1.2.3.4.5".to_string(),
                observer: "observer".to_string(),
                authored_epoch_ms: 1,
                item: num_item(None),
                known_referenced_sop_instance_uids: Vec::new(),
                idempotency_key: "k-create".to_string(),
                request_id: None,
            },
            &auth(),
        )
        .expect("create");

    let err = store
        .update(
            SrUpdateEnvelope {
                sop_instance_uid: "1.2.3.4.5".to_string(),
                expected_version: 99,
                item: num_item(None),
                observer: None,
                known_referenced_sop_instance_uids: Vec::new(),
                idempotency_key: "k-update".to_string(),
                request_id: None,
            },
            &auth(),
        )
        .expect_err("must fail");
    assert_eq!(err.code(), "DVF.INTEGRITY.ERROR");

    let err = store
        .create(
            SrCreateRequest {
                study_instance_uid: "1.2.3".to_string(),
                series_instance_uid: "1.2.3.4".to_string(),
                sop_instance_uid: "1.2.3.4.6".to_string(),
                observer: "observer".to_string(),
                authored_epoch_ms: 1,
                item: num_item(None),
                known_referenced_sop_instance_uids: Vec::new(),
                idempotency_key: "k-create-2".to_string(),
                request_id: Some("trace-create-2".to_string()),
            },
            &SrAuthContext {
                principal: None,
                can_write: false,
            },
        )
        .expect_err("auth fail");
    assert_eq!(err.code(), "DVF.DICOM.DECODE_ERROR");
}

#[test]
fn audit_records_hash_identifiers_only() {
    let snapshot = temp_file_path("sr_store_audit", "snapshot");
    let audit = temp_file_path("sr_store_audit", "audit");
    let mut store = SrWorkflowStore::open(
        Limits::default(),
        &snapshot.to_string_lossy(),
        &audit.to_string_lossy(),
    )
    .expect("open");
    store
        .create(
            SrCreateRequest {
                study_instance_uid: "1.2.3".to_string(),
                series_instance_uid: "1.2.3.4".to_string(),
                sop_instance_uid: "1.2.3.4.5".to_string(),
                observer: "observer".to_string(),
                authored_epoch_ms: 1,
                item: num_item(None),
                known_referenced_sop_instance_uids: Vec::new(),
                idempotency_key: "my-secret-key".to_string(),
                request_id: Some("audit-secret-id".to_string()),
            },
            &auth(),
        )
        .expect("create");
    let content = fs::read_to_string(audit).expect("read audit");
    assert!(!content.contains("1.2.3.4.5"));
    assert!(!content.contains("sr-writer"));
    assert!(!content.contains("my-secret-key"));
    assert!(content.contains(&format!(
        "\"request_id_hash\":\"{}\"",
        hash_text("audit-secret-id")
    )));
    assert!(content.contains("sop_uid_hash"));
}

#[test]
fn lifecycle_transitions_and_history_are_deterministic() {
    let snapshot = temp_file_path("sr_store_lifecycle", "snapshot");
    let audit = temp_file_path("sr_store_lifecycle", "audit");
    let mut store = SrWorkflowStore::open(
        Limits::default(),
        &snapshot.to_string_lossy(),
        &audit.to_string_lossy(),
    )
    .expect("open");

    store
        .create(
            SrCreateRequest {
                study_instance_uid: "1.2.3".to_string(),
                series_instance_uid: "1.2.3.4".to_string(),
                sop_instance_uid: "1.2.3.4.6".to_string(),
                observer: "operator".to_string(),
                authored_epoch_ms: 88,
                item: num_item(None),
                known_referenced_sop_instance_uids: Vec::new(),
                idempotency_key: "k-create-lifecycle".to_string(),
                request_id: Some("trace-create-lifecycle".to_string()),
            },
            &auth(),
        )
        .expect("create");
    let review = store
        .review(
            SrLifecycleTransitionRequest {
                sop_instance_uid: "1.2.3.4.6".to_string(),
                idempotency_key: "k-review".to_string(),
                request_id: Some("trace-review".to_string()),
            },
            &auth(),
        )
        .expect("review");
    assert_eq!(review.status, SrLifecycleStatus::Reviewed);
    assert_eq!(
        store.lifecycle_status("1.2.3.4.6"),
        Some(SrLifecycleStatus::Reviewed)
    );

    let finalize = store
        .finalize(
            SrLifecycleTransitionRequest {
                sop_instance_uid: "1.2.3.4.6".to_string(),
                idempotency_key: "k-finalize".to_string(),
                request_id: Some("trace-finalize".to_string()),
            },
            &auth(),
        )
        .expect("finalize");
    assert_eq!(finalize.status, SrLifecycleStatus::Finalized);
    assert_eq!(
        store.lifecycle_status("1.2.3.4.6"),
        Some(SrLifecycleStatus::Finalized)
    );

    let commit = store
        .commit(
            SrLifecycleTransitionRequest {
                sop_instance_uid: "1.2.3.4.6".to_string(),
                idempotency_key: "k-commit".to_string(),
                request_id: Some("trace-commit".to_string()),
            },
            &auth(),
        )
        .expect("commit");
    assert_eq!(commit.status, SrLifecycleStatus::Committed);

    let mut history = store.lifecycle_history("1.2.3.4.6");
    assert_eq!(history.len(), 4);
    assert_eq!(history[0].action, "create");
    assert_eq!(history[1].action, "review");
    assert_eq!(history[2].action, "finalize");
    assert_eq!(history[3].action, "commit");

    let reopened = SrWorkflowStore::open(
        Limits::default(),
        &snapshot.to_string_lossy(),
        &audit.to_string_lossy(),
    )
    .expect("reopen");
    assert_eq!(
        reopened.lifecycle_status("1.2.3.4.6"),
        Some(SrLifecycleStatus::Committed)
    );
    history = reopened.lifecycle_history("1.2.3.4.6");
    assert_eq!(history[3].status, SrLifecycleStatus::Committed);
}

#[test]
fn invalid_lifecycle_transition_is_rejected_typed() {
    let snapshot = temp_file_path("sr_store_invalid_transition", "snapshot");
    let audit = temp_file_path("sr_store_invalid_transition", "audit");
    let mut store = SrWorkflowStore::open(
        Limits::default(),
        &snapshot.to_string_lossy(),
        &audit.to_string_lossy(),
    )
    .expect("open");

    store
        .create(
            SrCreateRequest {
                study_instance_uid: "9.8.7".to_string(),
                series_instance_uid: "9.8.7.4".to_string(),
                sop_instance_uid: "9.8.7.4.3".to_string(),
                observer: "operator".to_string(),
                authored_epoch_ms: 99,
                item: num_item(None),
                known_referenced_sop_instance_uids: Vec::new(),
                idempotency_key: "invalid-k-create".to_string(),
                request_id: Some("invalid-trace-create".to_string()),
            },
            &auth(),
        )
        .expect("create");

    let err = store
        .commit(
            SrLifecycleTransitionRequest {
                sop_instance_uid: "9.8.7.4.3".to_string(),
                idempotency_key: "invalid-k-commit".to_string(),
                request_id: Some("invalid-trace-commit".to_string()),
            },
            &auth(),
        )
        .expect_err("invalid transition must fail");
    assert_eq!(err.code(), "DVF.INTEGRITY.ERROR");
}
