//! HTTP connection handling, request routing, and policy enforcement.

use super::*;

pub fn handle_connection(
    stream: &mut TcpStream,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
    auth_mode: &WorkflowAuthMode,
    transport_security: &'static str,
    auth_token: Option<&str>,
) -> std::io::Result<()> {
    let request_bytes = read_http_request(stream, limits).inspect_err(|err| {
        let response = http_error_response(
            400,
            "Bad Request",
            "request_read_failed",
            &err.to_string(),
            false,
        );
        let _ = stream.write_all(&response);
    })?;

    let request = match parse_http_request(&request_bytes, limits) {
        Ok(request) => request,
        Err(err) => {
            let response =
                http_error_response(400, "Bad Request", "request_parse_failed", &err, false);
            stream.write_all(&response)?;
            return Ok(());
        }
    };

    let head_only = request.method == "HEAD";

    if !authorize(&request, auth_mode, transport_security, auth_token) {
        let response = http_error_response(
            403,
            "Forbidden",
            "auth_denied",
            "request denied by auth policy",
            head_only,
        );
        stream.write_all(&response)?;
        return Ok(());
    }

    let response = match route_request(&request, state, limits) {
        Ok(response) => http_success_response(response, head_only),
        Err(err) => {
            let (status, label) = status_for_error(&err);
            http_error_response(status, label, err.code(), &err.message(), head_only)
        }
    };
    stream.write_all(&response)?;
    Ok(())
}

pub fn route_request(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let request_path = route_path_normalize(&request.path)?;
    let actor = workflow_actor_context(&request.headers);
    let contract = workflow_contract_for_request(&request.method, &request_path);
    let is_mutation = request.method != "GET" && request.method != "HEAD";

    let policy_anomaly = {
        let mut store = state
            .lock()
            .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
        ensure_tenant_indexes_initialized(&mut store);
        let policy_anomaly = match contract {
            Some(contract) => {
                enforce_route_policy(&mut store, &actor, request, &contract, &request_path)?
            }
            None => false,
        };
        if let Some(contract) = &contract {
            touch_tenant_index(&mut store, &actor.tenant);
            let metrics = store.metrics.entry(actor.tenant.clone()).or_default();
            metrics.read_operations = metrics.read_operations.saturating_add(1);
            if is_mutation {
                metrics.mutation_operations = metrics.mutation_operations.saturating_add(1);
            } else {
                metrics.query_operations = metrics.query_operations.saturating_add(1);
            }
            let _ = contract;
        }
        policy_anomaly
    };

    let response = route_request_core(&request_path, request, state, limits, &actor);
    let status = match &response {
        Ok(WorkflowResponse::Json(code, _)) => *code,
        Err(err) => status_for_error(err).0,
    };
    let is_anomaly = status == 429 || policy_anomaly;
    {
        let mut store = state
            .lock()
            .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
        let mut outcome = "ok".to_string();
        if let Some(contract) = &contract {
            let metrics = store.metrics.entry(actor.tenant.clone()).or_default();
            if status == 429 {
                metrics.anomaly_operations = metrics.anomaly_operations.saturating_add(1);
            }
            if status >= 400 {
                metrics.denied_operations = metrics.denied_operations.saturating_add(1);
                outcome = "error".to_string();
            }
            let _ = contract;
        }
        if let Some(contract) = &contract {
            let _ = store_append_workflow_audit(
                &mut store,
                &actor,
                request,
                contract.operation,
                contract.path_template,
                status,
                &outcome,
                is_anomaly,
            );
        }
    }
    response
}

pub fn workflow_provider_profile_from_env() -> Result<ProviderProfile, Box<Error>> {
    match env::var("DICOM_WORKFLOW_PROVIDER_PROFILE") {
        Ok(raw) => ProviderProfile::parse(raw.trim()).ok_or_else(|| {
            decode_error(
                "invalid DICOM_WORKFLOW_PROVIDER_PROFILE; supported values: azure | aws_health_imaging | orthanc | dcm4chee | generic",
            )
        }),
        Err(env::VarError::NotPresent) => Ok(ProviderProfile::Generic),
        Err(env::VarError::NotUnicode(_)) => {
            Err(decode_error("DICOM_WORKFLOW_PROVIDER_PROFILE must be valid UTF-8"))
        }
    }
}

