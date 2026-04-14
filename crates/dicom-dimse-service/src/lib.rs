#![deny(missing_docs)]

//! DIMSE SCU/SCP harness built on `dicom-net` and `dicom-dimse`.

use dicom_audit::{AuditEvent, AuditEventKind, AuditField, AuditValue};
use dicom_auth::{
    AllowAll, AuthAction, AuthDecision, AuthDenyReason, AuthRequest, AuthResource, AuthScope,
    AuthSubject, Authorizer, DenyAll,
};
use dicom_core::{Error, ErrorKind, Limits, Result, Tag, Value};
use dicom_dimse::{
    build_c_echo_request, build_c_echo_response, build_c_store_request, build_c_store_response,
    build_n_action_response, parse_command_set, DimseLimits, DimseMessage,
};
#[cfg(feature = "dimse-c-find")]
use dicom_dimse::{build_c_find_request, build_c_find_response};
#[cfg(feature = "dimse-c-get")]
use dicom_dimse::{build_c_get_request, build_c_get_response};
#[cfg(feature = "dimse-c-move")]
use dicom_dimse::{build_c_move_request, build_c_move_response};
use dicom_io::parse_dataset_bytes;
use dicom_net::{
    accept_association, encode_pdu, parse_pdu, AssociationAccept, AssociationPolicy,
    AssociationReject, AssociationRequest, NetworkLimits, Pdu, Pdv,
};
use dicom_storage::{
    Storage, StorageCommitmentReferencedInstance, StorageCommitmentRequest, StorageCommitmentState,
    WriteAheadLog,
};
use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[cfg(any(
    feature = "dimse-c-find",
    feature = "dimse-c-move",
    feature = "dimse-c-get"
))]
use dicom_core::Dataset;
#[cfg(any(
    feature = "dimse-c-find",
    feature = "dimse-c-move",
    feature = "dimse-c-get"
))]
use dicom_query::{query as run_query, query_from_identifier, QueryMatch};

/// Transport security state for DIMSE connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportSecurity {
    /// Unencrypted transport.
    Insecure,
    /// TLS-protected transport.
    Tls,
}

/// TLS policy for DIMSE connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsPolicy {
    /// Allow insecure transports.
    AllowInsecure,
    /// Require TLS-protected transports.
    RequireTls,
}

/// DIMSE TLS material contract.
#[derive(Debug, Clone)]
pub struct DimseTlsMaterialConfig {
    /// TLS server certificate path.
    pub certificate_path: String,
    /// TLS private key path.
    pub private_key_path: String,
    /// Optional trusted CA bundle path.
    pub ca_bundle_path: Option<String>,
    /// Optional certificate refresh interval in seconds.
    pub cert_rotation_interval_secs: Option<u64>,
}

/// DIMSE status wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DimseStatus {
    /// Status code value.
    pub code: u16,
}

impl DimseStatus {
    /// Success status.
    pub fn success() -> Self {
        Self { code: 0x0000 }
    }

    /// Processing failure status.
    pub fn processing_failure() -> Self {
        Self { code: 0x0110 }
    }

    /// SOP class not supported status.
    pub fn sop_class_not_supported() -> Self {
        Self { code: 0x0122 }
    }

    /// Pending status.
    pub fn pending() -> Self {
        Self { code: 0xFF00 }
    }

    /// True when status is pending.
    pub fn is_pending(self) -> bool {
        is_pending_status(self.code)
    }
}

const DEFAULT_MAX_IN_FLIGHT_OPERATIONS: usize = 64;
const DEFAULT_MAX_QUERY_RESPONSE_COUNT: usize = 4_096;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DimseOperationAuditPhase {
    Start,
    Response,
    Completion,
    Failure,
}

fn dimse_operation_audit_phase_label(phase: DimseOperationAuditPhase) -> &'static str {
    match phase {
        DimseOperationAuditPhase::Start => "start",
        DimseOperationAuditPhase::Response => "response",
        DimseOperationAuditPhase::Completion => "completion",
        DimseOperationAuditPhase::Failure => "failure",
    }
}

const SOP_CLASS_VERIFICATION: &str = "1.2.840.10008.1.1";
const SOP_CLASS_CT_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.2";
const SOP_CLASS_MR_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.4";
const SOP_CLASS_SECONDARY_CAPTURE: &str = "1.2.840.10008.5.1.4.1.1.7";
const SOP_CLASS_MULTI_FRAME_SC_BYTE: &str = "1.2.840.10008.5.1.4.1.1.7.2";
const SOP_CLASS_MULTI_FRAME_SC_WORD: &str = "1.2.840.10008.5.1.4.1.1.7.3";
const SOP_CLASS_MULTI_FRAME_SC_TRUE_COLOR: &str = "1.2.840.10008.5.1.4.1.1.7.4";
const SOP_CLASS_PET_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.128";
const SOP_CLASS_CR_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.1";
const SOP_CLASS_DX_PRESENTATION: &str = "1.2.840.10008.5.1.4.1.1.1.1";
const SOP_CLASS_STUDY_ROOT_FIND: &str = "1.2.840.10008.5.1.4.1.2.2.1";
const SOP_CLASS_STUDY_ROOT_MOVE: &str = "1.2.840.10008.5.1.4.1.2.2.2";
const SOP_CLASS_STUDY_ROOT_GET: &str = "1.2.840.10008.5.1.4.1.2.2.3";
const TRANSFER_SYNTAX_IMPLICIT_VR_LE: &str = "1.2.840.10008.1.2";
const TRANSFER_SYNTAX_EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1";
const TAG_TRANSACTION_UID: Tag = Tag(0x0008, 0x1195);
const TAG_REFERENCED_SOP_SEQUENCE: Tag = Tag(0x0008, 0x1199);
const TAG_REFERENCED_SOP_CLASS_UID: Tag = Tag(0x0008, 0x1150);
const TAG_REFERENCED_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x1155);

fn workstation_default_association_policy() -> AssociationPolicy {
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

/// Availability state for DIMSE command contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DimseCommandAvailability {
    /// Command is active in this build.
    Active,
    /// Command is disabled by feature profile in this build.
    DisabledByFeature,
}

/// Operator-visible DIMSE command contract row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DimseCommandContract {
    /// DIMSE command label.
    pub command: &'static str,
    /// Abstract syntax UID associated with the command.
    pub abstract_syntax_uid: &'static str,
    /// Availability state for this command.
    pub availability: DimseCommandAvailability,
    /// True when command responses require pending->final sequencing.
    pub requires_pending_final_sequence: bool,
}

/// Operator-visible DIMSE association contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DimseAssociationContract {
    /// True when called AE title is constrained by policy.
    pub called_ae_required: bool,
    /// Negotiated max PDU length exposed by the default policy.
    pub max_pdu_length: u32,
    /// Supported abstract syntax UIDs in deterministic order.
    pub abstract_syntax_uids: Vec<&'static str>,
    /// Supported transfer syntax UIDs in deterministic order.
    pub transfer_syntax_uids: Vec<&'static str>,
    /// DIMSE command contracts in deterministic order.
    pub command_contracts: Vec<DimseCommandContract>,
}

/// Operator-visible DIMSE role contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DimseRoleConfig {
    /// Whether the C-ECHO command is enabled.
    pub c_echo_enabled: bool,
    /// Whether the C-STORE command is enabled.
    pub c_store_enabled: bool,
    /// Whether the C-FIND command is enabled.
    pub c_find_enabled: bool,
    /// Whether the C-MOVE command is enabled.
    pub c_move_enabled: bool,
    /// Whether the C-GET command is enabled.
    pub c_get_enabled: bool,
}

impl Default for DimseRoleConfig {
    fn default() -> Self {
        Self {
            c_echo_enabled: true,
            c_store_enabled: true,
            c_find_enabled: true,
            c_move_enabled: true,
            c_get_enabled: true,
        }
    }
}

/// Per-operation payload size limits for runtime role configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DimseOperationSizeConfig {
    /// Maximum accepted C-ECHO data-set bytes (normally zero for DICOM).
    pub c_echo_data_set_bytes: u64,
    /// Maximum accepted C-STORE data-set bytes.
    pub c_store_data_set_bytes: u64,
    /// Maximum accepted C-FIND data-set bytes.
    pub c_find_data_set_bytes: u64,
    /// Maximum accepted C-MOVE data-set bytes.
    pub c_move_data_set_bytes: u64,
    /// Maximum accepted C-GET data-set bytes.
    pub c_get_data_set_bytes: u64,
}

impl DimseOperationSizeConfig {
    /// Build config with identical payload size limits from a shared bound.
    fn with_default_bound(max_input_bytes: u64) -> Self {
        Self {
            c_echo_data_set_bytes: 0,
            c_store_data_set_bytes: max_input_bytes,
            c_find_data_set_bytes: max_input_bytes,
            c_move_data_set_bytes: max_input_bytes,
            c_get_data_set_bytes: max_input_bytes,
        }
    }
}

/// Per-operation resource caps for runtime governance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DimseOperationResourceLimits {
    /// Maximum concurrent DIMSE operations across all associations.
    pub max_in_flight_operations: usize,
    /// Maximum number of responses a single query/retrieve operation may emit.
    pub max_query_responses: usize,
}

impl Default for DimseOperationResourceLimits {
    fn default() -> Self {
        Self {
            max_in_flight_operations: DEFAULT_MAX_IN_FLIGHT_OPERATIONS,
            max_query_responses: DEFAULT_MAX_QUERY_RESPONSE_COUNT,
        }
    }
}

/// Build the deterministic DIMSE association/command contract matrix.
pub fn dimse_association_contract() -> DimseAssociationContract {
    let policy = workstation_default_association_policy();
    #[allow(unused_mut)]
    let mut abstract_syntax_uids = vec![
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
    abstract_syntax_uids.push(SOP_CLASS_STUDY_ROOT_FIND);
    #[cfg(feature = "dimse-c-move")]
    abstract_syntax_uids.push(SOP_CLASS_STUDY_ROOT_MOVE);
    #[cfg(feature = "dimse-c-get")]
    abstract_syntax_uids.push(SOP_CLASS_STUDY_ROOT_GET);

    let command_contracts = vec![
        DimseCommandContract {
            command: "C-ECHO",
            abstract_syntax_uid: SOP_CLASS_VERIFICATION,
            availability: DimseCommandAvailability::Active,
            requires_pending_final_sequence: false,
        },
        DimseCommandContract {
            command: "C-STORE",
            abstract_syntax_uid: SOP_CLASS_CT_IMAGE_STORAGE,
            availability: DimseCommandAvailability::Active,
            requires_pending_final_sequence: false,
        },
        DimseCommandContract {
            command: "C-FIND",
            abstract_syntax_uid: SOP_CLASS_STUDY_ROOT_FIND,
            availability: command_availability(cfg!(feature = "dimse-c-find")),
            requires_pending_final_sequence: true,
        },
        DimseCommandContract {
            command: "C-MOVE",
            abstract_syntax_uid: SOP_CLASS_STUDY_ROOT_MOVE,
            availability: command_availability(cfg!(feature = "dimse-c-move")),
            requires_pending_final_sequence: true,
        },
        DimseCommandContract {
            command: "C-GET",
            abstract_syntax_uid: SOP_CLASS_STUDY_ROOT_GET,
            availability: command_availability(cfg!(feature = "dimse-c-get")),
            requires_pending_final_sequence: true,
        },
    ];

    DimseAssociationContract {
        called_ae_required: policy.called_ae.is_some(),
        max_pdu_length: policy.max_pdu_length,
        abstract_syntax_uids,
        transfer_syntax_uids: vec![
            TRANSFER_SYNTAX_IMPLICIT_VR_LE,
            TRANSFER_SYNTAX_EXPLICIT_VR_LE,
        ],
        command_contracts,
    }
}

fn command_availability(enabled: bool) -> DimseCommandAvailability {
    if enabled {
        DimseCommandAvailability::Active
    } else {
        DimseCommandAvailability::DisabledByFeature
    }
}

/// DIMSE service request metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssociationInfo {
    /// Called AE title.
    pub called_ae: String,
    /// Calling AE title.
    pub calling_ae: String,
    /// Presentation context ID.
    pub presentation_context_id: u8,
    /// Abstract syntax UID.
    pub abstract_syntax_uid: String,
    /// Transfer syntax UID.
    pub transfer_syntax_uid: String,
}

/// C-ECHO request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CEchoRequest {
    /// Message ID.
    pub message_id: u16,
    /// Association metadata.
    pub association: AssociationInfo,
}

