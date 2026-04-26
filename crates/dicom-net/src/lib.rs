#![deny(missing_docs)]
#![deny(clippy::cast_possible_truncation)]

//! DICOM UL (Upper Layer) parsing and association negotiation primitives.

use dicom_core::{validate_uid_strict, Error, ErrorKind, Result, Tag};
use std::collections::BTreeSet;

const APPLICATION_CONTEXT_UID: &str = "1.2.840.10008.3.1.1.1";
const TAG_UID: Tag = Tag(0x0000, 0x0000);

/// Configurable networking limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkLimits {
    /// Max PDU value bytes (excludes 6-byte PDU header).
    pub max_pdu_bytes: u64,
    /// Max PDV value bytes (excludes the PDV 2-byte header).
    pub max_pdv_bytes: u64,
    /// Max presentation contexts in an association request.
    pub max_presentation_contexts: u64,
}

impl Default for NetworkLimits {
    fn default() -> Self {
        Self {
            max_pdu_bytes: 1024 * 1024,
            max_pdv_bytes: 256 * 1024,
            max_presentation_contexts: 128,
        }
    }
}

/// A parsed PDU (Protocol Data Unit).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pdu {
    /// Association request.
    AssociateRq(AssociationRequest),
    /// Association accept.
    AssociateAc(AssociationAccept),
    /// Association reject.
    AssociateRj(AssociationReject),
    /// P-DATA-TF data transfer.
    PDataTf(Vec<Pdv>),
    /// Release request.
    ReleaseRq,
    /// Release response.
    ReleaseRp,
    /// Abort.
    Abort(Abort),
}

/// Association request fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssociationRequest {
    /// Called AE title.
    pub called_ae: String,
    /// Calling AE title.
    pub calling_ae: String,
    /// Application context UID.
    pub application_context: String,
    /// Presentation contexts.
    pub presentation_contexts: Vec<PresentationContext>,
    /// Requested max PDU length.
    pub max_pdu_length: u32,
    /// Optional implementation class UID.
    pub implementation_class_uid: Option<String>,
    /// Optional implementation version name.
    pub implementation_version_name: Option<String>,
}

/// Association accept fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssociationAccept {
    /// Called AE title.
    pub called_ae: String,
    /// Calling AE title.
    pub calling_ae: String,
    /// Application context UID.
    pub application_context: String,
    /// Accepted presentation contexts.
    pub presentation_contexts: Vec<PresentationContextAccept>,
    /// Negotiated max PDU length.
    pub max_pdu_length: u32,
    /// Optional implementation class UID.
    pub implementation_class_uid: Option<String>,
    /// Optional implementation version name.
    pub implementation_version_name: Option<String>,
}

/// Association reject details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssociationReject {
    /// Reject result.
    pub result: u8,
    /// Reject source.
    pub source: u8,
    /// Reject reason.
    pub reason: u8,
}

/// Abort details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abort {
    /// Abort source.
    pub source: u8,
    /// Abort reason.
    pub reason: u8,
}

/// Presentation context in an association request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationContext {
    /// Presentation context identifier (odd).
    pub id: u8,
    /// Abstract syntax UID.
    pub abstract_syntax: String,
    /// Transfer syntax UID list.
    pub transfer_syntaxes: Vec<String>,
}

/// Presentation context result in an association accept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationContextAccept {
    /// Presentation context identifier.
    pub id: u8,
    /// Result reason (0x00 accepted, 0x03 abstract syntax not supported, 0x04 transfer syntax not supported).
    pub result: u8,
    /// Accepted transfer syntax (if accepted).
    pub transfer_syntax: Option<String>,
}

/// A Presentation Data Value (PDV) within a P-DATA-TF PDU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pdv {
    /// Presentation context identifier.
    pub presentation_context_id: u8,
    /// Message control header byte.
    pub message_control_header: u8,
    /// PDV payload bytes.
    pub data: Vec<u8>,
}

impl Pdv {
    /// True if this PDV carries a command fragment.
    pub fn is_command(&self) -> bool {
        self.message_control_header & 0x01 == 0x01
    }

    /// True if this PDV is the last fragment in a message.
    pub fn is_last(&self) -> bool {
        self.message_control_header & 0x02 == 0x02
    }
}

/// Association acceptance policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssociationPolicy {
    /// Optional expected called AE title.
    pub called_ae: Option<String>,
    /// Supported abstract syntax UIDs.
    pub supported_abstract_syntaxes: Vec<String>,
    /// Supported transfer syntax UIDs (ordered by preference).
    pub supported_transfer_syntaxes: Vec<String>,
    /// Max PDU length this acceptor will use.
    pub max_pdu_length: u32,
}

impl AssociationPolicy {
    /// Default policy for Verification SOP class over Implicit VR Little Endian.
    pub fn verification_default() -> Self {
        Self {
            called_ae: None,
            supported_abstract_syntaxes: vec!["1.2.840.10008.1.1".to_string()],
            supported_transfer_syntaxes: vec!["1.2.840.10008.1.2".to_string()],
            max_pdu_length: 16_384,
        }
    }
}

/// Association role (requestor or acceptor).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssociationRole {
    /// Association requestor (SCU).
    Requestor,
    /// Association acceptor (SCP).
    Acceptor,
}

/// Association lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssociationState {
    /// No association established.
    Idle,
    /// Requestor sent A-ASSOCIATE-RQ and awaits response.
    AwaitingAssociateResponse,
    /// Acceptor received A-ASSOCIATE-RQ and awaits local response.
    AwaitingLocalResponse,
    /// Association is established.
    Established,
    /// Release requested by local side.
    ReleaseRequestedByLocal,
    /// Release requested by remote side.
    ReleaseRequestedByRemote,
    /// Association is closed.
    Closed,
}

/// UL PDU type for association sequencing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssociationPduType {
    /// A-ASSOCIATE-RQ.
    AssociateRq,
    /// A-ASSOCIATE-AC.
    AssociateAc,
    /// A-ASSOCIATE-RJ.
    AssociateRj,
    /// P-DATA-TF.
    PDataTf,
    /// A-RELEASE-RQ.
    ReleaseRq,
    /// A-RELEASE-RP.
    ReleaseRp,
    /// A-ABORT.
    Abort,
}

