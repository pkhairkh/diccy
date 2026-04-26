#![deny(missing_docs)]

//! DIMSE SCU/SCP harness built on `dicom-net` and `dicom-dimse`.

mod commitment;
mod ian;
mod middleware;
mod protocol;
mod transport;

pub use commitment::*;
pub use ian::*;
pub use middleware::*;
pub use protocol::*;
pub use transport::*;

use dicom_core::{Error, ErrorKind};
use dicom_net::AssociationPolicy;

// ---------------------------------------------------------------------------
// Shared SOP class and transfer syntax constants
// ---------------------------------------------------------------------------

pub(crate) const SOP_CLASS_VERIFICATION: &str = "1.2.840.10008.1.1";
pub(crate) const SOP_CLASS_CT_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.2";
pub(crate) const SOP_CLASS_MR_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.4";
pub(crate) const SOP_CLASS_SECONDARY_CAPTURE: &str = "1.2.840.10008.5.1.4.1.1.7";
pub(crate) const SOP_CLASS_MULTI_FRAME_SC_BYTE: &str = "1.2.840.10008.5.1.4.1.1.7.2";
pub(crate) const SOP_CLASS_MULTI_FRAME_SC_WORD: &str = "1.2.840.10008.5.1.4.1.1.7.3";
pub(crate) const SOP_CLASS_MULTI_FRAME_SC_TRUE_COLOR: &str = "1.2.840.10008.5.1.4.1.1.7.4";
pub(crate) const SOP_CLASS_PET_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.128";
pub(crate) const SOP_CLASS_CR_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.1";
pub(crate) const SOP_CLASS_DX_PRESENTATION: &str = "1.2.840.10008.5.1.4.1.1.1.1";
pub(crate) const SOP_CLASS_STUDY_ROOT_FIND: &str = "1.2.840.10008.5.1.4.1.2.2.1";
pub(crate) const SOP_CLASS_STUDY_ROOT_MOVE: &str = "1.2.840.10008.5.1.4.1.2.2.2";
pub(crate) const SOP_CLASS_STUDY_ROOT_GET: &str = "1.2.840.10008.5.1.4.1.2.2.3";
pub(crate) const TRANSFER_SYNTAX_IMPLICIT_VR_LE: &str = "1.2.840.10008.1.2";
pub(crate) const TRANSFER_SYNTAX_EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1";

// ---------------------------------------------------------------------------
// Shared helper functions
// ---------------------------------------------------------------------------

pub(crate) fn io_error(err: std::io::Error) -> Box<Error> {
    Error::from_kind(
        ErrorKind::IoError {
            detail: err.to_string(),
        },
        "io error",
    )
    .into()
}

pub(crate) fn decode_error(detail: impl Into<String>) -> Box<Error> {
    dicom_util::decode_error("dicom-dimse-service", &detail.into())
}

pub(crate) fn limit_exceeded(limit_name: &'static str, observed: u64, allowed: u64) -> Box<Error> {
    dicom_util::limit_exceeded(limit_name, observed, allowed)
}

// ---------------------------------------------------------------------------
// Shared policy builder
// ---------------------------------------------------------------------------