/// C-STORE request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CStoreRequest {
    /// Message ID.
    pub message_id: u16,
    /// Affected SOP Class UID.
    pub sop_class_uid: String,
    /// Affected SOP Instance UID.
    pub sop_instance_uid: String,
    /// Priority value.
    pub priority: u16,
    /// Data set bytes.
    pub data_set: Vec<u8>,
    /// Association metadata.
    pub association: AssociationInfo,
}

/// C-FIND request.
#[cfg(feature = "dimse-c-find")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CFindRequest {
    /// Message ID.
    pub message_id: u16,
    /// Affected SOP Class UID.
    pub sop_class_uid: String,
    /// Priority value.
    pub priority: u16,
    /// Identifier data set bytes.
    pub data_set: Vec<u8>,
    /// Association metadata.
    pub association: AssociationInfo,
}

/// C-MOVE request.
#[cfg(feature = "dimse-c-move")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CMoveRequest {
    /// Message ID.
    pub message_id: u16,
    /// Affected SOP Class UID.
    pub sop_class_uid: String,
    /// Move destination AE title.
    pub move_destination: String,
    /// Priority value.
    pub priority: u16,
    /// Identifier data set bytes.
    pub data_set: Vec<u8>,
    /// Association metadata.
    pub association: AssociationInfo,
}

/// C-GET request.
#[cfg(feature = "dimse-c-get")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CGetRequest {
    /// Message ID.
    pub message_id: u16,
    /// Affected SOP Class UID.
    pub sop_class_uid: String,
    /// Priority value.
    pub priority: u16,
    /// Identifier data set bytes.
    pub data_set: Vec<u8>,
    /// Association metadata.
    pub association: AssociationInfo,
}

/// Storage Commitment N-ACTION request payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageCommitmentNActionRequest {
    /// Message ID.
    pub message_id: u16,
    /// Transaction UID for request lifecycle tracking.
    pub transaction_uid: String,
    /// Calling AE title.
    pub calling_ae_title: String,
    /// Called AE title.
    pub called_ae_title: String,
    /// Referenced SOP instances.
    pub referenced_instances: Vec<StorageCommitmentReferencedInstance>,
    /// Association metadata.
    pub association: AssociationInfo,
}

/// Storage Commitment lifecycle event forwarded to workflow/audit sinks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageCommitmentLifecycleEvent {
    /// Transaction UID.
    pub transaction_uid: String,
    /// Current state after transition.
    pub state: StorageCommitmentState,
    /// Deterministic event code.
    pub event_code: &'static str,
}

/// Callback sink for Storage Commitment lifecycle transitions.
pub type StorageCommitmentLifecycleSink =
    Arc<dyn Fn(StorageCommitmentLifecycleEvent) -> Result<()> + Send + Sync>;

/// Instance Availability Notification (IAN) payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IanNotification {
    /// Stable event id.
    pub event_id: String,
    /// Study Instance UID.
    pub study_instance_uid: String,
    /// SOP Instance UID.
    pub sop_instance_uid: String,
    /// Outcome status code.
    pub status_code: u16,
}

/// DIMSE service handler.
pub trait DimseService: Send {
    /// Handle C-ECHO requests.
    fn on_c_echo(&mut self, request: CEchoRequest) -> Result<DimseStatus>;
    /// Handle C-STORE requests.
    fn on_c_store(&mut self, request: CStoreRequest) -> Result<DimseStatus>;
    /// Handle C-FIND requests.
    #[cfg(feature = "dimse-c-find")]
    fn on_c_find(&mut self, _request: CFindRequest) -> Result<Vec<DimseStatus>> {
        Ok(vec![DimseStatus::sop_class_not_supported()])
    }
    /// Handle C-MOVE requests.
    #[cfg(feature = "dimse-c-move")]
    fn on_c_move(&mut self, _request: CMoveRequest) -> Result<Vec<DimseStatus>> {
        Ok(vec![DimseStatus::sop_class_not_supported()])
    }
    /// Handle C-GET requests.
    #[cfg(feature = "dimse-c-get")]
    fn on_c_get(&mut self, _request: CGetRequest) -> Result<Vec<DimseStatus>> {
        Ok(vec![DimseStatus::sop_class_not_supported()])
    }
    /// Handle Storage Commitment N-ACTION requests.
    fn on_storage_commitment_n_action(
        &mut self,
        _request: StorageCommitmentNActionRequest,
    ) -> Result<DimseStatus> {
        Ok(DimseStatus::sop_class_not_supported())
    }
}

/// DIMSE service implementation backed by deterministic storage/index/query state.
#[derive(Clone)]
pub struct StorageBackedDimseService {
    limits: Limits,
    storage: Storage,
    storage_commitment_sink: Option<StorageCommitmentLifecycleSink>,
    ian_seen_ids: BTreeSet<String>,
    ian_notifications: Vec<IanNotification>,
}

impl StorageBackedDimseService {
    /// Create an empty storage-backed service with the provided limits.
    pub fn new(limits: Limits) -> Self {
        Self {
            storage: Storage::new(limits.clone()),
            limits,
            storage_commitment_sink: None,
            ian_seen_ids: BTreeSet::new(),
            ian_notifications: Vec::new(),
        }
    }

    /// Open a storage-backed service with durable WAL persistence.
    pub fn open(limits: Limits, wal_path: impl AsRef<Path>) -> Result<Self> {
        let storage = Storage::open(limits.clone(), wal_path)?;
        Ok(Self {
            limits,
            storage,
            storage_commitment_sink: None,
            ian_seen_ids: BTreeSet::new(),
            ian_notifications: Vec::new(),
        })
    }

    /// Create a storage-backed service by replaying a write-ahead log.
    pub fn from_log(limits: Limits, log: WriteAheadLog) -> Result<Self> {
        let storage = Storage::from_log(limits.clone(), log)?;
        Ok(Self {
            limits,
            storage,
            storage_commitment_sink: None,
            ian_seen_ids: BTreeSet::new(),
            ian_notifications: Vec::new(),
        })
    }

    /// Borrow the underlying storage state.
    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    /// Consume the service and return the underlying storage state.
    pub fn into_storage(self) -> Storage {
        self.storage
    }

    /// Configure lifecycle sink for Storage Commitment transitions.
    pub fn set_storage_commitment_sink(&mut self, sink: Option<StorageCommitmentLifecycleSink>) {
        self.storage_commitment_sink = sink;
    }

    fn emit_storage_commitment_event(
        &self,
        transaction_uid: &str,
        state: StorageCommitmentState,
        event_code: &'static str,
    ) -> Result<()> {
        if let Some(sink) = &self.storage_commitment_sink {
            sink(StorageCommitmentLifecycleEvent {
                transaction_uid: transaction_uid.to_string(),
                state,
                event_code,
            })?;
        }
        Ok(())
    }

    /// Process queued Storage Commitment event-report jobs deterministically.
    pub fn process_storage_commitment_event_reports(&mut self) -> Result<usize> {
        let mut processed = 0usize;
        while let Some(job) = self.storage.pop_next_storage_commitment_event_job() {
            let transaction_uid = job.transaction_uid.clone();
            self.storage
                .complete_storage_commitment_event_job(job, true)?;
            if let Some(request) = self.storage.storage_commitment_request(&transaction_uid) {
                self.emit_storage_commitment_event(
                    &transaction_uid,
                    request.state,
                    "storage_commitment.report_delivered",
                )?;
            }
            processed = processed.saturating_add(1);
        }
        Ok(processed)
    }

    /// Produce deterministic IAN notification after workflow completion.
    pub fn produce_ian_notification(
        &mut self,
        event_id: String,
        study_instance_uid: String,
        sop_instance_uid: String,
        status_code: u16,
    ) -> bool {
        if !self.ian_seen_ids.insert(event_id.clone()) {
            return false;
        }
        self.ian_notifications.push(IanNotification {
            event_id,
            study_instance_uid,
            sop_instance_uid,
            status_code,
        });
        true
    }

    /// Consume inbound IAN notification with deduplication.
    pub fn consume_ian_notification(&mut self, notification: IanNotification) -> bool {
        if !self.ian_seen_ids.insert(notification.event_id.clone()) {
            return false;
        }
        self.ian_notifications.push(notification);
        true
    }

    /// Return IAN notifications in deterministic insertion order.
    pub fn ian_notifications(&self) -> &[IanNotification] {
        &self.ian_notifications
    }
}

impl DimseService for StorageBackedDimseService {
    fn on_c_echo(&mut self, _request: CEchoRequest) -> Result<DimseStatus> {
        Ok(DimseStatus::success())
    }

    fn on_c_store(&mut self, request: CStoreRequest) -> Result<DimseStatus> {
        let p10 = wrap_dataset_as_p10(&request.association.transfer_syntax_uid, &request.data_set);
        match self.storage.ingest_bytes(p10) {
            Ok(_) => Ok(DimseStatus::success()),
            Err(_) => Ok(DimseStatus::processing_failure()),
        }
    }

    fn on_storage_commitment_n_action(
        &mut self,
        request: StorageCommitmentNActionRequest,
    ) -> Result<DimseStatus> {
        let transaction_uid = request.transaction_uid.clone();
        let commitment = StorageCommitmentRequest {
            transaction_uid: request.transaction_uid,
            calling_ae_title: request.calling_ae_title,
            called_ae_title: request.called_ae_title,
            referenced_instances: request.referenced_instances,
            state: StorageCommitmentState::Requested,
        };
        match self.storage.register_storage_commitment_request(commitment) {
            Ok(()) => {
                let _ = self
                    .storage
                    .queue_storage_commitment_event_report(&transaction_uid, 3);
                if let Some(request) = self.storage.storage_commitment_request(&transaction_uid) {
                    self.emit_storage_commitment_event(
                        &transaction_uid,
                        request.state,
                        "storage_commitment.request_registered",
                    )?;
                }
                Ok(DimseStatus::success())
            }
            Err(_) => Ok(DimseStatus::processing_failure()),
        }
    }

    #[cfg(feature = "dimse-c-find")]
    fn on_c_find(&mut self, request: CFindRequest) -> Result<Vec<DimseStatus>> {
        let datasets = match self.storage.datasets() {
            Ok(datasets) => datasets,
            Err(_) => return Ok(vec![DimseStatus::processing_failure()]),
        };
        let matches = match query_matches_from_dimse(
            &request.data_set,
            &request.association.transfer_syntax_uid,
            &datasets,
            &self.limits,
        ) {
            Ok(matches) => matches,
            Err(_) => return Ok(vec![DimseStatus::processing_failure()]),
        };
        let mut statuses = vec![DimseStatus::pending(); matches.len()];
        statuses.push(DimseStatus::success());
        Ok(statuses)
    }

    #[cfg(feature = "dimse-c-move")]
    fn on_c_move(&mut self, request: CMoveRequest) -> Result<Vec<DimseStatus>> {
        let datasets = match self.storage.datasets() {
            Ok(datasets) => datasets,
            Err(_) => return Ok(vec![DimseStatus::processing_failure()]),
        };
        let matches = match query_matches_from_dimse(
            &request.data_set,
            &request.association.transfer_syntax_uid,
            &datasets,
            &self.limits,
        ) {
            Ok(matches) => matches,
            Err(_) => return Ok(vec![DimseStatus::processing_failure()]),
        };
        let mut statuses = vec![DimseStatus::pending(); matches.len()];
        statuses.push(DimseStatus::success());
        Ok(statuses)
    }

    #[cfg(feature = "dimse-c-get")]
    fn on_c_get(&mut self, request: CGetRequest) -> Result<Vec<DimseStatus>> {
        let datasets = match self.storage.datasets() {
            Ok(datasets) => datasets,
            Err(_) => return Ok(vec![DimseStatus::processing_failure()]),
        };
        let matches = match query_matches_from_dimse(
            &request.data_set,
            &request.association.transfer_syntax_uid,
            &datasets,
            &self.limits,
        ) {
            Ok(matches) => matches,
            Err(_) => return Ok(vec![DimseStatus::processing_failure()]),
        };
        let mut statuses = vec![DimseStatus::pending(); matches.len()];
        statuses.push(DimseStatus::success());
        Ok(statuses)
    }
}

/// Audit callback for authorization decisions.
pub type AuditCallback = Arc<dyn Fn(AuditEvent) -> Result<()> + Send + Sync>;

