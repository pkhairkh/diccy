use dicom_core::{Error, ErrorKind, Limits, Result};
use pack_sr::{
    apply_sr_update, Code, SrAuthoredDocument, SrAuthoringBuilder, SrAuthoringContentItem,
    SrAuthoringError, SrBuilderDefaults, SrUpdateRequest,
};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const SR_STORE_HEADER: &str = "rdvf_sr_store_v1";

/// Service-level SR workflow architecture snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrWorkflowArchitecture {
    /// API layer baseline.
    pub api_layer: &'static str,
    /// Service-layer baseline.
    pub service_layer: &'static str,
    /// User-visible workflow baseline.
    pub ui_layer: &'static str,
}

/// Return current SR workflow architecture status.
pub fn sr_workflow_architecture() -> SrWorkflowArchitecture {
    SrWorkflowArchitecture {
        api_layer: "Implemented baseline deterministic authoring/update API in pack-sr",
        service_layer:
            "Implemented in dicom-workflow-server: create/update/retrieve with fail-closed auth, idempotency, and persistence",
        ui_layer: "Implemented minimal web-host SR author/review/commit prototype",
    }
}

/// SR service endpoint contract row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrEndpointContract {
    /// HTTP method.
    pub method: &'static str,
    /// Route template.
    pub path_template: &'static str,
    /// Lifecycle operation.
    pub operation: &'static str,
    /// Write auth gate requirement.
    pub requires_write_auth: bool,
    /// Idempotency-key requirement.
    pub requires_idempotency_key: bool,
}

/// Deterministic SR endpoint contract.
pub fn sr_endpoint_contract() -> [SrEndpointContract; 9] {
    [
        SrEndpointContract {
            method: "POST",
            path_template: "/sr/documents",
            operation: "create",
            requires_write_auth: true,
            requires_idempotency_key: true,
        },
        SrEndpointContract {
            method: "POST",
            path_template: "/sr/documents/{sop_instance_uid}/updates",
            operation: "update",
            requires_write_auth: true,
            requires_idempotency_key: true,
        },
        SrEndpointContract {
            method: "GET",
            path_template: "/sr/documents/{sop_instance_uid}",
            operation: "retrieve",
            requires_write_auth: false,
            requires_idempotency_key: false,
        },
        SrEndpointContract {
            method: "POST",
            path_template: "/sr/documents/{sop_instance_uid}/review",
            operation: "review",
            requires_write_auth: true,
            requires_idempotency_key: true,
        },
        SrEndpointContract {
            method: "POST",
            path_template: "/sr/documents/{sop_instance_uid}/finalize",
            operation: "finalize",
            requires_write_auth: true,
            requires_idempotency_key: true,
        },
        SrEndpointContract {
            method: "POST",
            path_template: "/sr/documents/{sop_instance_uid}/commit",
            operation: "commit",
            requires_write_auth: true,
            requires_idempotency_key: true,
        },
        SrEndpointContract {
            method: "POST",
            path_template: "/sr/documents/{sop_instance_uid}/cancel",
            operation: "cancel",
            requires_write_auth: true,
            requires_idempotency_key: true,
        },
        SrEndpointContract {
            method: "GET|HEAD",
            path_template: "/sr/documents/{sop_instance_uid}/history",
            operation: "history",
            requires_write_auth: false,
            requires_idempotency_key: false,
        },
        SrEndpointContract {
            method: "GET",
            path_template: "/sr/documents",
            operation: "list",
            requires_write_auth: false,
            requires_idempotency_key: false,
        },
    ]
}

/// Auth context for SR writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrAuthContext {
    /// Caller principal.
    pub principal: Option<String>,
    /// True when caller is authorized for SR writes.
    pub can_write: bool,
}

/// Create request for SR workflow service.
#[derive(Debug, Clone, PartialEq)]
pub struct SrCreateRequest {
    /// Study Instance UID.
    pub study_instance_uid: String,
    /// Series Instance UID.
    pub series_instance_uid: String,
    /// SOP Instance UID.
    pub sop_instance_uid: String,
    /// Observer identifier.
    pub observer: String,
    /// Deterministic authored timestamp.
    pub authored_epoch_ms: u64,
    /// One content item to seed document.
    pub item: SrAuthoringContentItem,
    /// Known SOP references for validation.
    pub known_referenced_sop_instance_uids: Vec<String>,
    /// Idempotency key.
    pub idempotency_key: String,
    /// Optional request tracing identifier.
    pub request_id: Option<String>,
}

/// Update request for SR workflow service.
#[derive(Debug, Clone, PartialEq)]
pub struct SrUpdateEnvelope {
    /// Target SOP Instance UID.
    pub sop_instance_uid: String,
    /// Expected current version.
    pub expected_version: u64,
    /// One content item to append.
    pub item: SrAuthoringContentItem,
    /// Optional observer override.
    pub observer: Option<String>,
    /// Known SOP references for validation.
    pub known_referenced_sop_instance_uids: Vec<String>,
    /// Idempotency key.
    pub idempotency_key: String,
    /// Optional request tracing identifier.
    pub request_id: Option<String>,
}

