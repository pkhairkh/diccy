//! DIMSE server (TCP listener), client, TLS configuration, and network transport code.

use crate::commitment::parse_storage_commitment_n_action_request;
#[cfg(any(
    feature = "dimse-c-find",
    feature = "dimse-c-move",
    feature = "dimse-c-get"
))]
use crate::middleware::enforce_query_response_limit;
use crate::middleware::{
    enforce_connection_limit, message_is_enabled, message_needs_data, validate_data_set_size,
    DimseOperationResourceLimits, DimseOperationSizeConfig, OperationLimiter,
};
#[cfg(feature = "dimse-c-find")]
use crate::protocol::CFindRequest;
#[cfg(feature = "dimse-c-get")]
use crate::protocol::CGetRequest;
#[cfg(feature = "dimse-c-move")]
use crate::protocol::CMoveRequest;
use crate::protocol::{
    dimse_protocol_violation_status, AssociationInfo, CEchoRequest, CStoreRequest, DimseRoleConfig,
    DimseService, DimseStatus,
};
#[cfg(any(
    feature = "dimse-c-find",
    feature = "dimse-c-move",
    feature = "dimse-c-get"
))]
use crate::protocol::{is_pending_status, validate_query_status_sequence};
use crate::{
    decode_error, io_error, limit_exceeded, workstation_default_association_policy,
    SOP_CLASS_CR_IMAGE_STORAGE, SOP_CLASS_CT_IMAGE_STORAGE, SOP_CLASS_DX_PRESENTATION,
    SOP_CLASS_MR_IMAGE_STORAGE, SOP_CLASS_MULTI_FRAME_SC_BYTE, SOP_CLASS_MULTI_FRAME_SC_TRUE_COLOR,
    SOP_CLASS_MULTI_FRAME_SC_WORD, SOP_CLASS_PET_IMAGE_STORAGE, SOP_CLASS_SECONDARY_CAPTURE,
    SOP_CLASS_STUDY_ROOT_FIND, SOP_CLASS_STUDY_ROOT_GET, SOP_CLASS_STUDY_ROOT_MOVE,
    SOP_CLASS_VERIFICATION,
};
use dicom_audit::{AuditEvent, AuditEventKind, AuditField, AuditValue};
use dicom_auth::{
    AllowAll, AuthAction, AuthDecision, AuthDenyReason, AuthRequest, AuthResource, AuthResourceKey,
    AuthScope, AuthSubject, Authorizer, DenyAll,
};
use dicom_core::{Limits, Result};
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
use dicom_net::{
    accept_association, encode_pdu, parse_pdu, AssociationAccept, AssociationPolicy,
    AssociationReject, AssociationRequest, NetworkLimits, Pdu, Pdv,
};
use std::collections::HashMap;
use std::fmt;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

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

/// Audit callback for authorization decisions.
///
/// # S13-T7 — Read-Only Trait Object
///
/// `Fn(AuditEvent) -> Result<()>` takes `&self`, so `Arc<AuditCallback>`
/// is safe for concurrent read-only invocation.
pub type AuditCallback = Arc<dyn Fn(AuditEvent) -> Result<()> + Send + Sync>;

/// Authorization and audit configuration for DIMSE services.
///
/// # S13-T7 — Read-Only `Arc<dyn Authorizer>`
///
/// The `Authorizer` trait's `authorize` method takes `&self`, so
/// `Arc<dyn Authorizer + Send + Sync>` is safe to share without
/// additional synchronization.
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

impl Default for DimseAuthConfig {
    /// Default is deny-all (fail closed) per S13-T5.
    fn default() -> Self {
        Self::deny_all()
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
                Limits::default().max_input_bytes(),
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
                    enforce_query_response_limit(
                        responses_seen,
                        crate::middleware::DEFAULT_MAX_QUERY_RESPONSE_COUNT,
                    )?;
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
                    enforce_query_response_limit(
                        responses_seen,
                        crate::middleware::DEFAULT_MAX_QUERY_RESPONSE_COUNT,
                    )?;
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
                    enforce_query_response_limit(
                        responses_seen,
                        crate::middleware::DEFAULT_MAX_QUERY_RESPONSE_COUNT,
                    )?;
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

// ---------------------------------------------------------------------------
// Connection handling
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DimseOperationAuditPhase {
    Start,
    Response,
    Completion,
    Failure,
}

pub(crate) fn dimse_operation_audit_phase_label(phase: DimseOperationAuditPhase) -> &'static str {
    match phase {
        DimseOperationAuditPhase::Start => "start",
        DimseOperationAuditPhase::Response => "response",
        DimseOperationAuditPhase::Completion => "completion",
        DimseOperationAuditPhase::Failure => "failure",
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
    pub(crate) fn effective_policy(&self) -> AssociationPolicy {
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
pub(crate) fn handle_pdus<H: DimseService>(
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
            if new_len as u64 > config.limits.max_input_bytes() {
                return Err(limit_exceeded(
                    "max_input_bytes",
                    new_len as u64,
                    config.limits.max_input_bytes(),
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

// ---------------------------------------------------------------------------
// Authorization and audit
// ---------------------------------------------------------------------------

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
            key: AuthResourceKey::Instance,
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

// ---------------------------------------------------------------------------
// PDU encoding / decoding
// ---------------------------------------------------------------------------

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
            .find(|ctx| ctx.id() == accepted.id)
        {
            map.insert(req.abstract_syntax().to_string(), accepted.id);
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
            .find(|ctx| ctx.id() == accepted.id)
        {
            map.insert(
                accepted.id,
                (req.abstract_syntax().to_string(), transfer_syntax),
            );
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
        contexts.push(dicom_net::PresentationContext::new(
            id,
            abstract_syntax.clone(),
            policy.supported_transfer_syntaxes.clone(),
        )?);
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
pub(crate) struct PendingMessage {
    command: Vec<u8>,
    command_done: bool,
    data: Vec<u8>,
    data_done: bool,
    message: Option<DimseMessage>,
}

// ---------------------------------------------------------------------------
// Transport security and host filtering
// ---------------------------------------------------------------------------

pub(crate) fn enforce_tls_policy(transport: TransportSecurity, policy: TlsPolicy) -> Result<()> {
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
