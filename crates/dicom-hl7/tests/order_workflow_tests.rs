//! Integration tests for Sprint 15 HL7 order workflow.
//!
//! Tests cover:
//! - ORM → MWL entry creation
//! - ORU generation from SR measurements
//! - ADT patient reconciliation
//! - Full round-trip: ORM → MWL → Study → SR → ORU
//! - MLLP framing/unframing

use dicom_hl7::*;
use dicom_hl7::order_workflow::{MwlEntry, MwlEntryStatus, OrderWorkflowEngine};
use dicom_hl7::result_delivery::{MeasurementData, OruPatientInfo, ResultDeliveryEngine};
use dicom_hl7::patient_recon::{AdtReconciliationResult, PatientReconciliationEngine};
use dicom_hl7::mllp_server::{Hl7MessageRouter, MllpServer};
use std::sync::Arc;

// --- Helper constructors ---

fn make_msh(control_id: &str, msg_type: &str) -> MshSegment {
    MshSegment {
        field_separator: '|',
        encoding_characters: "^~\\&".to_string(),
        sending_application: "RIS".to_string(),
        sending_facility: "HOSPITAL".to_string(),
        receiving_application: "PACS".to_string(),
        receiving_facility: "HOSPITAL".to_string(),
        datetime: "20240115120000".to_string(),
        message_type: msg_type.to_string(),
        message_control_id: control_id.to_string(),
        processing_id: "P".to_string(),
        version_id: "2.5.1".to_string(),
    }
}

fn make_pid(patient_id: &str, name: &str) -> PidSegment {
    PidSegment {
        patient_id: patient_id.to_string(),
        patient_name: name.to_string(),
        birth_date: Some("19800101".to_string()),
        sex: Some("M".to_string()),
        address: None,
        phone_home: None,
    }
}

fn sample_orm_message() -> OrmMessage {
    OrmMessage {
        msh: make_msh("ORM001", "ORM^O01"),
        pid: make_pid("PAT001", "Smith^John^M"),
        orc: OrcSegment {
            order_control: "NW".to_string(),
            placer_order_number: Some("ORD001".to_string()),
            filler_order_number: Some("ACC12345".to_string()),
            ordering_provider: Some("Dr^Smith".to_string()),
        },
        obr: ObrSegment {
            placer_order_number: Some("ORD001".to_string()),
            filler_order_number: Some("ACC12345".to_string()),
            service_identifier: Some("CT_CHEST".to_string()),
            requested_datetime: Some("20240115100000".to_string()),
            observation_datetime: None,
            ordering_provider: Some("Dr^Smith".to_string()),
            diagnostic_service_section: Some("RAD".to_string()),
        },
    }
}

fn sample_adt_a01() -> AdtMessage {
    AdtMessage {
        msh: make_msh("ADT001", "ADT^A01"),
        pid: make_pid("PAT001", "Smith^John^M"),
        pv1: None,
        event_type: AdtMessageType::A01,
    }
}

fn sample_adt_a08() -> AdtMessage {
    AdtMessage {
        msh: make_msh("ADT008", "ADT^A08"),
        pid: PidSegment {
            patient_id: "PAT001".to_string(),
            patient_name: "Smith^Jonathan^M".to_string(),
            birth_date: Some("19800615".to_string()),
            sex: Some("M".to_string()),
            address: Some("456 Oak Ave".to_string()),
            phone_home: None,
        },
        pv1: None,
        event_type: AdtMessageType::A08,
    }
}

fn sample_measurements() -> Vec<MeasurementData> {
    vec![
        MeasurementData {
            measurement_type: "length".to_string(),
            value: "25.3".to_string(),
            unit: "mm".to_string(),
            reference_range: Some("0-50".to_string()),
            observation_time: "20240115103000".to_string(),
        },
        MeasurementData {
            measurement_type: "area".to_string(),
            value: "312.7".to_string(),
            unit: "mm2".to_string(),
            reference_range: None,
            observation_time: "20240115103000".to_string(),
        },
    ]
}

// --- Test: ORM → MWL entry creation ---

#[test]
fn orm_to_mwl_entry_creation() {
    let engine = OrderWorkflowEngine::new();
    let orm = sample_orm_message();
    let mwl = engine.process_orm_order(&orm).expect("process ORM");

    assert_eq!(mwl.accession_number, "ACC12345");
    assert_eq!(mwl.patient_id, "PAT001");
    assert_eq!(mwl.patient_name, "Smith^John^M");
    assert_eq!(mwl.requested_procedure_description, "CT_CHEST");
    assert_eq!(mwl.modality, "CT");
    assert_eq!(mwl.status, MwlEntryStatus::Scheduled);
    assert!(mwl.study_uid.is_none());
    assert_eq!(mwl.ordering_physician.as_deref(), Some("Dr^Smith"));
    assert_eq!(
        mwl.scheduled_date_time.as_deref(),
        Some("20240115100000")
    );
}