/// SR write outcome classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SrWriteOutcomeKind {
    /// New document created.
    Created,
    /// Existing document updated.
    Updated,
    /// Replayed idempotent response.
    Duplicate,
}

/// SR write outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrWriteOutcome {
    /// Outcome kind.
    pub kind: SrWriteOutcomeKind,
    /// SOP Instance UID.
    pub sop_instance_uid: String,
    /// Current version after operation.
    pub version: u64,
    /// True when served via idempotent replay.
    pub idempotency_replay: bool,
}

/// SR lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SrLifecycleStatus {
    /// Draft draft state before review.
    Draft,
    /// Reviewed by clinician or QA role.
    Reviewed,
    /// Finalized report content.
    Finalized,
    /// Committed and locked.
    Committed,
    /// Cancelled/abandoned report.
    Cancelled,
}

impl SrLifecycleStatus {
    /// Return deterministic uppercase status label.
    pub fn as_label(self) -> &'static str {
        match self {
            Self::Draft => "DRAFT",
            Self::Reviewed => "REVIEWED",
            Self::Finalized => "FINALIZED",
            Self::Committed => "COMMITTED",
            Self::Cancelled => "CANCELLED",
        }
    }

    fn can_transition_to(self, target: Self) -> bool {
        matches!(
            (self, target),
            (Self::Draft, Self::Reviewed)
                | (Self::Draft, Self::Cancelled)
                | (Self::Reviewed, Self::Finalized)
                | (Self::Reviewed, Self::Cancelled)
                | (Self::Finalized, Self::Committed)
                | (Self::Finalized, Self::Cancelled)
                | (Self::Committed, Self::Committed)
                | (Self::Cancelled, Self::Cancelled)
        )
    }
}

/// SR lifecycle transition request payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrLifecycleTransitionRequest {
    /// Target SOP Instance UID.
    pub sop_instance_uid: String,
    /// Idempotency key.
    pub idempotency_key: String,
    /// Optional request trace identifier.
    pub request_id: Option<String>,
}

/// SR lifecycle transition outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrLifecycleTransitionOutcome {
    /// Transition operation label.
    pub operation: &'static str,
    /// SOP Instance UID.
    pub sop_instance_uid: String,
    /// Resulting SR lifecycle status.
    pub status: SrLifecycleStatus,
    /// Current document version.
    pub version: u64,
    /// True when served via idempotent replay.
    pub idempotency_replay: bool,
}

/// SR lifecycle history record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrLifecycleHistoryRecord {
    /// Unix milliseconds since epoch.
    pub at_epoch_ms: u64,
    /// Transition operation.
    pub action: &'static str,
    /// Resulting status.
    pub status: SrLifecycleStatus,
    /// Document version at transition.
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SrLifecycleIdempotencyRecord {
    payload_hash: u64,
    outcome: SrLifecycleTransitionOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IdempotencyRecord {
    payload_hash: u64,
    outcome: SrWriteOutcome,
}

/// Privacy-preserving SR audit record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrAuditRecord {
    /// Operation (`create` or `update`).
    pub operation: &'static str,
    /// Outcome label.
    pub outcome: &'static str,
    /// SOP UID hash (never raw UID).
    pub sop_uid_hash: String,
    /// Principal hash (never raw principal).
    pub principal_hash: String,
    /// Idempotency key hash.
    pub idempotency_key_hash: String,
    /// Version after operation.
    pub version: u64,
    /// Request identifier hash.
    pub request_id_hash: String,
}

/// Persistent SR workflow store.
#[derive(Debug, Clone)]
pub struct SrWorkflowStore {
    limits: Limits,
    snapshot_path: String,
    audit_path: String,
    documents: BTreeMap<String, SrAuthoredDocument>,
    lifecycle_status: BTreeMap<String, SrLifecycleStatus>,
    lifecycle_history: BTreeMap<String, Vec<SrLifecycleHistoryRecord>>,
    idempotency: BTreeMap<String, IdempotencyRecord>,
    lifecycle_idempotency: BTreeMap<String, SrLifecycleIdempotencyRecord>,
}

impl SrWorkflowStore {
    /// Open or initialize store from persistence artifacts.
    pub fn open(limits: Limits, snapshot_path: &str, audit_path: &str) -> Result<Self> {
        if let Some(parent) = Path::new(snapshot_path).parent() {
            fs::create_dir_all(parent).map_err(|err| io_error(err.to_string()))?;
        }
        if let Some(parent) = Path::new(audit_path).parent() {
            fs::create_dir_all(parent).map_err(|err| io_error(err.to_string()))?;
        }
        let mut store = Self {
            limits,
            snapshot_path: snapshot_path.to_string(),
            audit_path: audit_path.to_string(),
            documents: BTreeMap::new(),
            lifecycle_status: BTreeMap::new(),
            lifecycle_history: BTreeMap::new(),
            idempotency: BTreeMap::new(),
            lifecycle_idempotency: BTreeMap::new(),
        };
        store.load_snapshot()?;
        Ok(store)
    }

    /// Return all persisted documents in deterministic SOP UID order.
    pub fn documents(&self) -> Vec<&SrAuthoredDocument> {
        self.documents.values().collect()
    }