pub fn blocked_provider_profile_operation(
    method: &str,
    path: &str,
    profile: ProviderProfile,
) -> Option<&'static str> {
    let capabilities = profile.capabilities();
    if path == WORKITEM_COLLECTION_PATH || path.starts_with(WORKITEM_ITEM_PREFIX) {
        if !capabilities.workitem {
            return Some("workitem");
        }
        return None;
    }

    match method {
        "GET" | "HEAD" => {
            if path.contains("/search") {
                if !capabilities.search {
                    Some("search")
                } else {
                    None
                }
            } else if !capabilities.retrieve {
                Some("retrieve")
            } else {
                None
            }
        }
        "POST" | "PUT" | "PATCH" => {
            if !capabilities.store {
                Some("store")
            } else {
                None
            }
        }
        "DELETE" => {
            if !capabilities.delete {
                Some("delete")
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn route_request_core(
    request_path: &str,
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
    actor: &WorkflowActorContext,
) -> Result<WorkflowResponse, Box<Error>> {
    let provider_profile = workflow_provider_profile_from_env()?;
    if let Some(operation) =
        blocked_provider_profile_operation(&request.method, request_path, provider_profile)
    {
        return Err(decode_error(format!(
            "provider profile '{}' does not support '{}' operations",
            provider_profile.label(),
            operation,
        )));
    }

    if request_path == WORKITEM_COLLECTION_PATH || request_path.starts_with(WORKITEM_ITEM_PREFIX) {
        return handle_workitem_routes(request_path, request, state, limits, actor);
    }
    if request_path == INTEROP_IAN_PATH {
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_ian_list(request, state),
            "POST" => handle_ian_ingest(request, state, limits),
            _ => Err(decode_error("unsupported route")),
        };
    }
    if request_path == INTEROP_STORAGE_COMMITMENT_STATUS_PATH {
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_storage_commitment_status_list(request, state),
            "POST" => handle_storage_commitment_status_upsert(request, state, limits),
            _ => Err(decode_error("unsupported route")),
        };
    }
    if let Some(transaction_uid) =
        request_path.strip_prefix(INTEROP_STORAGE_COMMITMENT_STATUS_PREFIX)
    {
        let transaction_uid = normalize_route_identifier(transaction_uid)?;
        if transaction_uid.is_empty() {
            return Err(decode_error("unsupported route"));
        }
        return match request.method.as_str() {
            "GET" | "HEAD" => {
                handle_storage_commitment_status_get(&transaction_uid, request, state)
            }
            _ => Err(decode_error("unsupported route")),
        };
    }
    if request_path == INTEROP_HL7_UPS_CORRELATION_PATH {
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_hl7_ups_correlation_list(request, state),
            "POST" => handle_hl7_ups_correlation_upsert(request, state, limits),
            _ => Err(decode_error("unsupported route")),
        };
    }

    if request_path == task::TASK_COLLECTION_PATH
        || request_path.starts_with(task::TASK_ITEM_PREFIX)
    {
        if request_path == task::TASK_COLLECTION_PATH {
            return match request.method.as_str() {
                "GET" | "HEAD" => handle_task_list(request, state, limits),
                "POST" => handle_task_create(request, state, limits),
                _ => Err(decode_error("unsupported route")),
            };
        }
        let rest = request_path.trim_start_matches(task::TASK_ITEM_PREFIX);
        if rest.is_empty() {
            return Err(decode_error("unsupported route"));
        }
        if let Some((task_id, suffix)) = rest.split_once('/') {
            if suffix.is_empty() {
                return Err(decode_error("unsupported route"));
            }
            let task_id = normalize_route_identifier(task_id)?;
            return match suffix {
                "start" | "pause" | "resume" | "complete" | "review" | "commit" | "cancel" => {
                    handle_task_control(&task_id, suffix, request, state)
                }
                _ => Err(decode_error("unsupported route")),
            };
        }
        let task_id = normalize_route_identifier(rest)?;
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_task_get(&task_id, request, state),
            _ => Err(decode_error("unsupported route")),
        };
    }

    if let Some(rest) = request_path.strip_prefix(mpps::MPPS_ITEM_PREFIX) {
        if rest.is_empty() {
            return Err(decode_error("unsupported route"));
        }
        if let Some((sop_uid, suffix)) = rest.split_once('/') {
            let sop_uid = normalize_route_identifier(sop_uid)?;
            if suffix != "status" {
                return Err(decode_error("unsupported route"));
            }
            return match request.method.as_str() {
                "GET" | "HEAD" => handle_mpps_get_status(&sop_uid, actor, state),
                "POST" => handle_mpps_update_status(&sop_uid, request, actor, state),
                _ => Err(decode_error("unsupported route")),
            };
        }
        let sop_uid = normalize_route_identifier(rest)?;
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_mpps_get_single(&sop_uid, actor, state),
            _ => Err(decode_error("unsupported route")),
        };
    }

    if request_path == sr::SR_COLLECTION_PATH {
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_sr_list(request, actor, state, limits),
            "POST" => handle_sr_create(request, actor, state, limits),
            _ => Err(decode_error("unsupported route")),
        };
    }
    if let Some(interop_route) = interop::classify_interop_route(request_path) {
        return match interop_route {
            interop::InteropRoute::Hl7Failures => match request.method.as_str() {
                "GET" | "HEAD" => handle_hl7_failures(state),
                _ => Err(decode_error("unsupported route")),
            },
            interop::InteropRoute::Hl7Ingest => match request.method.as_str() {
                "POST" => handle_hl7_ingest(request, state, limits),
                _ => Err(decode_error("unsupported route")),
            },
            interop::InteropRoute::ConnectorStatus => match request.method.as_str() {
                "GET" | "HEAD" => handle_hl7_connector_status_dashboard(state),
                _ => Err(decode_error("unsupported route")),
            },
            interop::InteropRoute::ConnectorFeatures => match request.method.as_str() {
                "GET" | "HEAD" => handle_hl7_connector_features(state),
                _ => Err(decode_error("unsupported route")),
            },
            interop::InteropRoute::ConnectorRollout => match request.method.as_str() {
                "GET" | "HEAD" => handle_hl7_connector_rollout_list(state),
                "POST" => handle_hl7_connector_rollout_update(request, actor, state, limits),
                _ => Err(decode_error("unsupported route")),
            },
            interop::InteropRoute::ConnectorCapabilities => match request.method.as_str() {
                "GET" | "HEAD" => handle_hl7_connector_capabilities(state),
                _ => Err(decode_error("unsupported route")),
            },
            interop::InteropRoute::ConnectorHealth => match request.method.as_str() {
                "GET" | "HEAD" => handle_hl7_connector_health(state),
                _ => Err(decode_error("unsupported route")),
            },
            interop::InteropRoute::Subscriptions => match request.method.as_str() {
                "GET" | "HEAD" => handle_hl7_subscriptions_list(state),
                "POST" => handle_hl7_subscriptions_create(request, state),
                _ => Err(decode_error("unsupported route")),
            },
            interop::InteropRoute::FhirIngest => match request.method.as_str() {
                "POST" => handle_fhir_ingest(request, limits),
                _ => Err(decode_error("unsupported route")),
            },
            interop::InteropRoute::ReconciliationJobsCollection => match request.method.as_str() {
                "GET" | "HEAD" => handle_reconciliation_jobs_list(request, actor, state),
                "POST" => handle_reconciliation_jobs_create(request, actor, state, limits),
                _ => Err(decode_error("unsupported route")),
            },
            interop::InteropRoute::ReconciliationJobRun { job_id } => match request.method.as_str()
            {
                "POST" => handle_reconciliation_jobs_run(job_id, request, actor, state),
                _ => Err(decode_error("unsupported route")),
            },
        };
    }
    if let Some(sop_uid) = request_path.strip_prefix(sr::SR_ITEM_PREFIX) {
        if sop_uid.is_empty() {
            return Err(decode_error("unsupported route"));
        }
        if let Some((sop_uid, suffix)) = sop_uid.split_once('/') {
            let sop_uid = normalize_route_identifier(sop_uid)?;
            if suffix.is_empty() || suffix.contains('/') {
                return Err(decode_error("unsupported route"));
            }
            return match (request.method.as_str(), suffix) {
                ("POST", "updates") => handle_sr_update(&sop_uid, request, actor, state, limits),
                ("GET" | "HEAD", "history") => handle_sr_history(&sop_uid, actor, state),
                ("POST", "review") => {
                    handle_sr_transition("review", &sop_uid, request, actor, state)
                }
                ("POST", "finalize") => {
                    handle_sr_transition("finalize", &sop_uid, request, actor, state)
                }
                ("POST", "commit") => {
                    handle_sr_transition("commit", &sop_uid, request, actor, state)
                }
                ("POST", "cancel") => {
                    handle_sr_transition("cancel", &sop_uid, request, actor, state)
                }
                _ => Err(decode_error("unsupported route")),
            };
        }
        let sop_uid = normalize_route_identifier(sop_uid)?;
        if sop_uid.contains('/') {
            return Err(decode_error("unsupported route"));
        }
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_sr_get(&sop_uid, actor, state),
            _ => Err(decode_error("unsupported route")),
        };
    }
    if request_path == tenant_policy_domain::WORKFLOW_TENANT_QUOTAS_PATH {
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_workflow_tenant_quotas(request, actor, state),
            _ => Err(decode_error("unsupported route")),
        };
    }
    if request_path == tenant_policy_domain::WORKFLOW_TENANT_QUOTAS_SNAPSHOT_PATH {
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_workflow_tenant_quota_snapshot(request, actor, state),
            _ => Err(decode_error("unsupported route")),
        };
    }
    if request_path == tenant_policy_domain::WORKFLOW_METRICS_PATH {
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_workflow_metrics(request, actor, state),
            _ => Err(decode_error("unsupported route")),
        };
    }
    if request_path == tenant_policy_domain::WORKFLOW_AUDIT_PATH {
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_workflow_audit(request, actor, state),
            _ => Err(decode_error("unsupported route")),
        };
    }

    match (request.method.as_str(), request_path) {
        ("GET", "/healthz") | ("HEAD", "/healthz") => Ok(WorkflowResponse::Json(
            200,
            "{\"status\":\"ok\"}".to_string(),
        )),
        ("GET", "/readyz") | ("HEAD", "/readyz") => Ok(WorkflowResponse::Json(
            200,
            "{\"status\":\"ready\"}".to_string(),
        )),
        ("GET", "/worklist/items") | ("HEAD", "/worklist/items") => {
            handle_worklist_query(request, state, limits)
        }
        ("POST", "/worklist/items") => handle_worklist_upsert(request, state, limits),
        ("GET", mpps::MPPS_COLLECTION_PATH) | ("HEAD", mpps::MPPS_COLLECTION_PATH) => {
            handle_mpps_get(request, actor, state, limits)
        }
        ("POST", mpps::MPPS_COLLECTION_PATH) => handle_mpps_ingest(request, actor, state, limits),
        _ => Err(decode_error("unsupported route")),
    }
}

