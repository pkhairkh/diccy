#![deny(missing_docs)]

//! Runtime contracts for DICOMweb startup policy, persistence preflight,
//! recovery visibility, and worker/queue backpressure behavior.

use dicom_storage::Storage;
use dicom_web::{TlsPolicy, TransportSecurity};
use std::fs::{self, OpenOptions};
use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use std::path::{Path, PathBuf};

/// Startup diagnostics for environment-derived runtime policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartupPolicyDiagnostics {
    /// Effective TLS policy.
    pub tls_policy: TlsPolicy,
    /// Effective auth mode label.
    pub auth_mode: &'static str,
    /// Effective request transport classification.
    pub transport_security: TransportSecurity,
    /// True when `DICOM_WEB_TLS_POLICY` had an invalid value and fell back.
    pub tls_defaulted: bool,
    /// True when `DICOM_WEB_AUTH_MODE` had an invalid value and fell back.
    pub auth_defaulted: bool,
    /// True when `DICOM_WEB_TRANSPORT_SECURITY` had an invalid value and fell back.
    pub transport_defaulted: bool,
}

impl StartupPolicyDiagnostics {
    /// Return whether the resulting policy posture is fail-closed by default.
    pub fn fail_closed(&self) -> bool {
        self.auth_mode == "deny_all"
            || (self.tls_policy == TlsPolicy::RequireTls
                && self.transport_security == TransportSecurity::Insecure)
    }
}

/// Resolve startup policy values and fallback indicators from optional raw inputs.
pub fn startup_policy_diagnostics(
    tls_policy_raw: Option<&str>,
    auth_mode_raw: Option<&str>,
    transport_raw: Option<&str>,
) -> StartupPolicyDiagnostics {
    let (tls_policy, tls_defaulted) = match tls_policy_raw {
        Some("allow_insecure") => (TlsPolicy::AllowInsecure, false),
        Some("require_tls") | None => (TlsPolicy::RequireTls, false),
        Some(_) => (TlsPolicy::RequireTls, true),
    };
    let (auth_mode, auth_defaulted) = match auth_mode_raw {
        Some("allow_all") => ("allow_all", false),
        Some("deny_all") | None => ("deny_all", false),
        Some(_) => ("deny_all", true),
    };
    let (transport_security, transport_defaulted) = match transport_raw {
        Some("tls") => (TransportSecurity::Tls, false),
        Some("insecure") | None => (TransportSecurity::Insecure, false),
        Some(_) => (TransportSecurity::Insecure, true),
    };
    StartupPolicyDiagnostics {
        tls_policy,
        auth_mode,
        transport_security,
        tls_defaulted,
        auth_defaulted,
        transport_defaulted,
    }
}

/// Preflight diagnostics for file-backed runtime persistence artifacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistencePreflightDiagnostic {
    /// Human label for the artifact.
    pub label: String,
    /// Artifact file path.
    pub path: String,
    /// Whether the parent directory had to be created.
    pub parent_created: bool,
    /// Whether file-open readiness checks completed.
    pub file_ready: bool,
    /// Whether rollover rotation was performed due to `max_bytes` overflow.
    pub rotated: bool,
    /// Configured retained rotation depth.
    pub max_rotated_files: usize,
    /// Whether rollover used zero-retention destructive mode.
    pub destructive_rollover: bool,
}

/// Ensure persistence file readiness and return deterministic preflight diagnostics.
pub fn prepare_persistence_file_with_diagnostics(
    path: &str,
    max_bytes: u64,
    max_rotated_files: usize,
    label: &str,
) -> std::io::Result<PersistencePreflightDiagnostic> {
    let path = Path::new(path);
    let mut parent_created = false;
    if let Some(parent) = path.parent() {
        parent_created = !parent.exists();
        fs::create_dir_all(parent)?;
    }

    let mut rotated = false;
    if let Ok(meta) = fs::metadata(path) {
        if meta.is_dir() {
            return Err(IoError::new(
                IoErrorKind::InvalidInput,
                format!(
                    "{label} path must reference a file, not a directory; choose a file target path"
                ),
            ));
        }
        if meta.len() > max_bytes {
            rotate_file(path, max_rotated_files)?;
            rotated = true;
        }
    }

    let file = OpenOptions::new().create(true).append(true).open(path)?;
    file.sync_all()?;

    Ok(PersistencePreflightDiagnostic {
        label: label.to_string(),
        path: path.display().to_string(),
        parent_created,
        file_ready: true,
        rotated,
        max_rotated_files,
        destructive_rollover: rotated && max_rotated_files == 0,
    })
}