#[test]
fn orm_to_mwl_fallback_to_placer_order() {
    let engine = OrderWorkflowEngine::new();
    let mut orm = sample_orm_message();
    orm.orc.filler_order_number = None;
    orm.obr.filler_order_number = None;

    let mwl = engine.process_orm_order(&orm).expect("process ORM");
    assert_eq!(mwl.accession_number, "ORD001");
}

// --- Test: ORU generation from SR measurements ---

#[test]
fn oru_generation_from_sr_measurements() {
    let engine = ResultDeliveryEngine::new();
    let patient = OruPatientInfo {
        patient_id: "PAT001".to_string(),
        patient_name: "Smith^John".to_string(),
        accession_number: "ACC12345".to_string(),
    };
    let measurements = sample_measurements();

    let oru = engine
        .generate_oru_from_sr(&patient, &measurements)
        .expect("generate ORU");

    assert_eq!(oru.msh.message_type, "ORU^R01");
    assert_eq!(oru.pid.patient_id, "PAT001");
    assert_eq!(oru.obx.len(), 2);
    assert_eq!(oru.obx[0].set_id, "1");
    assert_eq!(oru.obx[0].observation_identifier, "SR_LENGTH");
    assert_eq!(oru.obx[0].observation_value.as_deref(), Some("25.3"));
    assert_eq!(oru.obx[0].value_type, "NM");
    assert_eq!(oru.obx[0].units.as_deref(), Some("mm"));
    assert_eq!(oru.obx[0].reference_range.as_deref(), Some("0-50"));
    assert_eq!(oru.obx[1].observation_identifier, "SR_AREA");
    assert_eq!(oru.obx[1].observation_value.as_deref(), Some("312.7"));
}

#[test]
fn oru_encode_to_wire_format() {
    let engine = ResultDeliveryEngine::new();
    let patient = OruPatientInfo {
        patient_id: "PAT001".to_string(),
        patient_name: "Smith^John".to_string(),
        accession_number: "ACC12345".to_string(),
    };
    let measurements = sample_measurements();

    let encoded = engine
        .encode_and_deliver(&patient, &measurements)
        .expect("encode");

    assert!(encoded.contains("MSH|"));
    assert!(encoded.contains("OBX|1|NM|SR_LENGTH|25.3|mm|0-50"));
    assert!(encoded.contains("OBX|2|NM|SR_AREA|312.7|mm2"));
}

// --- Test: ADT patient reconciliation ---

#[test]
fn adt_a01_admit_creates_patient_record() {
    let engine = PatientReconciliationEngine::new();
    let adt = sample_adt_a01();

    let record = engine.process_admit(&adt).expect("process admit");
    assert_eq!(record.patient_id, "PAT001");
    assert_eq!(record.name, "Smith^John^M");
    assert_eq!(record.birth_date.as_deref(), Some("19800101"));
    assert_eq!(record.sex.as_deref(), Some("M"));
}

#[test]
fn adt_a08_update_detects_changes() {
    let engine = PatientReconciliationEngine::new();
    let adt = sample_adt_a08();

    let update = engine.process_update(&adt).expect("process update");
    assert_eq!(update.patient_id, "PAT001");
    assert!(update.updated_fields.contains_key("name"));
    assert!(update.updated_fields.contains_key("birth_date"));
    assert!(update.updated_fields.contains_key("address"));
}

#[test]
fn adt_a40_merge_patients() {
    let engine = PatientReconciliationEngine::new();
    let mut adt = sample_adt_a01();
    adt.pid.patient_id = "PAT002".to_string();

    let merge = engine.process_merge(&adt, "PAT001").expect("merge");
    assert_eq!(merge.old_id, "PAT001");
    assert_eq!(merge.new_id, "PAT002");
}

#[test]
fn adt_reconciliation_routing() {
    let engine = PatientReconciliationEngine::new();

    // A01 → Admit
    let a01 = sample_adt_a01();
    let result = engine.process_adt(&a01).expect("route A01");
    assert!(matches!(result, AdtReconciliationResult::Admit(_)));

    // A08 → Update
    let a08 = sample_adt_a08();
    let result = engine.process_adt(&a08).expect("route A08");
    assert!(matches!(result, AdtReconciliationResult::Update(_)));

    // A02 → NoChange
    let mut a02 = sample_adt_a01();
    a02.event_type = AdtMessageType::A02;
    a02.msh.message_type = "ADT^A02".to_string();
    let result = engine.process_adt(&a02).expect("route A02");
    assert!(matches!(result, AdtReconciliationResult::NoChange { .. }));
}