pub fn workflow_contract_for_request(
    method: &str,
    path: &str,
) -> Option<dicom_workflow_server::WorkflowRouteContract> {
    use dicom_workflow_server::workflow_route_contract;
    let method = method.to_ascii_uppercase();
    for contract in workflow_route_contract() {
        if !method_matches(method.as_str(), contract.method) {
            continue;
        }
        if route_path_matches_contract(path, contract.path_template) {
            return Some(*contract);
        }
    }
    None
}

pub fn method_matches(method: &str, route_methods: &str) -> bool {
    route_methods
        .split('|')
        .any(|candidate| candidate.eq_ignore_ascii_case(method))
}

pub fn route_path_matches_contract(path: &str, template: &str) -> bool {
    let path_parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let template_parts: Vec<&str> = template
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
    if path_parts.len() != template_parts.len() {
        return false;
    }
    path_parts
        .iter()
        .zip(template_parts.iter())
        .all(|(path_part, template_part)| {
            if template_part.starts_with('{') && template_part.ends_with('}') {
                return true;
            }
            path_part == template_part
        })
}

pub fn is_route_denied(templates: &[String], path: &str) -> bool {
    templates
        .iter()
        .any(|template| route_path_matches_contract(path, template))
}

pub fn render_route_policy_enforcement_failure_log(
    actor: &WorkflowActorContext,
    request: &HttpRequest,
    contract: &dicom_workflow_server::WorkflowRouteContract,
    policy: &str,
    detail: &str,
) -> String {
    format!(
        "{{\"event\":\"workflow_route_policy_enforcement_failure\",\"tenant\":\"{}\",\"route\":\"{}\",\"policy\":\"{}\",\"correlation_id\":\"{}\",\"detail\":\"{}\"}}",
        escape_json(&actor.tenant),
        escape_json(contract.path_template),
        escape_json(policy),
        escape_json(&request_id_from_headers(&request.headers)),
        escape_json(detail),
    )
}