impl From<&Pdu> for AssociationPduType {
    fn from(pdu: &Pdu) -> Self {
        match pdu {
            Pdu::AssociateRq(_) => AssociationPduType::AssociateRq,
            Pdu::AssociateAc(_) => AssociationPduType::AssociateAc,
            Pdu::AssociateRj(_) => AssociationPduType::AssociateRj,
            Pdu::PDataTf(_) => AssociationPduType::PDataTf,
            Pdu::ReleaseRq => AssociationPduType::ReleaseRq,
            Pdu::ReleaseRp => AssociationPduType::ReleaseRp,
            Pdu::Abort(_) => AssociationPduType::Abort,
        }
    }
}

/// Association event (send or receive a PDU type).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssociationEvent {
    /// PDU sent by the local side.
    Send(AssociationPduType),
    /// PDU received from the remote side.
    Receive(AssociationPduType),
}

impl AssociationEvent {
    fn pdu_type(self) -> AssociationPduType {
        match self {
            AssociationEvent::Send(pdu_type) | AssociationEvent::Receive(pdu_type) => pdu_type,
        }
    }
}

/// Association state machine enforcing UL sequencing rules.
#[derive(Debug, Clone)]
pub struct AssociationStateMachine {
    role: AssociationRole,
    state: AssociationState,
}

impl AssociationStateMachine {
    /// Create a new association state machine.
    pub fn new(role: AssociationRole) -> Self {
        Self {
            role,
            state: AssociationState::Idle,
        }
    }

    /// Return the current association state.
    pub fn state(&self) -> AssociationState {
        self.state
    }

    /// Apply a send/receive event to the state machine.
    pub fn on_event(&mut self, event: AssociationEvent) -> Result<()> {
        if event.pdu_type() == AssociationPduType::Abort {
            self.state = AssociationState::Closed;
            return Ok(());
        }

        let next = match (self.role, self.state, event) {
            (
                AssociationRole::Requestor,
                AssociationState::Idle,
                AssociationEvent::Send(AssociationPduType::AssociateRq),
            ) => AssociationState::AwaitingAssociateResponse,
            (
                AssociationRole::Acceptor,
                AssociationState::Idle,
                AssociationEvent::Receive(AssociationPduType::AssociateRq),
            ) => AssociationState::AwaitingLocalResponse,
            (
                AssociationRole::Requestor,
                AssociationState::AwaitingAssociateResponse,
                AssociationEvent::Receive(AssociationPduType::AssociateAc),
            ) => AssociationState::Established,
            (
                AssociationRole::Requestor,
                AssociationState::AwaitingAssociateResponse,
                AssociationEvent::Receive(AssociationPduType::AssociateRj),
            ) => AssociationState::Closed,
            (
                AssociationRole::Acceptor,
                AssociationState::AwaitingLocalResponse,
                AssociationEvent::Send(AssociationPduType::AssociateAc),
            ) => AssociationState::Established,
            (
                AssociationRole::Acceptor,
                AssociationState::AwaitingLocalResponse,
                AssociationEvent::Send(AssociationPduType::AssociateRj),
            ) => AssociationState::Closed,
            (
                _,
                AssociationState::Established,
                AssociationEvent::Send(AssociationPduType::PDataTf),
            )
            | (
                _,
                AssociationState::Established,
                AssociationEvent::Receive(AssociationPduType::PDataTf),
            ) => AssociationState::Established,
            (
                _,
                AssociationState::Established,
                AssociationEvent::Send(AssociationPduType::ReleaseRq),
            ) => AssociationState::ReleaseRequestedByLocal,
            (
                _,
                AssociationState::Established,
                AssociationEvent::Receive(AssociationPduType::ReleaseRq),
            ) => AssociationState::ReleaseRequestedByRemote,
            (
                _,
                AssociationState::ReleaseRequestedByLocal,
                AssociationEvent::Receive(AssociationPduType::ReleaseRp),
            ) => AssociationState::Closed,
            (
                _,
                AssociationState::ReleaseRequestedByRemote,
                AssociationEvent::Send(AssociationPduType::ReleaseRp),
            ) => AssociationState::Closed,
            (_, AssociationState::Closed, _) => {
                return Err(state_error(self.state, event));
            }
            _ => {
                return Err(state_error(self.state, event));
            }
        };

        self.state = next;
        Ok(())
    }

    /// Apply a PDU sent by the local side.
    pub fn on_pdu_sent(&mut self, pdu: &Pdu) -> Result<()> {
        self.on_event(AssociationEvent::Send(AssociationPduType::from(pdu)))
    }

    /// Apply a PDU received from the remote side.
    pub fn on_pdu_received(&mut self, pdu: &Pdu) -> Result<()> {
        self.on_event(AssociationEvent::Receive(AssociationPduType::from(pdu)))
    }
}

/// Parse a single PDU from the provided buffer.
pub fn parse_pdu(input: &[u8], limits: &NetworkLimits) -> Result<Pdu> {
    let mut pdus = parse_pdu_stream(input, limits)?;
    if pdus.len() != 1 {
        return Err(decode_error("expected exactly one PDU"));
    }
    Ok(pdus.remove(0))
}

/// Parse a stream of concatenated PDUs from a buffer.
pub fn parse_pdu_stream(input: &[u8], limits: &NetworkLimits) -> Result<Vec<Pdu>> {
    let mut cursor = Cursor::new(input);
    let mut pdus = Vec::new();
    while cursor.remaining() > 0 {
        if cursor.remaining() < 6 {
            return Err(decode_error("truncated PDU header"));
        }
        let pdu_type = cursor.read_u8()?;
        let _reserved = cursor.read_u8()?;
        let length = usize::try_from(cursor.read_u32_be()?).map_err(|_| decode_error("PDU length exceeds usize"))?;
        enforce_limit("max_pdu_bytes", u64::try_from(length).map_err(|_| decode_error("PDU length exceeds u64"))?, limits.max_pdu_bytes)?;
        if cursor.remaining() < length {
            return Err(decode_error("PDU length exceeds buffer"));
        }
        let body = cursor.take(length)?;
        let pdu = parse_pdu_body(pdu_type, body, limits)?;
        pdus.push(pdu);
    }
    Ok(pdus)
}

/// Encode a PDU into bytes for transmission.
pub fn encode_pdu(pdu: &Pdu, limits: &NetworkLimits) -> Result<Vec<u8>> {
    let body = match pdu {
        Pdu::AssociateRq(request) => encode_associate_rq(request, limits)?,
        Pdu::AssociateAc(accept) => encode_associate_ac(accept, limits)?,
        Pdu::AssociateRj(reject) => encode_associate_rj(reject),
        Pdu::PDataTf(pdvs) => encode_pdata(pdvs, limits)?,
        Pdu::ReleaseRq => vec![0u8; 4],
        Pdu::ReleaseRp => vec![0u8; 4],
        Pdu::Abort(abort) => encode_abort(abort),
    };
    wrap_pdu(pdu_type(pdu), &body, limits)
}

