//! DIMSE service trait, request/response types, and protocol logic.

use crate::commitment::{
    StorageCommitmentLifecycleEvent, StorageCommitmentLifecycleSink,
    StorageCommitmentNActionRequest,
};
use crate::ian::IanNotification;
use crate::{
    decode_error, workstation_default_association_policy, SOP_CLASS_CR_IMAGE_STORAGE,
    SOP_CLASS_CT_IMAGE_STORAGE, SOP_CLASS_DX_PRESENTATION, SOP_CLASS_MR_IMAGE_STORAGE,
    SOP_CLASS_MULTI_FRAME_SC_BYTE, SOP_CLASS_MULTI_FRAME_SC_TRUE_COLOR,
    SOP_CLASS_MULTI_FRAME_SC_WORD, SOP_CLASS_PET_IMAGE_STORAGE, SOP_CLASS_SECONDARY_CAPTURE,
    SOP_CLASS_STUDY_ROOT_FIND, SOP_CLASS_STUDY_ROOT_GET, SOP_CLASS_STUDY_ROOT_MOVE,
    SOP_CLASS_VERIFICATION, TRANSFER_SYNTAX_EXPLICIT_VR_LE, TRANSFER_SYNTAX_IMPLICIT_VR_LE,
};
use dicom_core::{Error, ErrorKind, Limits, Result, Tag};
use dicom_storage::{Storage, StorageCommitmentRequest, StorageCommitmentState, WriteAheadLog};
use std::collections::BTreeSet;
use std::path::Path;

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

#[allow(missing_docs)]
pub(crate) fn is_pending_status(status: u16) -> bool {
    matches!(status, 0xFF00 | 0xFF01)
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

/// Validate deterministic pending/final status progression for query/retrieve responses.
pub fn validate_query_retrieve_status_sequence(statuses: &[DimseStatus]) -> Result<()> {
    validate_query_status_sequence(statuses)
}

#[allow(missing_docs)]
pub(crate) fn validate_query_status_sequence(statuses: &[DimseStatus]) -> Result<()> {
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

#[allow(missing_docs)]
pub fn dimse_protocol_violation_status(error: &Error) -> DimseStatus {
    match error.kind() {
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