pub(crate) fn workstation_default_association_policy() -> AssociationPolicy {
    #[allow(unused_mut)]
    let mut supported_abstract_syntaxes = vec![
        SOP_CLASS_VERIFICATION,
        SOP_CLASS_CT_IMAGE_STORAGE,
        SOP_CLASS_MR_IMAGE_STORAGE,
        SOP_CLASS_SECONDARY_CAPTURE,
        SOP_CLASS_MULTI_FRAME_SC_BYTE,
        SOP_CLASS_MULTI_FRAME_SC_WORD,
        SOP_CLASS_MULTI_FRAME_SC_TRUE_COLOR,
        SOP_CLASS_PET_IMAGE_STORAGE,
        SOP_CLASS_CR_IMAGE_STORAGE,
        SOP_CLASS_DX_PRESENTATION,
    ];
    #[cfg(feature = "dimse-c-find")]
    supported_abstract_syntaxes.push(SOP_CLASS_STUDY_ROOT_FIND);
    #[cfg(feature = "dimse-c-move")]
    supported_abstract_syntaxes.push(SOP_CLASS_STUDY_ROOT_MOVE);
    #[cfg(feature = "dimse-c-get")]
    supported_abstract_syntaxes.push(SOP_CLASS_STUDY_ROOT_GET);

    AssociationPolicy {
        called_ae: None,
        supported_abstract_syntaxes: supported_abstract_syntaxes
            .into_iter()
            .map(str::to_string)
            .collect(),
        supported_transfer_syntaxes: vec![
            TRANSFER_SYNTAX_IMPLICIT_VR_LE.to_string(),
            TRANSFER_SYNTAX_EXPLICIT_VR_LE.to_string(),
        ],
        max_pdu_length: 16_384,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use commitment::{
        TAG_REFERENCED_SOP_CLASS_UID, TAG_REFERENCED_SOP_INSTANCE_UID,
        TAG_REFERENCED_SOP_SEQUENCE, TAG_TRANSACTION_UID,
        parse_storage_commitment_n_action_request,
    };
    use dicom_audit::{AuditConfig, AuditEventKind, AuditLog, AuditRedactor};
    #[cfg(feature = "dimse-c-find")]
    use dicom_core::{Dataset, Element, Value, Vr};
    use dicom_core::{ErrorKind, Result, Tag};
    use dicom_storage::StorageCommitmentReferencedInstance;
    use middleware::{OperationLimiter, enforce_connection_limit, enforce_query_response_limit};
    use protocol::dimse_protocol_violation_status;
    use protocol::{AssociationInfo, DimseStatus, StorageBackedDimseService};
    use transport::{
        DimseAuthConfig, DimseClient, DimseClientConfig, DimseServer, DimseServerConfig,
        TlsPolicy, TransportSecurity, enforce_tls_policy, handle_pdus,
    };
    use std::collections::HashMap;
    use std::net::Ipv4Addr;
    use std::sync::{Arc, Mutex};
    use std::thread;

    struct EchoService;

    impl DimseService for EchoService {
        fn on_c_echo(&mut self, _request: CEchoRequest) -> Result<DimseStatus> {
            Ok(DimseStatus::success())
        }

        fn on_c_store(&mut self, _request: CStoreRequest) -> Result<DimseStatus> {
            Ok(DimseStatus::processing_failure())
        }
    }

    #[cfg(feature = "dimse-c-find")]
    struct FindService {
        seen_len: Arc<Mutex<Option<usize>>>,
    }

    #[cfg(feature = "dimse-c-find")]
    impl DimseService for FindService {
        fn on_c_echo(&mut self, _request: CEchoRequest) -> Result<DimseStatus> {
            Ok(DimseStatus::success())
        }

        fn on_c_store(&mut self, _request: CStoreRequest) -> Result<DimseStatus> {
            Ok(DimseStatus::processing_failure())
        }

        fn on_c_find(&mut self, request: CFindRequest) -> Result<Vec<DimseStatus>> {
            let mut seen = self.seen_len.lock().expect("lock");
            *seen = Some(request.data_set.len());
            Ok(vec![DimseStatus::pending(), DimseStatus::success()])
        }
    }

    #[cfg(feature = "dimse-c-move")]
    struct MoveService {
        seen_destination: Arc<Mutex<Option<String>>>,
    }

    #[cfg(feature = "dimse-c-move")]
    impl DimseService for MoveService {
        fn on_c_echo(&mut self, _request: CEchoRequest) -> Result<DimseStatus> {
            Ok(DimseStatus::success())
        }

        fn on_c_store(&mut self, _request: CStoreRequest) -> Result<DimseStatus> {
            Ok(DimseStatus::processing_failure())
        }

        fn on_c_move(&mut self, request: CMoveRequest) -> Result<Vec<DimseStatus>> {
            let mut seen = self.seen_destination.lock().expect("lock");
            *seen = Some(request.move_destination);
            Ok(vec![DimseStatus::pending(), DimseStatus::success()])
        }
    }

    #[cfg(feature = "dimse-c-get")]
    struct GetService {
        seen_len: Arc<Mutex<Option<usize>>>,
    }

    #[cfg(feature = "dimse-c-get")]
    impl DimseService for GetService {
        fn on_c_echo(&mut self, _request: CEchoRequest) -> Result<DimseStatus> {
            Ok(DimseStatus::success())
        }

        fn on_c_store(&mut self, _request: CStoreRequest) -> Result<DimseStatus> {
            Ok(DimseStatus::processing_failure())
        }

        fn on_c_get(&mut self, request: CGetRequest) -> Result<Vec<DimseStatus>> {
            let mut seen = self.seen_len.lock().expect("lock");
            *seen = Some(request.data_set.len());
            Ok(vec![DimseStatus::pending(), DimseStatus::success()])
        }
    }

    fn stream_pair() -> (std::net::TcpStream, std::net::TcpStream) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let client = std::net::TcpStream::connect(addr).expect("connect");
        let (server, _) = listener.accept().expect("accept");
        (server, client)
    }

    fn dummy_association_accept() -> dicom_net::AssociationAccept {
        dicom_net::AssociationAccept {
            called_ae: "CALLED_AE".to_string(),
            calling_ae: "CALLING_AE".to_string(),
            application_context: "1.2.840.10008.3.1.1.1".to_string(),
            presentation_contexts: Vec::new(),
            max_pdu_length: 16_384,
            implementation_class_uid: None,
            implementation_version_name: None,
        }
    }

    fn server_config_allow_all() -> DimseServerConfig {
        DimseServerConfig {
            auth: DimseAuthConfig::allow_all(),
            tls_policy: TlsPolicy::AllowInsecure,
            transport_security: TransportSecurity::Insecure,
            ..DimseServerConfig::default()
        }
    }

    #[cfg(feature = "dimse-c-find")]
    fn encode_explicit_element(tag: Tag, vr: &[u8; 2], value: &str) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&tag.0.to_le_bytes());
        out.extend_from_slice(&tag.1.to_le_bytes());
        out.extend_from_slice(vr);
        let mut bytes = value.as_bytes().to_vec();
        if bytes.len() % 2 == 1 {
            bytes.push(if vr == b"UI" { 0 } else { b' ' });
        }
        out.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
        out.extend_from_slice(&bytes);
        out
    }

    #[cfg(feature = "dimse-c-find")]
    fn identifier_dataset_bytes(study: &str, series: &str, sop: &str) -> Vec<u8> {
        const TAG_STUDY_UID: Tag = Tag(0x0020, 0x000D);
        const TAG_SERIES_UID: Tag = Tag(0x0020, 0x000E);
        const TAG_SOP_UID: Tag = Tag(0x0008, 0x0018);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&encode_explicit_element(TAG_STUDY_UID, b"UI", study));
        bytes.extend_from_slice(&encode_explicit_element(TAG_SERIES_UID, b"UI", series));
        bytes.extend_from_slice(&encode_explicit_element(TAG_SOP_UID, b"UI", sop));
        bytes
    }

    #[cfg(feature = "dimse-c-find")]
    fn identifier_dataset_with_clinical_filters(
        study: &str,
        accession_number: &str,
        study_date: &str,
    ) -> Vec<u8> {
        const TAG_STUDY_UID: Tag = Tag(0x0020, 0x000D);
        const TAG_ACCESSION_NUMBER: Tag = Tag(0x0008, 0x0050);
        const TAG_STUDY_DATE: Tag = Tag(0x0008, 0x0020);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&encode_explicit_element(TAG_STUDY_UID, b"UI", study));
        bytes.extend_from_slice(&encode_explicit_element(
            TAG_ACCESSION_NUMBER,
            b"LO",
            accession_number,
        ));
        bytes.extend_from_slice(&encode_explicit_element(TAG_STUDY_DATE, b"DA", study_date));
        bytes
    }

    #[cfg(feature = "dimse-c-find")]
    fn dataset_with_uids(study: &str, series: &str, sop: &str) -> Dataset {
        const TAG_STUDY_UID: Tag = Tag(0x0020, 0x000D);
        const TAG_SERIES_UID: Tag = Tag(0x0020, 0x000E);
        const TAG_SOP_UID: Tag = Tag(0x0008, 0x0018);
        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid(study.to_string()),
        ).unwrap());
        dataset.insert(Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid(series.to_string()),
        ).unwrap());
        dataset.insert(Element::new(TAG_SOP_UID, Vr::Ui, Value::Uid(sop.to_string()),
        ).unwrap());
        dataset
    }

    #[cfg(feature = "dimse-c-find")]
    fn dataset_with_query_metadata(
        study: &str,
        series: &str,
        sop: &str,
        accession_number: &str,
        study_date: &str,
    ) -> Dataset {
        const TAG_ACCESSION_NUMBER: Tag = Tag(0x0008, 0x0050);
        const TAG_STUDY_DATE: Tag = Tag(0x0008, 0x0020);
        let mut dataset = dataset_with_uids(study, series, sop);
        dataset.insert(Element::new(TAG_ACCESSION_NUMBER, Vr::Lo, Value::Str(accession_number.to_string()),
        ).unwrap());
        dataset.insert(Element::new(TAG_STUDY_DATE, Vr::Da, Value::Str(study_date.to_string()),
        ).unwrap());
        dataset
    }

    fn dataset_element_explicit(tag: Tag, vr: [u8; 2], value: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&tag.0.to_le_bytes());
        buf.extend_from_slice(&tag.1.to_le_bytes());
        buf.extend_from_slice(&vr);
        let mut bytes = value.to_vec();
        if bytes.len() % 2 == 1 {
            bytes.push(0);
        }
        match &vr {
            b"OB" | b"OW" | b"SQ" | b"UN" | b"UT" => {
                buf.extend_from_slice(&0u16.to_le_bytes());
                buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            }
            _ => {
                buf.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
            }
        }
        buf.extend_from_slice(&bytes);
        buf
    }

    fn sequence_item(value: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&0xFFFEu16.to_le_bytes());
        buf.extend_from_slice(&0xE000u16.to_le_bytes());
        buf.extend_from_slice(&(value.len() as u32).to_le_bytes());
        buf.extend_from_slice(value);
        buf
    }

    fn storage_commitment_action_information_dataset(
        transaction_uid: &str,
        referenced_sop_class_uid: &str,
        referenced_sop_instance_uid: &str,
    ) -> Vec<u8> {
        let mut item = Vec::new();
        item.extend_from_slice(&dataset_element_explicit(
            TAG_REFERENCED_SOP_CLASS_UID,
            *b"UI",
            referenced_sop_class_uid.as_bytes(),
        ));
        item.extend_from_slice(&dataset_element_explicit(
            TAG_REFERENCED_SOP_INSTANCE_UID,
            *b"UI",
            referenced_sop_instance_uid.as_bytes(),
        ));
        let mut dataset = Vec::new();
        dataset.extend_from_slice(&dataset_element_explicit(
            TAG_TRANSACTION_UID,
            *b"UI",
            transaction_uid.as_bytes(),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            TAG_REFERENCED_SOP_SEQUENCE,
            *b"SQ",
            &sequence_item(&item),
        ));
        dataset
    }

    fn u16_bytes(value: u16) -> [u8; 2] {
        value.to_le_bytes()
    }

    fn minimal_sc_dataset_explicit(study_uid: &str, series_uid: &str, sop_uid: &str) -> Vec<u8> {
        const SOP_CLASS_SC: &str = "1.2.840.10008.5.1.4.1.1.7";
        let mut dataset = Vec::new();
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0008, 0x0016),
            *b"UI",
            SOP_CLASS_SC.as_bytes(),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0008, 0x0018),
            *b"UI",
            sop_uid.as_bytes(),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0020, 0x000D),
            *b"UI",
            study_uid.as_bytes(),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0020, 0x000E),
            *b"UI",
            series_uid.as_bytes(),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0002),
            *b"US",
            &u16_bytes(1),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0004),
            *b"CS",
            b"MONOCHROME2",
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0010),
            *b"US",
            &u16_bytes(1),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0011),
            *b"US",
            &u16_bytes(1),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0100),
            *b"US",
            &u16_bytes(16),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0101),
            *b"US",
            &u16_bytes(12),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0102),
            *b"US",
            &u16_bytes(11),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0103),
            *b"US",
            &u16_bytes(0),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x7FE0, 0x0010),
            *b"OB",
            &[0u8],
        ));
        dataset
    }

    fn association_info_for_transfer_syntax(transfer_syntax_uid: &str) -> AssociationInfo {
        AssociationInfo {
            called_ae: "CALLED_AE".to_string(),
            calling_ae: "CALLING_AE".to_string(),
            presentation_context_id: 1,
            abstract_syntax_uid: "1.2.840.10008.5.1.4.1.1.7".to_string(),
            transfer_syntax_uid: transfer_syntax_uid.to_string(),
        }
    }

    #[test]
    fn c_echo_round_trip() {
        let config = server_config_allow_all();
        let server =
            DimseServer::bind("127.0.0.1:0".parse().unwrap(), config, EchoService).expect("bind");
        let addr = server.local_addr().expect("addr");
        let handle = thread::spawn(move || {
            let _ = server.run_once();
        });

        let mut client = DimseClient::connect(
            addr,
            "CALLED_AE",
            "CALLING_AE",
            &dicom_net::AssociationPolicy::verification_default(),
            DimseClientConfig::default(),
        )
        .expect("connect");
        let status = client.c_echo(1).expect("c-echo");
        assert_eq!(status.code, DimseStatus::success().code);
        client.release().expect("release");
        handle.join().expect("join");
    }

    #[test]
    fn c_store_respects_max_input_bytes() {
        // REQ-DIMSE-303: C-STORE data set bytes are bounded by max_input_bytes.
        let mut server_config = server_config_allow_all();
        server_config.limits.set_max_input_bytes(16);
        let server = DimseServer::bind("127.0.0.1:0".parse().unwrap(), server_config, EchoService)
            .expect("bind");
        let addr = server.local_addr().expect("addr");
        let handle = thread::spawn(move || {
            let _ = server.run_once();
        });

        let policy = dicom_net::AssociationPolicy {
            called_ae: None,
            supported_abstract_syntaxes: vec!["1.2.840.10008.5.1.4.1.1.2".to_string()],
            supported_transfer_syntaxes: vec!["1.2.840.10008.1.2".to_string()],
            max_pdu_length: 16_384,
        };

        let mut client = DimseClient::connect(
            addr,
            "CALLED_AE",
            "CALLING_AE",
            &policy,
            DimseClientConfig::default(),
        )
        .expect("connect");
        let data_set = vec![0u8; 32];
        let result = client.c_store(1, "1.2.840.10008.5.1.4.1.1.2", "1.2.3", &data_set);
        assert!(result.is_err());
        let _ = client.release();
        handle.join().expect("join");
    }

    #[test]
    fn tls_policy_requires_tls() {
        // REQ-NET-308: TLS policy must fail closed when TLS is required.
        let err = enforce_tls_policy(TransportSecurity::Insecure, TlsPolicy::RequireTls)
            .expect_err("expected TLS policy error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn default_server_config_is_fail_closed() {
        // REQ-NET-308 + REQ-AUTH-300: defaults require TLS and deny requests unless explicitly allowed.
        let config = DimseServerConfig::default();
        assert_eq!(config.tls_policy, TlsPolicy::RequireTls);
        assert_eq!(config.transport_security, TransportSecurity::Insecure);
        let decision = config
            .auth
            .authorizer
            .authorize(&dicom_auth::AuthRequest {
                scope: dicom_auth::AuthScope::Dimse,
                action: dicom_auth::AuthAction::Associate,
                subject: dicom_auth::AuthSubject {
                    principal: None,
                    peer: None,
                },
                resource: dicom_auth::AuthResource::none(),
            })
            .expect("auth decision");
        assert!(matches!(decision, dicom_auth::AuthDecision::Deny(_)));
    }

    #[test]
    fn default_server_rejects_insecure_transport_even_with_allow_all_auth() {
        // REQ-NET-308: runtime entrypoint must reject insecure transport when TLS is required.
        let config = DimseServerConfig {
            auth: DimseAuthConfig::allow_all(),
            ..DimseServerConfig::default()
        };
        let server =
            DimseServer::bind("127.0.0.1:0".parse().unwrap(), config, EchoService).expect("bind");
        let addr = server.local_addr().expect("addr");
        let handle = thread::spawn(move || {
            let _ = server.run_once();
        });

        let result = DimseClient::connect(
            addr,
            "CALLED_AE",
            "CALLING_AE",
            &dicom_net::AssociationPolicy::verification_default(),
            DimseClientConfig::default(),
        );
        assert!(result.is_err());
        handle.join().expect("join");
    }

    #[test]
    fn association_throttle_enforces_max_connections() {
        // REQ-NET-309: association throttling enforces max_connections.
        let err = enforce_connection_limit(2, 1).expect_err("expected limit error");
        assert!(matches!(
            err.kind(),
            ErrorKind::LimitExceeded {
                limit_name: "max_connections",
                ..
            }
        ));
    }

    #[test]
    fn operation_throttle_enforces_max_in_flight_operations() {
        let limiter = OperationLimiter::new(1);
        let _guard = limiter
            .try_acquire()
            .expect("expected first operation acquire");
        let err = match limiter.try_acquire() {
            Ok(_unexpected_guard) => panic!("expected throttle limit error"),
            Err(err) => err,
        };
        assert!(matches!(
            err.kind(),
            ErrorKind::LimitExceeded {
                limit_name: "max_in_flight_operations",
                ..
            }
        ));
    }

    #[test]
    fn enforce_query_response_limit_respects_configured_cap() {
        let err = enforce_query_response_limit(2, 1).expect_err("expected query response limit");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn storage_backed_service_persists_c_store_dataset() {
        // REQ-STOR-300: DIMSE C-STORE ingests into deterministic storage state.
        let mut service = StorageBackedDimseService::new(dicom_core::Limits::default());
        let data_set = minimal_sc_dataset_explicit("1.2.3", "2.3.4", "3.4.5");
        let status = service
            .on_c_store(CStoreRequest {
                message_id: 1,
                sop_class_uid: "1.2.840.10008.5.1.4.1.1.7".to_string(),
                sop_instance_uid: "3.4.5".to_string(),
                priority: 0,
                data_set,
                association: association_info_for_transfer_syntax("1.2.840.10008.1.2.1"),
            })
            .expect("c-store");
        assert_eq!(status.code, DimseStatus::success().code);
        assert_eq!(service.storage().index().total_instances(), 1);
    }

    #[test]
    fn storage_commitment_action_information_parses_transaction_and_sequence() {
        let association = association_info_for_transfer_syntax("1.2.840.10008.1.2.1");
        let data_set = storage_commitment_action_information_dataset(
            "1.2.840.10008.1.20.301",
            "1.2.840.10008.5.1.4.1.1.7",
            "1.2.840.10008.5.1.4.1.1.7.301",
        );
        let request = parse_storage_commitment_n_action_request(
            3,
            "1.2.840.10008.5.1.4.1.1.7",
            "1.2.840.10008.5.1.4.1.1.7.301",
            &data_set,
            &association,
            &dicom_core::Limits::default(),
        )
        .expect("parse storage commitment action information");
        assert_eq!(request.message_id, 3);
        assert_eq!(request.transaction_uid, "1.2.840.10008.1.20.301");
        assert_eq!(request.referenced_instances.len(), 1);
        assert_eq!(
            request.referenced_instances[0].sop_class_uid,
            "1.2.840.10008.5.1.4.1.1.7"
        );
        assert_eq!(
            request.referenced_instances[0].sop_instance_uid,
            "1.2.840.10008.5.1.4.1.1.7.301"
        );
    }

    #[test]
    fn storage_commitment_action_information_rejects_invalid_transaction_uid() {
        let association = association_info_for_transfer_syntax("1.2.840.10008.1.2.1");
        let data_set = storage_commitment_action_information_dataset(
            "1..2",
            "1.2.840.10008.5.1.4.1.1.7",
            "1.2.840.10008.5.1.4.1.1.7.302",
        );
        let err = parse_storage_commitment_n_action_request(
            4,
            "1.2.840.10008.5.1.4.1.1.7",
            "1.2.840.10008.5.1.4.1.1.7.302",
            &data_set,
            &association,
            &dicom_core::Limits::default(),
        )
        .expect_err("invalid transaction uid must fail");
        assert!(matches!(
            err.kind(),
            ErrorKind::InvalidTagValue {
                tag: TAG_TRANSACTION_UID,
                ..
            }
        ));
    }

    #[test]
    fn storage_backed_service_registers_storage_commitment_request() {
        let mut service = StorageBackedDimseService::new(dicom_core::Limits::default());
        let status = service
            .on_storage_commitment_n_action(StorageCommitmentNActionRequest {
                message_id: 1,
                transaction_uid: "1.2.840.10008.1.20.3".to_string(),
                calling_ae_title: "CALLING_AE".to_string(),
                called_ae_title: "CALLED_AE".to_string(),
                referenced_instances: vec![StorageCommitmentReferencedInstance {
                    sop_class_uid: "1.2.840.10008.5.1.4.1.1.7".to_string(),
                    sop_instance_uid: "1.2.840.10008.5.1.4.1.1.7.3".to_string(),
                }],
                association: association_info_for_transfer_syntax("1.2.840.10008.1.2.1"),
            })
            .expect("n-action");

        assert_eq!(status.code, DimseStatus::success().code);
        assert!(service
            .storage()
            .storage_commitment_request("1.2.840.10008.1.20.3")
            .is_some());
    }

    #[test]
    fn storage_backed_service_processes_storage_commitment_event_jobs() {
        let mut service = StorageBackedDimseService::new(dicom_core::Limits::default());
        let _ = service
            .on_storage_commitment_n_action(StorageCommitmentNActionRequest {
                message_id: 1,
                transaction_uid: "1.2.840.10008.1.20.4".to_string(),
                calling_ae_title: "CALLING_AE".to_string(),
                called_ae_title: "CALLED_AE".to_string(),
                referenced_instances: vec![StorageCommitmentReferencedInstance {
                    sop_class_uid: "1.2.840.10008.5.1.4.1.1.7".to_string(),
                    sop_instance_uid: "1.2.840.10008.5.1.4.1.1.7.4".to_string(),
                }],
                association: association_info_for_transfer_syntax("1.2.840.10008.1.2.1"),
            })
            .expect("n-action");

        let processed = service
            .process_storage_commitment_event_reports()
            .expect("process");
        assert_eq!(processed, 1);
        let request = service
            .storage()
            .storage_commitment_request("1.2.840.10008.1.20.4")
            .expect("request");
        assert_eq!(request.state, dicom_storage::StorageCommitmentState::ReportDelivered);
    }

    #[test]
    fn storage_commitment_lifecycle_sink_receives_state_transitions() {
        let mut service = StorageBackedDimseService::new(dicom_core::Limits::default());
        let events = Arc::new(Mutex::new(Vec::<StorageCommitmentLifecycleEvent>::new()));
        let events_clone = Arc::clone(&events);
        service.set_storage_commitment_sink(Some(Arc::new(move |event| {
            events_clone.lock().expect("lock").push(event);
            Ok(())
        })));

        let _ = service
            .on_storage_commitment_n_action(StorageCommitmentNActionRequest {
                message_id: 7,
                transaction_uid: "1.2.840.10008.1.20.47".to_string(),
                calling_ae_title: "CALLING_AE".to_string(),
                called_ae_title: "CALLED_AE".to_string(),
                referenced_instances: vec![StorageCommitmentReferencedInstance {
                    sop_class_uid: "1.2.840.10008.5.1.4.1.1.7".to_string(),
                    sop_instance_uid: "1.2.840.10008.5.1.4.1.1.7.47".to_string(),
                }],
                association: association_info_for_transfer_syntax("1.2.840.10008.1.2.1"),
            })
            .expect("n-action");
        let _ = service
            .process_storage_commitment_event_reports()
            .expect("process");

        let snapshot = events.lock().expect("lock").clone();
        assert_eq!(snapshot.len(), 2);
        assert_eq!(
            snapshot[0].event_code,
            "storage_commitment.request_registered"
        );
        assert_eq!(
            snapshot[1].event_code,
            "storage_commitment.report_delivered"
        );
    }

    #[test]
    fn ian_producer_consumer_paths_are_deduplicated() {
        let mut service = StorageBackedDimseService::new(dicom_core::Limits::default());
        assert!(service.produce_ian_notification(
            "ian-evt-1".to_string(),
            "1.2.3".to_string(),
            "1.2.3.4".to_string(),
            0x0000
        ));
        assert!(!service.produce_ian_notification(
            "ian-evt-1".to_string(),
            "1.2.3".to_string(),
            "1.2.3.4".to_string(),
            0x0000
        ));
        assert!(service.consume_ian_notification(IanNotification {
            event_id: "ian-evt-2".to_string(),
            study_instance_uid: "1.2.3".to_string(),
            sop_instance_uid: "1.2.3.5".to_string(),
            status_code: 0x0000,
        }));
        assert_eq!(service.ian_notifications().len(), 2);
    }

    #[cfg(feature = "dimse-c-find")]
    #[test]
    fn storage_backed_service_c_find_uses_persisted_state() {
        // REQ-QR-301: persisted datasets are queried with deterministic pending/final statuses.
        let mut service = StorageBackedDimseService::new(dicom_core::Limits::default());
        let data_set = minimal_sc_dataset_explicit("1.2.3", "2.3.4", "3.4.5");
        let _ = service
            .on_c_store(CStoreRequest {
                message_id: 1,
                sop_class_uid: "1.2.840.10008.5.1.4.1.1.7".to_string(),
                sop_instance_uid: "3.4.5".to_string(),
                priority: 0,
                data_set,
                association: association_info_for_transfer_syntax("1.2.840.10008.1.2.1"),
            })
            .expect("c-store");

        let statuses = service
            .on_c_find(CFindRequest {
                message_id: 2,
                sop_class_uid: "1.2.840.10008.5.1.4.1.2.2.1".to_string(),
                priority: 0,
                data_set: identifier_dataset_bytes("1.2.3", "2.3.4", "3.4.5"),
                association: association_info_for_transfer_syntax("1.2.840.10008.1.2.1"),
            })
            .expect("c-find");
        assert_eq!(
            statuses,
            vec![DimseStatus::pending(), DimseStatus::success()]
        );
    }

    #[cfg(feature = "dimse-c-find")]
    #[test]
    fn c_find_round_trip() {
        // REQ-DIMSE-340: C-FIND handlers dispatch and return deterministic pending/final status responses.
        let seen = Arc::new(Mutex::new(None));
        let service = FindService {
            seen_len: Arc::clone(&seen),
        };
        let policy = dicom_net::AssociationPolicy {
            called_ae: None,
            supported_abstract_syntaxes: vec!["1.2.840.10008.5.1.4.1.2.2.1".to_string()],
            supported_transfer_syntaxes: vec!["1.2.840.10008.1.2".to_string()],
            max_pdu_length: 16_384,
        };
        let server_config = DimseServerConfig {
            policy: policy.clone(),
            auth: DimseAuthConfig::allow_all(),
            tls_policy: TlsPolicy::AllowInsecure,
            transport_security: TransportSecurity::Insecure,
            ..DimseServerConfig::default()
        };
        let server = DimseServer::bind("127.0.0.1:0".parse().unwrap(), server_config, service)
            .expect("bind");
        let addr = server.local_addr().expect("addr");
        let handle = thread::spawn(move || {
            let _ = server.run_once();
        });

        let mut client = DimseClient::connect(
            addr,
            "CALLED_AE",
            "CALLING_AE",
            &policy,
            DimseClientConfig::default(),
        )
        .expect("connect");
        let data_set = vec![1u8, 2, 3];
        let status = client
            .c_find(1, "1.2.840.10008.5.1.4.1.2.2.1", 0, &data_set)
            .expect("c-find");
        assert_eq!(status.code, DimseStatus::success().code);
        client.release().expect("release");
        handle.join().expect("join");
        assert_eq!(*seen.lock().expect("lock"), Some(3));
    }

    #[cfg(feature = "dimse-c-find")]
    #[test]
    fn dimse_query_matches_identifier_dataset() {
        // REQ-QR-300: only supported UID keys may be used for matching.
        // REQ-QR-301: query results are deterministically ordered by UID.
        let datasets = vec![
            dataset_with_uids("1.2.3", "1.2.3.4", "1.2.3.4.5"),
            dataset_with_uids("9.9", "8.8", "7.7"),
        ];
        let identifier = identifier_dataset_bytes("1.2.3", "1.2.3.4", "1.2.3.4.5");
        let matches = query_matches_from_dimse(
            &identifier,
            "1.2.840.10008.1.2.1",
            &datasets,
            &dicom_core::Limits::default(),
        )
        .expect("query matches");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].study_uid, "1.2.3");
        assert_eq!(matches[0].series_uid.as_deref(), Some("1.2.3.4"));
        assert_eq!(matches[0].instance_uid.as_deref(), Some("1.2.3.4.5"));
    }

    #[cfg(feature = "dimse-c-find")]
    #[test]
    fn dimse_query_matches_accession_and_study_date() {
        // REQ-QR-300: DIMSE identifier mapping supports Accession Number and Study Date keys.
        let datasets = vec![
            dataset_with_query_metadata("1.2.3", "1.2.3.4", "1.2.3.4.5", "ACC123", "20260211"),
            dataset_with_query_metadata("9.9", "8.8", "7.7", "ACC999", "20260101"),
        ];
        let identifier = identifier_dataset_with_clinical_filters("1.2.3", "ACC123", "20260211");
        let matches = query_matches_from_dimse(
            &identifier,
            "1.2.840.10008.1.2.1",
            &datasets,
            &dicom_core::Limits::default(),
        )
        .expect("query matches");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].study_uid, "1.2.3");
        assert_eq!(matches[0].series_uid, None);
        assert_eq!(matches[0].instance_uid, None);
    }

    #[cfg(feature = "dimse-c-find")]
    #[test]
    fn dimse_query_rejects_unsupported_keys() {
        // REQ-QR-300: unsupported keys must fail closed.
        const TAG_STUDY_DESCRIPTION: Tag = Tag(0x0008, 0x1030);
        let mut identifier = Vec::new();
        identifier.extend_from_slice(&encode_explicit_element(
            TAG_STUDY_DESCRIPTION,
            b"LO",
            "CT HEAD",
        ));
        let err =
            query_matches_from_dimse(&identifier, "1.2.840.10008.1.2.1", &[], &dicom_core::Limits::default())
                .expect_err("expected query error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[cfg(feature = "dimse-c-move")]
    #[test]
    fn c_move_round_trip() {
        // REQ-DIMSE-341: C-MOVE handlers dispatch and return deterministic pending/final status responses.
        let seen = Arc::new(Mutex::new(None));
        let service = MoveService {
            seen_destination: Arc::clone(&seen),
        };
        let policy = dicom_net::AssociationPolicy {
            called_ae: None,
            supported_abstract_syntaxes: vec!["1.2.840.10008.5.1.4.1.2.2.2".to_string()],
            supported_transfer_syntaxes: vec!["1.2.840.10008.1.2".to_string()],
            max_pdu_length: 16_384,
        };
        let server_config = DimseServerConfig {
            policy: policy.clone(),
            auth: DimseAuthConfig::allow_all(),
            tls_policy: TlsPolicy::AllowInsecure,
            transport_security: TransportSecurity::Insecure,
            ..DimseServerConfig::default()
        };
        let server = DimseServer::bind("127.0.0.1:0".parse().unwrap(), server_config, service)
            .expect("bind");
        let addr = server.local_addr().expect("addr");
        let handle = thread::spawn(move || {
            let _ = server.run_once();
        });

        let mut client = DimseClient::connect(
            addr,
            "CALLED_AE",
            "CALLING_AE",
            &policy,
            DimseClientConfig::default(),
        )
        .expect("connect");
        let data_set = vec![9u8, 8, 7];
        let status = client
            .c_move(2, "1.2.840.10008.5.1.4.1.2.2.2", "DEST_AE", 0, &data_set)
            .expect("c-move");
        assert_eq!(status.code, DimseStatus::success().code);
        client.release().expect("release");
        handle.join().expect("join");
        assert_eq!(*seen.lock().expect("lock"), Some("DEST_AE".to_string()));
    }

    #[cfg(feature = "dimse-c-get")]
    #[test]
    fn c_get_round_trip() {
        // REQ-DIMSE-342: C-GET handlers dispatch and return deterministic pending/final status responses.
        let seen = Arc::new(Mutex::new(None));
        let service = GetService {
            seen_len: Arc::clone(&seen),
        };
        let policy = dicom_net::AssociationPolicy {
            called_ae: None,
            supported_abstract_syntaxes: vec!["1.2.840.10008.5.1.4.1.2.2.3".to_string()],
            supported_transfer_syntaxes: vec!["1.2.840.10008.1.2".to_string()],
            max_pdu_length: 16_384,
        };
        let server_config = DimseServerConfig {
            policy: policy.clone(),
            auth: DimseAuthConfig::allow_all(),
            tls_policy: TlsPolicy::AllowInsecure,
            transport_security: TransportSecurity::Insecure,
            ..DimseServerConfig::default()
        };
        let server = DimseServer::bind("127.0.0.1:0".parse().unwrap(), server_config, service)
            .expect("bind");
        let addr = server.local_addr().expect("addr");
        let handle = thread::spawn(move || {
            let _ = server.run_once();
        });

        let mut client = DimseClient::connect(
            addr,
            "CALLED_AE",
            "CALLING_AE",
            &policy,
            DimseClientConfig::default(),
        )
        .expect("connect");
        let data_set = vec![4u8, 5, 6];
        let status = client
            .c_get(3, "1.2.840.10008.5.1.4.1.2.2.3", 0, &data_set)
            .expect("c-get");
        assert_eq!(status.code, DimseStatus::success().code);
        client.release().expect("release");
        handle.join().expect("join");
        assert_eq!(*seen.lock().expect("lock"), Some(3));
    }

    #[test]
    fn command_limit_enforced_before_accumulation() {
        // REQ-SEC-404: Command bytes must be bounded before accumulation.
        let mut config = server_config_allow_all();
        config.dimse_limits.max_command_bytes = 8;
        let handler = Arc::new(Mutex::new(EchoService));
        let association = dummy_association_accept();
        let context_map = HashMap::new();
        let mut pending = HashMap::new();
        let (mut server_stream, _client_stream) = stream_pair();

        let pdv = dicom_net::Pdv {
            presentation_context_id: 1,
            message_control_header: 0x03,
            data: vec![0u8; 16],
        };

        let err = handle_pdus(
            vec![pdv],
            &association,
            &context_map,
            &mut pending,
            &config,
            &handler,
            &OperationLimiter::new(64),
            &mut server_stream,
        )
        .expect_err("error");
        match err.kind() {
            ErrorKind::LimitExceeded { limit_name, .. } => {
                assert_eq!(*limit_name, "max_command_bytes");
            }
            _ => panic!("expected limit exceeded"),
        }
    }

    #[test]
    fn data_limit_enforced_before_accumulation() {
        // REQ-DIMSE-303: Data set bytes are bounded before accumulation.
        let mut config = server_config_allow_all();
        config.limits.set_max_input_bytes(8);
        let handler = Arc::new(Mutex::new(EchoService));
        let association = dummy_association_accept();
        let context_map = HashMap::new();
        let mut pending = HashMap::new();
        let (mut server_stream, _client_stream) = stream_pair();

        let pdv = dicom_net::Pdv {
            presentation_context_id: 1,
            message_control_header: 0x02,
            data: vec![0u8; 16],
        };

        let err = handle_pdus(
            vec![pdv],
            &association,
            &context_map,
            &mut pending,
            &config,
            &handler,
            &OperationLimiter::new(64),
            &mut server_stream,
        )
        .expect_err("error");
        match err.kind() {
            ErrorKind::LimitExceeded { limit_name, .. } => {
                assert_eq!(*limit_name, "max_input_bytes");
            }
            _ => panic!("expected limit exceeded"),
        }
    }

    struct FixedRedactor;

    impl AuditRedactor for FixedRedactor {
        fn redact(&self, _value: &str) -> String {
            "redacted".to_string()
        }
    }

    struct DenyEchoAllowAssociate;

    impl dicom_auth::Authorizer for DenyEchoAllowAssociate {
        fn authorize(&self, request: &dicom_auth::AuthRequest<'_>) -> dicom_core::Result<dicom_auth::AuthDecision> {
            if request.action == dicom_auth::AuthAction::Associate {
                return Ok(dicom_auth::AuthDecision::Allow);
            }
            Ok(dicom_auth::AuthDecision::Deny(dicom_auth::AuthDenyReason::Unauthorized))
        }
    }

    #[test]
    fn c_echo_denied_by_auth_records_audit() {
        // REQ-AUTH-300, REQ-AUTH-302, REQ-AUDIT-350
        let audit_log = Arc::new(Mutex::new(AuditLog::new(
            AuditConfig::default(),
            FixedRedactor,
        )));
        let audit_log_handle = Arc::clone(&audit_log);
        let audit: AuditCallback = Arc::new(move |event| {
            let mut log = audit_log_handle
                .lock()
                .map_err(|_| decode_error("audit lock poisoned"))?;
            log.record(event)
        });

        let config = DimseServerConfig {
            auth: DimseAuthConfig {
                authorizer: Arc::new(DenyEchoAllowAssociate),
                audit: Some(audit),
            },
            tls_policy: TlsPolicy::AllowInsecure,
            transport_security: TransportSecurity::Insecure,
            ..DimseServerConfig::default()
        };
        let server =
            DimseServer::bind("127.0.0.1:0".parse().unwrap(), config, EchoService).expect("bind");
        let addr = server.local_addr().expect("addr");
        let handle = thread::spawn(move || {
            let _ = server.run_once();
        });

        let mut client = DimseClient::connect(
            addr,
            "CALLED_AE",
            "CALLING_AE",
            &dicom_net::AssociationPolicy::verification_default(),
            DimseClientConfig::default(),
        )
        .expect("connect");
        let status = client.c_echo(1).expect("c-echo");
        assert_eq!(status.code, DimseStatus::processing_failure().code);
        let _ = client.release();
        handle.join().expect("join");

        let records = audit_log.lock().expect("lock").records().to_vec();
        assert_eq!(records.len(), 4);
        let start_record = records
            .iter()
            .find(|record| {
                record
                    .fields
                    .iter()
                    .any(|field| field.key == "operation" && field.value.as_str() == "c_echo")
                    && record
                        .fields
                        .iter()
                        .any(|field| field.key == "phase" && field.value.as_str() == "start")
            })
            .expect("c_echo start audit event");
        assert_eq!(start_record.kind, AuditEventKind::ServiceEvent);
        let deny_record = records
            .iter()
            .find(|record| {
                record
                    .fields
                    .iter()
                    .find(|field| field.key == "decision")
                    .map(|field| field.value.as_str())
                    == Some("deny")
            })
            .expect("deny audit record");
        assert_eq!(deny_record.kind, AuditEventKind::AuthzDecision);
        let decision = deny_record
            .fields
            .iter()
            .find(|field| field.key == "decision")
            .map(|field| field.value.as_str())
            .unwrap_or("");
        assert_eq!(decision, "deny");
        let called = deny_record
            .fields
            .iter()
            .find(|field| field.key == "called_ae")
            .map(|field| field.value.as_str())
            .unwrap_or("");
        assert_eq!(called, "redacted");

        let operation_record = records
            .iter()
            .find(|record| {
                record
                    .fields
                    .iter()
                    .any(|field| field.key == "operation" && field.value.as_str() == "c_echo")
                    && record
                        .fields
                        .iter()
                        .any(|field| field.key == "phase" && field.value.as_str() == "failure")
            })
            .expect("c_echo service event");
        assert_eq!(operation_record.kind, AuditEventKind::ServiceEvent);
        let operation_id = operation_record
            .fields
            .iter()
            .find(|field| field.key == "operation_id")
            .map(|field| field.value.as_str())
            .unwrap_or("");
        assert!(operation_id.starts_with("c_echo|msg="));
        let status_code = operation_record
            .fields
            .iter()
            .find(|field| field.key == "status_code")
            .map(|field| field.value.as_str())
            .unwrap_or("");
        assert_eq!(
            status_code,
            DimseStatus::processing_failure().code.to_string()
        );
    }

    #[test]
    fn role_config_controls_supported_abstract_syntaxes() {
        let config = DimseServerConfig {
            roles: DimseRoleConfig {
                c_store_enabled: false,
                c_find_enabled: false,
                c_move_enabled: false,
                c_get_enabled: false,
                ..DimseRoleConfig::default()
            },
            ..DimseServerConfig::default()
        };
        let policy = config.effective_policy();
        assert!(policy
            .supported_abstract_syntaxes
            .contains(&SOP_CLASS_VERIFICATION.to_string()));
        assert!(policy
            .supported_abstract_syntaxes
            .iter()
            .all(|uid| uid != SOP_CLASS_CT_IMAGE_STORAGE));
        assert!(policy
            .supported_abstract_syntaxes
            .iter()
            .all(|uid| uid != SOP_CLASS_STUDY_ROOT_FIND));
        assert!(policy
            .supported_abstract_syntaxes
            .iter()
            .all(|uid| uid != SOP_CLASS_STUDY_ROOT_MOVE));
        assert!(policy
            .supported_abstract_syntaxes
            .iter()
            .all(|uid| uid != SOP_CLASS_STUDY_ROOT_GET));
    }

    #[test]
    fn c_store_respects_role_specific_size_limit() {
        let mut config = server_config_allow_all();
        config.operation_sizes = DimseOperationSizeConfig {
            c_echo_data_set_bytes: 0,
            c_store_data_set_bytes: 1,
            c_find_data_set_bytes: config.limits.max_input_bytes(),
            c_move_data_set_bytes: config.limits.max_input_bytes(),
            c_get_data_set_bytes: config.limits.max_input_bytes(),
        };
        config.roles = DimseRoleConfig {
            c_echo_enabled: true,
            c_store_enabled: true,
            c_find_enabled: cfg!(feature = "dimse-c-find"),
            c_move_enabled: cfg!(feature = "dimse-c-move"),
            c_get_enabled: cfg!(feature = "dimse-c-get"),
        };
        let server =
            DimseServer::bind("127.0.0.1:0".parse().unwrap(), config, EchoService).expect("bind");
        let addr = server.local_addr().expect("addr");
        let handle = thread::spawn(move || {
            let _ = server.run_once();
        });

        let policy = dicom_net::AssociationPolicy {
            called_ae: None,
            supported_abstract_syntaxes: vec!["1.2.840.10008.5.1.4.1.1.2".to_string()],
            supported_transfer_syntaxes: vec!["1.2.840.10008.1.2".to_string()],
            max_pdu_length: 16_384,
        };

        let mut client = DimseClient::connect(
            addr,
            "CALLED_AE",
            "CALLING_AE",
            &policy,
            DimseClientConfig::default(),
        )
        .expect("connect");
        let data_set = vec![0u8; 16];
        let result = client.c_store(1, "1.2.840.10008.5.1.4.1.1.2", "1.2.3", &data_set);
        assert!(result.is_err());
        let err = result.expect_err("c-store expected to fail");
        match err.kind() {
            ErrorKind::LimitExceeded { limit_name, .. } => {
                assert_eq!(*limit_name, "max_input_bytes");
            }
            ErrorKind::IoError { .. } => {}
            _ => panic!("expected limit exceeded"),
        }
        let _ = client.release();
        handle.join().expect("join");
    }

    #[test]
    fn c_echo_audit_records_operation_event() {
        let audit_log = Arc::new(Mutex::new(AuditLog::new(
            AuditConfig::default(),
            FixedRedactor,
        )));
        let audit_log_handle = Arc::clone(&audit_log);
        let audit: AuditCallback = Arc::new(move |event| {
            let mut log = audit_log_handle
                .lock()
                .map_err(|_| decode_error("audit lock poisoned"))?;
            log.record(event)
        });

        let config = DimseServerConfig {
            auth: DimseAuthConfig {
                authorizer: Arc::new(dicom_auth::AllowAll),
                audit: Some(audit),
            },
            tls_policy: TlsPolicy::AllowInsecure,
            transport_security: TransportSecurity::Insecure,
            ..DimseServerConfig::default()
        };
        let server =
            DimseServer::bind("127.0.0.1:0".parse().unwrap(), config, EchoService).expect("bind");
        let addr = server.local_addr().expect("addr");
        let handle = thread::spawn(move || {
            let _ = server.run_once();
        });

        let mut client = DimseClient::connect(
            addr,
            "CALLED_AE",
            "CALLING_AE",
            &dicom_net::AssociationPolicy::verification_default(),
            DimseClientConfig::default(),
        )
        .expect("connect");
        let status = client.c_echo(1).expect("c-echo");
        assert_eq!(status.code, DimseStatus::success().code);
        let _ = client.release();
        handle.join().expect("join");

        let records = audit_log.lock().expect("lock").records().to_vec();
        let operation_record = records
            .iter()
            .find(|record| {
                record
                    .fields
                    .iter()
                    .any(|field| field.key == "operation" && field.value.as_str() == "c_echo")
                    && record
                        .fields
                        .iter()
                        .any(|field| field.key == "phase" && field.value.as_str() == "completion")
            })
            .expect("c_echo service event");
        assert_eq!(operation_record.kind, AuditEventKind::ServiceEvent);
        let start_record = records
            .iter()
            .find(|record| {
                record
                    .fields
                    .iter()
                    .any(|field| field.key == "operation" && field.value.as_str() == "c_echo")
                    && record
                        .fields
                        .iter()
                        .any(|field| field.key == "phase" && field.value.as_str() == "start")
            })
            .expect("c_echo start audit event");
        assert_eq!(start_record.kind, AuditEventKind::ServiceEvent);
        let status_code = operation_record
            .fields
            .iter()
            .find(|field| field.key == "status_code")
            .map(|field| field.value.as_str())
            .unwrap_or("");
        assert_eq!(status_code, DimseStatus::success().code.to_string());
    }

    #[test]
    fn run_once_rejects_disallowed_peer_when_host_allowlist_set() {
        let config = DimseServerConfig {
            allowed_hosts: Some(vec![std::net::IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1))]),
            auth: DimseAuthConfig::allow_all(),
            tls_policy: TlsPolicy::AllowInsecure,
            transport_security: TransportSecurity::Insecure,
            ..DimseServerConfig::default()
        };
        let server =
            DimseServer::bind("127.0.0.1:0".parse().unwrap(), config, EchoService).expect("bind");
        let addr = server.local_addr().expect("addr");
        let handle = thread::spawn(move || {
            let _ = server.run_once();
        });

        let err = DimseClient::connect(
            addr,
            "CALLED_AE",
            "CALLING_AE",
            &dicom_net::AssociationPolicy::verification_default(),
            DimseClientConfig::default(),
        )
        .err()
        .expect("expected connect failure");
        assert!(matches!(
            err.kind(),
            ErrorKind::DecodeError { .. } | ErrorKind::IoError { .. }
        ));

        handle.join().expect("join");
    }

    #[test]
    fn protocol_violation_status_maps_unsupported_sop_class() {
        let err = decode_error("unsupported SOP class UID: 9.9.9");
        assert_eq!(
            dimse_protocol_violation_status(&err),
            DimseStatus::sop_class_not_supported()
        );
    }

    #[test]
    fn protocol_violation_status_maps_unsupported_dimse_command() {
        let err = decode_error("unsupported DIMSE command");
        assert_eq!(
            dimse_protocol_violation_status(&err),
            DimseStatus::sop_class_not_supported()
        );
    }

    #[test]
    fn protocol_violation_status_defaults_to_processing_failure() {
        let err = decode_error("unexpected command value");
        assert_eq!(
            dimse_protocol_violation_status(&err),
            DimseStatus::processing_failure()
        );
    }
}
