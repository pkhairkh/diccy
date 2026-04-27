// Auto-extracted from /home/z/diccy/crates/dicom-hl7/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_hl7::*;
    use dicom_audit::{AuditEvent, AuditValue};
    use std::sync::Arc;

    fn sample_adt_a01() -> String {
        "MSH|^~\\&|SEND_APP|SEND_FAC|RECV_APP|RECV_FAC|20240115120000||ADT^A01|MSG001|P|2.5.1\rPID|||PAT001||Smith^John^M||19800101|M".to_string()
    }

    fn sample_orm_o01() -> String {
        "MSH|^~\\&|SEND_APP|SEND_FAC|RECV_APP|RECV_FAC|20240115120000||ORM^O01|MSG002|P|2.5.1\rPID|||PAT001||Smith^John^M||19800101|M\rORC|NW|ORD001||||||Smith^Dr\rOBR||ORD001||CT_CHEST||20240115100000||||Smith^Dr||||||RAD".to_string()
    }

    #[test]
    fn parse_adt_a01() {
        let parser = Hl7Parser::new();
        let msg = parser.parse_adt(&sample_adt_a01()).expect("parse ADT");

        assert_eq!(msg.msh.sending_application, "SEND_APP");
        assert_eq!(msg.msh.sending_facility, "SEND_FAC");
        assert_eq!(msg.msh.receiving_application, "RECV_APP");
        assert_eq!(msg.msh.message_type, "ADT^A01");
        assert_eq!(msg.msh.message_control_id, "MSG001");
        assert_eq!(msg.msh.processing_id, "P");
        assert_eq!(msg.msh.version_id, "2.5.1");

        assert_eq!(msg.pid.patient_id, "PAT001");
        assert_eq!(msg.pid.patient_name, "Smith^John^M");
        assert_eq!(msg.pid.birth_date.as_deref(), Some("19800101"));
        assert_eq!(msg.pid.sex.as_deref(), Some("M"));

        assert_eq!(msg.event_type, AdtMessageType::A01);
    }

    #[test]
    fn parse_adt_missing_msh_fails() {
        let parser = Hl7Parser::new();
        let msg = "PID|||PAT001||Smith".to_string();
        let result = parser.parse_adt(&msg);
        assert!(matches!(result, Err(Hl7Error::MissingSegment { segment }) if segment == "MSH"));
    }

    #[test]
    fn parse_adt_missing_pid_fails() {
        let parser = Hl7Parser::new();
        let msg = "MSH|^~\\&|APP|FAC|APP|FAC|20240115||ADT^A01|MSG001|P|2.5.1".to_string();
        let result = parser.parse_adt(&msg);
        assert!(matches!(result, Err(Hl7Error::MissingSegment { segment }) if segment == "PID"));
    }

    #[test]
    fn parse_adt_invalid_event_type() {
        let parser = Hl7Parser::new();
        let msg = "MSH|^~\\&|APP|FAC|APP|FAC|20240115||ADT^A99|MSG001|P|2.5.1\rPID|||PAT001||Smith"
            .to_string();
        let result = parser.parse_adt(&msg);
        assert!(matches!(result, Err(Hl7Error::InvalidMessageType { .. })));
    }

    #[test]
    fn parse_adt_a02() {
        let parser = Hl7Parser::new();
        let msg = "MSH|^~\\&|APP|FAC|APP|FAC|20240115||ADT^A02|MSG002|P|2.5.1\rPID|||PAT001||Smith"
            .to_string();
        let result = parser.parse_adt(&msg).expect("parse A02");
        assert_eq!(result.event_type, AdtMessageType::A02);
    }

    #[test]
    fn parse_adt_a03() {
        let parser = Hl7Parser::new();
        let msg = "MSH|^~\\&|APP|FAC|APP|FAC|20240115||ADT^A03|MSG003|P|2.5.1\rPID|||PAT001||Smith"
            .to_string();
        let result = parser.parse_adt(&msg).expect("parse A03");
        assert_eq!(result.event_type, AdtMessageType::A03);
    }

    #[test]
    fn parse_adt_a08() {
        let parser = Hl7Parser::new();
        let msg = "MSH|^~\\&|APP|FAC|APP|FAC|20240115||ADT^A08|MSG008|P|2.5.1\rPID|||PAT001||Smith"
            .to_string();
        let result = parser.parse_adt(&msg).expect("parse A08");
        assert_eq!(result.event_type, AdtMessageType::A08);
    }

    #[test]
    fn parse_orm_o01() {
        let parser = Hl7Parser::new();
        let msg = parser.parse_orm(&sample_orm_o01()).expect("parse ORM");

        assert_eq!(msg.msh.message_type, "ORM^O01");
        assert_eq!(msg.pid.patient_id, "PAT001");
        assert_eq!(msg.orc.order_control, "NW");
        assert_eq!(msg.orc.placer_order_number.as_deref(), Some("ORD001"));
        assert_eq!(msg.obr.service_identifier.as_deref(), Some("CT_CHEST"));
        assert_eq!(msg.obr.diagnostic_service_section.as_deref(), Some("RAD"));
    }

    #[test]
    fn parse_orm_missing_orc_fails() {
        let parser = Hl7Parser::new();
        let msg = "MSH|^~\\&|APP|FAC|APP|FAC|20240115||ORM^O01|MSG001|P|2.5.1\rPID|||PAT001||Smith\rOBR||ORD001||CT_CHEST".to_string();
        let result = parser.parse_orm(&msg);
        assert!(matches!(result, Err(Hl7Error::MissingSegment { segment }) if segment == "ORC"));
    }

    #[test]
    fn parse_orm_missing_obr_fails() {
        let parser = Hl7Parser::new();
        let msg = "MSH|^~\\&|APP|FAC|APP|FAC|20240115||ORM^O01|MSG001|P|2.5.1\rPID|||PAT001||Smith\rORC|NW|ORD001".to_string();
        let result = parser.parse_orm(&msg);
        assert!(matches!(result, Err(Hl7Error::MissingSegment { segment }) if segment == "OBR"));
    }

    #[test]
    fn encode_oru_message() {
        let encoder = Hl7Encoder::new();
        let oru = OruBuilder::new(
            MshSegment {
                field_separator: '|',
                encoding_characters: "^~\\&".to_string(),
                sending_application: "PACS".to_string(),
                sending_facility: "HOSPITAL".to_string(),
                receiving_application: "RIS".to_string(),
                receiving_facility: "HOSPITAL".to_string(),
                datetime: "20240115120000".to_string(),
                message_type: "ORU^R01".to_string(),
                message_control_id: "ORU001".to_string(),
                processing_id: "P".to_string(),
                version_id: "2.5.1".to_string(),
            },
            PidSegment {
                patient_id: "PAT001".to_string(),
                patient_name: "Smith^John".to_string(),
                birth_date: Some("19800101".to_string()),
                sex: Some("M".to_string()),
                address: None,
                phone_home: None,
            },
            OrcSegment {
                order_control: "NW".to_string(),
                placer_order_number: Some("ORD001".to_string()),
                filler_order_number: None,
                ordering_provider: None,
            },
            ObrSegment {
                placer_order_number: Some("ORD001".to_string()),
                filler_order_number: None,
                service_identifier: Some("CT_CHEST".to_string()),
                requested_datetime: None,
                observation_datetime: Some("20240115100000".to_string()),
                ordering_provider: None,
                diagnostic_service_section: Some("RAD".to_string()),
            },
        )
        .add_obx(ObxSegment {
            set_id: "1".to_string(),
            value_type: "NM".to_string(),
            observation_identifier: "FINDING_001".to_string(),
            observation_value: Some("25.3".to_string()),
            units: Some("mm".to_string()),
            reference_range: None,
            abnormal_flags: None,
            result_status: Some("F".to_string()),
        })
        .build();

        let encoded = encoder.encode_oru(&oru).expect("encode");
        assert!(encoded.contains("MSH|"));
        assert!(encoded.contains("PID|"));
        assert!(encoded.contains("ORC|"));
        assert!(encoded.contains("OBR|"));
        assert!(encoded.contains("OBX|"));
        assert!(encoded.contains("25.3"));
    }

    #[test]
    fn mllp_frame_unframe_roundtrip() {
        let message = "MSH|^~\\&|APP|FAC|APP|FAC|20240115||ADT^A01|MSG001|P|2.5.1\rPID|||PAT001";
        let framed = MllpFramer::frame(message);
        assert_eq!(framed[0], MllpFramer::SB);
        assert_eq!(*framed.last().unwrap(), MllpFramer::CR);

        let unframed = MllpFramer::unframe(&framed).expect("unframe");
        assert_eq!(unframed, message);
    }

    #[test]
    fn mllp_unframe_rejects_too_short() {
        let data = vec![0x0B, 0x1C];
        let result = MllpFramer::unframe(&data);
        assert!(matches!(result, Err(Hl7Error::ParseFailed { .. })));
    }

    #[test]
    fn mllp_unframe_rejects_missing_start() {
        let data = vec![0x00, b'M', b'S', b'H', 0x1C, 0x0D];
        let result = MllpFramer::unframe(&data);
        assert!(matches!(result, Err(Hl7Error::ParseFailed { .. })));
    }

    #[test]
    fn mllp_unframe_rejects_missing_end() {
        let data = vec![0x0B, b'M', b'S', b'H', 0x0D];
        let result = MllpFramer::unframe(&data);
        assert!(matches!(result, Err(Hl7Error::ParseFailed { .. })));
    }

    #[test]
    fn ack_builder_produces_valid_ack() {
        let ack = AckBuilder::build_ack("RECV_APP", "RECV_FAC", "MSG001", "AA", "Message accepted");
        assert!(ack.starts_with("MSH|"));
        assert!(ack.contains("ACK"));
        assert!(ack.contains("MSA|AA|MSG001"));
    }

    #[test]
    fn adt_with_pv1_segment() {
        let parser = Hl7Parser::new();
        let msg = "MSH|^~\\&|APP|FAC|APP|FAC|20240115||ADT^A01|MSG001|P|2.5.1\rPID|||PAT001||Smith^John||19800101|M\rPV1|I|ICU^101^A||||Smith^Dr".to_string();
        let result = parser.parse_adt(&msg).expect("parse with PV1");
        assert!(result.pv1.is_some());
        let pv1 = result.pv1.unwrap();
        assert_eq!(pv1.patient_class.as_deref(), Some("I"));
        assert_eq!(pv1.assigned_location.as_deref(), Some("ICU^101^A"));
    }

    #[test]
    fn audit_callback_emits_on_parse() {
        use std::sync::Mutex;

        let events = Arc::new(Mutex::new(Vec::<AuditEvent>::new()));
        let events_handle = Arc::clone(&events);
        let audit: AuditCallback = Arc::new(move |event| {
            events_handle.lock().expect("lock").push(event);
            Ok(())
        });

        let parser = Hl7Parser::with_audit(Some(audit));
        parser.parse_adt(&sample_adt_a01()).expect("parse");

        let events = events.lock().expect("lock");
        assert_eq!(events.len(), 1);
        assert!(events[0].fields.iter().any(|f| f.key == "operation"
            && matches!(&f.value, AuditValue::Plain(v) if v == "parse_adt")));
    }

    #[test]
    fn audit_callback_emits_on_encode() {
        use std::sync::Mutex;

        let events = Arc::new(Mutex::new(Vec::<AuditEvent>::new()));
        let events_handle = Arc::clone(&events);
        let audit: AuditCallback = Arc::new(move |event| {
            events_handle.lock().expect("lock").push(event);
            Ok(())
        });

        let encoder = Hl7Encoder::with_audit(Some(audit));
        let oru = OruBuilder::new(
            MshSegment {
                field_separator: '|',
                encoding_characters: "^~\\&".to_string(),
                sending_application: "PACS".to_string(),
                sending_facility: "HOSP".to_string(),
                receiving_application: "RIS".to_string(),
                receiving_facility: "HOSP".to_string(),
                datetime: "20240115120000".to_string(),
                message_type: "ORU^R01".to_string(),
                message_control_id: "ORU001".to_string(),
                processing_id: "P".to_string(),
                version_id: "2.5.1".to_string(),
            },
            PidSegment {
                patient_id: "PAT001".to_string(),
                patient_name: "Smith".to_string(),
                birth_date: None,
                sex: None,
                address: None,
                phone_home: None,
            },
            OrcSegment {
                order_control: "NW".to_string(),
                placer_order_number: None,
                filler_order_number: None,
                ordering_provider: None,
            },
            ObrSegment {
                placer_order_number: None,
                filler_order_number: None,
                service_identifier: None,
                requested_datetime: None,
                observation_datetime: None,
                ordering_provider: None,
                diagnostic_service_section: None,
            },
        )
        .build();

        encoder.encode_oru(&oru).expect("encode");

        let events = events.lock().expect("lock");
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn parse_segments_splits_correctly() {
        let parser = Hl7Parser::new();
        let msg = "MSH|^~\\&|APP\rPID|||PAT001";
        let segments = parser.parse_segments(msg);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].segment_id(), "MSH");
        assert_eq!(segments[1].segment_id(), "PID");
    }

    #[test]
    fn field_access_works() {
        let parser = Hl7Parser::new();
        let segments = parser.parse_segments("MSH|^~\\&|APP|FAC");
        let msh = &segments[0];
        assert_eq!(msh.segment_id(), "MSH");
        assert_eq!(msh.field_str(2), Some("APP"));
        assert_eq!(msh.field_str(3), Some("FAC"));
    }