// --- Test: Full round-trip ORM → MWL → Study → SR → ORU ---

#[test]
fn full_round_trip_orm_to_oru() {
    // Step 1: Receive ORM order → Create MWL entry
    let order_engine = OrderWorkflowEngine::new();
    let orm = sample_orm_message();
    let mut mwl = order_engine.process_orm_order(&orm).expect("step 1: ORM → MWL");
    assert_eq!(mwl.status, MwlEntryStatus::Scheduled);
    assert!(mwl.study_uid.is_none());

    // Step 2: Study received → Link to MWL entry
    let study_uid = "1.2.840.113619.2.55.3.12345";
    order_engine
        .link_study_to_order(&mut mwl, study_uid)
        .expect("step 2: link study");
    assert_eq!(mwl.status, MwlEntryStatus::InProgress);
    assert_eq!(mwl.study_uid.as_deref(), Some(study_uid));

    // Step 3: Complete MWL entry (all instances stored)
    order_engine
        .complete_mwl_entry(&mut mwl)
        .expect("step 3: complete MWL");
    assert_eq!(mwl.status, MwlEntryStatus::Completed);

    // Step 4: SR measurements generated → Create ORU
    let result_engine = ResultDeliveryEngine::new();
    let patient = OruPatientInfo {
        patient_id: mwl.patient_id.clone(),
        patient_name: mwl.patient_name.clone(),
        accession_number: mwl.accession_number.clone(),
    };
    let measurements = sample_measurements();

    let oru = result_engine
        .generate_oru_from_sr(&patient, &measurements)
        .expect("step 4: SR → ORU");

    assert_eq!(oru.msh.message_type, "ORU^R01");
    assert_eq!(oru.pid.patient_id, "PAT001");
    assert_eq!(
        oru.orc.placer_order_number.as_deref(),
        Some("ACC12345")
    );
    assert_eq!(oru.obx.len(), 2);

    // Step 5: Encode ORU to wire format
    let encoder = Hl7Encoder::new();
    let encoded = encoder.encode_oru(&oru).expect("step 5: encode ORU");
    assert!(encoded.contains("MSH|"));
    assert!(encoded.contains("OBX|"));

    // Step 6: Frame for MLLP transport
    let framed = MllpFramer::frame(&encoded);
    assert_eq!(framed[0], MllpFramer::SB);
    assert_eq!(*framed.last().unwrap(), MllpFramer::CR);

    // Step 7: Unframe at receiving end
    let unframed = MllpFramer::unframe(&framed).expect("step 7: unframe");
    assert_eq!(unframed, encoded);
}

// --- Test: MLLP framing/unframing ---

#[test]
fn mllp_frame_unframe_roundtrip_various_messages() {
    let messages = vec![
        "MSH|^~\\&|APP|FAC|APP|FAC|20240115||ADT^A01|MSG001|P|2.5.1\rPID|||PAT001",
        "MSH|^~\\&|RIS|HOSP|PACS|HOSP|20240115||ORM^O01|MSG002|P|2.5.1\rPID|||PAT001\rORC|NW|ORD001",
        "MSH|^~\\&|PACS|HOSP|RIS|HOSP|20240115||ORU^R01|MSG003|P|2.5.1\rPID|||PAT001\rOBX|1|NM|SR_LENGTH|25.3|mm",
    ];

    for msg in messages {
        let framed = MllpFramer::frame(msg);
        assert_eq!(framed[0], MllpFramer::SB);

        let unframed = MllpFramer::unframe(&framed).expect("unframe");
        assert_eq!(unframed, msg);
    }
}

// --- Test: MLLP server message processing ---

#[test]
fn mllp_server_processes_orm_via_router() {
    let router = Arc::new(Hl7MessageRouter::new());
    let server = MllpServer::new("127.0.0.1:2575", router);

    let orm_wire = "MSH|^~\\&|RIS|HOSPITAL|PACS|HOSPITAL|20240115120000||ORM^O01|MSG001|P|2.5.1\rPID|||PAT001||Smith^John||19800101|M\rORC|NW|ORD001|ACC12345||||||Dr^Smith\rOBR||ORD001|ACC12345|CT_CHEST||20240115100000||||Dr^Smith||||||RAD";
    let framed = MllpFramer::frame(orm_wire);

    let response_frame = server.process_mllp_frame(&framed).expect("process");
    let response = MllpFramer::unframe(&response_frame).expect("unframe");

    assert!(response.contains("AA"));
    assert!(response.contains("MWL entry created"));
    assert!(response.contains("ACC12345"));
}