/// Accept an association request based on a policy.
pub fn accept_association(
    request: &AssociationRequest,
    policy: &AssociationPolicy,
    limits: &NetworkLimits,
) -> Result<AssociationAccept> {
    if let Some(expected) = &policy.called_ae {
        if &request.called_ae != expected {
            return Err(decode_error("called AE title not allowed"));
        }
    }

    let mut accepted = Vec::with_capacity(request.presentation_contexts.len());
    for context in &request.presentation_contexts {
        if u64::try_from(accepted.len()).unwrap_or(u64::MAX) >= limits.max_presentation_contexts {
            return Err(limit_exceeded(
                "max_presentation_contexts",
                u64::try_from(accepted.len()).unwrap_or(u64::MAX),
                limits.max_presentation_contexts,
            ));
        }
        let mut result = 0x03;
        let mut chosen = None;
        if policy
            .supported_abstract_syntaxes
            .iter()
            .any(|uid| uid == &context.abstract_syntax)
        {
            for ts in &policy.supported_transfer_syntaxes {
                if context.transfer_syntaxes.iter().any(|cand| cand == ts) {
                    result = 0x00;
                    chosen = Some(ts.clone());
                    break;
                }
            }
            if result != 0x00 {
                result = 0x04;
            }
        }
        accepted.push(PresentationContextAccept {
            id: context.id,
            result,
            transfer_syntax: chosen,
        });
    }

    let negotiated_max_pdu = std::cmp::min(request.max_pdu_length, policy.max_pdu_length);

    Ok(AssociationAccept {
        called_ae: request.called_ae.clone(),
        calling_ae: request.calling_ae.clone(),
        application_context: request.application_context.clone(),
        presentation_contexts: accepted,
        max_pdu_length: negotiated_max_pdu,
        implementation_class_uid: request.implementation_class_uid.clone(),
        implementation_version_name: request.implementation_version_name.clone(),
    })
}

fn parse_pdu_body(pdu_type: u8, body: &[u8], limits: &NetworkLimits) -> Result<Pdu> {
    match pdu_type {
        0x01 => parse_associate_rq(body, limits).map(Pdu::AssociateRq),
        0x02 => parse_associate_ac(body, limits).map(Pdu::AssociateAc),
        0x03 => parse_associate_rj(body).map(Pdu::AssociateRj),
        0x04 => parse_pdata(body, limits).map(Pdu::PDataTf),
        0x05 => parse_release(body).map(|_| Pdu::ReleaseRq),
        0x06 => parse_release(body).map(|_| Pdu::ReleaseRp),
        0x07 => parse_abort(body).map(Pdu::Abort),
        _ => Err(decode_error("unsupported PDU type")),
    }
}

fn pdu_type(pdu: &Pdu) -> u8 {
    match pdu {
        Pdu::AssociateRq(_) => 0x01,
        Pdu::AssociateAc(_) => 0x02,
        Pdu::AssociateRj(_) => 0x03,
        Pdu::PDataTf(_) => 0x04,
        Pdu::ReleaseRq => 0x05,
        Pdu::ReleaseRp => 0x06,
        Pdu::Abort(_) => 0x07,
    }
}

fn wrap_pdu(pdu_type: u8, body: &[u8], limits: &NetworkLimits) -> Result<Vec<u8>> {
    enforce_limit("max_pdu_bytes", u64::try_from(body.len()).map_err(|_| decode_error("PDU body length exceeds u64"))?, limits.max_pdu_bytes)?;
    let mut out = Vec::with_capacity(body.len() + 6);
    out.push(pdu_type);
    out.push(0x00);
    out.extend_from_slice(&u32::try_from(body.len()).map_err(|_| decode_error("PDU body length exceeds u32"))?.to_be_bytes());
    out.extend_from_slice(body);
    Ok(out)
}

fn encode_associate_rq(request: &AssociationRequest, limits: &NetworkLimits) -> Result<Vec<u8>> {
    let mut body = encode_association_header(&request.called_ae, &request.calling_ae)?;
    body.extend_from_slice(&encode_uid_item(0x10, &request.application_context)?);
    if request.presentation_contexts.is_empty() {
        return Err(decode_error("missing presentation contexts"));
    }
    enforce_limit(
        "max_presentation_contexts",
        u64::try_from(request.presentation_contexts.len()).unwrap_or(u64::MAX),
        limits.max_presentation_contexts,
    )?;
    for context in &request.presentation_contexts {
        body.extend_from_slice(&encode_presentation_context_rq(context)?);
    }
    body.extend_from_slice(&encode_user_info(
        request.max_pdu_length,
        request.implementation_class_uid.as_deref(),
        request.implementation_version_name.as_deref(),
    )?);
    Ok(body)
}

fn encode_associate_ac(accept: &AssociationAccept, limits: &NetworkLimits) -> Result<Vec<u8>> {
    let mut body = encode_association_header(&accept.called_ae, &accept.calling_ae)?;
    body.extend_from_slice(&encode_uid_item(0x10, &accept.application_context)?);
    if accept.presentation_contexts.is_empty() {
        return Err(decode_error("missing presentation contexts"));
    }
    enforce_limit(
        "max_presentation_contexts",
        u64::try_from(accept.presentation_contexts.len()).unwrap_or(u64::MAX),
        limits.max_presentation_contexts,
    )?;
    for context in &accept.presentation_contexts {
        body.extend_from_slice(&encode_presentation_context_ac(context)?);
    }
    body.extend_from_slice(&encode_user_info(
        accept.max_pdu_length,
        accept.implementation_class_uid.as_deref(),
        accept.implementation_version_name.as_deref(),
    )?);
    Ok(body)
}

fn encode_associate_rj(reject: &AssociationReject) -> Vec<u8> {
    vec![0x00, reject.result, reject.source, reject.reason]
}

fn encode_abort(abort: &Abort) -> Vec<u8> {
    vec![0x00, 0x00, abort.source, abort.reason]
}

fn encode_pdata(pdvs: &[Pdv], limits: &NetworkLimits) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    for pdv in pdvs {
        let pdv_len = pdv.data.len().checked_add(2).ok_or_else(|| decode_error("PDV length overflow"))?;
        enforce_limit("max_pdv_bytes", u64::try_from(pdv_len).map_err(|_| decode_error("PDV length exceeds u64"))?, limits.max_pdv_bytes)?;
        body.extend_from_slice(&u32::try_from(pdv_len).map_err(|_| decode_error("PDV length exceeds u32"))?.to_be_bytes());
        body.push(pdv.presentation_context_id);
        body.push(pdv.message_control_header);
        body.extend_from_slice(&pdv.data);
    }
    Ok(body)
}