    /// Return one document by SOP UID.
    pub fn get(&self, sop_instance_uid: &str) -> Option<&SrAuthoredDocument> {
        self.documents.get(sop_instance_uid)
    }

    /// Return current SR lifecycle status.
    pub fn lifecycle_status(&self, sop_instance_uid: &str) -> Option<SrLifecycleStatus> {
        self.lifecycle_status.get(sop_instance_uid).copied()
    }

    /// Return lifecycle history for one report.
    pub fn lifecycle_history(&self, sop_instance_uid: &str) -> Vec<SrLifecycleHistoryRecord> {
        self.lifecycle_history
            .get(sop_instance_uid)
            .cloned()
            .unwrap_or_default()
    }

    /// Create a new SR document with idempotency + auth guards.
    pub fn create(
        &mut self,
        request: SrCreateRequest,
        auth: &SrAuthContext,
    ) -> Result<SrWriteOutcome> {
        enforce_write_auth(auth)?;
        enforce_idempotency_key(&request.idempotency_key, &self.limits)?;
        let payload_hash = stable_payload_hash(&create_payload_key(&request));
        let idempotency_scope = format!("create:{}", request.sop_instance_uid);
        if let Some(replayed) =
            self.replay_if_idempotent(&idempotency_scope, &request.idempotency_key, payload_hash)?
        {
            return Ok(replayed);
        }

        if self.documents.contains_key(&request.sop_instance_uid) {
            return Err(document_exists_error());
        }

        let known = normalize_known_refs(request.known_referenced_sop_instance_uids);
        validate_referenced_item(&request.item, &known)?;
        let authored = SrAuthoringBuilder::new(
            request.study_instance_uid.clone(),
            request.series_instance_uid.clone(),
            request.sop_instance_uid.clone(),
        )
        .with_defaults(SrBuilderDefaults {
            observer: request.observer,
            authored_epoch_ms: request.authored_epoch_ms,
        })
        .push_item(request.item)
        .build();

        let outcome = SrWriteOutcome {
            kind: SrWriteOutcomeKind::Created,
            sop_instance_uid: authored.provenance.sop_instance_uid.clone(),
            version: authored.version,
            idempotency_replay: false,
        };
        self.documents
            .insert(authored.provenance.sop_instance_uid.clone(), authored);
        self.set_status(
            &outcome.sop_instance_uid,
            SrLifecycleStatus::Draft,
            "create",
            outcome.version,
            request_id_hash(&request.request_id),
        )?;
        self.persist_snapshot()?;
        let idempotency_key = request.idempotency_key.clone();
        self.record_idempotency(
            idempotency_scope,
            idempotency_key.clone(),
            payload_hash,
            &outcome,
        );
        self.append_audit(SrAuditRecord {
            operation: "create",
            outcome: "created",
            sop_uid_hash: hash_text(&outcome.sop_instance_uid),
            principal_hash: hash_text(auth.principal.as_deref().unwrap_or("anonymous")),
            idempotency_key_hash: hash_text(&idempotency_key),
            version: outcome.version,
            request_id_hash: hash_text(&request.request_id.clone().unwrap_or_default()),
        })?;
        Ok(outcome)
    }

    /// Update an existing SR document with idempotency + version conflict checks.
    pub fn update(
        &mut self,
        request: SrUpdateEnvelope,
        auth: &SrAuthContext,
    ) -> Result<SrWriteOutcome> {
        enforce_write_auth(auth)?;
        enforce_idempotency_key(&request.idempotency_key, &self.limits)?;
        let payload_hash = stable_payload_hash(&update_payload_key(&request));
        let idempotency_scope = format!("update:{}", request.sop_instance_uid);
        if let Some(replayed) =
            self.replay_if_idempotent(&idempotency_scope, &request.idempotency_key, payload_hash)?
        {
            return Ok(replayed);
        }

        let known = normalize_known_refs(request.known_referenced_sop_instance_uids);
        validate_referenced_item(&request.item, &known)?;

        let document = self
            .documents
            .get_mut(&request.sop_instance_uid)
            .ok_or_else(document_not_found_error)?;
        let update = SrUpdateRequest {
            expected_version: request.expected_version,
            append_items: vec![request.item],
            observer: request.observer,
        };
        apply_sr_update(document, update, &known).map_err(sr_authoring_error)?;

        let outcome = SrWriteOutcome {
            kind: SrWriteOutcomeKind::Updated,
            sop_instance_uid: request.sop_instance_uid.clone(),
            version: document.version,
            idempotency_replay: false,
        };
        self.persist_snapshot()?;
        self.record_idempotency(
            idempotency_scope,
            request.idempotency_key.clone(),
            payload_hash,
            &outcome,
        );
        self.append_audit(SrAuditRecord {
            operation: "update",
            outcome: "updated",
            sop_uid_hash: hash_text(&outcome.sop_instance_uid),
            principal_hash: hash_text(auth.principal.as_deref().unwrap_or("anonymous")),
            idempotency_key_hash: hash_text(&request.idempotency_key),
            version: outcome.version,
            request_id_hash: hash_text(&request.request_id.clone().unwrap_or_default()),
        })?;
        Ok(outcome)
    }

