//! Storage Commitment bounded-context types.
//!
//! Contains types for the Storage Commitment lifecycle: request registration,
//! event-report delivery, and policy configuration.

use dicom_core::{validate_uid_strict, Error, ErrorKind, Result, Tag};
use std::collections::{BTreeMap, VecDeque};

/// Storage Commitment request state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageCommitmentState {
    /// Request accepted and pending asynchronous outcome delivery.
    Requested,
    /// Event report queued for asynchronous delivery.
    EventQueued,
    /// Event report delivery succeeded.
    ReportDelivered,
    /// Event report delivery failed permanently.
    ReportFailed,
    /// Request was canceled before terminal delivery.
    Canceled,
    /// Request timed out before terminal delivery.
    TimedOut,
}

/// Referenced instance payload row in a Storage Commitment request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageCommitmentReferencedInstance {
    /// Referenced SOP Class UID.
    pub sop_class_uid: String,
    /// Referenced SOP Instance UID.
    pub sop_instance_uid: String,
}

/// Persisted Storage Commitment request model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageCommitmentRequest {
    /// Transaction UID for the request lifecycle.
    pub transaction_uid: String,
    /// Calling AE title.
    pub calling_ae_title: String,
    /// Called AE title.
    pub called_ae_title: String,
    /// Referenced instances included in the commitment contract.
    pub referenced_instances: Vec<StorageCommitmentReferencedInstance>,
    /// Current deterministic state for this request.
    pub state: StorageCommitmentState,
}

/// Deterministic event-report delivery job for Storage Commitment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageCommitmentEventJob {
    /// Transaction UID for request lifecycle.
    pub transaction_uid: String,
    /// Delivery attempt counter.
    pub attempt: u32,
    /// Maximum allowed attempts before terminal failure.
    pub max_attempts: u32,
    /// Logical tick at which this job was queued.
    pub queued_tick: u64,
    /// Logical tick at which this job times out.
    pub timeout_tick: u64,
}

/// Policy knobs for Storage Commitment async delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageCommitmentPolicy {
    /// Maximum queued jobs allowed before backpressure rejection.
    pub max_queued_jobs: usize,
    /// Logical tick budget for queued jobs before timeout.
    pub timeout_ticks: u64,
}

impl Default for StorageCommitmentPolicy {
    fn default() -> Self {
        Self {
            max_queued_jobs: 1024,
            timeout_ticks: 256,
        }
    }
}

/// Internal helper for Storage Commitment operations within `Storage`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CommitmentEngine {
    pub(crate) requests: BTreeMap<String, StorageCommitmentRequest>,
    pub(crate) event_queue: VecDeque<StorageCommitmentEventJob>,
    pub(crate) policy: StorageCommitmentPolicy,
    pub(crate) tick: u64,
}

impl CommitmentEngine {
    /// Create a new commitment engine with default policy.
    pub(crate) fn new() -> Self {
        Self {
            requests: BTreeMap::new(),
            event_queue: VecDeque::new(),
            policy: StorageCommitmentPolicy::default(),
            tick: 0,
        }
    }

    /// Register and persist a Storage Commitment request model in deterministic key order.
    pub(crate) fn register_request(&mut self, request: StorageCommitmentRequest) -> Result<()> {
        validate_uid_strict(Tag(0x0008, 0x1195), &request.transaction_uid)?;
        if request.referenced_instances.is_empty() {
            return Err(Error::from_kind(
                ErrorKind::DecodeError {
                    stage: "storage_commitment".to_string(),
                    detail:
                        "storage commitment request must include at least one referenced instance"
                            .to_string(),
                },
                "invalid input",
            )
            .into());
        }
        for reference in &request.referenced_instances {
            validate_uid_strict(Tag(0x0008, 0x1150), &reference.sop_class_uid)?;
            validate_uid_strict(Tag(0x0008, 0x1155), &reference.sop_instance_uid)?;
        }
        if self.requests.contains_key(&request.transaction_uid) {
            return Err(Error::from_kind(
                ErrorKind::IntegrityError {
                    detail: format!(
                        "duplicate storage commitment transaction UID: {}",
                        request.transaction_uid
                    ),
                },
                "already exists",
            )
            .into());
        }
        self.requests
            .insert(request.transaction_uid.clone(), request);
        Ok(())
    }

    /// Return a persisted Storage Commitment request model by transaction UID.
    pub(crate) fn request(&self, transaction_uid: &str) -> Option<&StorageCommitmentRequest> {
        self.requests.get(transaction_uid)
    }