fn encode_association_header(called_ae: &str, calling_ae: &str) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    body.extend_from_slice(&1u16.to_be_bytes());
    body.extend_from_slice(&0u16.to_be_bytes());
    body.extend_from_slice(&encode_ae_title(called_ae)?);
    body.extend_from_slice(&encode_ae_title(calling_ae)?);
    body.extend_from_slice(&[0u8; 32]);
    Ok(body)
}

fn encode_ae_title(title: &str) -> Result<[u8; 16]> {
    if title.is_empty() || title.len() > 16 {
        return Err(decode_error("AE title length is invalid"));
    }
    let mut out = [b' '; 16];
    for (idx, ch) in title.bytes().enumerate() {
        if !(0x20..=0x7e).contains(&ch) {
            return Err(decode_error("AE title contains non-ASCII characters"));
        }
        out[idx] = ch;
    }
    Ok(out)
}

fn encode_uid_item(item_type: u8, uid: &str) -> Result<Vec<u8>> {
    validate_uid_strict(TAG_UID, uid)?;
    encode_item(item_type, uid.as_bytes())
}

fn encode_text_item(item_type: u8, text: &str) -> Result<Vec<u8>> {
    if text.is_empty() {
        return Err(decode_error("text item must not be empty"));
    }
    if text.bytes().any(|b| !(0x20..=0x7e).contains(&b)) {
        return Err(decode_error("text item must be ASCII"));
    }
    encode_item(item_type, text.as_bytes())
}

fn encode_item(item_type: u8, body: &[u8]) -> Result<Vec<u8>> {
    if body.len() > usize::from(u16::MAX) {
        return Err(decode_error("item length exceeds u16"));
    }
    let mut out = Vec::with_capacity(body.len() + 4);
    out.push(item_type);
    out.push(0x00);
    out.extend_from_slice(&u16::try_from(body.len()).map_err(|_| decode_error("item length exceeds u16"))?.to_be_bytes());
    out.extend_from_slice(body);
    Ok(out)
}

fn encode_presentation_context_rq(context: &PresentationContext) -> Result<Vec<u8>> {
    if context.id == 0 || context.id.is_multiple_of(2) {
        return Err(decode_error("presentation context ID must be odd"));
    }
    if context.transfer_syntaxes.is_empty() {
        return Err(decode_error(
            "presentation context requires transfer syntax",
        ));
    }
    let mut body = Vec::new();
    body.push(context.id);
    body.push(0x00);
    body.extend_from_slice(&0u16.to_be_bytes());
    body.extend_from_slice(&encode_uid_item(0x30, &context.abstract_syntax)?);
    for ts in &context.transfer_syntaxes {
        body.extend_from_slice(&encode_uid_item(0x40, ts)?);
    }
    encode_item(0x20, &body)
}

fn encode_presentation_context_ac(context: &PresentationContextAccept) -> Result<Vec<u8>> {
    if context.id == 0 || context.id.is_multiple_of(2) {
        return Err(decode_error("presentation context ID must be odd"));
    }
    let mut body = vec![context.id, 0x00, context.result, 0x00];
    if context.result == 0x00 {
        let ts = context
            .transfer_syntax
            .as_ref()
            .ok_or_else(|| decode_error("accepted context missing transfer syntax"))?;
        body.extend_from_slice(&encode_uid_item(0x40, ts)?);
    }
    encode_item(0x21, &body)
}

fn encode_user_info(
    max_pdu_length: u32,
    impl_class_uid: Option<&str>,
    impl_version_name: Option<&str>,
) -> Result<Vec<u8>> {
    if max_pdu_length == 0 {
        return Err(decode_error("max PDU length must be non-zero"));
    }
    let mut body = Vec::new();
    let mut max_pdu = vec![0x51, 0x00];
    max_pdu.extend_from_slice(&4u16.to_be_bytes());
    max_pdu.extend_from_slice(&max_pdu_length.to_be_bytes());
    body.extend_from_slice(&max_pdu);
    if let Some(uid) = impl_class_uid {
        body.extend_from_slice(&encode_uid_item(0x52, uid)?);
    }
    if let Some(version) = impl_version_name {
        body.extend_from_slice(&encode_text_item(0x55, version)?);
    }
    encode_item(0x50, &body)
}

fn parse_associate_rq(body: &[u8], limits: &NetworkLimits) -> Result<AssociationRequest> {
    let mut cursor = Cursor::new(body);
    let (called_ae, calling_ae) = parse_associate_header(&mut cursor)?;
    let mut application_context = None;
    let mut contexts = Vec::new();
    let mut max_pdu_length = None;
    let mut impl_class_uid = None;
    let mut impl_version = None;
    let mut context_ids = BTreeSet::new();

    while cursor.remaining() > 0 {
        let (item_type, item_body) = parse_item(&mut cursor)?;
        match item_type {
            0x10 => {
                if application_context.is_some() {
                    return Err(decode_error("duplicate application context"));
                }
                let uid = parse_uid(&item_body)?;
                if uid != APPLICATION_CONTEXT_UID {
                    return Err(decode_error("unsupported application context"));
                }
                application_context = Some(uid);
            }
            0x20 => {
                if u64::try_from(contexts.len()).unwrap_or(u64::MAX) >= limits.max_presentation_contexts {
                    return Err(limit_exceeded(
                        "max_presentation_contexts",
                        u64::try_from(contexts.len()).unwrap_or(u64::MAX),
                        limits.max_presentation_contexts,
                    ));
                }
                let ctx = parse_presentation_context_rq(&item_body)?;
                if !context_ids.insert(ctx.id) {
                    return Err(decode_error("duplicate presentation context ID"));
                }
                contexts.push(ctx);
            }
            0x50 => {
                let (max_pdu, class_uid, version) = parse_user_info(&item_body)?;
                max_pdu_length = Some(max_pdu);
                impl_class_uid = class_uid;
                impl_version = version;
            }
            _ => return Err(decode_error("unsupported association item type")),
        }
    }

    let application_context =
        application_context.ok_or_else(|| decode_error("missing application context"))?;
    let max_pdu_length = max_pdu_length.ok_or_else(|| decode_error("missing max PDU length"))?;
    if contexts.is_empty() {
        return Err(decode_error("missing presentation contexts"));
    }

    Ok(AssociationRequest {
        called_ae,
        calling_ae,
        application_context,
        presentation_contexts: contexts,
        max_pdu_length,
        implementation_class_uid: impl_class_uid,
        implementation_version_name: impl_version,
    })
}