    /// Move SR lifecycle status to REVIEWED.
    pub fn review(
        &mut self,
        request: SrLifecycleTransitionRequest,
        auth: &SrAuthContext,
    ) -> Result<SrLifecycleTransitionOutcome> {
        enforce_write_auth(auth)?;
        self.transition_lifecycle(request, auth, SrLifecycleStatus::Reviewed, "review")
    }

    /// Move SR lifecycle status to FINALIZED.
    pub fn finalize(
        &mut self,
        request: SrLifecycleTransitionRequest,
        auth: &SrAuthContext,
    ) -> Result<SrLifecycleTransitionOutcome> {
        enforce_write_auth(auth)?;
        self.transition_lifecycle(request, auth, SrLifecycleStatus::Finalized, "finalize")
    }

    /// Move SR lifecycle status to COMMITTED.
    pub fn commit(
        &mut self,
        request: SrLifecycleTransitionRequest,
        auth: &SrAuthContext,
    ) -> Result<SrLifecycleTransitionOutcome> {
        enforce_write_auth(auth)?;
        self.transition_lifecycle(request, auth, SrLifecycleStatus::Committed, "commit")
    }

    /// Move SR lifecycle status to CANCELLED.
    pub fn cancel(
        &mut self,
        request: SrLifecycleTransitionRequest,
        auth: &SrAuthContext,
    ) -> Result<SrLifecycleTransitionOutcome> {
        enforce_write_auth(auth)?;
        self.transition_lifecycle(request, auth, SrLifecycleStatus::Cancelled, "cancel")
    }

    fn transition_lifecycle(
        &mut self,
        request: SrLifecycleTransitionRequest,
        auth: &SrAuthContext,
        target: SrLifecycleStatus,
        action: &'static str,
    ) -> Result<SrLifecycleTransitionOutcome> {
        enforce_idempotency_key(&request.idempotency_key, &self.limits)?;
        let payload_hash = stable_payload_hash(action);
        let idempotency_scope = format!("{action}:{}", request.sop_instance_uid);
        if let Some(replayed) = self.replay_transition_if_idempotent(
            &idempotency_scope,
            &request.idempotency_key,
            payload_hash,
        )? {
            return Ok(replayed);
        }

        let status = self
            .lifecycle_status
            .get_mut(&request.sop_instance_uid)
            .ok_or_else(document_not_found_error)?;
        if !status.can_transition_to(target) {
            return Err(sr_invalid_transition_error());
        }
        if *status == target {
            let outcome = SrLifecycleTransitionOutcome {
                operation: action,
                sop_instance_uid: request.sop_instance_uid.clone(),
                status: target,
                version: self
                    .documents
                    .get(&request.sop_instance_uid)
                    .map(|document| document.version)
                    .ok_or_else(document_not_found_error)?,
                idempotency_replay: false,
            };
            self.record_transition_idempotency(
                &idempotency_scope,
                request.idempotency_key.clone(),
                payload_hash,
                &outcome,
            );
            return Ok(outcome);
        }
        *status = target;
        let sop_instance_uid = request.sop_instance_uid;
        let version = self
            .documents
            .get(&sop_instance_uid)
            .map(|document| document.version)
            .ok_or_else(document_not_found_error)?;
        self.set_status(
            &sop_instance_uid,
            target,
            action,
            version,
            request_id_hash(&request.request_id),
        )?;
        self.persist_snapshot()?;
        let outcome = SrLifecycleTransitionOutcome {
            operation: action,
            sop_instance_uid,
            status: target,
            version,
            idempotency_replay: false,
        };
        let idempotency_key = request.idempotency_key;
        self.record_transition_idempotency(
            &idempotency_scope,
            idempotency_key.clone(),
            payload_hash,
            &outcome,
        );
        self.append_audit(SrAuditRecord {
            operation: "update_status",
            outcome: action,
            sop_uid_hash: hash_text(&outcome.sop_instance_uid),
            principal_hash: hash_text(auth.principal.as_deref().unwrap_or("anonymous")),
            idempotency_key_hash: hash_text(&idempotency_key),
            version: outcome.version,
            request_id_hash: hash_text(&request.request_id.unwrap_or_default()),
        })?;
        Ok(outcome)
    }

    fn set_status(
        &mut self,
        sop_instance_uid: &str,
        status: SrLifecycleStatus,
        action: &'static str,
        version: u64,
        request_id_hash: String,
    ) -> Result<()> {
        self.lifecycle_status
            .insert(sop_instance_uid.to_string(), status);
        self.lifecycle_history
            .entry(sop_instance_uid.to_string())
            .or_default()
            .push(SrLifecycleHistoryRecord {
                at_epoch_ms: now_epoch_millis(),
                action,
                status,
                version,
            });
        self.append_audit(SrAuditRecord {
            operation: "status",
            outcome: action,
            sop_uid_hash: hash_text(sop_instance_uid),
            principal_hash: String::new(),
            idempotency_key_hash: String::new(),
            version,
            request_id_hash,
        })?;
        Ok(())
    }