/// Authorization and audit configuration for DIMSE services.
#[derive(Clone)]
pub struct DimseAuthConfig {
    /// Authorization policy.
    pub authorizer: Arc<dyn Authorizer + Send + Sync>,
    /// Optional audit callback.
    pub audit: Option<AuditCallback>,
}

impl DimseAuthConfig {
    /// Build a config that allows all requests.
    pub fn allow_all() -> Self {
        Self {
            authorizer: Arc::new(AllowAll),
            audit: None,
        }
    }

    /// Build a config that denies all requests (fail closed).
    pub fn deny_all() -> Self {
        Self {
            authorizer: Arc::new(DenyAll::new(AuthDenyReason::Policy)),
            audit: None,
        }
    }
}

impl fmt::Debug for DimseAuthConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DimseAuthConfig")
            .field("authorizer", &"Authorizer")
            .field("audit", &self.audit.is_some())
            .finish()
    }
}

/// DIMSE server configuration.
#[derive(Debug, Clone)]
pub struct DimseServerConfig {
    /// Network limits (PDU/PDV/contexts).
    pub network_limits: NetworkLimits,
    /// DIMSE command limits.
    pub dimse_limits: DimseLimits,
    /// Dataset and core limits.
    pub limits: Limits,
    /// Read timeout.
    pub read_timeout: Duration,
    /// Write timeout.
    pub write_timeout: Duration,
    /// Maximum concurrent connections.
    pub max_connections: usize,
    /// TLS policy for incoming connections.
    pub tls_policy: TlsPolicy,
    /// Transport security state for incoming connections.
    pub transport_security: TransportSecurity,
    /// Association acceptance policy.
    pub policy: AssociationPolicy,
    /// Authorization and audit configuration.
    pub auth: DimseAuthConfig,
    /// DIMSE command role enablement.
    pub roles: DimseRoleConfig,
    /// Maximum data-set payload sizes by command.
    pub operation_sizes: DimseOperationSizeConfig,
    /// Per-operation resource limits.
    pub operation_resource_limits: DimseOperationResourceLimits,
    /// Optional host allowlist (exact peer IP addresses).
    pub allowed_hosts: Option<Vec<IpAddr>>,
    /// Optional TLS material contract.
    pub tls_material: Option<DimseTlsMaterialConfig>,
}

impl Default for DimseServerConfig {
    fn default() -> Self {
        Self {
            network_limits: NetworkLimits::default(),
            dimse_limits: DimseLimits::default(),
            limits: Limits::default(),
            read_timeout: Duration::from_secs(30),
            write_timeout: Duration::from_secs(30),
            max_connections: 64,
            tls_policy: TlsPolicy::RequireTls,
            transport_security: TransportSecurity::Insecure,
            policy: workstation_default_association_policy(),
            auth: DimseAuthConfig::deny_all(),
            roles: DimseRoleConfig::default(),
            operation_sizes: DimseOperationSizeConfig::with_default_bound(
                Limits::default().max_input_bytes,
            ),
            operation_resource_limits: DimseOperationResourceLimits::default(),
            allowed_hosts: None,
            tls_material: None,
        }
    }
}

#[derive(Clone)]
struct ConnectionLimiter {
    max_connections: usize,
    active: Arc<Mutex<usize>>,
}

impl ConnectionLimiter {
    fn new(max_connections: usize) -> Self {
        Self {
            max_connections,
            active: Arc::new(Mutex::new(0)),
        }
    }

    fn try_acquire(&self) -> Result<ConnectionGuard> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| decode_error("connection limiter lock poisoned"))?;
        let next = active.saturating_add(1);
        enforce_connection_limit(next, self.max_connections)?;
        *active = next;
        Ok(ConnectionGuard {
            active: Arc::clone(&self.active),
        })
    }
}

struct ConnectionGuard {
    active: Arc<Mutex<usize>>,
}

#[derive(Clone)]
struct OperationLimiter {
    max_in_flight_operations: usize,
    active: Arc<Mutex<usize>>,
}

impl OperationLimiter {
    fn new(max_in_flight_operations: usize) -> Self {
        Self {
            max_in_flight_operations,
            active: Arc::new(Mutex::new(0)),
        }
    }

    fn try_acquire(&self) -> Result<OperationGuard> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| decode_error("operation limiter lock poisoned"))?;
        let next = active.saturating_add(1);
        enforce_operation_limit(next, self.max_in_flight_operations)?;
        *active = next;
        Ok(OperationGuard {
            active: Arc::clone(&self.active),
        })
    }
}

struct OperationGuard {
    active: Arc<Mutex<usize>>,
}

impl Drop for OperationGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active.lock() {
            *active = active.saturating_sub(1);
        }
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active.lock() {
            *active = active.saturating_sub(1);
        }
    }
}

/// DIMSE server with SCP handling.
pub struct DimseServer<H: DimseService> {
    listener: TcpListener,
    config: DimseServerConfig,
    handler: Arc<Mutex<H>>,
    limiter: ConnectionLimiter,
    operation_limiter: OperationLimiter,
}

impl<H: DimseService + 'static> DimseServer<H> {
    /// Bind a DIMSE server to an address.
    pub fn bind(addr: SocketAddr, config: DimseServerConfig, handler: H) -> Result<Self> {
        let listener = TcpListener::bind(addr).map_err(io_error)?;
        let max_connections = config.max_connections;
        let max_in_flight_operations = config.operation_resource_limits.max_in_flight_operations;
        Ok(Self {
            listener,
            config,
            handler: Arc::new(Mutex::new(handler)),
            limiter: ConnectionLimiter::new(max_connections),
            operation_limiter: OperationLimiter::new(max_in_flight_operations),
        })
    }

    /// Return the local address for this server.
    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.listener.local_addr().map_err(io_error)
    }

    /// Run the server loop (blocking).
    pub fn run(&self) -> Result<()> {
        for stream in self.listener.incoming() {
            let stream = stream.map_err(io_error)?;
            if !is_allowed_peer(&stream, &self.config.allowed_hosts) {
                let _ = reject_peer(stream);
                continue;
            }
            let guard = match self.limiter.try_acquire() {
                Ok(guard) => guard,
                Err(_throttle_err) => {
                    let _ = reject_association(stream, &self.config, throttle_reject());
                    continue;
                }
            };
            let handler = Arc::clone(&self.handler);
            let config = self.config.clone();
            let operation_limiter = self.operation_limiter.clone();
            thread::spawn(move || {
                let _guard = guard;
                let _ = handle_connection(stream, &config, handler, operation_limiter);
            });
        }
        Ok(())
    }

    /// Run a single-connection server loop (useful for tests).
    pub fn run_once(&self) -> Result<()> {
        let (stream, _) = self.listener.accept().map_err(io_error)?;
        if !is_allowed_peer(&stream, &self.config.allowed_hosts) {
            let _ = reject_peer(stream);
            return Err(decode_error("peer address rejected by host allowlist"));
        }
        let guard = match self.limiter.try_acquire() {
            Ok(guard) => guard,
            Err(err) => {
                let _ = reject_association(stream, &self.config, throttle_reject());
                return Err(err);
            }
        };
        let result = handle_connection(
            stream,
            &self.config,
            Arc::clone(&self.handler),
            self.operation_limiter.clone(),
        );
        drop(guard);
        result
    }
}

/// DIMSE client configuration.
#[derive(Debug, Clone)]
pub struct DimseClientConfig {
    /// Network limits.
    pub network_limits: NetworkLimits,
    /// DIMSE command limits.
    pub dimse_limits: DimseLimits,
    /// Read timeout.
    pub read_timeout: Duration,
    /// Write timeout.
    pub write_timeout: Duration,
    /// TLS policy for outgoing connections.
    pub tls_policy: TlsPolicy,
    /// Transport security state for outgoing connections.
    pub transport_security: TransportSecurity,
}

impl Default for DimseClientConfig {
    fn default() -> Self {
        Self {
            network_limits: NetworkLimits::default(),
            dimse_limits: DimseLimits::default(),
            read_timeout: Duration::from_secs(30),
            write_timeout: Duration::from_secs(30),
            tls_policy: TlsPolicy::AllowInsecure,
            transport_security: TransportSecurity::Insecure,
        }
    }
}

/// DIMSE SCU client.
pub struct DimseClient {
    stream: TcpStream,
    config: DimseClientConfig,
    association: AssociationAccept,
    context_map: HashMap<String, u8>,
}

impl DimseClient {
    /// Connect and negotiate an association using a policy.
    pub fn connect(
        addr: SocketAddr,
        called_ae: &str,
        calling_ae: &str,
        policy: &AssociationPolicy,
        config: DimseClientConfig,
    ) -> Result<Self> {
        enforce_tls_policy(config.transport_security, config.tls_policy)?;
        let mut stream = TcpStream::connect(addr).map_err(io_error)?;
        stream
            .set_read_timeout(Some(config.read_timeout))
            .map_err(io_error)?;
        stream
            .set_write_timeout(Some(config.write_timeout))
            .map_err(io_error)?;

        let request = build_associate_request(called_ae, calling_ae, policy)?;
        send_pdu(
            &mut stream,
            &Pdu::AssociateRq(request.clone()),
            &config.network_limits,
        )?;
        let response = read_pdu(&mut stream, &config.network_limits)?;
        let association = match response {
            Pdu::AssociateAc(ac) => ac,
            Pdu::AssociateRj(_) => {
                return Err(decode_error("association rejected"));
            }
            _ => return Err(decode_error("unexpected PDU during association")),
        };

        let context_map = build_client_context_map(&request, &association);

        Ok(Self {
            stream,
            config,
            association,
            context_map,
        })
    }

    /// Send a C-ECHO request and wait for a response.
    pub fn c_echo(&mut self, message_id: u16) -> Result<DimseStatus> {
        let pcid = self.presentation_context_for("1.2.840.10008.1.1")?;
        let command = build_c_echo_request(message_id);
        let pdus = encode_message_pdus(
            pcid,
            &command,
            None,
            &self.config.network_limits,
            self.association.max_pdu_length,
        )?;
        for pdu in pdus {
            send_pdu(&mut self.stream, &pdu, &self.config.network_limits)?;
        }
        let response = read_dimse_response(&mut self.stream, &self.config)?;
        match response {
            DimseMessage::CEchoRsp { status, .. } => Ok(DimseStatus { code: status }),
            _ => Err(decode_error("unexpected response to C-ECHO")),
        }
    }

    /// Send a C-STORE request and wait for a response.
    pub fn c_store(
        &mut self,
        message_id: u16,
        sop_class_uid: &str,
        sop_instance_uid: &str,
        data_set: &[u8],
    ) -> Result<DimseStatus> {
        let pcid = self.presentation_context_for(sop_class_uid)?;
        let command = build_c_store_request(message_id, sop_class_uid, sop_instance_uid, 0);
        let pdus = encode_message_pdus(
            pcid,
            &command,
            Some(data_set),
            &self.config.network_limits,
            self.association.max_pdu_length,
        )?;
        for pdu in pdus {
            send_pdu(&mut self.stream, &pdu, &self.config.network_limits)?;
        }
        let response = read_dimse_response(&mut self.stream, &self.config)?;
        match response {
            DimseMessage::CStoreRsp { status, .. } => Ok(DimseStatus { code: status }),
            _ => Err(decode_error("unexpected response to C-STORE")),
        }
    }

    /// Send a C-FIND request and wait for a response.
    #[cfg(feature = "dimse-c-find")]
    pub fn c_find(
        &mut self,
        message_id: u16,
        sop_class_uid: &str,
        priority: u16,
        data_set: &[u8],
    ) -> Result<DimseStatus> {
        let pcid = self.presentation_context_for(sop_class_uid)?;
        let command = build_c_find_request(message_id, sop_class_uid, priority);
        let pdus = encode_message_pdus(
            pcid,
            &command,
            Some(data_set),
            &self.config.network_limits,
            self.association.max_pdu_length,
        )?;
        for pdu in pdus {
            send_pdu(&mut self.stream, &pdu, &self.config.network_limits)?;
        }
        let mut responses_seen = 0usize;
        loop {
            let response = read_dimse_response(&mut self.stream, &self.config)?;
            match response {
                DimseMessage::CFindRsp {
                    status,
                    command_data_set_type,
                    ..
                } => {
                    responses_seen = responses_seen.saturating_add(1);
                    enforce_query_response_limit(responses_seen, DEFAULT_MAX_QUERY_RESPONSE_COUNT)?;
                    if command_data_set_type != 0x0101 {
                        return Err(decode_error("unexpected data set in C-FIND response"));
                    }
                    if is_pending_status(status) {
                        continue;
                    }
                    return Ok(DimseStatus { code: status });
                }
                _ => return Err(decode_error("unexpected response to C-FIND")),
            }
        }
    }

