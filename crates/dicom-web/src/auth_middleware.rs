//! Authentication/authorization middleware types and functions.

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
use dicom_audit::{AuditEventKind, AuditField, AuditValue};
#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
use dicom_auth::{AuthDenyReason, AuthDecision};

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
use dicom_auth::{enforce_tenant_scope, AuthAction, AuthRequest, AuthResource, AuthResourceKey, AuthSubject};

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
use super::{
    AuditCallback, DicomWebRequest, WebAuthConfig,
};

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct WebAuthSubject {
    pub(crate) principal: Option<String>,
    pub(crate) peer: Option<String>,
    pub(crate) tenant_id: Option<String>,
    pub(crate) tenant_scope: Option<String>,
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
impl WebAuthSubject {
    pub(crate) fn as_auth_subject(&self) -> AuthSubject<'_> {
        AuthSubject {
            principal: self.principal.as_deref(),
            peer: self.peer.as_deref(),
        }
    }
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
pub(crate) fn derive_auth_subject(request: &super::WebRequest) -> WebAuthSubject {
    use super::header_value;

    let principal = header_value(&request.headers, "x-auth-principal")
        .or_else(|| header_value(&request.headers, "x-forwarded-user"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());

    let peer = header_value(&request.headers, "x-peer-id")
        .or_else(|| header_value(&request.headers, "x-real-ip"))
        .or_else(|| {
            header_value(&request.headers, "x-forwarded-for")
                .and_then(|value| value.split(',').next())
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
        .map(|value| value.to_string());
    let tenant_id = header_value(&request.headers, "x-tenant-id")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let tenant_scope = header_value(&request.headers, "x-tenant-scope")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());

    WebAuthSubject {
        principal,
        peer,
        tenant_id,
        tenant_scope,
    }
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
pub(crate) fn authorize_and_audit_web(
    auth: &WebAuthConfig,
    request: &DicomWebRequest,
    subject: &WebAuthSubject,
) -> super::Result<()> {
    use super::request_requires_write;

    if request_requires_write(request) {
        enforce_tenant_scope(
            subject.tenant_scope.as_deref(),
            subject.tenant_id.as_deref(),
        )?;
    }
    let auth_request = AuthRequest {
        scope: dicom_auth::AuthScope::Dicomweb,
        action: auth_action_for_request(request),
        subject: subject.as_auth_subject(),
        resource: auth_resource_for_request(request),
    };
    let decision = auth.authorizer.authorize(&auth_request)?;
    record_auth_audit(
        &auth.audit,
        auth_request.action,
        auth_request.resource,
        decision,
        subject.tenant_id.as_deref(),
        dicomweb_operation_label(request),
    )?;
    decision.enforce()
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
fn auth_action_for_request(request: &DicomWebRequest) -> AuthAction {
    match request {
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoStudies { .. }
        | DicomWebRequest::QidoAllSeries { .. }
        | DicomWebRequest::QidoAllInstances { .. }
        | DicomWebRequest::QidoSeries { .. }
        | DicomWebRequest::QidoStudyInstances { .. }
        | DicomWebRequest::QidoInstances { .. } => AuthAction::Query,
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoInstance { .. }
        | DicomWebRequest::WadoStudyRetrieve { .. }
        | DicomWebRequest::WadoSeriesRetrieve { .. }
        | DicomWebRequest::WadoStudyMetadata { .. }
        | DicomWebRequest::WadoSeriesMetadata { .. }
        | DicomWebRequest::WadoInstanceMetadata { .. }
        | DicomWebRequest::WadoRendered { .. }
        | DicomWebRequest::WadoBulkData { .. } => AuthAction::Retrieve,
        #[cfg(feature = "stow")]
        DicomWebRequest::Stow { .. } => AuthAction::Store,
        DicomWebRequest::DeleteStudy { .. }
        | DicomWebRequest::DeleteSeries { .. }
        | DicomWebRequest::DeleteInstance { .. } => AuthAction::Delete,
        #[allow(unreachable_patterns)]
        _ => AuthAction::WebRequest,
    }
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
fn auth_resource_for_request<'a>(request: &'a DicomWebRequest) -> AuthResource<'a> {
    match request {
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoStudies { .. } => AuthResource {
            study_uid: None,
            series_uid: None,
            instance_uid: None,
            key: AuthResourceKey::Study,
        },
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoAllSeries { .. } => AuthResource {
            study_uid: None,
            series_uid: None,
            instance_uid: None,
            key: AuthResourceKey::Study,
        },
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoAllInstances { .. } => AuthResource {
            study_uid: None,
            series_uid: None,
            instance_uid: None,
            key: AuthResourceKey::Study,
        },
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoSeries { study_uid, .. } => AuthResource {
            study_uid: Some(study_uid.as_str()),
            series_uid: None,
            instance_uid: None,
            key: AuthResourceKey::Study,
        },
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoStudyInstances { study_uid, .. } => AuthResource {
            study_uid: Some(study_uid.as_str()),
            series_uid: None,
            instance_uid: None,
            key: AuthResourceKey::Study,
        },
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoInstances {
            study_uid,
            series_uid,
            ..
        } => AuthResource {
            study_uid: Some(study_uid.as_str()),
            series_uid: Some(series_uid.as_str()),
            instance_uid: None,
            key: AuthResourceKey::Series,
        },
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoInstance {
            study_uid,
            series_uid,
            instance_uid,
            ..
        }
        | DicomWebRequest::WadoInstanceMetadata {
            study_uid,
            series_uid,
            instance_uid,
            ..
        }
        | DicomWebRequest::WadoRendered {
            study_uid,
            series_uid,
            instance_uid,
            ..
        }
        | DicomWebRequest::WadoBulkData {
            study_uid,
            series_uid,
            instance_uid,
            ..
        } => AuthResource {
            study_uid: Some(study_uid.as_str()),
            series_uid: Some(series_uid.as_str()),
            instance_uid: Some(instance_uid.as_str()),
            key: AuthResourceKey::Instance,
        },
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoStudyRetrieve { study_uid, .. }
        | DicomWebRequest::WadoStudyMetadata { study_uid, .. } => AuthResource {
            study_uid: Some(study_uid.as_str()),
            series_uid: None,
            instance_uid: None,
            key: AuthResourceKey::Study,
        },
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoSeriesRetrieve {
            study_uid,
            series_uid,
            ..
        }
        | DicomWebRequest::WadoSeriesMetadata {
            study_uid,
            series_uid,
            ..
        } => AuthResource {
            study_uid: Some(study_uid.as_str()),
            series_uid: Some(series_uid.as_str()),
            instance_uid: None,
            key: AuthResourceKey::Series,
        },
        #[cfg(feature = "stow")]
        DicomWebRequest::Stow { study_uid, .. } => AuthResource {
            study_uid: study_uid.as_deref(),
            series_uid: None,
            instance_uid: None,
            key: AuthResourceKey::Study,
        },
        DicomWebRequest::DeleteStudy { study_uid, .. } => AuthResource {
            study_uid: Some(study_uid.as_str()),
            series_uid: None,
            instance_uid: None,
            key: AuthResourceKey::Study,
        },
        DicomWebRequest::DeleteSeries {
            study_uid,
            series_uid,
            ..
        } => AuthResource {
            study_uid: Some(study_uid.as_str()),
            series_uid: Some(series_uid.as_str()),
            instance_uid: None,
            key: AuthResourceKey::Series,
        },
        DicomWebRequest::DeleteInstance {
            study_uid,
            series_uid,
            instance_uid,
            ..
        } => AuthResource {
            study_uid: Some(study_uid.as_str()),
            series_uid: Some(series_uid.as_str()),
            instance_uid: Some(instance_uid.as_str()),
            key: AuthResourceKey::Instance,
        },
        #[allow(unreachable_patterns)]
        _ => AuthResource::none(),
    }
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
fn record_auth_audit(
    audit: &Option<AuditCallback>,
    action: AuthAction,
    resource: AuthResource<'_>,
    decision: AuthDecision,
    tenant_id: Option<&str>,
    operation: &'static str,
) -> super::Result<()> {
    use dicom_audit::AuditEvent;

    let Some(callback) = audit else {
        return Ok(());
    };

    let mut fields = vec![
        AuditField {
            key: "scope",
            value: AuditValue::Plain("dicomweb".to_string()),
        },
        AuditField {
            key: "protocol_path",
            value: AuditValue::Plain("dicomweb".to_string()),
        },
        AuditField {
            key: "action",
            value: AuditValue::Plain(auth_action_label(action).to_string()),
        },
        AuditField {
            key: "operation",
            value: AuditValue::Plain(operation.to_string()),
        },
        AuditField {
            key: "decision",
            value: AuditValue::Plain(auth_decision_label(decision).to_string()),
        },
        AuditField {
            key: "event_code",
            value: AuditValue::Plain(if decision.is_allowed() {
                "auth.allow".to_string()
            } else {
                "auth.deny".to_string()
            }),
        },
    ];
    if let AuthDecision::Deny(reason) = decision {
        fields.push(AuditField {
            key: "deny_reason",
            value: AuditValue::Plain(auth_deny_reason_label(reason).to_string()),
        });
    }
    if let Some(uid) = resource.study_uid {
        fields.push(AuditField {
            key: "study_uid",
            value: AuditValue::Sensitive(uid.to_string()),
        });
    }
    if let Some(uid) = resource.series_uid {
        fields.push(AuditField {
            key: "series_uid",
            value: AuditValue::Sensitive(uid.to_string()),
        });
    }
    if let Some(uid) = resource.instance_uid {
        fields.push(AuditField {
            key: "instance_uid",
            value: AuditValue::Sensitive(uid.to_string()),
        });
    }
    if let Some(tenant_id) = tenant_id {
        fields.push(AuditField {
            key: "tenant",
            value: AuditValue::Plain(tenant_id.to_string()),
        });
    }

    callback(AuditEvent {
        kind: AuditEventKind::AuthzDecision,
        fields,
    })
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
pub(crate) fn dicomweb_operation_label(request: &DicomWebRequest) -> &'static str {
    match request {
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoStudies { .. } => "qido.studies",
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoAllSeries { .. } => "qido.all_series",
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoAllInstances { .. } => "qido.all_instances",
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoSeries { .. } => "qido.series",
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoStudyInstances { .. } => "qido.study_instances",
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoInstances { .. } => "qido.instances",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoInstance {
            frame_number: Some(_),
            ..
        } => "wado.frame",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoInstance {
            transfer_syntax_uid: Some(_),
            ..
        } => "wado.instance.transfer_syntax",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoInstance { .. } => "wado.instance",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoStudyRetrieve {
            transfer_syntax_uid: Some(_),
            ..
        } => "wado.study.transfer_syntax",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoStudyRetrieve { .. } => "wado.study",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoSeriesRetrieve {
            transfer_syntax_uid: Some(_),
            ..
        } => "wado.series.transfer_syntax",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoSeriesRetrieve { .. } => "wado.series",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoStudyMetadata { .. } => "wado.metadata.study",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoSeriesMetadata { .. } => "wado.metadata.series",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoInstanceMetadata { .. } => "wado.metadata.instance",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoRendered {
            frame_number: Some(_),
            ..
        } => "wado.rendered.frame",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoRendered { .. } => "wado.rendered.instance",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoBulkData { .. } => "wado.bulkdata",
        #[cfg(feature = "stow")]
        DicomWebRequest::Stow { .. } => "stow.studies",
        DicomWebRequest::DeleteStudy { .. } => "delete.study",
        DicomWebRequest::DeleteSeries { .. } => "delete.series",
        DicomWebRequest::DeleteInstance { .. } => "delete.instance",
    }
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
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

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
fn auth_decision_label(decision: AuthDecision) -> &'static str {
    match decision {
        AuthDecision::Allow => "allow",
        AuthDecision::Deny(_) => "deny",
    }
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
fn auth_deny_reason_label(reason: AuthDenyReason) -> &'static str {
    match reason {
        AuthDenyReason::Unauthenticated => "unauthenticated",
        AuthDenyReason::Unauthorized => "unauthorized",
        AuthDenyReason::Policy => "policy",
    }
}