    fn replay_transition_if_idempotent(
        &self,
        scope: &str,
        key: &str,
        payload_hash: u64,
    ) -> Result<Option<SrLifecycleTransitionOutcome>> {
        let index = format!("{scope}:{key}");
        let Some(record) = self.lifecycle_idempotency.get(&index) else {
            return Ok(None);
        };
        if record.payload_hash != payload_hash {
            return Err(idempotency_conflict_error());
        }
        let mut replay = record.outcome.clone();
        replay.idempotency_replay = true;
        Ok(Some(replay))
    }

    fn record_transition_idempotency(
        &mut self,
        scope: &str,
        key: String,
        payload_hash: u64,
        outcome: &SrLifecycleTransitionOutcome,
    ) {
        let index = format!("{scope}:{key}");
        self.lifecycle_idempotency.insert(
            index,
            SrLifecycleIdempotencyRecord {
                payload_hash,
                outcome: outcome.clone(),
            },
        );
    }

    fn replay_if_idempotent(
        &self,
        scope: &str,
        key: &str,
        payload_hash: u64,
    ) -> Result<Option<SrWriteOutcome>> {
        let index = format!("{scope}:{key}");
        let Some(record) = self.idempotency.get(&index) else {
            return Ok(None);
        };
        if record.payload_hash != payload_hash {
            return Err(idempotency_conflict_error());
        }
        let mut replay = record.outcome.clone();
        replay.kind = SrWriteOutcomeKind::Duplicate;
        replay.idempotency_replay = true;
        Ok(Some(replay))
    }

    fn record_idempotency(
        &mut self,
        scope: String,
        key: String,
        payload_hash: u64,
        outcome: &SrWriteOutcome,
    ) {
        let index = format!("{scope}:{key}");
        self.idempotency.insert(
            index,
            IdempotencyRecord {
                payload_hash,
                outcome: outcome.clone(),
            },
        );
    }

    fn load_snapshot(&mut self) -> Result<()> {
        let path = Path::new(&self.snapshot_path);
        if !path.exists() {
            return Ok(());
        }
        let file = OpenOptions::new()
            .read(true)
            .open(path)
            .map_err(|err| io_error(err.to_string()))?;
        let mut lines = BufReader::new(file).lines();
        let header = match lines
            .next()
            .transpose()
            .map_err(|err| io_error(err.to_string()))?
        {
            Some(value) if !value.trim().is_empty() => value,
            _ => return Ok(()),
        };
        if header != SR_STORE_HEADER {
            return Err(snapshot_error());
        }

        let mut docs: BTreeMap<String, SrAuthoredDocument> = BTreeMap::new();
        let mut status = BTreeMap::new();
        let mut history = BTreeMap::new();
        for line in lines {
            let line = line.map_err(|err| io_error(err.to_string()))?;
            if line.trim().is_empty() {
                continue;
            }
            let fields = split_fields(&line)?;
            if fields.len() < 3 {
                return Err(snapshot_error());
            }
            if fields[0] == "DOC" {
                if fields.len() != 8 {
                    return Err(snapshot_error());
                }
                let version = fields[6].parse::<u64>().map_err(|_| snapshot_error())?;
                let authored_epoch_ms = fields[5].parse::<u64>().map_err(|_| snapshot_error())?;
                let sop_uid = unescape_field(fields[1])?;
                let doc = SrAuthoredDocument {
                    provenance: pack_sr::SrProvenance {
                        study_instance_uid: unescape_field(fields[2])?,
                        series_instance_uid: unescape_field(fields[3])?,
                        sop_instance_uid: sop_uid.clone(),
                        observer: unescape_field(fields[4])?,
                        authored_epoch_ms,
                    },
                    items: Vec::new(),
                    version,
                };
                docs.insert(sop_uid, doc);
                continue;
            }
            if fields[0] == "ITEM" {
                if fields.len() < 5 {
                    return Err(snapshot_error());
                }
                let sop_uid = unescape_field(fields[1])?;
                let kind = fields[2];
                let document = docs.get_mut(&sop_uid).ok_or_else(snapshot_error)?;
                let item = parse_item_fields(kind, &fields[3..])?;
                document.items.push(item);
                continue;
            }
            if fields[0] == "HIST" {
                if fields.len() != 6 {
                    return Err(snapshot_error());
                }
                let sop_uid = unescape_field(fields[1])?;
                let action = parse_sr_history_action(fields[2])?;
                let status_next = parse_sr_status(fields[3])?;
                let at_epoch_ms = fields[4].parse::<u64>().map_err(|_| snapshot_error())?;
                let version = fields[5].parse::<u64>().map_err(|_| snapshot_error())?;
                history
                    .entry(sop_uid)
                    .or_insert_with(Vec::new)
                    .push(SrLifecycleHistoryRecord {
                        at_epoch_ms,
                        action,
                        status: status_next,
                        version,
                    });
                continue;
            }
            return Err(snapshot_error());
        }
        for (sop_uid, entries) in &history {
            if !status.contains_key(sop_uid) {
                if let Some(entry) = entries.last() {
                    status.insert(sop_uid.clone(), entry.status);
                }
            }
        }
        self.documents = docs;
        self.lifecycle_status = status;
        self.lifecycle_history = history;
        self.repair_legacy_status();
        Ok(())
    }