#[test]
fn mllp_server_processes_adt_via_router() {
    let router = Arc::new(Hl7MessageRouter::new());
    let server = MllpServer::new("127.0.0.1:2575", router);

    let adt_wire = "MSH|^~\\&|HIS|HOSPITAL|PACS|HOSPITAL|20240115120000||ADT^A01|MSG002|P|2.5.1\rPID|||PAT001||Smith^John||19800101|M";
    let framed = MllpFramer::frame(adt_wire);

    let response_frame = server.process_mllp_frame(&framed).expect("process");
    let response = MllpFramer::unframe(&response_frame).expect("unframe");

    assert!(response.contains("AA"));
    assert!(response.contains("Patient admitted"));
}

#[test]
fn mllp_server_rejects_unknown_message_type() {
    let router = Arc::new(Hl7MessageRouter::new());
    let server = MllpServer::new("127.0.0.1:2575", router);

    let unknown = "MSH|^~\\&|APP|FAC|APP|FAC|20240115||SIU^S12|MSG003|P|2.5.1\rPID|||PAT001";
    let framed = MllpFramer::frame(unknown);

    let response_frame = server.process_mllp_frame(&framed).expect("process");
    let response = MllpFramer::unframe(&response_frame).expect("unframe");

    assert!(response.contains("AR"));
}

// --- Test: MWL entry status lifecycle ---

#[test]
fn mwl_entry_status_lifecycle() {
    let engine = OrderWorkflowEngine::new();
    let orm = sample_orm_message();
    let mut mwl = engine.process_orm_order(&orm).expect("ORM → MWL");

    // Scheduled → InProgress
    assert_eq!(mwl.status, MwlEntryStatus::Scheduled);
    engine
        .link_study_to_order(&mut mwl, "1.2.3.4.5")
        .expect("link");
    assert_eq!(mwl.status, MwlEntryStatus::InProgress);

    // InProgress → Completed
    engine.complete_mwl_entry(&mut mwl).expect("complete");
    assert_eq!(mwl.status, MwlEntryStatus::Completed);
}

#[test]
fn mwl_entry_cancellation() {
    let engine = OrderWorkflowEngine::new();
    let orm = sample_orm_message();
    let mut mwl = engine.process_orm_order(&orm).expect("ORM → MWL");

    engine.cancel_mwl_entry(&mut mwl).expect("cancel");
    assert_eq!(mwl.status, MwlEntryStatus::Cancelled);
}

// --- Test: Audit callback integration ---

#[test]
fn audit_callback_with_order_workflow() {
    use dicom_audit::AuditEvent;
    use std::sync::Mutex;

    let events = Arc::new(Mutex::new(Vec::<AuditEvent>::new()));
    let events_handle = Arc::clone(&events);
    let audit: AuditCallback = Arc::new(move |event| {
        events_handle.lock().expect("lock").push(event);
        Ok(())
    });

    let engine = OrderWorkflowEngine::with_audit(Some(audit));
    let orm = sample_orm_message();
    engine.process_orm_order(&orm).expect("process");

    let events = events.lock().expect("lock");
    assert!(!events.is_empty());
    assert!(events[0].fields.iter().any(|f| f.key == "operation"));
}

// --- Test: Serialization roundtrips ---

#[test]
fn mwl_entry_json_roundtrip() {
    let engine = OrderWorkflowEngine::new();
    let orm = sample_orm_message();
    let mwl = engine.process_orm_order(&orm).expect("process");

    let json = serde_json::to_string_pretty(&mwl).expect("serialize");
    let restored: MwlEntry = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(restored.accession_number, mwl.accession_number);
    assert_eq!(restored.patient_id, mwl.patient_id);
    assert_eq!(restored.status, mwl.status);
}

#[test]
fn measurement_data_json_roundtrip() {
    let m = MeasurementData {
        measurement_type: "length".to_string(),
        value: "25.3".to_string(),
        unit: "mm".to_string(),
        reference_range: Some("0-50".to_string()),
        observation_time: "20240115103000".to_string(),
    };

    let json = serde_json::to_string(&m).expect("serialize");
    let restored: MeasurementData = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.measurement_type, m.measurement_type);
    assert_eq!(restored.value, m.value);
}