    /// Send a C-MOVE request and wait for a response.
    #[cfg(feature = "dimse-c-move")]
    pub fn c_move(
        &mut self,
        message_id: u16,
        sop_class_uid: &str,
        move_destination: &str,
        priority: u16,
        data_set: &[u8],
    ) -> Result<DimseStatus> {
        let pcid = self.presentation_context_for(sop_class_uid)?;
        let command = build_c_move_request(message_id, sop_class_uid, move_destination, priority);
        let pdus = encode_message_pdus(
            pcid,
            &command,
            Some(data_set),
            &self.config.network_limits,
            self.association.max_pdu_length,
        )?;
        for pdu in pdus {
            send_pdu(&mut self.stream, &pdu, &self.config.network_limits)?;
        }
        let mut responses_seen = 0usize;
        loop {
            let response = read_dimse_response(&mut self.stream, &self.config)?;
            match response {
                DimseMessage::CMoveRsp {
                    status,
                    command_data_set_type,
                    ..
                } => {
                    responses_seen = responses_seen.saturating_add(1);
                    enforce_query_response_limit(responses_seen, DEFAULT_MAX_QUERY_RESPONSE_COUNT)?;
                    if command_data_set_type != 0x0101 {
                        return Err(decode_error("unexpected data set in C-MOVE response"));
                    }
                    if is_pending_status(status) {
                        continue;
                    }
                    return Ok(DimseStatus { code: status });
                }
                _ => return Err(decode_error("unexpected response to C-MOVE")),
            }
        }
    }

    /// Send a C-GET request and wait for a response.
    #[cfg(feature = "dimse-c-get")]
    pub fn c_get(
        &mut self,
        message_id: u16,
        sop_class_uid: &str,
        priority: u16,
        data_set: &[u8],
    ) -> Result<DimseStatus> {
        let pcid = self.presentation_context_for(sop_class_uid)?;
        let command = build_c_get_request(message_id, sop_class_uid, priority);
        let pdus = encode_message_pdus(
            pcid,
            &command,
            Some(data_set),
            &self.config.network_limits,
            self.association.max_pdu_length,
        )?;
        for pdu in pdus {
            send_pdu(&mut self.stream, &pdu, &self.config.network_limits)?;
        }
        let mut responses_seen = 0usize;
        loop {
            let response = read_dimse_response(&mut self.stream, &self.config)?;
            match response {
                DimseMessage::CGetRsp {
                    status,
                    command_data_set_type,
                    ..
                } => {
                    responses_seen = responses_seen.saturating_add(1);
                    enforce_query_response_limit(responses_seen, DEFAULT_MAX_QUERY_RESPONSE_COUNT)?;
                    if command_data_set_type != 0x0101 {
                        return Err(decode_error("unexpected data set in C-GET response"));
                    }
                    if is_pending_status(status) {
                        continue;
                    }
                    return Ok(DimseStatus { code: status });
                }
                _ => return Err(decode_error("unexpected response to C-GET")),
            }
        }
    }

    /// Release the association gracefully.
    pub fn release(&mut self) -> Result<()> {
        send_pdu(
            &mut self.stream,
            &Pdu::ReleaseRq,
            &self.config.network_limits,
        )?;
        let response = read_pdu(&mut self.stream, &self.config.network_limits)?;
        match response {
            Pdu::ReleaseRp => Ok(()),
            _ => Err(decode_error("unexpected PDU during release")),
        }
    }

    fn presentation_context_for(&self, abstract_syntax: &str) -> Result<u8> {
        self.context_map
            .get(abstract_syntax)
            .copied()
            .ok_or_else(|| decode_error("missing presentation context for abstract syntax"))
    }
}

fn handle_connection<H: DimseService>(
    mut stream: TcpStream,
    config: &DimseServerConfig,
    handler: Arc<Mutex<H>>,
    operation_limiter: OperationLimiter,
) -> Result<()> {
    stream
        .set_read_timeout(Some(config.read_timeout))
        .map_err(io_error)?;
    stream
        .set_write_timeout(Some(config.write_timeout))
        .map_err(io_error)?;

    let first = read_pdu(&mut stream, &config.network_limits)?;
    let association_request = match first {
        Pdu::AssociateRq(req) => req,
        _ => {
            return Err(decode_error("expected association request"));
        }
    };

    if let Err(err) = enforce_tls_policy(config.transport_security, config.tls_policy) {
        send_pdu(
            &mut stream,
            &Pdu::AssociateRj(tls_reject()),
            &config.network_limits,
        )?;
        return Err(err);
    }

    if let Err(err) = authorize_dimse_association(&config.auth, &association_request) {
        send_pdu(
            &mut stream,
            &Pdu::AssociateRj(auth_reject()),
            &config.network_limits,
        )?;
        return Err(err);
    }

    let policy = config.effective_policy();
    let association_accept =
        match accept_association(&association_request, &policy, &config.network_limits) {
            Ok(ac) => ac,
            Err(_) => {
                let reject = AssociationReject {
                    result: 0x01,
                    source: 0x01,
                    reason: 0x01,
                };
                send_pdu(
                    &mut stream,
                    &Pdu::AssociateRj(reject),
                    &config.network_limits,
                )?;
                return Err(decode_error("association rejected"));
            }
        };

    send_pdu(
        &mut stream,
        &Pdu::AssociateAc(association_accept.clone()),
        &config.network_limits,
    )?;

    let context_map = build_server_context_map(&association_request, &association_accept);
    let mut pending: HashMap<u8, PendingMessage> = HashMap::new();

    loop {
        let pdu = read_pdu(&mut stream, &config.network_limits)?;
        match pdu {
            Pdu::PDataTf(pdvs) => {
                handle_pdus(
                    pdvs,
                    &association_accept,
                    &context_map,
                    &mut pending,
                    config,
                    &handler,
                    &operation_limiter,
                    &mut stream,
                )?;
            }
            Pdu::ReleaseRq => {
                send_pdu(&mut stream, &Pdu::ReleaseRp, &config.network_limits)?;
                break;
            }
            Pdu::Abort(_) => break,
            _ => return Err(decode_error("unexpected PDU during association")),
        }
    }
    Ok(())
}