    fn persist_snapshot(&self) -> Result<()> {
        let path = Path::new(&self.snapshot_path);
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .map_err(|err| io_error(err.to_string()))?;
        let mut writer = BufWriter::new(file);
        writeln!(writer, "{SR_STORE_HEADER}").map_err(|err| io_error(err.to_string()))?;
        for (sop_uid, doc) in &self.documents {
            writeln!(
                writer,
                "DOC|{}|{}|{}|{}|{}|{}|{}",
                escape_field(sop_uid),
                escape_field(&doc.provenance.study_instance_uid),
                escape_field(&doc.provenance.series_instance_uid),
                escape_field(&doc.provenance.observer),
                doc.provenance.authored_epoch_ms,
                doc.version,
                doc.items.len()
            )
            .map_err(|err| io_error(err.to_string()))?;
            for item in &doc.items {
                write_item_line(&mut writer, sop_uid, item)?;
            }
            if let Some(history) = self.lifecycle_history.get(sop_uid) {
                for entry in history {
                    write_history_line(&mut writer, sop_uid, entry)?;
                }
            }
        }
        writer.flush().map_err(|err| io_error(err.to_string()))?;
        Ok(())
    }

    fn repair_legacy_status(&mut self) {
        for (sop_uid, document) in self.documents.clone() {
            if !self.lifecycle_status.contains_key(&sop_uid) {
                self.lifecycle_status
                    .insert(sop_uid.clone(), SrLifecycleStatus::Draft);
                let legacy = vec![SrLifecycleHistoryRecord {
                    at_epoch_ms: 0,
                    action: "create",
                    status: SrLifecycleStatus::Draft,
                    version: document.version,
                }];
                self.lifecycle_history.insert(sop_uid.clone(), legacy);
                continue;
            }
            self.lifecycle_history.entry(sop_uid).or_default();
        }
    }

    fn append_audit(&self, record: SrAuditRecord) -> Result<()> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.audit_path)
            .map_err(|err| io_error(err.to_string()))?;
        let mut writer = BufWriter::new(file);
        let line = format!(
            "{{\"scope\":\"sr_workflow\",\"operation\":\"{}\",\"outcome\":\"{}\",\"sop_uid_hash\":\"{}\",\"principal_hash\":\"{}\",\"idempotency_key_hash\":\"{}\",\"version\":{},\"request_id_hash\":\"{}\"}}",
            record.operation,
            record.outcome,
            record.sop_uid_hash,
            record.principal_hash,
            record.idempotency_key_hash,
            record.version,
            record.request_id_hash,
        );
        writeln!(writer, "{line}").map_err(|err| io_error(err.to_string()))?;
        writer.flush().map_err(|err| io_error(err.to_string()))?;
        Ok(())
    }
}

const EMPTY_FIELD: &str = "";

fn write_item_line(
    writer: &mut BufWriter<std::fs::File>,
    sop_uid: &str,
    item: &SrAuthoringContentItem,
) -> Result<()> {
    match item {
        SrAuthoringContentItem::Num {
            concept,
            value,
            units,
            referenced_sop_instance_uid,
        } => writeln!(
            writer,
            "ITEM|{}|NUM|{}|{}|{}|{}|{}|{}|{}",
            escape_field(sop_uid),
            encode_code(concept),
            value,
            encode_code(units),
            escape_field(referenced_sop_instance_uid.as_deref().unwrap_or("")),
            EMPTY_FIELD,
            EMPTY_FIELD,
            EMPTY_FIELD
        )
        .map_err(|err| io_error(err.to_string())),
        SrAuthoringContentItem::Text {
            concept,
            text,
            referenced_sop_instance_uid,
        } => writeln!(
            writer,
            "ITEM|{}|TEXT|{}|{}|{}|{}|{}|{}|{}",
            escape_field(sop_uid),
            encode_code(concept),
            escape_field(text),
            escape_field(referenced_sop_instance_uid.as_deref().unwrap_or("")),
            EMPTY_FIELD,
            EMPTY_FIELD,
            EMPTY_FIELD,
            EMPTY_FIELD
        )
        .map_err(|err| io_error(err.to_string())),
        SrAuthoringContentItem::Code {
            concept,
            value,
            referenced_sop_instance_uid,
        } => writeln!(
            writer,
            "ITEM|{}|CODE|{}|{}|{}|{}|{}|{}|{}",
            escape_field(sop_uid),
            encode_code(concept),
            encode_code(value),
            escape_field(referenced_sop_instance_uid.as_deref().unwrap_or("")),
            EMPTY_FIELD,
            EMPTY_FIELD,
            EMPTY_FIELD,
            EMPTY_FIELD
        )
        .map_err(|err| io_error(err.to_string())),
    }
}