pub fn log_route_policy_enforcement_failure(
    actor: &WorkflowActorContext,
    request: &HttpRequest,
    contract: &dicom_workflow_server::WorkflowRouteContract,
    policy: &str,
    detail: &str,
) {
    eprintln!(
        "{}",
        render_route_policy_enforcement_failure_log(actor, request, contract, policy, detail)
    );
}

pub fn enforce_route_policy(
    state: &mut RuntimeState,
    actor: &WorkflowActorContext,
    request: &HttpRequest,
    contract: &dicom_workflow_server::WorkflowRouteContract,
    request_path: &str,
) -> Result<bool, Box<Error>> {
    if is_route_denied(&state.denylist_routes, request_path) {
        log_route_policy_enforcement_failure(
            actor,
            request,
            contract,
            "denylist_route",
            "request denied by workflow operation deny-list",
        );
        return Err(auth_denied_error(
            "request denied by workflow operation deny-list",
        ));
    }

    let route = contract;
    if route.requires_writer_role && !actor.can_write() {
        log_route_policy_enforcement_failure(
            actor,
            request,
            contract,
            "writer_role_required",
            "writer role required",
        );
        return Err(auth_denied_error("writer role required"));
    }
    if route.requires_idempotency_key {
        let key = request
            .headers
            .get("x-idempotency-key")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty());
        if key.is_none() {
            log_route_policy_enforcement_failure(
                actor,
                request,
                contract,
                "idempotency_key_required",
                "missing required x-idempotency-key header",
            );
            return Err(decode_error("missing required x-idempotency-key header"));
        }
    }

    let policy = tenant_policy(state, &actor.tenant);
    if request.method != "GET" && request.method != "HEAD" {
        if request.body.len() as u64 > policy.upload_cap_bytes {
            log_route_policy_enforcement_failure(
                actor,
                request,
                contract,
                "upload_cap_bytes",
                &format!(
                    "observed body {} exceeds upload cap {}",
                    request.body.len(),
                    policy.upload_cap_bytes
                ),
            );
            return Err(limit_exceeded(
                "workflow_upload_cap_bytes",
                request.body.len() as u64,
                policy.upload_cap_bytes,
            ));
        }
    }

    let anomaly = if request.method == "GET" || request.method == "HEAD" {
        enforce_rate_limit(state, actor, request, request_path, policy.query_rate_limit).map_err(
            |err| {
                log_route_policy_enforcement_failure(
                    actor,
                    request,
                    contract,
                    "query_rate_limit",
                    &err.message().clone(),
                );
                err
            },
        )?
    } else {
        enforce_rate_limit(
            state,
            actor,
            request,
            request_path,
            policy.mutation_rate_limit,
        )
        .map_err(|err| {
            log_route_policy_enforcement_failure(
                actor,
                request,
                contract,
                "mutation_rate_limit",
                &err.message().clone(),
            );
            err
        })?
    };
    Ok(anomaly)
}