impl DimseServerConfig {
    fn effective_policy(&self) -> AssociationPolicy {
        let mut supported_abstract_syntaxes = self.policy.supported_abstract_syntaxes.clone();

        if !self.roles.c_echo_enabled {
            supported_abstract_syntaxes.retain(|uid| uid != SOP_CLASS_VERIFICATION);
        }
        if !self.roles.c_store_enabled {
            supported_abstract_syntaxes.retain(|uid| {
                uid != SOP_CLASS_CT_IMAGE_STORAGE
                    && uid != SOP_CLASS_MR_IMAGE_STORAGE
                    && uid != SOP_CLASS_SECONDARY_CAPTURE
                    && uid != SOP_CLASS_MULTI_FRAME_SC_BYTE
                    && uid != SOP_CLASS_MULTI_FRAME_SC_WORD
                    && uid != SOP_CLASS_MULTI_FRAME_SC_TRUE_COLOR
                    && uid != SOP_CLASS_PET_IMAGE_STORAGE
                    && uid != SOP_CLASS_CR_IMAGE_STORAGE
                    && uid != SOP_CLASS_DX_PRESENTATION
            });
        }
        if !self.roles.c_find_enabled {
            supported_abstract_syntaxes.retain(|uid| uid != SOP_CLASS_STUDY_ROOT_FIND);
        }
        if !self.roles.c_move_enabled {
            supported_abstract_syntaxes.retain(|uid| uid != SOP_CLASS_STUDY_ROOT_MOVE);
        }
        if !self.roles.c_get_enabled {
            supported_abstract_syntaxes.retain(|uid| uid != SOP_CLASS_STUDY_ROOT_GET);
        }

        AssociationPolicy {
            called_ae: self.policy.called_ae.clone(),
            supported_abstract_syntaxes,
            supported_transfer_syntaxes: self.policy.supported_transfer_syntaxes.clone(),
            max_pdu_length: self.policy.max_pdu_length,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_pdus<H: DimseService>(
    pdvs: Vec<Pdv>,
    association_accept: &AssociationAccept,
    context_map: &HashMap<u8, (String, String)>,
    pending: &mut HashMap<u8, PendingMessage>,
    config: &DimseServerConfig,
    handler: &Arc<Mutex<H>>,
    operation_limiter: &OperationLimiter,
    stream: &mut TcpStream,
) -> Result<()> {
    for pdv in pdvs {
        let entry = pending.entry(pdv.presentation_context_id).or_default();
        if pdv.is_command() {
            let new_len = entry
                .command
                .len()
                .checked_add(pdv.data.len())
                .ok_or_else(|| decode_error("command length overflow"))?;
            if new_len as u64 > config.dimse_limits.max_command_bytes {
                return Err(limit_exceeded(
                    "max_command_bytes",
                    new_len as u64,
                    config.dimse_limits.max_command_bytes,
                ));
            }
            entry.command.extend_from_slice(&pdv.data);
            if pdv.is_last() {
                entry.command_done = true;
            }
        } else {
            let new_len = entry
                .data
                .len()
                .checked_add(pdv.data.len())
                .ok_or_else(|| decode_error("data set length overflow"))?;
            if new_len as u64 > config.limits.max_input_bytes {
                return Err(limit_exceeded(
                    "max_input_bytes",
                    new_len as u64,
                    config.limits.max_input_bytes,
                ));
            }
            entry.data.extend_from_slice(&pdv.data);
            if pdv.is_last() {
                entry.data_done = true;
            }
        }

        if entry.command_done && entry.message.is_none() {
            let msg = parse_command_set(&entry.command, &config.dimse_limits)?;
            entry.message = Some(msg);
        }

        if let Some(message) = entry.message.clone() {
            let needs_data = message_needs_data(&message);
            if needs_data && !entry.data_done {
                continue;
            }
            if !needs_data && !entry.data.is_empty() {
                return Err(decode_error("unexpected data set for command"));
            }
            handle_message(
                message,
                entry.data.clone(),
                association_accept,
                context_map,
                config,
                handler,
                operation_limiter,
                stream,
            )?;
            pending.remove(&pdv.presentation_context_id);
        }
    }
    Ok(())
}

fn message_needs_data(message: &DimseMessage) -> bool {
    match message {
        DimseMessage::CStoreRq { .. } => true,
        DimseMessage::NActionRq { .. } => true,
        #[cfg(feature = "dimse-c-find")]
        DimseMessage::CFindRq { .. } => true,
        #[cfg(feature = "dimse-c-move")]
        DimseMessage::CMoveRq { .. } => true,
        #[cfg(feature = "dimse-c-get")]
        DimseMessage::CGetRq { .. } => true,
        _ => false,
    }
}

fn message_is_enabled(message: &DimseMessage, roles: &DimseRoleConfig) -> bool {
    match message {
        DimseMessage::CEchoRq { .. } => roles.c_echo_enabled,
        DimseMessage::CStoreRq { .. } => roles.c_store_enabled,
        DimseMessage::NActionRq { .. } => roles.c_store_enabled,
        #[cfg(feature = "dimse-c-find")]
        DimseMessage::CFindRq { .. } => roles.c_find_enabled,
        #[cfg(feature = "dimse-c-move")]
        DimseMessage::CMoveRq { .. } => roles.c_move_enabled,
        #[cfg(feature = "dimse-c-get")]
        DimseMessage::CGetRq { .. } => roles.c_get_enabled,
        _ => false,
    }
}

fn message_data_set_limit(
    message: &DimseMessage,
    limits: &DimseOperationSizeConfig,
) -> Option<u64> {
    match message {
        DimseMessage::CStoreRq { .. } => Some(limits.c_store_data_set_bytes),
        DimseMessage::NActionRq { .. } => Some(limits.c_store_data_set_bytes),
        #[cfg(feature = "dimse-c-find")]
        DimseMessage::CFindRq { .. } => Some(limits.c_find_data_set_bytes),
        #[cfg(feature = "dimse-c-move")]
        DimseMessage::CMoveRq { .. } => Some(limits.c_move_data_set_bytes),
        #[cfg(feature = "dimse-c-get")]
        DimseMessage::CGetRq { .. } => Some(limits.c_get_data_set_bytes),
        DimseMessage::CEchoRq { .. } => Some(limits.c_echo_data_set_bytes),
        _ => None,
    }
}

fn validate_data_set_size(
    message: &DimseMessage,
    data_set: &[u8],
    limits: &DimseOperationSizeConfig,
) -> Result<()> {
    if !message_needs_data(message) {
        return Ok(());
    }
    let max_input_bytes = message_data_set_limit(message, limits)
        .ok_or_else(|| decode_error("unsupported operation size limit"))?;
    if data_set.len() as u64 > max_input_bytes {
        return Err(limit_exceeded(
            "max_input_bytes",
            data_set.len() as u64,
            max_input_bytes,
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn handle_message<H: DimseService>(
    message: DimseMessage,
    data_set: Vec<u8>,
    association_accept: &AssociationAccept,
    context_map: &HashMap<u8, (String, String)>,
    config: &DimseServerConfig,
    handler: &Arc<Mutex<H>>,
    operation_limiter: &OperationLimiter,
    stream: &mut TcpStream,
) -> Result<()> {
    let (operation, message_id, sop_class_uid, sop_instance_uid) = match &message {
        DimseMessage::CEchoRq { message_id, .. } => {
            ("c_echo", *message_id, SOP_CLASS_VERIFICATION, "")
        }
        DimseMessage::CStoreRq {
            message_id,
            sop_class_uid,
            sop_instance_uid,
            ..
        } => (
            "c_store",
            *message_id,
            sop_class_uid.as_str(),
            sop_instance_uid.as_str(),
        ),
        DimseMessage::NActionRq {
            message_id,
            sop_class_uid,
            sop_instance_uid,
            ..
        } => (
            "n_action",
            *message_id,
            sop_class_uid.as_str(),
            sop_instance_uid.as_str(),
        ),
        #[cfg(feature = "dimse-c-find")]
        DimseMessage::CFindRq {
            message_id,
            sop_class_uid,
            ..
        } => ("c_find", *message_id, sop_class_uid.as_str(), ""),
        #[cfg(feature = "dimse-c-move")]
        DimseMessage::CMoveRq {
            message_id,
            sop_class_uid,
            ..
        } => ("c_move", *message_id, sop_class_uid.as_str(), ""),
        #[cfg(feature = "dimse-c-get")]
        DimseMessage::CGetRq {
            message_id,
            sop_class_uid,
            ..
        } => ("c_get", *message_id, sop_class_uid.as_str(), ""),
        _ => return Err(decode_error("unsupported DIMSE command")),
    };

    let association = build_association_info(association_accept, context_map, sop_class_uid)?;
    let operation_id = format!("{operation}|msg={message_id}");
    let _operation_guard = operation_limiter.try_acquire()?;
    record_dimse_operation_audit(
        &config.auth.audit,
        operation,
        &operation_id,
        &association,
        u32::from(message_id),
        None,
        if !sop_instance_uid.is_empty() {
            Some(sop_instance_uid)
        } else {
            None
        },
        None,
        DimseOperationAuditPhase::Start,
    )?;

    if !message_is_enabled(&message, &config.roles) {
        let status = DimseStatus::sop_class_not_supported();
        let response = match &message {
            DimseMessage::CEchoRq { message_id, .. } => {
                build_c_echo_response(*message_id, status.code)
            }
            DimseMessage::CStoreRq {
                message_id,
                sop_class_uid,
                sop_instance_uid,
                ..
            } => build_c_store_response(
                *message_id,
                status.code,
                sop_class_uid.as_str(),
                sop_instance_uid.as_str(),
            ),
            DimseMessage::NActionRq {
                message_id,
                sop_class_uid,
                sop_instance_uid,
                action_type_id,
                ..
            } => build_n_action_response(
                *message_id,
                status.code,
                sop_class_uid.as_str(),
                sop_instance_uid.as_str(),
                *action_type_id,
                0x0101,
            ),
            #[cfg(feature = "dimse-c-find")]
            DimseMessage::CFindRq {
                message_id,
                sop_class_uid,
                ..
            } => build_c_find_response(*message_id, status.code, sop_class_uid, 0x0101),
            #[cfg(feature = "dimse-c-move")]
            DimseMessage::CMoveRq {
                message_id,
                sop_class_uid,
                ..
            } => build_c_move_response(*message_id, status.code, sop_class_uid, 0x0101),
            #[cfg(feature = "dimse-c-get")]
            DimseMessage::CGetRq {
                message_id,
                sop_class_uid,
                ..
            } => build_c_get_response(*message_id, status.code, sop_class_uid, 0x0101),
            _ => return Err(decode_error("unsupported DIMSE command")),
        };
        if let Err(err) = send_dimse_response(
            stream,
            association.presentation_context_id,
            &response,
            association_accept.max_pdu_length,
            config,
        ) {
            record_dimse_operation_audit(
                &config.auth.audit,
                operation,
                &operation_id,
                &association,
                u32::from(message_id),
                Some(DimseStatus::processing_failure().code),
                if !sop_instance_uid.is_empty() {
                    Some(sop_instance_uid)
                } else {
                    None
                },
                None,
                DimseOperationAuditPhase::Failure,
            )?;
            return Err(err);
        }
        return record_dimse_operation_audit(
            &config.auth.audit,
            operation,
            &operation_id,
            &association,
            u32::from(message_id),
            Some(status.code),
            if !sop_instance_uid.is_empty() {
                Some(sop_instance_uid)
            } else {
                None
            },
            None,
            DimseOperationAuditPhase::Failure,
        );
    }

    if let Err(err) = validate_data_set_size(&message, &data_set, &config.operation_sizes) {
        let status = dimse_protocol_violation_status(&err);
        record_dimse_operation_audit(
            &config.auth.audit,
            operation,
            &operation_id,
            &association,
            u32::from(message_id),
            Some(status.code),
            if !sop_instance_uid.is_empty() {
                Some(sop_instance_uid)
            } else {
                None
            },
            None,
            DimseOperationAuditPhase::Failure,
        )?;
        return Err(err);
    }

    match message {
        DimseMessage::CEchoRq { message_id, .. } => {
            let status = match authorize_dimse_request(
                &config.auth,
                AuthAction::Echo,
                &association,
                Some("1.2.840.10008.1.1"),
                None,
            ) {
                Ok(()) => handler
                    .lock()
                    .map_err(|_| decode_error("handler lock poisoned"))?
                    .on_c_echo(CEchoRequest {
                        message_id,
                        association: association.clone(),
                    })?,
                Err(err) => dimse_protocol_violation_status(&err),
            };
            let response = build_c_echo_response(message_id, status.code);
            if let Err(err) = send_dimse_response(
                stream,
                association.presentation_context_id,
                &response,
                association_accept.max_pdu_length,
                config,
            ) {
                record_dimse_operation_audit(
                    &config.auth.audit,
                    operation,
                    &operation_id,
                    &association,
                    u32::from(message_id),
                    Some(DimseStatus::processing_failure().code),
                    None,
                    None,
                    DimseOperationAuditPhase::Failure,
                )?;
                return Err(err);
            }
            let phase = if status == DimseStatus::success() {
                DimseOperationAuditPhase::Completion
            } else {
                DimseOperationAuditPhase::Failure
            };
            record_dimse_operation_audit(
                &config.auth.audit,
                operation,
                &operation_id,
                &association,
                u32::from(message_id),
                Some(status.code),
                None,
                None,
                phase,
            )
        }
        DimseMessage::CStoreRq {
            message_id,
            sop_class_uid,
            sop_instance_uid,
            priority,
            ..
        } => {
            let status = match authorize_dimse_request(
                &config.auth,
                AuthAction::Store,
                &association,
                Some(&sop_class_uid),
                Some(&sop_instance_uid),
            ) {
                Ok(()) => handler
                    .lock()
                    .map_err(|_| decode_error("handler lock poisoned"))?
                    .on_c_store(CStoreRequest {
                        message_id,
                        sop_class_uid: sop_class_uid.clone(),
                        sop_instance_uid: sop_instance_uid.clone(),
                        priority,
                        data_set,
                        association: association.clone(),
                    })?,
                Err(err) => dimse_protocol_violation_status(&err),
            };
            let response =
                build_c_store_response(message_id, status.code, &sop_class_uid, &sop_instance_uid);
            if let Err(err) = send_dimse_response(
                stream,
                association.presentation_context_id,
                &response,
                association_accept.max_pdu_length,
                config,
            ) {
                record_dimse_operation_audit(
                    &config.auth.audit,
                    operation,
                    &operation_id,
                    &association,
                    u32::from(message_id),
                    Some(DimseStatus::processing_failure().code),
                    Some(&sop_instance_uid),
                    None,
                    DimseOperationAuditPhase::Failure,
                )?;
                return Err(err);
            }
            let phase = if status == DimseStatus::success() {
                DimseOperationAuditPhase::Completion
            } else {
                DimseOperationAuditPhase::Failure
            };
            record_dimse_operation_audit(
                &config.auth.audit,
                operation,
                &operation_id,
                &association,
                u32::from(message_id),
                Some(status.code),
                Some(&sop_instance_uid),
                None,
                phase,
            )
        }
        DimseMessage::NActionRq {
            message_id,
            sop_class_uid,
            sop_instance_uid,
            action_type_id,
            ..
        } => {
            let status = match authorize_dimse_request(
                &config.auth,
                AuthAction::Store,
                &association,
                Some(&sop_class_uid),
                Some(&sop_instance_uid),
            ) {
                Ok(()) => match parse_storage_commitment_n_action_request(
                    message_id,
                    &sop_class_uid,
                    &sop_instance_uid,
                    &data_set,
                    &association,
                    &config.limits,
                ) {
                    Ok(request) => handler
                        .lock()
                        .map_err(|_| decode_error("handler lock poisoned"))?
                        .on_storage_commitment_n_action(request)
                        .unwrap_or_else(|err| dimse_protocol_violation_status(&err)),
                    Err(err) => dimse_protocol_violation_status(&err),
                },
                Err(err) => dimse_protocol_violation_status(&err),
            };
            let response = build_n_action_response(
                message_id,
                status.code,
                &sop_class_uid,
                &sop_instance_uid,
                action_type_id,
                0x0101,
            );
            if let Err(err) = send_dimse_response(
                stream,
                association.presentation_context_id,
                &response,
                association_accept.max_pdu_length,
                config,
            ) {
                record_dimse_operation_audit(
                    &config.auth.audit,
                    operation,
                    &operation_id,
                    &association,
                    u32::from(message_id),
                    Some(DimseStatus::processing_failure().code),
                    Some(&sop_instance_uid),
                    None,
                    DimseOperationAuditPhase::Failure,
                )?;
                return Err(err);
            }
            let phase = if status == DimseStatus::success() {
                DimseOperationAuditPhase::Completion
            } else {
                DimseOperationAuditPhase::Failure
            };
            record_dimse_operation_audit(
                &config.auth.audit,
                operation,
                &operation_id,
                &association,
                u32::from(message_id),
                Some(status.code),
                Some(&sop_instance_uid),
                None,
                phase,
            )
        }
        #[cfg(feature = "dimse-c-find")]
        DimseMessage::CFindRq {
            message_id,
            sop_class_uid,
            priority,
            ..
        } => {
            let statuses = match authorize_dimse_request(
                &config.auth,
                AuthAction::Query,
                &association,
                Some(&sop_class_uid),
                None,
            ) {
                Ok(()) => {
                    let statuses = handler
                        .lock()
                        .map_err(|_| decode_error("handler lock poisoned"))?
                        .on_c_find(CFindRequest {
                            message_id,
                            sop_class_uid: sop_class_uid.clone(),
                            priority,
                            data_set,
                            association: association.clone(),
                        })
                        .unwrap_or_else(|err| vec![dimse_protocol_violation_status(&err)])
                        .into_iter()
                        .collect::<Vec<_>>();
                    match validate_query_status_sequence(&statuses) {
                        Ok(()) => statuses,
                        Err(err) => vec![dimse_protocol_violation_status(&err)],
                    }
                }
                Err(err) => vec![dimse_protocol_violation_status(&err)],
            };
            let response_count = statuses.len();
            enforce_query_response_limit(
                response_count,
                config.operation_resource_limits.max_query_responses,
            )?;
            for status in &statuses {
                record_dimse_operation_audit(
                    &config.auth.audit,
                    operation,
                    &operation_id,
                    &association,
                    u32::from(message_id),
                    Some(status.code),
                    None,
                    Some(response_count),
                    DimseOperationAuditPhase::Response,
                )?;
                let response =
                    build_c_find_response(message_id, status.code, &sop_class_uid, 0x0101);
                if let Err(err) = send_dimse_response(
                    stream,
                    association.presentation_context_id,
                    &response,
                    association_accept.max_pdu_length,
                    config,
                ) {
                    record_dimse_operation_audit(
                        &config.auth.audit,
                        operation,
                        &operation_id,
                        &association,
                        u32::from(message_id),
                        Some(DimseStatus::processing_failure().code),
                        None,
                        Some(response_count),
                        DimseOperationAuditPhase::Failure,
                    )?;
                    return Err(err);
                }
            }
            let final_status = statuses.last().ok_or_else(|| {
                decode_error("query/retrieve handlers must return at least one response")
            })?;
            let phase = if final_status == &DimseStatus::success() {
                DimseOperationAuditPhase::Completion
            } else {
                DimseOperationAuditPhase::Failure
            };
            record_dimse_operation_audit(
                &config.auth.audit,
                operation,
                &operation_id,
                &association,
                u32::from(message_id),
                Some(final_status.code),
                None,
                Some(response_count),
                phase,
            )?;
            Ok(())
        }
        #[cfg(feature = "dimse-c-move")]
        DimseMessage::CMoveRq {
            message_id,
            sop_class_uid,
            move_destination,
            priority,
            ..
        } => {
            let statuses = match authorize_dimse_request(
                &config.auth,
                AuthAction::Retrieve,
                &association,
                Some(&sop_class_uid),
                None,
            ) {
                Ok(()) => {
                    let statuses = handler
                        .lock()
                        .map_err(|_| decode_error("handler lock poisoned"))?
                        .on_c_move(CMoveRequest {
                            message_id,
                            sop_class_uid: sop_class_uid.clone(),
                            move_destination,
                            priority,
                            data_set,
                            association: association.clone(),
                        })
                        .unwrap_or_else(|err| vec![dimse_protocol_violation_status(&err)])
                        .into_iter()
                        .collect::<Vec<_>>();
                    match validate_query_status_sequence(&statuses) {
                        Ok(()) => statuses,
                        Err(err) => vec![dimse_protocol_violation_status(&err)],
                    }
                }
                Err(err) => vec![dimse_protocol_violation_status(&err)],
            };
            let response_count = statuses.len();
            enforce_query_response_limit(
                response_count,
                config.operation_resource_limits.max_query_responses,
            )?;
            for status in &statuses {
                record_dimse_operation_audit(
                    &config.auth.audit,
                    operation,
                    &operation_id,
                    &association,
                    u32::from(message_id),
                    Some(status.code),
                    None,
                    Some(response_count),
                    DimseOperationAuditPhase::Response,
                )?;
                let response =
                    build_c_move_response(message_id, status.code, &sop_class_uid, 0x0101);
                if let Err(err) = send_dimse_response(
                    stream,
                    association.presentation_context_id,
                    &response,
                    association_accept.max_pdu_length,
                    config,
                ) {
                    record_dimse_operation_audit(
                        &config.auth.audit,
                        operation,
                        &operation_id,
                        &association,
                        u32::from(message_id),
                        Some(DimseStatus::processing_failure().code),
                        None,
                        Some(response_count),
                        DimseOperationAuditPhase::Failure,
                    )?;
                    return Err(err);
                }
            }
            let final_status = statuses.last().ok_or_else(|| {
                decode_error("query/retrieve handlers must return at least one response")
            })?;
            let phase = if final_status == &DimseStatus::success() {
                DimseOperationAuditPhase::Completion
            } else {
                DimseOperationAuditPhase::Failure
            };
            record_dimse_operation_audit(
                &config.auth.audit,
                operation,
                &operation_id,
                &association,
                u32::from(message_id),
                Some(final_status.code),
                None,
                Some(response_count),
                phase,
            )?;
            Ok(())
        }
        #[cfg(feature = "dimse-c-get")]
        DimseMessage::CGetRq {
            message_id,
            sop_class_uid,
            priority,
            ..
        } => {
            let statuses = match authorize_dimse_request(
                &config.auth,
                AuthAction::Retrieve,
                &association,
                Some(&sop_class_uid),
                None,
            ) {
                Ok(()) => {
                    let statuses = handler
                        .lock()
                        .map_err(|_| decode_error("handler lock poisoned"))?
                        .on_c_get(CGetRequest {
                            message_id,
                            sop_class_uid: sop_class_uid.clone(),
                            priority,
                            data_set,
                            association: association.clone(),
                        })
                        .unwrap_or_else(|err| vec![dimse_protocol_violation_status(&err)])
                        .into_iter()
                        .collect::<Vec<_>>();
                    match validate_query_status_sequence(&statuses) {
                        Ok(()) => statuses,
                        Err(err) => vec![dimse_protocol_violation_status(&err)],
                    }
                }
                Err(err) => vec![dimse_protocol_violation_status(&err)],
            };
            let response_count = statuses.len();
            enforce_query_response_limit(
                response_count,
                config.operation_resource_limits.max_query_responses,
            )?;
            for status in &statuses {
                record_dimse_operation_audit(
                    &config.auth.audit,
                    operation,
                    &operation_id,
                    &association,
                    u32::from(message_id),
                    Some(status.code),
                    None,
                    Some(response_count),
                    DimseOperationAuditPhase::Response,
                )?;
                let response =
                    build_c_get_response(message_id, status.code, &sop_class_uid, 0x0101);
                if let Err(err) = send_dimse_response(
                    stream,
                    association.presentation_context_id,
                    &response,
                    association_accept.max_pdu_length,
                    config,
                ) {
                    record_dimse_operation_audit(
                        &config.auth.audit,
                        operation,
                        &operation_id,
                        &association,
                        u32::from(message_id),
                        Some(DimseStatus::processing_failure().code),
                        None,
                        Some(response_count),
                        DimseOperationAuditPhase::Failure,
                    )?;
                    return Err(err);
                }
            }
            let final_status = statuses.last().ok_or_else(|| {
                decode_error("query/retrieve handlers must return at least one response")
            })?;
            let phase = if final_status == &DimseStatus::success() {
                DimseOperationAuditPhase::Completion
            } else {
                DimseOperationAuditPhase::Failure
            };
            record_dimse_operation_audit(
                &config.auth.audit,
                operation,
                &operation_id,
                &association,
                u32::from(message_id),
                Some(final_status.code),
                None,
                Some(response_count),
                phase,
            )?;
            Ok(())
        }
        _ => Err(decode_error("unsupported DIMSE command")),
    }
}

fn authorize_dimse_association(auth: &DimseAuthConfig, request: &AssociationRequest) -> Result<()> {
    let subject = AuthSubject {
        principal: None,
        peer: Some(request.calling_ae.as_str()),
    };
    let auth_request = AuthRequest {
        scope: AuthScope::Dimse,
        action: AuthAction::Associate,
        subject,
        resource: AuthResource::none(),
    };
    authorize_and_audit(
        auth,
        auth_request,
        Some(request.called_ae.as_str()),
        Some(request.calling_ae.as_str()),
        None,
        None,
    )
}

fn authorize_dimse_request(
    auth: &DimseAuthConfig,
    action: AuthAction,
    association: &AssociationInfo,
    sop_class_uid: Option<&str>,
    sop_instance_uid: Option<&str>,
) -> Result<()> {
    let subject = AuthSubject {
        principal: None,
        peer: Some(association.calling_ae.as_str()),
    };
    let auth_request = AuthRequest {
        scope: AuthScope::Dimse,
        action,
        subject,
        resource: AuthResource {
            study_uid: None,
            series_uid: None,
            instance_uid: sop_instance_uid,
        },
    };
    authorize_and_audit(
        auth,
        auth_request,
        Some(association.called_ae.as_str()),
        Some(association.calling_ae.as_str()),
        sop_class_uid,
        sop_instance_uid,
    )
}

struct AuthAuditContext<'a> {
    called_ae: Option<&'a str>,
    calling_ae: Option<&'a str>,
    sop_class_uid: Option<&'a str>,
    sop_instance_uid: Option<&'a str>,
}

fn authorize_and_audit(
    auth: &DimseAuthConfig,
    request: AuthRequest<'_>,
    called_ae: Option<&str>,
    calling_ae: Option<&str>,
    sop_class_uid: Option<&str>,
    sop_instance_uid: Option<&str>,
) -> Result<()> {
    let decision = auth.authorizer.authorize(&request)?;
    let context = AuthAuditContext {
        called_ae,
        calling_ae,
        sop_class_uid,
        sop_instance_uid,
    };
    record_auth_audit(
        &auth.audit,
        request.scope,
        request.action,
        decision,
        context,
    )?;
    decision.enforce()
}

fn record_auth_audit(
    audit: &Option<AuditCallback>,
    scope: AuthScope,
    action: AuthAction,
    decision: AuthDecision,
    context: AuthAuditContext<'_>,
) -> Result<()> {
    let Some(callback) = audit else {
        return Ok(());
    };
    let mut fields = Vec::new();
    fields.push(AuditField {
        key: "scope",
        value: AuditValue::Plain(scope_label(scope).to_string()),
    });
    fields.push(AuditField {
        key: "action",
        value: AuditValue::Plain(auth_action_label(action).to_string()),
    });
    fields.push(AuditField {
        key: "decision",
        value: AuditValue::Plain(auth_decision_label(decision).to_string()),
    });
    if let AuthDecision::Deny(reason) = decision {
        fields.push(AuditField {
            key: "deny_reason",
            value: AuditValue::Plain(deny_reason_label(reason).to_string()),
        });
    }
    if let Some(called_ae) = context.called_ae {
        fields.push(AuditField {
            key: "called_ae",
            value: AuditValue::Sensitive(called_ae.to_string()),
        });
    }
    if let Some(calling_ae) = context.calling_ae {
        fields.push(AuditField {
            key: "calling_ae",
            value: AuditValue::Sensitive(calling_ae.to_string()),
        });
    }
    if let Some(uid) = context.sop_class_uid {
        fields.push(AuditField {
            key: "sop_class_uid",
            value: AuditValue::Sensitive(uid.to_string()),
        });
    }
    if let Some(uid) = context.sop_instance_uid {
        fields.push(AuditField {
            key: "sop_instance_uid",
            value: AuditValue::Sensitive(uid.to_string()),
        });
    }
    callback(AuditEvent {
        kind: AuditEventKind::AuthzDecision,
        fields,
    })
}

#[allow(clippy::too_many_arguments)]
fn record_dimse_operation_audit(
    audit: &Option<AuditCallback>,
    operation: &'static str,
    operation_id: &str,
    association: &AssociationInfo,
    message_id: u32,
    status_code: Option<u16>,
    sop_instance_uid: Option<&str>,
    response_count: Option<usize>,
    phase: DimseOperationAuditPhase,
) -> Result<()> {
    let Some(callback) = audit else {
        return Ok(());
    };

    let mut fields = vec![
        AuditField {
            key: "scope",
            value: AuditValue::Plain("dimse".to_string()),
        },
        AuditField {
            key: "operation",
            value: AuditValue::Plain(operation.to_string()),
        },
        AuditField {
            key: "operation_id",
            value: AuditValue::Plain(operation_id.to_string()),
        },
        AuditField {
            key: "message_id",
            value: AuditValue::Plain(message_id.to_string()),
        },
        AuditField {
            key: "phase",
            value: AuditValue::Plain(dimse_operation_audit_phase_label(phase).to_string()),
        },
        AuditField {
            key: "called_ae",
            value: AuditValue::Sensitive(association.called_ae.to_string()),
        },
        AuditField {
            key: "calling_ae",
            value: AuditValue::Sensitive(association.calling_ae.to_string()),
        },
        AuditField {
            key: "sop_class_uid",
            value: AuditValue::Sensitive(association.abstract_syntax_uid.clone()),
        },
    ];
    if let Some(status_code) = status_code {
        fields.push(AuditField {
            key: "status_code",
            value: AuditValue::Plain(status_code.to_string()),
        });
    }
    if let Some(uid) = sop_instance_uid {
        fields.push(AuditField {
            key: "sop_instance_uid",
            value: AuditValue::Sensitive(uid.to_string()),
        });
    }
    if let Some(count) = response_count {
        fields.push(AuditField {
            key: "response_count",
            value: AuditValue::Plain(count.to_string()),
        });
    }

    callback(AuditEvent {
        kind: AuditEventKind::ServiceEvent,
        fields,
    })
}

fn scope_label(scope: AuthScope) -> &'static str {
    match scope {
        AuthScope::Dimse => "dimse",
        AuthScope::Dicomweb => "dicomweb",
        AuthScope::Viewer => "viewer",
    }
}

fn auth_action_label(action: AuthAction) -> &'static str {
    match action {
        AuthAction::Associate => "associate",
        AuthAction::Command => "command",
        AuthAction::Query => "query",
        AuthAction::Retrieve => "retrieve",
        AuthAction::Store => "store",
        AuthAction::Delete => "delete",
        AuthAction::Echo => "echo",
        AuthAction::WebRequest => "web_request",
        AuthAction::StorageCommitment => "storage_commitment",
        AuthAction::Ups => "ups",
        AuthAction::Ian => "ian",
        AuthAction::ViewerMeasurementWrite => "viewer_measurement_write",
        AuthAction::ViewerSegmentationWrite => "viewer_segmentation_write",
        AuthAction::ViewerOverlayWrite => "viewer_overlay_write",
        AuthAction::ViewerAnnotationWrite => "viewer_annotation_write",
    }
}

fn auth_decision_label(decision: AuthDecision) -> &'static str {
    match decision {
        AuthDecision::Allow => "allow",
        AuthDecision::Deny(_) => "deny",
    }
}

fn deny_reason_label(reason: AuthDenyReason) -> &'static str {
    match reason {
        AuthDenyReason::Unauthenticated => "unauthenticated",
        AuthDenyReason::Unauthorized => "unauthorized",
        AuthDenyReason::Policy => "policy",
    }
}

/// Resolve a DIMSE identifier data set to query matches using `dicom-query`.
#[cfg(any(
    feature = "dimse-c-find",
    feature = "dimse-c-move",
    feature = "dimse-c-get"
))]
pub fn query_matches_from_dimse(
    data_set: &[u8],
    transfer_syntax_uid: &str,
    datasets: &[Dataset],
    limits: &Limits,
) -> Result<Vec<QueryMatch>> {
    let identifier = parse_dataset_bytes(data_set, transfer_syntax_uid, limits)?;
    let query = query_from_identifier(&identifier, limits)?;
    run_query(datasets, &query, limits)
}