fn parse_item_fields(kind: &str, fields: &[&str]) -> Result<SrAuthoringContentItem> {
    match kind {
        "NUM" => {
            if fields.len() < 5 {
                return Err(snapshot_error());
            }
            let concept = decode_code(fields[0])?;
            let value = fields[1].parse::<f64>().map_err(|_| snapshot_error())?;
            let units = decode_code(fields[2])?;
            let referenced_sop_instance_uid = parse_optional(fields[3])?;
            Ok(SrAuthoringContentItem::Num {
                concept,
                value,
                units,
                referenced_sop_instance_uid,
            })
        }
        "TEXT" => {
            if fields.len() < 3 {
                return Err(snapshot_error());
            }
            let concept = decode_code(fields[0])?;
            let text = unescape_field(fields[1])?;
            let referenced_sop_instance_uid = parse_optional(fields[2])?;
            Ok(SrAuthoringContentItem::Text {
                concept,
                text,
                referenced_sop_instance_uid,
            })
        }
        "CODE" => {
            if fields.len() < 3 {
                return Err(snapshot_error());
            }
            let concept = decode_code(fields[0])?;
            let value = decode_code(fields[1])?;
            let referenced_sop_instance_uid = parse_optional(fields[2])?;
            Ok(SrAuthoringContentItem::Code {
                concept,
                value,
                referenced_sop_instance_uid,
            })
        }
        _ => Err(snapshot_error()),
    }
}

fn parse_optional(value: &str) -> Result<Option<String>> {
    let decoded = unescape_field(value)?;
    if decoded.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(decoded))
}

fn normalize_known_refs(mut refs: Vec<String>) -> Vec<String> {
    refs.sort();
    refs.dedup();
    refs
}

fn validate_referenced_item(item: &SrAuthoringContentItem, known: &[String]) -> Result<()> {
    let referenced = match item {
        SrAuthoringContentItem::Num {
            referenced_sop_instance_uid,
            ..
        } => referenced_sop_instance_uid.as_deref(),
        SrAuthoringContentItem::Text {
            referenced_sop_instance_uid,
            ..
        } => referenced_sop_instance_uid.as_deref(),
        SrAuthoringContentItem::Code {
            referenced_sop_instance_uid,
            ..
        } => referenced_sop_instance_uid.as_deref(),
    };
    if let Some(uid) = referenced {
        if !known.iter().any(|value| value == uid) {
            return Err(reference_error(uid));
        }
    }
    Ok(())
}

fn enforce_write_auth(auth: &SrAuthContext) -> Result<()> {
    if !auth.can_write || auth.principal.as_deref().unwrap_or("").trim().is_empty() {
        return Err(auth_denied_error());
    }
    Ok(())
}

fn enforce_idempotency_key(key: &str, limits: &Limits) -> Result<()> {
    if key.trim().is_empty() {
        return Err(idempotency_missing_error());
    }
    if key.len() as u64 > limits.max_string_bytes {
        return Err(Error::from_kind(
            ErrorKind::LimitExceeded {
                limit_name: "max_string_bytes",
                observed: key.len() as u64,
                allowed: limits.max_string_bytes,
            },
            "idempotency key too long",
        )
        .into());
    }
    Ok(())
}

fn create_payload_key(request: &SrCreateRequest) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}|{:?}",
        request.study_instance_uid,
        request.series_instance_uid,
        request.sop_instance_uid,
        request.observer,
        request.authored_epoch_ms,
        request.known_referenced_sop_instance_uids.join(","),
        request.item
    )
}

fn update_payload_key(request: &SrUpdateEnvelope) -> String {
    format!(
        "{}|{}|{:?}|{}|{}|{}",
        request.sop_instance_uid,
        request.expected_version,
        request.item,
        request.observer.as_deref().unwrap_or(""),
        request.known_referenced_sop_instance_uids.join(","),
        request.idempotency_key
    )
}

fn sr_authoring_error(error: SrAuthoringError) -> Box<Error> {
    match error {
        SrAuthoringError::VersionConflict { expected, actual } => Error::new(
            "DVF.WORKFLOW.SR.VERSION_CONFLICT",
            ErrorKind::IntegrityError {
                detail: format!("sr version conflict expected={expected} actual={actual}"),
            },
            "sr version conflict",
        )
        .into(),
        SrAuthoringError::UnknownReferencedSopInstanceUid(uid) => reference_error(&uid),
    }
}

fn now_epoch_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |delta| delta.as_millis() as u64)
}

fn parse_sr_history_action(raw: &str) -> Result<&'static str> {
    match raw {
        "create" => Ok("create"),
        "update" => Ok("update"),
        "review" => Ok("review"),
        "finalize" => Ok("finalize"),
        "commit" => Ok("commit"),
        "cancel" => Ok("cancel"),
        _ => Err(snapshot_error()),
    }
}

fn parse_sr_status(raw: &str) -> Result<SrLifecycleStatus> {
    match raw {
        "DRAFT" => Ok(SrLifecycleStatus::Draft),
        "REVIEWED" => Ok(SrLifecycleStatus::Reviewed),
        "FINALIZED" => Ok(SrLifecycleStatus::Finalized),
        "COMMITTED" => Ok(SrLifecycleStatus::Committed),
        "CANCELLED" => Ok(SrLifecycleStatus::Cancelled),
        _ => Err(snapshot_error()),
    }
}

fn write_history_line(
    writer: &mut BufWriter<std::fs::File>,
    sop_uid: &str,
    entry: &SrLifecycleHistoryRecord,
) -> Result<()> {
    writeln!(
        writer,
        "HIST|{}|{}|{}|{}|{}",
        escape_field(sop_uid),
        entry.action,
        entry.status.as_label(),
        entry.at_epoch_ms,
        entry.version
    )
    .map_err(|err| io_error(err.to_string()))
}