    /// Return all persisted Storage Commitment request models in deterministic key order.
    pub(crate) fn requests(&self) -> Vec<&StorageCommitmentRequest> {
        self.requests.values().collect()
    }

    /// Queue deterministic asynchronous N-EVENT report delivery for a transaction.
    pub(crate) fn queue_event_report(
        &mut self,
        transaction_uid: &str,
        max_attempts: u32,
    ) -> Result<()> {
        if self.event_queue.len() >= self.policy.max_queued_jobs {
            return Err(Error::from_kind(
                ErrorKind::LimitExceeded {
                    limit_name: "storage_commitment_max_queued_jobs",
                    observed: self.event_queue.len() as u64 + 1,
                    allowed: self.policy.max_queued_jobs as u64,
                },
                "storage commitment queue backpressure",
            )
            .into());
        }
        let request = self
            .requests
            .get_mut(transaction_uid)
            .ok_or_else(|| {
                Error::from_kind(
                    ErrorKind::MissingRequiredTag {
                        tag: Tag(0x0008, 0x1195),
                    },
                    "missing storage commitment transaction UID",
                )
            })?;
        request.state = StorageCommitmentState::EventQueued;
        self.tick = self.tick.saturating_add(1);
        let queued_tick = self.tick;
        let timeout_tick = queued_tick.saturating_add(self.policy.timeout_ticks);
        self.event_queue
            .push_back(StorageCommitmentEventJob {
                transaction_uid: transaction_uid.to_string(),
                attempt: 0,
                max_attempts: max_attempts.max(1),
                queued_tick,
                timeout_tick,
            });
        Ok(())
    }

    /// Pop next event delivery job in FIFO order.
    pub(crate) fn pop_next_job(&mut self) -> Option<StorageCommitmentEventJob> {
        self.event_queue.pop_front()
    }

    /// Record delivery outcome and schedule retry when allowed.
    pub(crate) fn complete_job(
        &mut self,
        mut job: StorageCommitmentEventJob,
        delivered: bool,
    ) -> Result<()> {
        let request = self
            .requests
            .get_mut(&job.transaction_uid)
            .ok_or_else(|| {
                Error::from_kind(
                    ErrorKind::MissingRequiredTag {
                        tag: Tag(0x0008, 0x1195),
                    },
                    "missing storage commitment transaction UID",
                )
            })?;
        if delivered {
            request.state = StorageCommitmentState::ReportDelivered;
            return Ok(());
        }
        job.attempt = job.attempt.saturating_add(1);
        if job.attempt >= job.max_attempts {
            request.state = StorageCommitmentState::ReportFailed;
            return Ok(());
        }
        request.state = StorageCommitmentState::EventQueued;
        self.tick = self.tick.saturating_add(1);
        job.queued_tick = self.tick;
        job.timeout_tick = job
            .queued_tick
            .saturating_add(self.policy.timeout_ticks);
        self.event_queue.push_back(job);
        Ok(())
    }

    /// Cancel a Storage Commitment request and drop queued jobs for the transaction.
    pub(crate) fn cancel_request(&mut self, transaction_uid: &str) -> Result<()> {
        let request = self
            .requests
            .get_mut(transaction_uid)
            .ok_or_else(|| {
                Error::from_kind(
                    ErrorKind::MissingRequiredTag {
                        tag: Tag(0x0008, 0x1195),
                    },
                    "missing storage commitment transaction UID",
                )
            })?;
        request.state = StorageCommitmentState::Canceled;
        self.event_queue
            .retain(|job| job.transaction_uid != transaction_uid);
        Ok(())
    }

    /// Advance logical delivery clock and mark expired queued jobs as timed out.
    pub(crate) fn advance_timeouts(&mut self, ticks: u64) -> usize {
        self.tick = self.tick.saturating_add(ticks);
        let now = self.tick;
        let mut timed_out = 0usize;
        let mut retained = VecDeque::new();
        while let Some(job) = self.event_queue.pop_front() {
            if job.timeout_tick <= now {
                if let Some(request) = self.requests.get_mut(&job.transaction_uid) {
                    request.state = StorageCommitmentState::TimedOut;
                }
                timed_out = timed_out.saturating_add(1);
            } else {
                retained.push_back(job);
            }
        }
        self.event_queue = retained;
        timed_out
    }

    /// Override Storage Commitment delivery policy.
    pub(crate) fn set_policy(&mut self, policy: StorageCommitmentPolicy) {
        self.policy = policy;
    }
}