fn parse_storage_commitment_n_action_request(
    message_id: u16,
    sop_class_uid: &str,
    sop_instance_uid: &str,
    data_set: &[u8],
    association: &AssociationInfo,
    limits: &Limits,
) -> Result<StorageCommitmentNActionRequest> {
    let action_information =
        parse_dataset_bytes(data_set, &association.transfer_syntax_uid, limits)?;
    let transaction_uid = action_information
        .get_uid_strict(TAG_TRANSACTION_UID, limits)?
        .ok_or_else(|| decode_error("missing Storage Commitment transaction UID"))?
        .to_string();
    let referenced_instances = match action_information.get(TAG_REFERENCED_SOP_SEQUENCE) {
        Some(element) => match &element.value {
            Value::Sequence(items) => {
                if items.is_empty() {
                    vec![StorageCommitmentReferencedInstance {
                        sop_class_uid: sop_class_uid.to_string(),
                        sop_instance_uid: sop_instance_uid.to_string(),
                    }]
                } else {
                    let mut out = Vec::with_capacity(items.len());
                    for item in items {
                        let referenced_sop_class_uid = item
                            .get_uid_strict(TAG_REFERENCED_SOP_CLASS_UID, limits)?
                            .ok_or_else(|| {
                                decode_error(
                                    "missing Referenced SOP Class UID in Storage Commitment sequence item",
                                )
                            })?;
                        let referenced_sop_instance_uid = item
                            .get_uid_strict(TAG_REFERENCED_SOP_INSTANCE_UID, limits)?
                            .ok_or_else(|| {
                                decode_error(
                                    "missing Referenced SOP Instance UID in Storage Commitment sequence item",
                                )
                            })?;
                        out.push(StorageCommitmentReferencedInstance {
                            sop_class_uid: referenced_sop_class_uid.to_string(),
                            sop_instance_uid: referenced_sop_instance_uid.to_string(),
                        });
                    }
                    out
                }
            }
            _ => {
                return Err(decode_error(
                    "Storage Commitment Referenced SOP Sequence must be encoded as SQ",
                ))
            }
        },
        None => vec![StorageCommitmentReferencedInstance {
            sop_class_uid: sop_class_uid.to_string(),
            sop_instance_uid: sop_instance_uid.to_string(),
        }],
    };

    Ok(StorageCommitmentNActionRequest {
        message_id,
        transaction_uid,
        calling_ae_title: association.calling_ae.clone(),
        called_ae_title: association.called_ae.clone(),
        referenced_instances,
        association: association.clone(),
    })
}