fn reference_error(uid: &str) -> Box<Error> {
    Error::new(
        "DVF.WORKFLOW.SR.UNKNOWN_REFERENCE",
        ErrorKind::InvalidTagValue {
            tag: dicom_core::Tag(0x0008, 0x1155),
            detail: format!("unknown referenced SOP Instance UID {uid}"),
        },
        "unknown referenced SOP Instance UID",
    )
    .into()
}

fn document_exists_error() -> Box<Error> {
    Error::new(
        "DVF.WORKFLOW.SR.ALREADY_EXISTS",
        ErrorKind::IntegrityError {
            detail: "sr document already exists".to_string(),
        },
        "sr document already exists",
    )
    .into()
}

fn document_not_found_error() -> Box<Error> {
    Error::new(
        "DVF.WORKFLOW.SR.NOT_FOUND",
        ErrorKind::DecodeError {
            stage: "sr-workflow".to_string(),
            detail: "sr document not found".to_string(),
        },
        "sr document not found",
    )
    .into()
}

fn idempotency_missing_error() -> Box<Error> {
    Error::new(
        "DVF.WORKFLOW.SR.IDEMPOTENCY_REQUIRED",
        ErrorKind::DecodeError {
            stage: "sr-workflow".to_string(),
            detail: "idempotency key is required".to_string(),
        },
        "idempotency key is required",
    )
    .into()
}

fn idempotency_conflict_error() -> Box<Error> {
    Error::new(
        "DVF.WORKFLOW.SR.IDEMPOTENCY_CONFLICT",
        ErrorKind::IntegrityError {
            detail: "idempotency key replay payload mismatch".to_string(),
        },
        "idempotency key replay payload mismatch",
    )
    .into()
}

fn sr_invalid_transition_error() -> Box<Error> {
    Error::new(
        "DVF.WORKFLOW.SR.INVALID_TRANSITION",
        ErrorKind::IntegrityError {
            detail: "invalid sr lifecycle transition".to_string(),
        },
        "invalid sr lifecycle transition",
    )
    .into()
}

fn auth_denied_error() -> Box<Error> {
    Error::new(
        "DVF.WORKFLOW.SR.AUTH_DENIED",
        ErrorKind::DecodeError {
            stage: "sr-workflow-auth".to_string(),
            detail: "write principal is not authorized for SR operations".to_string(),
        },
        "write principal is not authorized for SR operations",
    )
    .into()
}

fn io_error(detail: String) -> Box<Error> {
    Error::new(
        "DVF.WORKFLOW.SR.IO_ERROR",
        ErrorKind::IoError { detail },
        "sr workflow persistence error",
    )
    .into()
}

fn snapshot_error() -> Box<Error> {
    Error::new(
        "DVF.WORKFLOW.SR.SNAPSHOT_INVALID",
        ErrorKind::DecodeError {
            stage: "sr-workflow-snapshot".to_string(),
            detail: "invalid sr workflow snapshot".to_string(),
        },
        "invalid sr workflow snapshot",
    )
    .into()
}

fn split_fields(line: &str) -> Result<Vec<&str>> {
    let fields: Vec<&str> = line.split('|').collect();
    if fields.is_empty() {
        return Err(snapshot_error());
    }
    Ok(fields)
}

fn encode_code(code: &Code) -> String {
    format!(
        "{},{},{}",
        escape_field(&code.code_value),
        escape_field(&code.scheme),
        escape_field(&code.meaning)
    )
}

fn decode_code(value: &str) -> Result<Code> {
    let mut parts = value.splitn(3, ',');
    let code_value = unescape_field(parts.next().ok_or_else(snapshot_error)?)?;
    let scheme = unescape_field(parts.next().ok_or_else(snapshot_error)?)?;
    let meaning = unescape_field(parts.next().ok_or_else(snapshot_error)?)?;
    Ok(Code {
        code_value,
        scheme,
        meaning,
    })
}

fn escape_field(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\p")
        .replace(',', "\\c")
}

fn unescape_field(value: &str) -> Result<String> {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        let Some(next) = chars.next() else {
            return Err(snapshot_error());
        };
        match next {
            '\\' => out.push('\\'),
            'p' => out.push('|'),
            'c' => out.push(','),
            _ => return Err(snapshot_error()),
        }
    }
    Ok(out)
}

fn stable_payload_hash(input: &str) -> u64 {
    let mut hash = 1469598103934665603u64;
    for byte in input.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    hash
}

fn hash_text(value: &str) -> String {
    format!("{:016x}", stable_payload_hash(value))
}

fn request_id_hash(request_id: &Option<String>) -> String {
    hash_text(request_id.as_deref().unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file_path(name: &str, ext: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("rdvf_{name}_{nonce}.{ext}"))
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
        assert_eq!(err.code, "DVF.WORKFLOW.SR.VERSION_CONFLICT");

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
        assert_eq!(err.code, "DVF.WORKFLOW.SR.AUTH_DENIED");
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
        assert_eq!(err.code, "DVF.WORKFLOW.SR.INVALID_TRANSITION");
    }
}