fn parse_associate_ac(body: &[u8], limits: &NetworkLimits) -> Result<AssociationAccept> {
    let mut cursor = Cursor::new(body);
    let (called_ae, calling_ae) = parse_associate_header(&mut cursor)?;
    let mut application_context = None;
    let mut contexts = Vec::new();
    let mut max_pdu_length = None;
    let mut impl_class_uid = None;
    let mut impl_version = None;
    let mut context_ids = BTreeSet::new();

    while cursor.remaining() > 0 {
        let (item_type, item_body) = parse_item(&mut cursor)?;
        match item_type {
            0x10 => {
                if application_context.is_some() {
                    return Err(decode_error("duplicate application context"));
                }
                let uid = parse_uid(&item_body)?;
                if uid != APPLICATION_CONTEXT_UID {
                    return Err(decode_error("unsupported application context"));
                }
                application_context = Some(uid);
            }
            0x21 => {
                if u64::try_from(contexts.len()).unwrap_or(u64::MAX) >= limits.max_presentation_contexts {
                    return Err(limit_exceeded(
                        "max_presentation_contexts",
                        u64::try_from(contexts.len()).unwrap_or(u64::MAX),
                        limits.max_presentation_contexts,
                    ));
                }
                let ctx = parse_presentation_context_ac(&item_body)?;
                if !context_ids.insert(ctx.id) {
                    return Err(decode_error("duplicate presentation context ID"));
                }
                contexts.push(ctx);
            }
            0x50 => {
                let (max_pdu, class_uid, version) = parse_user_info(&item_body)?;
                max_pdu_length = Some(max_pdu);
                impl_class_uid = class_uid;
                impl_version = version;
            }
            _ => return Err(decode_error("unsupported association item type")),
        }
    }

    let application_context =
        application_context.ok_or_else(|| decode_error("missing application context"))?;
    let max_pdu_length = max_pdu_length.ok_or_else(|| decode_error("missing max PDU length"))?;
    if contexts.is_empty() {
        return Err(decode_error("missing presentation contexts"));
    }

    Ok(AssociationAccept {
        called_ae,
        calling_ae,
        application_context,
        presentation_contexts: contexts,
        max_pdu_length,
        implementation_class_uid: impl_class_uid,
        implementation_version_name: impl_version,
    })
}

fn parse_associate_rj(body: &[u8]) -> Result<AssociationReject> {
    if body.len() != 4 {
        return Err(decode_error("invalid A-ASSOCIATE-RJ length"));
    }
    Ok(AssociationReject {
        result: body[1],
        source: body[2],
        reason: body[3],
    })
}

fn parse_release(body: &[u8]) -> Result<()> {
    if body.len() != 4 {
        return Err(decode_error("invalid A-RELEASE length"));
    }
    Ok(())
}

fn parse_abort(body: &[u8]) -> Result<Abort> {
    if body.len() != 4 {
        return Err(decode_error("invalid A-ABORT length"));
    }
    Ok(Abort {
        source: body[2],
        reason: body[3],
    })
}

fn parse_pdata(body: &[u8], limits: &NetworkLimits) -> Result<Vec<Pdv>> {
    let mut cursor = Cursor::new(body);
    let mut pdvs = Vec::new();
    while cursor.remaining() > 0 {
        if cursor.remaining() < 4 {
            return Err(decode_error("truncated PDV header"));
        }
        let pdv_len = usize::try_from(cursor.read_u32_be()?).map_err(|_| decode_error("PDV length exceeds usize"))?;
        enforce_limit("max_pdv_bytes", u64::try_from(pdv_len).map_err(|_| decode_error("PDV length exceeds u64"))?, limits.max_pdv_bytes)?;
        if pdv_len < 2 {
            return Err(decode_error("invalid PDV length"));
        }
        if cursor.remaining() < pdv_len {
            return Err(decode_error("PDV length exceeds buffer"));
        }
        let presentation_context_id = cursor.read_u8()?;
        let message_control_header = cursor.read_u8()?;
        let data_len = pdv_len - 2;
        let data = cursor.take(data_len)?.to_vec();
        pdvs.push(Pdv {
            presentation_context_id,
            message_control_header,
            data,
        });
    }
    Ok(pdvs)
}

fn parse_associate_header(cursor: &mut Cursor<'_>) -> Result<(String, String)> {
    if cursor.remaining() < 68 {
        return Err(decode_error("association header too short"));
    }
    let protocol_version = cursor.read_u16_be()?;
    let _reserved = cursor.read_u16_be()?;
    if protocol_version != 1 {
        return Err(decode_error("unsupported protocol version"));
    }
    let called_ae = parse_ae_title(cursor.take(16)?)?;
    let calling_ae = parse_ae_title(cursor.take(16)?)?;
    let _reserved = cursor.take(32)?;
    Ok((called_ae, calling_ae))
}

fn parse_item(cursor: &mut Cursor<'_>) -> Result<(u8, Vec<u8>)> {
    if cursor.remaining() < 4 {
        return Err(decode_error("truncated UL item header"));
    }
    let item_type = cursor.read_u8()?;
    let _reserved = cursor.read_u8()?;
    let item_length = usize::from(cursor.read_u16_be()?);
    if cursor.remaining() < item_length {
        return Err(decode_error("UL item length exceeds buffer"));
    }
    let body = cursor.take(item_length)?.to_vec();
    Ok((item_type, body))
}

fn parse_presentation_context_rq(item_body: &[u8]) -> Result<PresentationContext> {
    let mut cursor = Cursor::new(item_body);
    if cursor.remaining() < 4 {
        return Err(decode_error("presentation context header too short"));
    }
    let id = cursor.read_u8()?;
    let _reserved = cursor.read_u8()?;
    let _reserved = cursor.read_u16_be()?;
    if id == 0 || id % 2 == 0 {
        return Err(decode_error("presentation context ID must be odd"));
    }
    let mut abstract_syntax = None;
    let mut transfer_syntaxes = Vec::new();
    while cursor.remaining() > 0 {
        let (item_type, body) = parse_item(&mut cursor)?;
        match item_type {
            0x30 => {
                if abstract_syntax.is_some() {
                    return Err(decode_error("duplicate abstract syntax"));
                }
                abstract_syntax = Some(parse_uid(&body)?);
            }
            0x40 => transfer_syntaxes.push(parse_uid(&body)?),
            _ => return Err(decode_error("unsupported presentation context item")),
        }
    }
    let abstract_syntax = abstract_syntax.ok_or_else(|| decode_error("missing abstract syntax"))?;
    if transfer_syntaxes.is_empty() {
        return Err(decode_error("missing transfer syntax"));
    }
    Ok(PresentationContext {
        id,
        abstract_syntax,
        transfer_syntaxes,
    })
}