/// Rotate a persistence artifact with bounded suffix depth.
pub fn rotate_file(path: &Path, max_rotated_files: usize) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if max_rotated_files == 0 {
        fs::remove_file(path)?;
        return Ok(());
    }

    let oldest = path_with_suffix(path, max_rotated_files);
    if oldest.exists() {
        fs::remove_file(&oldest)?;
    }

    for index in (1..=max_rotated_files).rev() {
        let src = if index == 1 {
            path.to_path_buf()
        } else {
            path_with_suffix(path, index - 1)
        };
        let dst = path_with_suffix(path, index);
        if src.exists() {
            fs::rename(src, dst)?;
        }
    }
    Ok(())
}

/// Build a deterministic rotation suffix path (`.1`, `.2`, ...).
pub fn path_with_suffix(path: &Path, suffix_index: usize) -> PathBuf {
    let mut os = path.as_os_str().to_os_string();
    os.push(format!(".{suffix_index}"));
    PathBuf::from(os)
}

/// Deterministic worker/accept-queue contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkerQueueContract {
    /// Number of request workers.
    pub workers: usize,
    /// Accept queue depth.
    pub queue_depth: usize,
}

/// Validate worker/queue sizing and reject invalid zero values.
pub fn worker_queue_contract(
    workers: usize,
    queue_depth: usize,
) -> std::io::Result<WorkerQueueContract> {
    if workers == 0 {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            "DICOM_WEB_WORKERS must be >= 1",
        ));
    }
    if queue_depth == 0 {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            "DICOM_WEB_ACCEPT_QUEUE_DEPTH must be >= 1",
        ));
    }
    Ok(WorkerQueueContract {
        workers,
        queue_depth,
    })
}

/// Deterministic accept-queue saturation state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueBackpressureState {
    /// Queue has available capacity.
    Healthy,
    /// Queue is saturated.
    Saturated,
}

/// Classify queue occupancy against queue capacity.
pub fn classify_accept_queue_state(
    queued_connections: usize,
    queue_depth: usize,
) -> QueueBackpressureState {
    if queue_depth == 0 || queued_connections >= queue_depth {
        QueueBackpressureState::Saturated
    } else {
        QueueBackpressureState::Healthy
    }
}

/// Deterministic operator guidance for queue backpressure state.
pub fn queue_backpressure_guidance(state: QueueBackpressureState) -> &'static str {
    match state {
        QueueBackpressureState::Healthy => "accept queue healthy",
        QueueBackpressureState::Saturated => {
            "accept queue saturated; allow workers to drain before retrying"
        }
    }
}

/// Deterministic storage recovery summary for startup diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageRecoveryDiagnostic {
    /// Whether persistence is durable (`path`-backed).
    pub durable: bool,
    /// Persistence artifact path when durable mode is active.
    pub persistence_path: Option<String>,
    /// Number of recovered datasets available after open/restart.
    pub recovered_records: usize,
}

/// Build a recovery summary from an opened storage instance.
pub fn storage_recovery_diagnostic(
    storage: &Storage,
) -> std::io::Result<StorageRecoveryDiagnostic> {
    let recovered_records = storage
        .datasets()
        .map_err(|err| IoError::other(format!("failed to read recovered datasets: {err}")))?
        .len();
    Ok(StorageRecoveryDiagnostic {
        durable: storage.persistence_path().is_some(),
        persistence_path: storage
            .persistence_path()
            .map(|path| path.display().to_string()),
        recovered_records,
    })
}