pub fn enforce_rate_limit(
    state: &mut RuntimeState,
    actor: &WorkflowActorContext,
    request: &HttpRequest,
    request_path: &str,
    limit: u64,
) -> Result<bool, Box<Error>> {
    if limit == 0 {
        return Ok(false);
    }
    let route = route_request_scope_path(request_path, request.method.as_str());
    let key = format!(
        "{}|{}|{}|{}",
        actor.tenant,
        actor
            .principal
            .clone()
            .unwrap_or_else(|| "anonymous".to_string()),
        request.method,
        route
    );
    let now_ms = now_epoch_millis();
    let window_ms = state.audit_rate_window_ms;
    let bucket = state
        .rate_windows
        .entry(key)
        .or_insert_with(RequestWindow::empty);
    if bucket.window_start_ms == 0 || now_ms.saturating_sub(bucket.window_start_ms) >= window_ms {
        bucket.window_start_ms = now_ms;
        bucket.count = 0;
    }
    bucket.count = bucket.count.saturating_add(1);
    let anomaly = if state.anomaly_alert_threshold > 0 {
        bucket.count >= state.anomaly_alert_threshold
    } else {
        false
    };
    if bucket.count > limit {
        return Err(Error::from_kind(
            ErrorKind::LimitExceeded {
                limit_name: "workflow_rate_limit",
                observed: bucket.count,
                allowed: limit,
            },
            "rate limit exceeded",
        )
        .into());
    }
    Ok(anomaly && bucket.count < limit)
}