fn send_dimse_response(
    stream: &mut TcpStream,
    presentation_context_id: u8,
    command: &[u8],
    max_pdu_length: u32,
    config: &DimseServerConfig,
) -> Result<()> {
    let pdus = encode_message_pdus(
        presentation_context_id,
        command,
        None,
        &config.network_limits,
        max_pdu_length,
    )?;
    for pdu in pdus {
        send_pdu(stream, &pdu, &config.network_limits)?;
    }
    Ok(())
}

fn build_association_info(
    association_accept: &AssociationAccept,
    context_map: &HashMap<u8, (String, String)>,
    abstract_syntax_uid: &str,
) -> Result<AssociationInfo> {
    let (presentation_context_id, transfer_syntax_uid) = context_map
        .iter()
        .find_map(|(id, (abs, ts))| {
            if abs == abstract_syntax_uid {
                Some((*id, ts.clone()))
            } else {
                None
            }
        })
        .ok_or_else(|| decode_error("missing presentation context"))?;
    Ok(AssociationInfo {
        called_ae: association_accept.called_ae.clone(),
        calling_ae: association_accept.calling_ae.clone(),
        presentation_context_id,
        abstract_syntax_uid: abstract_syntax_uid.to_string(),
        transfer_syntax_uid,
    })
}

fn build_client_context_map(
    request: &AssociationRequest,
    association: &AssociationAccept,
) -> HashMap<String, u8> {
    let mut map = HashMap::new();
    for accepted in &association.presentation_contexts {
        if accepted.result != 0x00 {
            continue;
        }
        if let Some(req) = request
            .presentation_contexts
            .iter()
            .find(|ctx| ctx.id == accepted.id)
        {
            map.insert(req.abstract_syntax.clone(), accepted.id);
        }
    }
    map
}

fn build_server_context_map(
    request: &AssociationRequest,
    association: &AssociationAccept,
) -> HashMap<u8, (String, String)> {
    let mut map = HashMap::new();
    for accepted in &association.presentation_contexts {
        if accepted.result != 0x00 {
            continue;
        }
        let transfer_syntax = match &accepted.transfer_syntax {
            Some(ts) => ts.clone(),
            None => continue,
        };
        if let Some(req) = request
            .presentation_contexts
            .iter()
            .find(|ctx| ctx.id == accepted.id)
        {
            map.insert(accepted.id, (req.abstract_syntax.clone(), transfer_syntax));
        }
    }
    map
}

fn encode_message_pdus(
    presentation_context_id: u8,
    command: &[u8],
    data_set: Option<&[u8]>,
    limits: &NetworkLimits,
    max_pdu_length: u32,
) -> Result<Vec<Pdu>> {
    let mut pdvs = Vec::new();
    let command_pdvs = chunk_pdvs(presentation_context_id, command, limits.max_pdv_bytes, true)?;
    pdvs.extend(command_pdvs);
    if let Some(data) = data_set {
        let data_pdvs = chunk_pdvs(presentation_context_id, data, limits.max_pdv_bytes, false)?;
        pdvs.extend(data_pdvs);
    }
    pack_pdus(pdvs, limits, max_pdu_length)
}

fn chunk_pdvs(
    presentation_context_id: u8,
    bytes: &[u8],
    max_pdv_bytes: u64,
    is_command: bool,
) -> Result<Vec<Pdv>> {
    let payload_max = max_pdv_bytes
        .checked_sub(2)
        .ok_or_else(|| decode_error("max_pdv_bytes too small"))? as usize;
    if payload_max == 0 {
        return Err(decode_error("max_pdv_bytes too small"));
    }
    let mut pdvs = Vec::new();
    let mut offset = 0usize;
    while offset < bytes.len() {
        let end = std::cmp::min(offset + payload_max, bytes.len());
        let is_last = end == bytes.len();
        let mut header = if is_command { 0x01 } else { 0x00 };
        if is_last {
            header |= 0x02;
        }
        pdvs.push(Pdv {
            presentation_context_id,
            message_control_header: header,
            data: bytes[offset..end].to_vec(),
        });
        offset = end;
    }
    if bytes.is_empty() && is_command {
        pdvs.push(Pdv {
            presentation_context_id,
            message_control_header: 0x03,
            data: Vec::new(),
        });
    }
    Ok(pdvs)
}

fn pack_pdus(pdvs: Vec<Pdv>, limits: &NetworkLimits, max_pdu_length: u32) -> Result<Vec<Pdu>> {
    let max_pdu_bytes = std::cmp::min(limits.max_pdu_bytes, max_pdu_length as u64);
    let mut out = Vec::new();
    let mut current = Vec::new();
    let mut current_len = 0usize;
    for pdv in pdvs {
        let pdv_len = pdv.data.len() + 6;
        if current_len + pdv_len > max_pdu_bytes as usize && !current.is_empty() {
            out.push(Pdu::PDataTf(current));
            current = Vec::new();
            current_len = 0;
        }
        current_len += pdv_len;
        current.push(pdv);
    }
    if !current.is_empty() {
        out.push(Pdu::PDataTf(current));
    }
    Ok(out)
}

fn send_pdu(stream: &mut TcpStream, pdu: &Pdu, limits: &NetworkLimits) -> Result<()> {
    let bytes = encode_pdu(pdu, limits)?;
    stream.write_all(&bytes).map_err(io_error)?;
    Ok(())
}

fn read_pdu(stream: &mut TcpStream, limits: &NetworkLimits) -> Result<Pdu> {
    let mut header = [0u8; 6];
    stream.read_exact(&mut header).map_err(io_error)?;
    let length = u32::from_be_bytes([header[2], header[3], header[4], header[5]]) as usize;
    if length as u64 > limits.max_pdu_bytes {
        return Err(limit_exceeded(
            "max_pdu_bytes",
            length as u64,
            limits.max_pdu_bytes,
        ));
    }
    let mut body = vec![0u8; length];
    stream.read_exact(&mut body).map_err(io_error)?;
    let mut buf = Vec::with_capacity(length + 6);
    buf.extend_from_slice(&header);
    buf.extend_from_slice(&body);
    parse_pdu(&buf, limits)
}

fn read_dimse_response(stream: &mut TcpStream, config: &DimseClientConfig) -> Result<DimseMessage> {
    let mut command = Vec::new();
    loop {
        let pdu = read_pdu(stream, &config.network_limits)?;
        match pdu {
            Pdu::PDataTf(pdvs) => {
                for pdv in pdvs {
                    if pdv.is_command() {
                        let new_len = command
                            .len()
                            .checked_add(pdv.data.len())
                            .ok_or_else(|| decode_error("command length overflow"))?;
                        if new_len as u64 > config.dimse_limits.max_command_bytes {
                            return Err(limit_exceeded(
                                "max_command_bytes",
                                new_len as u64,
                                config.dimse_limits.max_command_bytes,
                            ));
                        }
                        command.extend_from_slice(&pdv.data);
                        if pdv.is_last() {
                            return parse_command_set(&command, &config.dimse_limits);
                        }
                    } else if pdv.is_last() {
                        return Err(decode_error("unexpected data set in response"));
                    }
                }
            }
            Pdu::Abort(_) => return Err(decode_error("association aborted")),
            Pdu::ReleaseRq => return Err(decode_error("association released")),
            _ => {}
        }
    }
}

fn build_associate_request(
    called_ae: &str,
    calling_ae: &str,
    policy: &AssociationPolicy,
) -> Result<AssociationRequest> {
    let mut contexts = Vec::new();
    let mut id = 1u8;
    for abstract_syntax in &policy.supported_abstract_syntaxes {
        contexts.push(dicom_net::PresentationContext {
            id,
            abstract_syntax: abstract_syntax.clone(),
            transfer_syntaxes: policy.supported_transfer_syntaxes.clone(),
        });
        id = id.saturating_add(2);
    }
    Ok(AssociationRequest {
        called_ae: called_ae.to_string(),
        calling_ae: calling_ae.to_string(),
        application_context: "1.2.840.10008.3.1.1.1".to_string(),
        presentation_contexts: contexts,
        max_pdu_length: policy.max_pdu_length,
        implementation_class_uid: None,
        implementation_version_name: None,
    })
}