fn parse_presentation_context_ac(item_body: &[u8]) -> Result<PresentationContextAccept> {
    let mut cursor = Cursor::new(item_body);
    if cursor.remaining() < 4 {
        return Err(decode_error("presentation context AC header too short"));
    }
    let id = cursor.read_u8()?;
    let _reserved = cursor.read_u8()?;
    let result = cursor.read_u8()?;
    let _reserved = cursor.read_u8()?;
    if id == 0 || id % 2 == 0 {
        return Err(decode_error("presentation context ID must be odd"));
    }
    let mut transfer_syntax = None;
    while cursor.remaining() > 0 {
        let (item_type, body) = parse_item(&mut cursor)?;
        match item_type {
            0x40 => {
                if transfer_syntax.is_some() {
                    return Err(decode_error("duplicate transfer syntax"));
                }
                transfer_syntax = Some(parse_uid(&body)?);
            }
            _ => return Err(decode_error("unsupported presentation context item")),
        }
    }
    Ok(PresentationContextAccept {
        id,
        result,
        transfer_syntax,
    })
}

fn parse_user_info(item_body: &[u8]) -> Result<(u32, Option<String>, Option<String>)> {
    let mut cursor = Cursor::new(item_body);
    let mut max_pdu_length = None;
    let mut impl_class_uid = None;
    let mut impl_version = None;
    while cursor.remaining() > 0 {
        let (item_type, body) = parse_item(&mut cursor)?;
        match item_type {
            0x51 => {
                if body.len() != 4 {
                    return Err(decode_error("invalid max PDU length item"));
                }
                let value = u32::from_be_bytes([body[0], body[1], body[2], body[3]]);
                max_pdu_length = Some(value);
            }
            0x52 => {
                impl_class_uid = Some(parse_uid(&body)?);
            }
            0x55 => {
                let value = parse_text(&body)?;
                impl_version = Some(value);
            }
            _ => return Err(decode_error("unsupported user info item")),
        }
    }
    let max_pdu_length = max_pdu_length.ok_or_else(|| decode_error("missing max PDU length"))?;
    Ok((max_pdu_length, impl_class_uid, impl_version))
}

fn parse_ae_title(raw: &[u8]) -> Result<String> {
    if raw.len() != 16 {
        return Err(decode_error("AE title length must be 16 bytes"));
    }
    if raw.iter().any(|&b| !(0x20..=0x7e).contains(&b)) {
        return Err(decode_error("AE title contains non-ASCII characters"));
    }
    let text = std::str::from_utf8(raw).map_err(|_| decode_error("AE title is not valid UTF-8"))?;
    let trimmed = text.trim_end_matches(' ');
    if trimmed.is_empty() {
        return Err(decode_error("AE title must not be empty"));
    }
    Ok(trimmed.to_string())
}

fn parse_uid(raw: &[u8]) -> Result<String> {
    let text = std::str::from_utf8(raw).map_err(|_| decode_error("UID is not UTF-8"))?;
    let trimmed = text.trim_end_matches('\0').trim_end_matches(' ');
    validate_uid_strict(TAG_UID, trimmed)?;
    Ok(trimmed.to_string())
}

fn parse_text(raw: &[u8]) -> Result<String> {
    let text = std::str::from_utf8(raw).map_err(|_| decode_error("text item is not UTF-8"))?;
    Ok(text
        .trim_end_matches('\0')
        .trim_end_matches(' ')
        .to_string())
}

fn decode_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::DecodeError {
            stage: "dicom-net".to_string(),
            detail: detail.into(),
        },
        "decode error",
    )
    .into()
}