pub fn route_request_scope_path(path: &str, method: &str) -> &'static str {
    match (method, path) {
        ("GET", "/healthz") | ("HEAD", "/healthz") => "healthz",
        ("POST", "/worklist/items") | ("GET", "/worklist/items") | ("HEAD", "/worklist/items") => {
            "worklist/items"
        }
        ("POST", mpps::MPPS_COLLECTION_PATH)
        | ("GET", mpps::MPPS_COLLECTION_PATH)
        | ("HEAD", mpps::MPPS_COLLECTION_PATH) => "mpps/updates",
        (method, path) if path.starts_with(mpps::MPPS_ITEM_PREFIX) => match method {
            "POST" => "mpps/updates/{sop}/status",
            _ => "mpps/updates/{sop}",
        },
        (_, path) if path.starts_with(sr::SR_ITEM_PREFIX) => {
            if path.ends_with("/updates") {
                "sr/documents/{sop}/updates"
            } else if path.ends_with("/history") {
                "sr/documents/{sop}/history"
            } else if path.ends_with("/review") {
                "sr/documents/{sop}/review"
            } else if path.ends_with("/finalize") {
                "sr/documents/{sop}/finalize"
            } else if path.ends_with("/commit") {
                "sr/documents/{sop}/commit"
            } else if path.ends_with("/cancel") {
                "sr/documents/{sop}/cancel"
            } else {
                "sr/documents/{sop}"
            }
        }
        (method, sr::SR_COLLECTION_PATH) => {
            if method == "POST" {
                "sr/documents"
            } else {
                "sr/documents"
            }
        }
        (method, WORKITEM_COLLECTION_PATH) => {
            if method == "POST" {
                "workflow/workitems"
            } else {
                "workflow/workitems"
            }
        }
        (_, path) if path.starts_with(task::TASK_ITEM_PREFIX) => {
            if path == task::TASK_COLLECTION_PATH {
                "workflow/tasks"
            } else if path.ends_with("/start") {
                "workflow/tasks/{task_id}/start"
            } else if path.ends_with("/pause") {
                "workflow/tasks/{task_id}/pause"
            } else if path.ends_with("/resume") {
                "workflow/tasks/{task_id}/resume"
            } else if path.ends_with("/complete") {
                "workflow/tasks/{task_id}/complete"
            } else if path.ends_with("/review") {
                "workflow/tasks/{task_id}/review"
            } else if path.ends_with("/commit") {
                "workflow/tasks/{task_id}/commit"
            } else if path.ends_with("/cancel") {
                "workflow/tasks/{task_id}/cancel"
            } else {
                "workflow/tasks/{task_id}"
            }
        }
        (_, path) if path.starts_with(WORKITEM_ITEM_PREFIX) => {
            if path == WORKITEM_COLLECTION_PATH {
                "workflow/workitems"
            } else if path.ends_with("/state") {
                "workflow/workitems/{task_id}/state"
            } else if path.ends_with("/cancel") {
                "workflow/workitems/{task_id}/cancel"
            } else if path.ends_with("/search") {
                "workflow/workitems/search"
            } else {
                "workflow/workitems/{task_id}"
            }
        }
        (method, INTEROP_IAN_PATH) => {
            if method == "POST" {
                "interop/ian"
            } else {
                "interop/ian"
            }
        }
        (method, INTEROP_STORAGE_COMMITMENT_STATUS_PATH) => {
            if method == "POST" {
                "interop/storage-commitment/status"
            } else {
                "interop/storage-commitment/status"
            }
        }
        (_, path) if path.starts_with(INTEROP_STORAGE_COMMITMENT_STATUS_PREFIX) => {
            "interop/storage-commitment/status/{task_id}"
        }
        (method, INTEROP_HL7_UPS_CORRELATION_PATH) => {
            if method == "POST" {
                "interop/hl7/ups-correlation"
            } else {
                "interop/hl7/ups-correlation"
            }
        }
        ("POST", interop::INTEROP_HL7_PATH) => "interop/hl7",
        (method, interop::INTEROP_HL7_FAILURES_PATH) if method == "GET" || method == "HEAD" => {
            "interop/hl7/failures"
        }
        (method, interop::INTEROP_CONNECTORS_STATUS_PATH)
            if method == "GET" || method == "HEAD" =>
        {
            "interop/connectors/status"
        }
        (method, interop::INTEROP_CONNECTORS_FEATURES_PATH)
            if method == "GET" || method == "HEAD" =>
        {
            "interop/connectors/features"
        }
        (method, interop::INTEROP_CONNECTORS_ROLLOUT_PATH) => {
            if method == "POST" {
                "interop/connectors/rollout/update"
            } else {
                "interop/connectors/rollout"
            }
        }
        (method, interop::INTEROP_CONNECTORS_CAPABILITIES_PATH)
            if method == "GET" || method == "HEAD" =>
        {
            "interop/connectors/capabilities"
        }
        (method, interop::INTEROP_CONNECTORS_HEALTH_PATH)
            if method == "GET" || method == "HEAD" =>
        {
            "interop/connectors/health"
        }
        (method, interop::INTEROP_SUBSCRIPTIONS_PATH) => {
            if method == "POST" {
                "interop/subscriptions"
            } else {
                "interop/subscriptions"
            }
        }
        ("POST", interop::INTEROP_FHIR_PATH) => "interop/fhir",
        (_, path) if path.starts_with(interop::INTEROP_RECONCILIATION_JOBS_PREFIX) => {
            if path.ends_with("/run") {
                "interop/reconciliation/jobs/{job_id}/run"
            } else if path == interop::INTEROP_RECONCILIATION_JOBS_PREFIX {
                "interop/reconciliation/jobs"
            } else {
                "interop/reconciliation/jobs/{job_id}/run"
            }
        }
        (method, tenant_policy_domain::WORKFLOW_TENANT_QUOTAS_PATH)
            if method == "GET" || method == "HEAD" =>
        {
            "workflow/policy/quotas"
        }
        (method, tenant_policy_domain::WORKFLOW_TENANT_QUOTAS_SNAPSHOT_PATH)
            if method == "GET" || method == "HEAD" =>
        {
            "workflow/policy/quotas/snapshot"
        }
        (method, tenant_policy_domain::WORKFLOW_METRICS_PATH)
            if method == "GET" || method == "HEAD" =>
        {
            "workflow/metrics"
        }
        (method, tenant_policy_domain::WORKFLOW_AUDIT_PATH)
            if method == "GET" || method == "HEAD" =>
        {
            "workflow/audit"
        }
        _ => "unknown",
    }
}