#[derive(Debug, Default, Clone)]
struct PendingMessage {
    command: Vec<u8>,
    command_done: bool,
    data: Vec<u8>,
    data_done: bool,
    message: Option<DimseMessage>,
}

fn wrap_dataset_as_p10(transfer_syntax_uid: &str, data_set: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0u8; 128];
    bytes.extend_from_slice(b"DICM");
    bytes.extend_from_slice(&meta_element_ui(Tag(0x0002, 0x0010), transfer_syntax_uid));
    bytes.extend_from_slice(data_set);
    bytes
}

fn meta_element_ui(tag: Tag, value: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&tag.0.to_le_bytes());
    buf.extend_from_slice(&tag.1.to_le_bytes());
    buf.extend_from_slice(b"UI");
    let mut bytes = value.as_bytes().to_vec();
    if bytes.len() % 2 == 1 {
        bytes.push(0);
    }
    buf.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
    buf.extend_from_slice(&bytes);
    buf
}

fn io_error(err: std::io::Error) -> Box<Error> {
    Error::from_kind(
        ErrorKind::IoError {
            detail: err.to_string(),
        },
        "io error",
    )
    .into()
}

fn decode_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::DecodeError {
            stage: "dicom-dimse-service".to_string(),
            detail: detail.into(),
        },
        "decode error",
    )
    .into()
}

fn limit_exceeded(limit_name: &'static str, observed: u64, allowed: u64) -> Box<Error> {
    Error::from_kind(
        ErrorKind::LimitExceeded {
            limit_name,
            observed,
            allowed,
        },
        "limit exceeded",
    )
    .into()
}

fn is_allowed_peer(stream: &TcpStream, allowed_hosts: &Option<Vec<IpAddr>>) -> bool {
    let peer = match stream.peer_addr() {
        Ok(addr) => addr,
        Err(_) => return false,
    };

    match allowed_hosts {
        Some(allowed) => allowed.iter().any(|allowed_ip| *allowed_ip == peer.ip()),
        None => true,
    }
}

fn reject_peer(stream: TcpStream) -> std::io::Result<()> {
    stream.shutdown(std::net::Shutdown::Both)
}

fn enforce_connection_limit(observed: usize, allowed: usize) -> Result<()> {
    if observed > allowed {
        return Err(limit_exceeded(
            "max_connections",
            observed as u64,
            allowed as u64,
        ));
    }
    Ok(())
}

fn enforce_operation_limit(observed: usize, allowed: usize) -> Result<()> {
    if observed > allowed {
        return Err(limit_exceeded(
            "max_in_flight_operations",
            observed as u64,
            allowed as u64,
        ));
    }
    Ok(())
}

fn is_pending_status(status: u16) -> bool {
    matches!(status, 0xFF00 | 0xFF01)
}

/// Validate deterministic pending/final status progression for query/retrieve responses.
pub fn validate_query_retrieve_status_sequence(statuses: &[DimseStatus]) -> Result<()> {
    validate_query_status_sequence(statuses)
}

fn dimse_protocol_violation_status(error: &Error) -> DimseStatus {
    match &error.kind {
        ErrorKind::DecodeError { detail, .. } if detail.contains("unsupported DIMSE command") => {
            DimseStatus::sop_class_not_supported()
        }
        ErrorKind::DecodeError { detail, .. } if detail.contains("unsupported SOP class") => {
            DimseStatus::sop_class_not_supported()
        }
        ErrorKind::DecodeError { detail, .. } if detail.contains("unsupported command field") => {
            DimseStatus::sop_class_not_supported()
        }
        ErrorKind::DecodeError { .. } => DimseStatus::processing_failure(),
        ErrorKind::LimitExceeded { .. } => DimseStatus::processing_failure(),
        _ => DimseStatus::processing_failure(),
    }
}

fn validate_query_status_sequence(statuses: &[DimseStatus]) -> Result<()> {
    if statuses.is_empty() {
        return Err(decode_error(
            "query/retrieve handlers must return at least one response status",
        ));
    }
    for (idx, status) in statuses.iter().enumerate() {
        let is_last = idx + 1 == statuses.len();
        if is_last {
            if status.is_pending() {
                return Err(decode_error(
                    "final query/retrieve response status must not be pending",
                ));
            }
        } else if !status.is_pending() {
            return Err(decode_error(
                "intermediate query/retrieve response statuses must be pending",
            ));
        }
    }
    Ok(())
}

fn enforce_query_response_limit(observed: usize, max_query_responses: usize) -> Result<()> {
    if observed > max_query_responses {
        return Err(decode_error(format!(
            "query/retrieve response sequence exceeded {max_query_responses} responses",
        )));
    }
    Ok(())
}

fn enforce_tls_policy(transport: TransportSecurity, policy: TlsPolicy) -> Result<()> {
    if policy == TlsPolicy::RequireTls && transport != TransportSecurity::Tls {
        return Err(decode_error("TLS required for DIMSE association"));
    }
    Ok(())
}

fn tls_reject() -> AssociationReject {
    AssociationReject {
        result: 0x01,
        source: 0x01,
        reason: 0x01,
    }
}

fn auth_reject() -> AssociationReject {
    AssociationReject {
        result: 0x01,
        source: 0x01,
        reason: 0x01,
    }
}

fn throttle_reject() -> AssociationReject {
    AssociationReject {
        result: 0x02,
        source: 0x03,
        reason: 0x02,
    }
}

fn reject_association(
    mut stream: TcpStream,
    config: &DimseServerConfig,
    reject: AssociationReject,
) -> Result<()> {
    stream
        .set_read_timeout(Some(config.read_timeout))
        .map_err(io_error)?;
    stream
        .set_write_timeout(Some(config.write_timeout))
        .map_err(io_error)?;
    let first = read_pdu(&mut stream, &config.network_limits)?;
    match first {
        Pdu::AssociateRq(_) => {
            send_pdu(
                &mut stream,
                &Pdu::AssociateRj(reject),
                &config.network_limits,
            )?;
            Ok(())
        }
        _ => Err(decode_error("expected association request")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_audit::{AuditConfig, AuditEventKind, AuditLog, AuditRedactor};
    #[cfg(feature = "dimse-c-find")]
    use dicom_core::{Dataset, Element, Value, Vr};
    use dicom_core::{ErrorKind, Tag};
    use std::net::Ipv4Addr;

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

    fn stream_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let client = TcpStream::connect(addr).expect("connect");
        let (server, _) = listener.accept().expect("accept");
        (server, client)
    }

    fn dummy_association_accept() -> AssociationAccept {
        AssociationAccept {
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
        dataset.insert(Element {
            tag: TAG_STUDY_UID,
            vr: Vr::Ui,
            value: Value::Uid(study.to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SERIES_UID,
            vr: Vr::Ui,
            value: Value::Uid(series.to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SOP_UID,
            vr: Vr::Ui,
            value: Value::Uid(sop.to_string()),
        });
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
        dataset.insert(Element {
            tag: TAG_ACCESSION_NUMBER,
            vr: Vr::Lo,
            value: Value::Str(accession_number.to_string()),
        });
        dataset.insert(Element {
            tag: TAG_STUDY_DATE,
            vr: Vr::Da,
            value: Value::Str(study_date.to_string()),
        });
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
            &AssociationPolicy::verification_default(),
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
        server_config.limits.max_input_bytes = 16;
        let server = DimseServer::bind("127.0.0.1:0".parse().unwrap(), server_config, EchoService)
            .expect("bind");
        let addr = server.local_addr().expect("addr");
        let handle = thread::spawn(move || {
            let _ = server.run_once();
        });

        let policy = AssociationPolicy {
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
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
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
            .authorize(&AuthRequest {
                scope: AuthScope::Dimse,
                action: AuthAction::Associate,
                subject: AuthSubject {
                    principal: None,
                    peer: None,
                },
                resource: AuthResource::none(),
            })
            .expect("auth decision");
        assert!(matches!(decision, AuthDecision::Deny(_)));
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
            &AssociationPolicy::verification_default(),
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
            err.kind,
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
            err.kind,
            ErrorKind::LimitExceeded {
                limit_name: "max_in_flight_operations",
                ..
            }
        ));
    }

    #[test]
    fn enforce_query_response_limit_respects_configured_cap() {
        let err = enforce_query_response_limit(2, 1).expect_err("expected query response limit");
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn storage_backed_service_persists_c_store_dataset() {
        // REQ-STOR-300: DIMSE C-STORE ingests into deterministic storage state.
        let mut service = StorageBackedDimseService::new(Limits::default());
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
            &Limits::default(),
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
            &Limits::default(),
        )
        .expect_err("invalid transaction uid must fail");
        assert!(matches!(
            err.kind,
            ErrorKind::InvalidTagValue {
                tag: TAG_TRANSACTION_UID,
                ..
            }
        ));
    }

    #[test]
    fn storage_backed_service_registers_storage_commitment_request() {
        let mut service = StorageBackedDimseService::new(Limits::default());
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
        let mut service = StorageBackedDimseService::new(Limits::default());
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
        assert_eq!(request.state, StorageCommitmentState::ReportDelivered);
    }

    #[test]
    fn storage_commitment_lifecycle_sink_receives_state_transitions() {
        let mut service = StorageBackedDimseService::new(Limits::default());
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
        let mut service = StorageBackedDimseService::new(Limits::default());
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
        let mut service = StorageBackedDimseService::new(Limits::default());
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
        let policy = AssociationPolicy {
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
            &Limits::default(),
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
            &Limits::default(),
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
            query_matches_from_dimse(&identifier, "1.2.840.10008.1.2.1", &[], &Limits::default())
                .expect_err("expected query error");
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
    }

    #[cfg(feature = "dimse-c-move")]
    #[test]
    fn c_move_round_trip() {
        // REQ-DIMSE-341: C-MOVE handlers dispatch and return deterministic pending/final status responses.
        let seen = Arc::new(Mutex::new(None));
        let service = MoveService {
            seen_destination: Arc::clone(&seen),
        };
        let policy = AssociationPolicy {
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
        let policy = AssociationPolicy {
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

        let pdv = Pdv {
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
        match err.kind {
            ErrorKind::LimitExceeded { limit_name, .. } => {
                assert_eq!(limit_name, "max_command_bytes");
            }
            _ => panic!("expected limit exceeded"),
        }
    }

    #[test]
    fn data_limit_enforced_before_accumulation() {
        // REQ-DIMSE-303: Data set bytes are bounded before accumulation.
        let mut config = server_config_allow_all();
        config.limits.max_input_bytes = 8;
        let handler = Arc::new(Mutex::new(EchoService));
        let association = dummy_association_accept();
        let context_map = HashMap::new();
        let mut pending = HashMap::new();
        let (mut server_stream, _client_stream) = stream_pair();

        let pdv = Pdv {
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
        match err.kind {
            ErrorKind::LimitExceeded { limit_name, .. } => {
                assert_eq!(limit_name, "max_input_bytes");
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

    impl Authorizer for DenyEchoAllowAssociate {
        fn authorize(&self, request: &AuthRequest<'_>) -> Result<AuthDecision> {
            if request.action == AuthAction::Associate {
                return Ok(AuthDecision::Allow);
            }
            Ok(AuthDecision::Deny(AuthDenyReason::Unauthorized))
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
            &AssociationPolicy::verification_default(),
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
            c_find_data_set_bytes: config.limits.max_input_bytes,
            c_move_data_set_bytes: config.limits.max_input_bytes,
            c_get_data_set_bytes: config.limits.max_input_bytes,
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

        let policy = AssociationPolicy {
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
        match err.kind {
            ErrorKind::LimitExceeded { limit_name, .. } => {
                assert_eq!(limit_name, "max_input_bytes");
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
                authorizer: Arc::new(AllowAll),
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
            &AssociationPolicy::verification_default(),
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
            allowed_hosts: Some(vec![IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1))]),
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
            &AssociationPolicy::verification_default(),
            DimseClientConfig::default(),
        )
        .err()
        .expect("expected connect failure");
        assert!(matches!(
            err.kind,
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