fn state_error(state: AssociationState, event: AssociationEvent) -> Box<Error> {
    decode_error(format!(
        "invalid association transition: state={state:?}, event={event:?}"
    ))
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

fn enforce_limit(limit_name: &'static str, observed: u64, allowed: u64) -> Result<()> {
    if observed > allowed {
        return Err(limit_exceeded(limit_name, observed, allowed));
    }
    Ok(())
}

struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        if self.remaining() < len {
            return Err(decode_error("truncated buffer"));
        }
        let start = self.pos;
        self.pos += len;
        Ok(&self.buf[start..start + len])
    }

    fn read_u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn read_u16_be(&mut self) -> Result<u16> {
        let bytes = self.take(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32_be(&mut self) -> Result<u32> {
        let bytes = self.take(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_core::ErrorKind;

    fn build_uid_item(item_type: u8, uid: &str) -> Vec<u8> {
        let mut out = vec![item_type, 0x00];
        out.extend_from_slice(&u16::try_from(uid.len()).unwrap_or_else(|_| panic!("UID item too long")).to_be_bytes());
        out.extend_from_slice(uid.as_bytes());
        out
    }

    fn build_item(item_type: u8, body: &[u8]) -> Vec<u8> {
        let mut out = vec![item_type, 0x00];
        out.extend_from_slice(&u16::try_from(body.len()).unwrap_or_else(|_| panic!("item body too long")).to_be_bytes());
        out.extend_from_slice(body);
        out
    }

    fn build_presentation_context_rq(id: u8) -> Vec<u8> {
        let mut body = Vec::new();
        body.push(id);
        body.push(0x00);
        body.extend_from_slice(&0u16.to_be_bytes());
        body.extend_from_slice(&build_uid_item(0x30, "1.2.840.10008.1.1"));
        body.extend_from_slice(&build_uid_item(0x40, "1.2.840.10008.1.2"));
        build_item(0x20, &body)
    }

    fn build_user_info() -> Vec<u8> {
        let mut body = Vec::new();
        let mut max_pdu = vec![0x51, 0x00];
        max_pdu.extend_from_slice(&4u16.to_be_bytes());
        max_pdu.extend_from_slice(&16_384u32.to_be_bytes());
        body.extend_from_slice(&max_pdu);
        build_item(0x50, &body)
    }

    fn build_associate_rq() -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&1u16.to_be_bytes());
        body.extend_from_slice(&0u16.to_be_bytes());
        body.extend_from_slice(b"CALLED_AE       ");
        body.extend_from_slice(b"CALLING_AE      ");
        body.extend_from_slice(&[0u8; 32]);
        body.extend_from_slice(&build_uid_item(0x10, APPLICATION_CONTEXT_UID));
        body.extend_from_slice(&build_presentation_context_rq(0x01));
        body.extend_from_slice(&build_user_info());

        let mut pdu = vec![0x01, 0x00];
        pdu.extend_from_slice(&u32::try_from(body.len()).unwrap_or_else(|_| panic!("PDU body too long")).to_be_bytes());
        pdu.extend_from_slice(&body);
        pdu
    }

    #[test]
    fn parse_associate_rq_basic() {
        // REQ-NET-300: PDU length must be validated and parsed deterministically.
        let pdu = build_associate_rq();
        let parsed = parse_pdu(&pdu, &NetworkLimits::default()).expect("parse");
        match parsed {
            Pdu::AssociateRq(req) => {
                assert_eq!(req.called_ae, "CALLED_AE");
                assert_eq!(req.calling_ae, "CALLING_AE");
                assert_eq!(req.application_context, APPLICATION_CONTEXT_UID);
                assert_eq!(req.presentation_contexts.len(), 1);
            }
            _ => panic!("expected associate rq"),
        }
    }

    #[test]
    fn encode_associate_rq_roundtrip() {
        // REQ-NET-300: encoded association PDUs must parse deterministically.
        let request = AssociationRequest {
            called_ae: "CALLED_AE".to_string(),
            calling_ae: "CALLING_AE".to_string(),
            application_context: APPLICATION_CONTEXT_UID.to_string(),
            presentation_contexts: vec![PresentationContext {
                id: 0x01,
                abstract_syntax: "1.2.840.10008.1.1".to_string(),
                transfer_syntaxes: vec!["1.2.840.10008.1.2".to_string()],
            }],
            max_pdu_length: 16_384,
            implementation_class_uid: None,
            implementation_version_name: None,
        };
        let pdu = Pdu::AssociateRq(request.clone());
        let bytes = encode_pdu(&pdu, &NetworkLimits::default()).expect("encode");
        let parsed = parse_pdu(&bytes, &NetworkLimits::default()).expect("parse");
        match parsed {
            Pdu::AssociateRq(decoded) => {
                assert_eq!(decoded.called_ae, request.called_ae);
                assert_eq!(decoded.calling_ae, request.calling_ae);
                assert_eq!(decoded.presentation_contexts.len(), 1);
            }
            _ => panic!("expected associate rq"),
        }
    }

    #[test]
    fn encode_pdata_roundtrip() {
        // REQ-NET-303: PDV encoding must be deterministic.
        let pdv = Pdv {
            presentation_context_id: 1,
            message_control_header: 0x03,
            data: vec![1, 2, 3],
        };
        let bytes = encode_pdu(&Pdu::PDataTf(vec![pdv.clone()]), &NetworkLimits::default())
            .expect("encode");
        let parsed = parse_pdu(&bytes, &NetworkLimits::default()).expect("parse");
        match parsed {
            Pdu::PDataTf(pdvs) => {
                assert_eq!(pdvs.len(), 1);
                assert_eq!(pdvs[0], pdv);
            }
            _ => panic!("expected pdata"),
        }
    }

    #[test]
    fn accept_association_negotiates_transfer_syntax() {
        // REQ-NET-305: Association negotiation selects the first supported transfer syntax and result codes.
        let request = AssociationRequest {
            called_ae: "CALLED_AE".to_string(),
            calling_ae: "CALLING_AE".to_string(),
            application_context: APPLICATION_CONTEXT_UID.to_string(),
            presentation_contexts: vec![
                PresentationContext {
                    id: 0x01,
                    abstract_syntax: "1.2.3".to_string(),
                    transfer_syntaxes: vec!["1.2.840.10008.1.2".to_string()],
                },
                PresentationContext {
                    id: 0x03,
                    abstract_syntax: "1.2.3".to_string(),
                    transfer_syntaxes: vec!["1.2.840.10008.1.2.1".to_string()],
                },
                PresentationContext {
                    id: 0x05,
                    abstract_syntax: "9.9.9".to_string(),
                    transfer_syntaxes: vec!["1.2.840.10008.1.2".to_string()],
                },
            ],
            max_pdu_length: 32_768,
            implementation_class_uid: None,
            implementation_version_name: None,
        };
        let policy = AssociationPolicy {
            called_ae: None,
            supported_abstract_syntaxes: vec!["1.2.3".to_string()],
            supported_transfer_syntaxes: vec!["1.2.840.10008.1.2".to_string()],
            max_pdu_length: 16_384,
        };

        let accept =
            accept_association(&request, &policy, &NetworkLimits::default()).expect("accept");
        assert_eq!(accept.max_pdu_length, 16_384);
        assert_eq!(accept.presentation_contexts.len(), 3);
        assert_eq!(accept.presentation_contexts[0].result, 0x00);
        assert_eq!(
            accept.presentation_contexts[0].transfer_syntax.as_deref(),
            Some("1.2.840.10008.1.2")
        );
        assert_eq!(accept.presentation_contexts[1].result, 0x04);
        assert_eq!(accept.presentation_contexts[2].result, 0x03);
    }

    #[test]
    fn association_state_machine_requestor_flow() {
        // REQ-NET-306: Association state machine enforces valid UL sequencing.
        let mut fsm = AssociationStateMachine::new(AssociationRole::Requestor);
        fsm.on_event(AssociationEvent::Send(AssociationPduType::AssociateRq))
            .expect("send rq");
        fsm.on_event(AssociationEvent::Receive(AssociationPduType::AssociateAc))
            .expect("receive ac");
        fsm.on_event(AssociationEvent::Send(AssociationPduType::PDataTf))
            .expect("send data");
        fsm.on_event(AssociationEvent::Receive(AssociationPduType::ReleaseRq))
            .expect("receive release");
        fsm.on_event(AssociationEvent::Send(AssociationPduType::ReleaseRp))
            .expect("send release rp");
        assert_eq!(fsm.state(), AssociationState::Closed);
    }

    #[test]
    fn association_state_machine_rejects_out_of_order() {
        // REQ-NET-306: Out-of-order UL PDUs fail closed.
        // REQ-NET-307: UL sequencing failures surface DecodeError with dicom-net stage.
        let mut fsm = AssociationStateMachine::new(AssociationRole::Requestor);
        let err = fsm
            .on_event(AssociationEvent::Send(AssociationPduType::PDataTf))
            .expect_err("error");
        match err.kind() {
            ErrorKind::DecodeError { stage, .. } => {
                assert_eq!(stage, "dicom-net");
            }
            _ => panic!("expected decode error"),
        }
    }

    #[test]
    fn parse_rejects_invalid_uid() {
        // REQ-NET-304: Invalid UIDs must fail closed.
        let mut pdu = build_associate_rq();
        let idx = pdu
            .windows(APPLICATION_CONTEXT_UID.len())
            .position(|w| w == APPLICATION_CONTEXT_UID.as_bytes())
            .expect("uid position");
        pdu[idx] = b'x';
        let err = parse_pdu(&pdu, &NetworkLimits::default()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::InvalidTagValue { .. }));
    }

    #[test]
    fn parse_rejects_invalid_ae_title() {
        // REQ-NET-301: AE titles must be ASCII and non-empty.
        let mut pdu = build_associate_rq();
        let start = 6 + 4;
        for byte in &mut pdu[start..start + 16] {
            *byte = b' ';
        }
        let err = parse_pdu(&pdu, &NetworkLimits::default()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn parse_rejects_duplicate_presentation_context_ids() {
        // REQ-NET-307: Presentation context IDs must be odd and unique.
        let mut body = Vec::new();
        body.extend_from_slice(&1u16.to_be_bytes());
        body.extend_from_slice(&0u16.to_be_bytes());
        body.extend_from_slice(b"CALLED_AE       ");
        body.extend_from_slice(b"CALLING_AE      ");
        body.extend_from_slice(&[0u8; 32]);
        body.extend_from_slice(&build_uid_item(0x10, APPLICATION_CONTEXT_UID));
        body.extend_from_slice(&build_presentation_context_rq(0x01));
        body.extend_from_slice(&build_presentation_context_rq(0x01));
        body.extend_from_slice(&build_user_info());

        let mut pdu = vec![0x01, 0x00];
        pdu.extend_from_slice(&u32::try_from(body.len()).unwrap_or_else(|_| panic!("PDU body too long")).to_be_bytes());
        pdu.extend_from_slice(&body);

        let err = parse_pdu(&pdu, &NetworkLimits::default()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn parse_rejects_even_presentation_context_id() {
        // REQ-NET-307: Presentation context IDs must be odd and unique.
        let mut body = Vec::new();
        body.extend_from_slice(&1u16.to_be_bytes());
        body.extend_from_slice(&0u16.to_be_bytes());
        body.extend_from_slice(b"CALLED_AE       ");
        body.extend_from_slice(b"CALLING_AE      ");
        body.extend_from_slice(&[0u8; 32]);
        body.extend_from_slice(&build_uid_item(0x10, APPLICATION_CONTEXT_UID));
        body.extend_from_slice(&build_presentation_context_rq(0x02));
        body.extend_from_slice(&build_user_info());

        let mut pdu = vec![0x01, 0x00];
        pdu.extend_from_slice(&u32::try_from(body.len()).unwrap_or_else(|_| panic!("PDU body too long")).to_be_bytes());
        pdu.extend_from_slice(&body);

        let err = parse_pdu(&pdu, &NetworkLimits::default()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn parse_rejects_unsupported_application_context() {
        // REQ-NET-302: Application context must match the DICOM UID.
        let mut pdu = build_associate_rq();
        let idx = pdu
            .windows(APPLICATION_CONTEXT_UID.len())
            .position(|w| w == APPLICATION_CONTEXT_UID.as_bytes())
            .expect("uid position");
        pdu[idx + APPLICATION_CONTEXT_UID.len() - 1] = b'2';
        let err = parse_pdu(&pdu, &NetworkLimits::default()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn parse_enforces_pdu_limit() {
        // REQ-SEC-401: default limits are enforced for networking PDUs.
        let pdu = build_associate_rq();
        let limits = NetworkLimits {
            max_pdu_bytes: 64,
            ..NetworkLimits::default()
        };
        let err = parse_pdu(&pdu, &limits).expect_err("error");
        match err.kind() {
            ErrorKind::LimitExceeded { limit_name, .. } => {
                assert_eq!(*limit_name, "max_pdu_bytes");
            }
            _ => panic!("expected limit exceeded"),
        }
    }

    #[test]
    fn presentation_context_limit_enforced() {
        // REQ-SEC-401: presentation context limits are enforced.
        let mut body = Vec::new();
        body.extend_from_slice(&1u16.to_be_bytes());
        body.extend_from_slice(&0u16.to_be_bytes());
        body.extend_from_slice(b"CALLED_AE       ");
        body.extend_from_slice(b"CALLING_AE      ");
        body.extend_from_slice(&[0u8; 32]);
        body.extend_from_slice(&build_uid_item(0x10, APPLICATION_CONTEXT_UID));
        body.extend_from_slice(&build_presentation_context_rq(0x01));
        body.extend_from_slice(&build_presentation_context_rq(0x03));
        body.extend_from_slice(&build_user_info());

        let mut pdu = vec![0x01, 0x00];
        pdu.extend_from_slice(&u32::try_from(body.len()).unwrap_or_else(|_| panic!("PDU body too long")).to_be_bytes());
        pdu.extend_from_slice(&body);

        let limits = NetworkLimits {
            max_presentation_contexts: 1,
            ..NetworkLimits::default()
        };
        let err = parse_pdu(&pdu, &limits).expect_err("error");
        match err.kind() {
            ErrorKind::LimitExceeded { limit_name, .. } => {
                assert_eq!(*limit_name, "max_presentation_contexts");
            }
            _ => panic!("expected limit exceeded"),
        }
    }

    #[test]
    fn pdv_length_limit_enforced() {
        // REQ-NET-303: PDV limits fail closed.
        let mut body = Vec::new();
        let pdv_len = 300u32;
        body.extend_from_slice(&pdv_len.to_be_bytes());
        body.push(0x01);
        body.push(0x03);
        body.extend_from_slice(&vec![0u8; usize::try_from(pdv_len - 2).unwrap_or(0)]);
        let mut pdu = vec![0x04, 0x00];
        pdu.extend_from_slice(&u32::try_from(body.len()).unwrap_or_else(|_| panic!("PDU body too long")).to_be_bytes());
        pdu.extend_from_slice(&body);
        let limits = NetworkLimits {
            max_pdv_bytes: 128,
            ..NetworkLimits::default()
        };
        let err = parse_pdu(&pdu, &limits).expect_err("error");
        match err.kind() {
            ErrorKind::LimitExceeded { limit_name, .. } => {
                assert_eq!(*limit_name, "max_pdv_bytes");
            }
            _ => panic!("expected limit exceeded"),
        }
    }
}
