//! HTTP handler functions for audit, tasks, worklist, MPPS, IAN, and storage commitment.

use super::*;

pub fn store_append_workflow_audit(
    state: &mut RuntimeState,
    actor: &WorkflowActorContext,
    request: &HttpRequest,
    operation: &str,
    route: &str,
    status: u16,
    outcome: &str,
    anomaly: bool,
) {
    let _ = outcome;
    let event = WorkflowAuditEvent {
        ts_ms: now_epoch_millis(),
        tenant: actor.tenant.clone(),
        principal_hash: hash_text(actor.principal.as_deref().unwrap_or("anonymous")),
        request_id_hash: hash_text(&request_id_from_headers(&request.headers)),
        previous_audit_hash: String::new(),
        audit_hash: String::new(),
        route: route.to_string(),
        method: request.method.clone(),
        operation: operation.to_string(),
        outcome: outcome.to_string(),
        scope: request
            .query
            .get("scope")
            .cloned()
            .unwrap_or_else(|| "default".to_string()),
        status,
        anomaly,
    };
    let _ = append_workflow_audit_event(
        &state.health.read().audit_path,
        state.health.read().audit_max_bytes,
        state.health.read().audit_max_rotated_files,
        state.health.read().audit_export_limit,
        &event,
    );
}

pub fn append_workflow_audit_event(
    path: &str,
    max_bytes: u64,
    max_rotated_files: usize,
    _export_limit: usize,
    event: &WorkflowAuditEvent,
) {
    if max_bytes == 0 {
        return;
    }
    let _ = rotate_audit_if_needed(path, max_bytes, max_rotated_files);
    let previous_audit_hash = read_workflow_audit_previous_hash(path);
    let event = WorkflowAuditEvent {
        previous_audit_hash: previous_audit_hash.clone(),
        audit_hash: workflow_audit_line_hash(&previous_audit_hash, &event),
        ..event.clone()
    };
    let line = format!(
        "{{\"ts_ms\":{},\"tenant\":\"{}\",\"principal_hash\":\"{}\",\"request_id_hash\":\"{}\",\"route\":\"{}\",\"method\":\"{}\",\"operation\":\"{}\",\"outcome\":\"{}\",\"scope\":\"{}\",\"status\":{},\"anomaly\":{},\"previous_audit_hash\":\"{}\",\"audit_hash\":\"{}\"}}\n",
        event.ts_ms,
        escape_json(&event.tenant),
        escape_json(&event.principal_hash),
        escape_json(&event.request_id_hash),
        escape_json(&event.route),
        escape_json(&event.method),
        escape_json(&event.operation),
        escape_json(&event.outcome),
        escape_json(&event.scope),
        event.status,
        if event.anomaly { "true" } else { "false" },
        escape_json(&event.previous_audit_hash),
        escape_json(&event.audit_hash),
    );
    if let Ok(mut audit_file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = audit_file.write_all(line.as_bytes());
        let _ = audit_file.sync_all();
    }
}

pub fn workflow_audit_line_hash(previous_audit_hash: &str, event: &WorkflowAuditEvent) -> String {
    let anomaly = if event.anomaly { "true" } else { "false" };
    hash_text(&format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        event.ts_ms,
        previous_audit_hash,
        event.tenant,
        event.principal_hash,
        event.request_id_hash,
        event.route,
        event.method,
        event.operation,
        event.outcome,
        event.scope,
        event.status,
        anomaly,
    ))
}

pub fn read_workflow_audit_previous_hash(path: &str) -> String {
    let content = match fs::read_to_string(path) {
        Ok(value) => value,
        Err(_) => return String::new(),
    };
    content
        .lines()
        .rev()
        .find_map(|line| parse_workflow_audit_json_field(line, "audit_hash"))
        .unwrap_or_default()
}

pub fn parse_workflow_audit_json_field(line: &str, key: &str) -> Option<String> {
    let token = format!("\\\"{}\\\":\\\"", key);
    let start = line.find(&token)?;
    let start = start + token.len();
    let rest = line.get(start..)?;
    let end = rest.find('"')?;
    Some(rest.get(0..end)?.to_string())
}

pub fn parse_workflow_audit_json_u64_field(line: &str, key: &str) -> Option<u64> {
    let token = format!("\\\"{}\\\":", key);
    let start = line.find(&token)?;
    let start = start + token.len();
    let rest = line.get(start..)?;
    let trimmed = rest.trim_start();
    let mut len = 0usize;
    for byte in trimmed.bytes() {
        if !byte.is_ascii_digit() {
            break;
        }
        len = len.saturating_add(1);
    }
    if len == 0 {
        return None;
    }
    trimmed.get(0..len)?.parse().ok()
}

pub fn parse_workflow_audit_json_bool_field(line: &str, key: &str) -> Option<bool> {
    let token = format!("\"{}\":", key);
    let start = line.find(&token)?;
    let start = start + token.len();
    let rest = line.get(start..)?;
    let trimmed = rest.trim_start();
    if trimmed.starts_with("true") {
        Some(true)
    } else if trimmed.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

pub fn parse_workflow_audit_line(line: &str) -> Option<WorkflowAuditEvent> {
    let ts_ms = parse_workflow_audit_json_u64_field(line, "ts_ms")?;
    let status = parse_workflow_audit_json_u64_field(line, "status")?;
    let anomaly = parse_workflow_audit_json_bool_field(line, "anomaly")?;
    if status > u16::MAX as u64 {
        return None;
    }
    Some(WorkflowAuditEvent {
        ts_ms,
        tenant: parse_workflow_audit_json_field(line, "tenant")?,
        principal_hash: parse_workflow_audit_json_field(line, "principal_hash")?,
        request_id_hash: parse_workflow_audit_json_field(line, "request_id_hash")?,
        previous_audit_hash: parse_workflow_audit_json_field(line, "previous_audit_hash")?,
        audit_hash: parse_workflow_audit_json_field(line, "audit_hash")?,
        route: parse_workflow_audit_json_field(line, "route")?,
        method: parse_workflow_audit_json_field(line, "method")?,
        operation: parse_workflow_audit_json_field(line, "operation")?,
        outcome: parse_workflow_audit_json_field(line, "outcome")?,
        scope: parse_workflow_audit_json_field(line, "scope")?,
        status: status as u16,
        anomaly,
    })
}

#[derive(Debug, Clone)]
pub struct WorkflowAuditVerificationFailure {
    pub line: usize,
    pub detail: String,
}

pub fn verify_workflow_audit_chain(lines: &[String]) -> Vec<WorkflowAuditVerificationFailure> {
    let mut failures = Vec::new();
    let mut previous_audit_hash = String::new();
    for (index, line) in lines.iter().enumerate() {
        let line_no = index + 1;
        let event = match parse_workflow_audit_line(line) {
            Some(event) => event,
            None => {
                failures.push(WorkflowAuditVerificationFailure {
                    line: line_no,
                    detail: "parse failure for workflow audit line".to_string(),
                });
                previous_audit_hash.clear();
                continue;
            }
        };
        if event.previous_audit_hash != previous_audit_hash {
            failures.push(WorkflowAuditVerificationFailure {
                line: line_no,
                detail: format!(
                    "previous hash mismatch at line {line_no}: expected {previous_audit_hash}, got {}",
                    event.previous_audit_hash
                ),
            });
        }
        let expected_hash = workflow_audit_line_hash(&previous_audit_hash, &event);
        if event.audit_hash != expected_hash {
            failures.push(WorkflowAuditVerificationFailure {
                line: line_no,
                detail: format!("audit hash mismatch at line {line_no}"),
            });
        }
        previous_audit_hash = event.audit_hash;
    }
    failures
}

pub fn rotate_audit_if_needed(
    path: &str,
    max_bytes: u64,
    max_rotated_files: usize,
) -> std::io::Result<()> {
    let path = Path::new(path);
    let Some(parent) = path.parent() else {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            "workflow audit path must include a file parent",
        ));
    };
    std::fs::create_dir_all(parent)?;
    let metadata = fs::metadata(path);
    if let Ok(meta) = metadata {
        if meta.len() > max_bytes {
            rotate_file(path, max_rotated_files)?;
        }
    }
    Ok(())
}

pub fn render_workflow_audit_payload(lines: &[String]) -> String {
    let mut out = String::from("[");
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(line);
    }
    out.push(']');
    out
}

pub fn render_workflow_audit_payload_with_verification(
    lines: &[String],
    failures: &[WorkflowAuditVerificationFailure],
) -> String {
    let mut violation_json = String::new();
    for (index, failure) in failures.iter().enumerate() {
        if index > 0 {
            violation_json.push(',');
        }
        violation_json.push_str(&format!(
            "{{\"line\":{},\"detail\":\"{}\"}}",
            failure.line,
            escape_json(&failure.detail),
        ));
    }
    format!(
        "{{\"verification\":{{\"ok\":{},\"failures\":[{}]}},\"events\":{}}}",
        failures.is_empty(),
        violation_json,
        render_workflow_audit_payload(lines),
    )
}

pub fn render_tenant_quota_override_json(override_limits: Option<TenantQuotaOverride>) -> String {
    match override_limits {
        Some(override_limits) => format!(
            "{{\"task_quota\":{},\"subscription_quota\":{}}}",
            override_limits.task_quota, override_limits.subscription_quota
        ),
        None => "null".to_string(),
    }
}

pub fn render_tenant_rate_override_json(override_limits: Option<TenantRateLimitOverride>) -> String {
    match override_limits {
        Some(override_limits) => format!(
            "{{\"query_rate_limit\":{},\"mutation_rate_limit\":{},\"upload_cap_bytes\":{}}}",
            override_limits.query_rate_limit,
            override_limits.mutation_rate_limit,
            override_limits.upload_cap_bytes
        ),
        None => "null".to_string(),
    }
}

pub fn handle_workflow_tenant_quotas(
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    enforce_admin_api_version(request)?;
    let tenant_filter = request
        .query
        .get("tenant")
        .cloned()
        .unwrap_or_else(|| actor.tenant.clone());
    let is_admin = actor.role.as_deref() == Some("admin");
    if !is_admin && tenant_filter != actor.tenant && tenant_filter != "*" && tenant_filter != "all"
    {
        return Err(auth_denied_error(
            "quota tenant scope outside caller tenant",
        ));
    }

    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if tenant_filter == "*" || tenant_filter == "all" {
        if !is_admin {
            return Err(auth_denied_error(
                "quota tenant scope outside caller tenant",
            ));
        }
        let overrides = collect_tenant_quota_overrides_from_env();
        let rate_overrides = tenant_rate_limit_overrides_snapshot();
        let mut tenants = Vec::new();
        for (tenant_key, override_limits) in overrides {
            tenants.push(format!(
                "\"{}\":{{\"task_quota\":{},\"subscription_quota\":{}}}",
                escape_json(&tenant_key),
                override_limits.task_quota,
                override_limits.subscription_quota
            ));
        }
        let mut rate_rows = Vec::new();
        for (tenant_key, override_limits) in rate_overrides {
            rate_rows.push(format!(
                "\"{}\":{{\"query_rate_limit\":{},\"mutation_rate_limit\":{},\"upload_cap_bytes\":{}}}",
                escape_json(&tenant_key),
                override_limits.query_rate_limit,
                override_limits.mutation_rate_limit,
                override_limits.upload_cap_bytes
            ));
        }
        return Ok(WorkflowResponse::Json(
            200,
            format!(
                "{{\"defaults\":{{\"task_quota\":{},\"subscription_quota\":{}}},\"overrides\":{{{}}},\"rate_defaults\":{{\"query_rate_limit\":{},\"mutation_rate_limit\":{},\"upload_cap_bytes\":{}}},\"rate_overrides\":{{{}}}}}",
                DEFAULT_TENANT_TASK_QUOTA,
                DEFAULT_TENANT_SUBSCRIPTION_QUOTA,
                tenants.join(","),
                store.health.read().query_rate_limit,
                store.health.read().mutation_rate_limit,
                store.health.read().upload_cap_bytes,
                rate_rows.join(","),
            ),
        ));
    }

    let effective = tenant_policy(&store, &tenant_filter);
    let override_limits = tenant_quota_override_for_tenant(&tenant_filter);
    let rate_override = tenant_rate_limit_override_for_tenant(&tenant_filter);
    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"tenant\":\"{}\",\"defaults\":{{\"task_quota\":{},\"subscription_quota\":{}}},\"effective\":{{\"task_quota\":{},\"subscription_quota\":{}}},\"override\":{},\"rate_defaults\":{{\"query_rate_limit\":{},\"mutation_rate_limit\":{},\"upload_cap_bytes\":{}}},\"rate_effective\":{{\"query_rate_limit\":{},\"mutation_rate_limit\":{},\"upload_cap_bytes\":{}}},\"rate_override\":{}}}",
            escape_json(&tenant_filter),
            DEFAULT_TENANT_TASK_QUOTA,
            DEFAULT_TENANT_SUBSCRIPTION_QUOTA,
            effective.task_quota,
            effective.subscription_quota,
            render_tenant_quota_override_json(override_limits),
            store.health.read().query_rate_limit,
            store.health.read().mutation_rate_limit,
            store.health.read().upload_cap_bytes,
            effective.query_rate_limit,
            effective.mutation_rate_limit,
            effective.upload_cap_bytes,
            render_tenant_rate_override_json(rate_override),
        ),
    ))
}

pub fn handle_workflow_tenant_quota_snapshot(
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    enforce_admin_api_version(request)?;
    if actor.role.as_deref() != Some("admin") {
        return Err(auth_denied_error("quota snapshot requires admin role"));
    }
    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let overrides = collect_tenant_quota_overrides_from_env();
    let rate_overrides = tenant_rate_limit_overrides_snapshot();
    let mut rows = Vec::new();
    for (tenant_key, override_limits) in overrides {
        rows.push(format!(
            "\"{}\":{{\"task_quota\":{},\"subscription_quota\":{}}}",
            escape_json(&tenant_key),
            override_limits.task_quota,
            override_limits.subscription_quota,
        ));
    }
    let mut rate_rows = Vec::new();
    for (tenant_key, override_limits) in rate_overrides {
        rate_rows.push(format!(
            "\"{}\":{{\"query_rate_limit\":{},\"mutation_rate_limit\":{},\"upload_cap_bytes\":{}}}",
            escape_json(&tenant_key),
            override_limits.query_rate_limit,
            override_limits.mutation_rate_limit,
            override_limits.upload_cap_bytes,
        ));
    }
    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"generated_at_ms\":{},\"defaults\":{{\"task_quota\":{},\"subscription_quota\":{}}},\"overrides\":{{{}}},\"rate_defaults\":{{\"query_rate_limit\":{},\"mutation_rate_limit\":{},\"upload_cap_bytes\":{}}},\"rate_overrides\":{{{}}}}}",
            now_epoch_millis(),
            DEFAULT_TENANT_TASK_QUOTA,
            DEFAULT_TENANT_SUBSCRIPTION_QUOTA,
            rows.join(","),
            store.health.read().query_rate_limit,
            store.health.read().mutation_rate_limit,
            store.health.read().upload_cap_bytes,
            rate_rows.join(","),
        ),
    ))
}

pub fn handle_workflow_metrics(
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let tenant_filter = request
        .query
        .get("tenant")
        .cloned()
        .unwrap_or_else(|| actor.tenant.clone());
    let is_admin = actor.role.as_deref() == Some("admin");
    if !is_admin && tenant_filter != actor.tenant && tenant_filter != "*" && tenant_filter != "all"
    {
        return Err(auth_denied_error(
            "metrics tenant scope outside caller tenant",
        ));
    }

    let metrics_snapshot = {
        let store = state
            .lock()
            .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
        let anomaly_alert_threshold = store.health.read().anomaly_alert_threshold;
        if tenant_filter == "*" || tenant_filter == "all" {
            let tenants: Vec<_> = store
                .tenant.lock().metrics
                .iter()
                .map(|(tenant, counters)| {
                    format!(
                        "\"{}\":{{\"read_operations\":{},\"query_operations\":{},\"mutation_operations\":{},\"anomaly_operations\":{},\"denied_operations\":{},\"alerts\":{}}}",
                        escape_json(tenant),
                        counters.read_operations,
                        counters.query_operations,
                        counters.mutation_operations,
                        counters.anomaly_operations,
                        counters.denied_operations,
                        render_tenant_alerts_json(counters, anomaly_alert_threshold),
                        )
                })
                .collect();
            format!(
                "{{\"tenants\":{{{}}},\"anomaly_alert_threshold\":{}}}",
                tenants.join(","),
                anomaly_alert_threshold,
            )
        } else {
            let tenant_data = store.tenant.lock();
            let counters = tenant_data.metrics.get(&tenant_filter);
            let counters = counters.cloned().unwrap_or_default();
            format!(
                "{{\"tenant\":\"{}\",\"read_operations\":{},\"query_operations\":{},\"mutation_operations\":{},\"anomaly_operations\":{},\"denied_operations\":{},\"anomaly_alert_threshold\":{},\"alerts\":{}}}",
                escape_json(&tenant_filter),
                counters.read_operations,
                counters.query_operations,
                counters.mutation_operations,
                counters.anomaly_operations,
                counters.denied_operations,
                anomaly_alert_threshold,
                render_tenant_alerts_json(&counters, anomaly_alert_threshold),
            )
        }
    };
    Ok(WorkflowResponse::Json(200, metrics_snapshot))
}

pub fn render_tenant_alerts_json(
    counters: &TenantOperationMetrics,
    anomaly_alert_threshold: u64,
) -> String {
    let mut alerts = Vec::new();
    if anomaly_alert_threshold > 0 && counters.anomaly_operations >= anomaly_alert_threshold {
        alerts.push(
            "{\"type\":\"rate_burst\",\"severity\":\"warn\",\"observed\":\"true\",\"unit\":\"requests\"}"
                .to_string(),
        );
    }
    format!("[{}]", alerts.join(","))
}

pub fn handle_workflow_audit(
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    if !actor.can_write() {
        return Err(auth_denied_error("writer role required"));
    }
    let tenant = request
        .query
        .get("tenant")
        .map(|value| value.as_str())
        .unwrap_or(actor.tenant.as_str());
    let is_admin = actor.role.as_deref() == Some("admin");
    if !is_admin && tenant != "*" && tenant != "all" && tenant != actor.tenant {
        return Err(auth_denied_error(
            "audit tenant scope outside caller tenant",
        ));
    }

    let verify = request
        .query
        .get("verify")
        .is_some_and(|value| matches!(value.as_str(), "true" | "1"));
    let limit = request
        .query
        .get("limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(usize::MAX)
        .min({
            let store = state
                .lock()
                .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
            let x = store.health.read().audit_export_limit; x
        });
    let path = {
        let store = state
            .lock()
            .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
        let x = store.health.read().audit_path.clone(); x
    };
    let all_lines: Vec<String> = fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(std::string::ToString::to_string)
        .collect();
    let failures = if verify {
        Some(verify_workflow_audit_chain(&all_lines))
    } else {
        None
    };

    let mut lines: Vec<String> = all_lines;
    lines.reverse();
    if tenant != "*" && tenant != "all" {
        lines = lines
            .into_iter()
            .filter(|entry| {
                let token = format!("\"tenant\":\"{}\"", escape_json(tenant));
                entry.contains(&token)
            })
            .collect();
    }
    let lines: Vec<String> = lines.into_iter().take(limit).collect();
    if let Some(failures) = failures {
        Ok(WorkflowResponse::Json(
            200,
            render_workflow_audit_payload_with_verification(&lines, &failures),
        ))
    } else {
        Ok(WorkflowResponse::Json(
            200,
            render_workflow_audit_payload(&lines),
        ))
    }
}

pub fn now_epoch_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |delta| delta.as_millis() as u64)
}

pub fn task_status_from_label(label: &str, context: &str) -> Result<TaskStatus, Box<Error>> {
    match label.trim().replace('_', " ").to_ascii_uppercase().as_str() {
        "SCHEDULED" => Ok(TaskStatus::Scheduled),
        "IN PROGRESS" | "INPROGRESS" => Ok(TaskStatus::InProgress),
        "ON HOLD" | "ONHOLD" | "HOLD" => Ok(TaskStatus::OnHold),
        "COMPLETED" => Ok(TaskStatus::Completed),
        "REVIEWED" => Ok(TaskStatus::Reviewed),
        "COMMITTED" => Ok(TaskStatus::Committed),
        "DISCONTINUED" => Ok(TaskStatus::Discontinued),
        _ => Err(decode_error(context)),
    }
}

impl TaskStatus {
    pub fn as_label(self) -> &'static str {
        match self {
            TaskStatus::Scheduled => "SCHEDULED",
            TaskStatus::InProgress => "IN PROGRESS",
            TaskStatus::OnHold => "ON HOLD",
            TaskStatus::Completed => "COMPLETED",
            TaskStatus::Reviewed => "REVIEWED",
            TaskStatus::Committed => "COMMITTED",
            TaskStatus::Discontinued => "DISCONTINUED",
        }
    }

    pub fn can_transition_to(self, next: TaskStatus) -> bool {
        match (self, next) {
            (TaskStatus::Scheduled, TaskStatus::InProgress)
            | (TaskStatus::Scheduled, TaskStatus::Discontinued) => true,
            (TaskStatus::InProgress, TaskStatus::Completed)
            | (TaskStatus::InProgress, TaskStatus::OnHold)
            | (TaskStatus::InProgress, TaskStatus::Discontinued) => true,
            (TaskStatus::OnHold, TaskStatus::InProgress)
            | (TaskStatus::OnHold, TaskStatus::Discontinued) => true,
            (TaskStatus::Completed, TaskStatus::Reviewed)
            | (TaskStatus::Completed, TaskStatus::Discontinued) => true,
            (TaskStatus::Reviewed, TaskStatus::Committed) => true,
            (TaskStatus::Completed, TaskStatus::Completed)
            | (TaskStatus::Reviewed, TaskStatus::Reviewed)
            | (TaskStatus::Committed, TaskStatus::Committed)
            | (TaskStatus::Discontinued, TaskStatus::Discontinued) => true,
            _ => false,
        }
    }
}

pub fn parse_task_status_filter(raw: Option<&String>) -> Result<Option<TaskStatus>, Box<Error>> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    Ok(Some(task_status_from_label(
        raw,
        "invalid task status filter",
    )?))
}

pub fn parse_query_param_language(
    raw_query: &str,
    limits: &Limits,
) -> Result<BTreeMap<String, String>, Box<Error>> {
    if raw_query.is_empty() {
        return Ok(BTreeMap::new());
    }
    parse_query_map(raw_query, limits)
        .map_err(decode_error)
        .map(|query| {
            let mut query = query;
            query.remove("query");
            query
        })
}

pub fn merge_query_parameters(
    raw_params: &BTreeMap<String, String>,
    limits: &Limits,
) -> Result<BTreeMap<String, String>, Box<Error>> {
    let mut query = if let Some(raw_query) = raw_params.get("query") {
        parse_query_param_language(raw_query, limits)?
    } else {
        BTreeMap::new()
    };
    for (key, value) in raw_params {
        if key == "query" {
            continue;
        }
        query.insert(key.clone(), value.clone());
    }
    Ok(query)
}

pub fn task_not_found_error() -> Box<Error> {
    Error::from_kind(
        ErrorKind::NotFound {
            detail: "task not found".to_string(),
        },
        "task not found",
    )
    .into()
}

pub fn task_status_invalid_transition_error() -> Box<Error> {
    Error::from_kind(
        ErrorKind::IntegrityError {
            detail: "invalid task status transition".to_string(),
        },
        "task status transition not allowed",
    )
    .into()
}

pub fn task_idempotency_limit(cache: &mut BTreeMap<String, CachedMppsRequest>) {
    while cache.len() > MAX_TASK_IDEMPOTENCY_ENTRIES {
        let Some(oldest) = cache.keys().next().cloned() else {
            break;
        };
        let _ = cache.remove(&oldest);
    }
}

pub fn hl7_replay_cache_limit(cache: &mut BTreeMap<String, CachedMppsRequest>) {
    while cache.len() > MAX_HL7_IDEMPOTENCY_ENTRIES {
        let Some(oldest) = cache.keys().next().cloned() else {
            break;
        };
        let _ = cache.remove(&oldest);
    }
}

pub fn hl7_callback_idempotency_limit(cache: &mut BTreeMap<String, u64>) {
    while cache.len() > MAX_HL7_IDEMPOTENCY_ENTRIES {
        let Some(oldest) = cache.keys().next().cloned() else {
            break;
        };
        let _ = cache.remove(&oldest);
    }
}

pub fn callback_idempotency_snapshot_path(workflow_audit_path: &str) -> String {
    format!("{workflow_audit_path}.callback-idempotency")
}

pub fn hl7_callback_idempotency_prune(cache: &mut BTreeMap<String, u64>, now_ms: u64, ttl_ms: u64) {
    if ttl_ms > 0 {
        cache.retain(|_, observed_ms| now_ms.saturating_sub(*observed_ms) <= ttl_ms);
    }
    hl7_callback_idempotency_limit(cache);
}

pub fn load_hl7_callback_idempotency_cache(
    path: &str,
    now_ms: u64,
    ttl_ms: u64,
) -> BTreeMap<String, u64> {
    let mut out = BTreeMap::new();
    let raw = match fs::read_to_string(path) {
        Ok(value) => value,
        Err(_) => return out,
    };
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some((timestamp_raw, key_raw)) = trimmed.split_once('\t') else {
            continue;
        };
        let Ok(observed_ms) = timestamp_raw.parse::<u64>() else {
            continue;
        };
        let key = key_raw.trim().to_string();
        if key.is_empty() {
            continue;
        }
        let _ = out.insert(key, observed_ms);
    }
    hl7_callback_idempotency_prune(&mut out, now_ms, ttl_ms);
    out
}

pub fn persist_hl7_callback_idempotency_cache(path: &str, cache: &BTreeMap<String, u64>) {
    if path.trim().is_empty() {
        return;
    }
    let mut lines = String::new();
    for (key, observed_ms) in cache {
        lines.push_str(&format!("{observed_ms}\t{key}\n"));
    }
    let _ = fs::write(path, lines);
}

pub fn tenant_rate_limit_override_snapshot_path(workflow_audit_path: &str) -> String {
    format!("{workflow_audit_path}.tenant-rate-limit-overrides")
}

pub fn load_tenant_rate_limit_overrides(path: &str) -> BTreeMap<String, TenantRateLimitOverride> {
    let mut out = BTreeMap::new();
    let raw = match fs::read_to_string(path) {
        Ok(value) => value,
        Err(_) => return out,
    };
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parts: Vec<&str> = trimmed.split('\t').collect();
        if parts.len() != 4 {
            continue;
        }
        let tenant_key = parts[0].trim().to_ascii_lowercase();
        if tenant_key.is_empty() {
            continue;
        }
        let Ok(query_rate_limit) = parts[1].parse::<u64>() else {
            continue;
        };
        let Ok(mutation_rate_limit) = parts[2].parse::<u64>() else {
            continue;
        };
        let Ok(upload_cap_bytes) = parts[3].parse::<u64>() else {
            continue;
        };
        if query_rate_limit == 0 || mutation_rate_limit == 0 || upload_cap_bytes == 0 {
            continue;
        }
        let _ = out.insert(
            tenant_key,
            TenantRateLimitOverride {
                query_rate_limit,
                mutation_rate_limit,
                upload_cap_bytes,
            },
        );
    }
    out
}

pub fn persist_tenant_rate_limit_overrides(
    path: &str,
    overrides: &BTreeMap<String, TenantRateLimitOverride>,
) {
    if path.trim().is_empty() {
        return;
    }
    let mut lines = String::new();
    for (tenant_key, override_limits) in overrides {
        lines.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            dlq_field_encode(tenant_key),
            override_limits.query_rate_limit,
            override_limits.mutation_rate_limit,
            override_limits.upload_cap_bytes
        ));
    }
    let _ = fs::write(path, lines);
}

pub fn connector_rollout_snapshot_path(workflow_audit_path: &str) -> String {
    format!("{workflow_audit_path}.connector-rollout")
}

pub fn load_hl7_connector_rollout_state(
    path: &str,
    defaults: &BTreeMap<String, u64>,
) -> BTreeMap<String, u64> {
    let mut out = defaults.clone();
    let raw = match fs::read_to_string(path) {
        Ok(value) => value,
        Err(_) => return out,
    };
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some((alias_raw, rollout_raw)) = trimmed.split_once('\t') else {
            continue;
        };
        let alias = normalize_connector_alias(alias_raw);
        if alias.is_empty() {
            continue;
        }
        let Ok(rollout) = rollout_raw.parse::<u64>() else {
            continue;
        };
        if rollout > 100 {
            continue;
        }
        let _ = out.insert(alias, rollout);
    }
    out
}

pub fn persist_hl7_connector_rollout_state(path: &str, rollout: &BTreeMap<String, u64>) {
    if path.trim().is_empty() {
        return;
    }
    let mut lines = String::new();
    for (alias, percent) in rollout {
        lines.push_str(&format!("{alias}\t{percent}\n"));
    }
    let _ = fs::write(path, lines);
}

pub fn hl7_failure_queue_snapshot_path(workflow_audit_path: &str) -> String {
    format!("{workflow_audit_path}.hl7-failures.dlq")
}

pub fn dlq_field_encode(raw: &str) -> String {
    raw.replace('\t', " ").replace('\n', " ")
}

pub fn load_hl7_failure_queue(path: &str, max_entries: usize) -> VecDeque<Hl7FailureRecord> {
    let mut out = VecDeque::new();
    let raw = match fs::read_to_string(path) {
        Ok(value) => value,
        Err(_) => return out,
    };
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parts: Vec<&str> = trimmed.split('\t').collect();
        if parts.len() != 11 && parts.len() != 13 {
            continue;
        }
        let Ok(created_at_ms) = parts[5].parse::<u64>() else {
            continue;
        };
        let Ok(sequence) = parts[10].parse::<u64>() else {
            continue;
        };
        let (attempt, max_attempts) = if parts.len() == 13 {
            let Ok(attempt) = parts[11].parse::<u32>() else {
                continue;
            };
            let Ok(max_attempts) = parts[12].parse::<u32>() else {
                continue;
            };
            (attempt, max_attempts)
        } else {
            (1, MAX_WORKFLOW_CALLBACK_ATTEMPTS)
        };
        out.push_back(Hl7FailureRecord {
            id: normalize_identifier(parts[0]),
            source: normalize_identifier(parts[1]),
            message_type: normalize_identifier(parts[2]),
            reason: normalize_identifier(parts[3]),
            payload_excerpt: normalize_identifier(parts[4]),
            created_at_ms,
            scope: normalize_identifier(parts[6]),
            subscription_id: normalize_identifier(parts[7]),
            event_id: normalize_identifier(parts[8]),
            correlation_id: normalize_identifier(parts[9]),
            sequence,
            attempt,
            max_attempts,
        });
    }
    while out.len() > max_entries {
        let _ = out.pop_back();
    }
    out
}

pub fn persist_hl7_failure_queue(path: &str, queue: &VecDeque<Hl7FailureRecord>) {
    if path.trim().is_empty() {
        return;
    }
    let mut lines = String::new();
    for record in queue {
        lines.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            dlq_field_encode(&record.id),
            dlq_field_encode(&record.source),
            dlq_field_encode(&record.message_type),
            dlq_field_encode(&record.reason),
            dlq_field_encode(&record.payload_excerpt),
            record.created_at_ms,
            dlq_field_encode(&record.scope),
            dlq_field_encode(&record.subscription_id),
            dlq_field_encode(&record.event_id),
            dlq_field_encode(&record.correlation_id),
            record.sequence,
            record.attempt,
            record.max_attempts
        ));
    }
    let _ = fs::write(path, lines);
}

#[allow(missing_docs)]
pub fn render_hl7_failure_queue_legacy_v1(queue: &VecDeque<Hl7FailureRecord>) -> String {
    let mut lines = String::new();
    for record in queue {
        lines.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            dlq_field_encode(&record.id),
            dlq_field_encode(&record.source),
            dlq_field_encode(&record.message_type),
            dlq_field_encode(&record.reason),
            dlq_field_encode(&record.payload_excerpt),
            record.created_at_ms,
            dlq_field_encode(&record.scope),
            dlq_field_encode(&record.subscription_id),
            dlq_field_encode(&record.event_id),
            dlq_field_encode(&record.correlation_id),
            record.sequence,
        ));
    }
    lines
}

pub fn parse_hl7_failure_id_sequence(id: &str) -> Option<u64> {
    let suffix = id.strip_prefix("hl7-fail-")?;
    suffix.parse::<u64>().ok()
}

pub fn hl7_failure_sequence_seed(queue: &VecDeque<Hl7FailureRecord>) -> u64 {
    queue
        .iter()
        .filter_map(|record| parse_hl7_failure_id_sequence(&record.id))
        .max()
        .unwrap_or(0)
}

pub fn hl7_event_sequence_seed(queue: &VecDeque<Hl7FailureRecord>) -> u64 {
    queue
        .iter()
        .map(|record| record.sequence)
        .max()
        .unwrap_or(0)
}

pub fn reconciliation_run_idempotency_snapshot_path(workflow_audit_path: &str) -> String {
    format!("{workflow_audit_path}.reconciliation-run-idempotency")
}

pub fn reconciliation_run_idempotency_limit(cache: &mut BTreeMap<String, CachedMppsRequest>) {
    while cache.len() > MAX_HL7_IDEMPOTENCY_ENTRIES {
        let Some(oldest) = cache.keys().next().cloned() else {
            break;
        };
        let _ = cache.remove(&oldest);
    }
}

pub fn load_reconciliation_run_idempotency_cache(path: &str) -> BTreeMap<String, CachedMppsRequest> {
    let mut out = BTreeMap::new();
    let raw = match fs::read_to_string(path) {
        Ok(value) => value,
        Err(_) => return out,
    };
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.splitn(3, '\t');
        let Some(key) = parts.next() else {
            continue;
        };
        let Some(signature) = parts.next() else {
            continue;
        };
        let Some(response) = parts.next() else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || !key.starts_with(RECONCILIATION_RUN_IDEMPOTENCY_PREFIX) {
            continue;
        }
        let _ = out.insert(
            key.to_string(),
            CachedMppsRequest {
                signature: signature.to_string(),
                response: response.to_string(),
            },
        );
    }
    reconciliation_run_idempotency_limit(&mut out);
    out
}

pub fn persist_reconciliation_run_idempotency_cache(
    path: &str,
    cache: &BTreeMap<String, CachedMppsRequest>,
) {
    if path.trim().is_empty() {
        return;
    }
    let mut lines = String::new();
    for (key, value) in cache {
        if !key.starts_with(RECONCILIATION_RUN_IDEMPOTENCY_PREFIX) {
            continue;
        }
        lines.push_str(&format!(
            "{}\t{}\t{}\n",
            dlq_field_encode(key),
            dlq_field_encode(&value.signature),
            dlq_field_encode(&value.response)
        ));
    }
    let _ = fs::write(path, lines);
}

pub fn task_next_id(sequence: &mut u64) -> String {
    let next = format!("TASK-{sequence:06}");
    *sequence = sequence.saturating_add(1);
    next
}

pub fn render_task_json(task: &ProcedureTask) -> String {
    format!(
        "{{\"task_id\":\"{}\",\"scheduled_step_id\":\"{}\",\"requested_procedure_id\":{},\"status\":\"{}\",\"worker\":{},\"created_at_ms\":{},\"updated_at_ms\":{}}}",
        escape_json(&task.task_id),
        escape_json(&task.scheduled_step_id),
        task.requested_procedure_id
            .as_ref()
            .map(|value| format!("\"{}\"", escape_json(value)))
            .unwrap_or_else(|| "null".to_string()),
        task.status.as_label(),
        task.worker
            .as_ref()
            .map(|value| format!("\"{}\"", escape_json(value)))
            .unwrap_or_else(|| "null".to_string()),
        task.created_at_ms,
        task.updated_at_ms,
    )
}

pub fn handle_task_list(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let query = merge_query_parameters(&request.query, limits)?;
    let status_filter = parse_task_status_filter(query.get("status"))?;
    let scheduled_step_filter = query.get("scheduled_step_id").cloned();

    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let mut tasks: Vec<ProcedureTask> = store
        .worker.lock().tasks
        .iter()
        .filter(|(task_id, _)| actor_has_resource_access(&actor, &store.tenant.lock().tenant_tasks, task_id))
        .map(|(_, task)| task.clone())
        .collect();
    drop(store);

    if let Some(status) = status_filter {
        tasks.retain(|task| task.status == status);
    }
    if let Some(step_id) = scheduled_step_filter {
        tasks.retain(|task| task.scheduled_step_id == step_id);
    }

    match query.get("sort").map(|value| value.as_str()) {
        None | Some("default") | Some("task_id") => tasks.sort_by(|a, b| a.task_id.cmp(&b.task_id)),
        Some("status") => tasks.sort_by(|a, b| a.status.cmp(&b.status)),
        Some("scheduled_step_id") => {
            tasks.sort_by(|a, b| a.scheduled_step_id.cmp(&b.scheduled_step_id))
        }
        Some("created_at") => tasks.sort_by(|a, b| a.created_at_ms.cmp(&b.created_at_ms)),
        Some(_) => return Err(decode_error("unsupported task sort")),
    }
    if query.get("order") == Some(&"desc".to_string()) {
        tasks.reverse();
    }

    let (offset, limit) = parse_pagination(query.get("page"), query.get("page_size"))?;
    let end = tasks.len().min(offset.saturating_add(limit));
    let mut tasks_slice = if offset > tasks.len() {
        Vec::new()
    } else {
        tasks[offset..end].to_vec()
    };
    let mut json = String::from("[");
    for (idx, task) in tasks_slice.drain(..).enumerate() {
        if idx > 0 {
            json.push(',');
        }
        json.push_str(&render_task_json(&task));
    }
    json.push(']');
    Ok(WorkflowResponse::Json(200, json))
}

pub fn handle_task_get(
    task_id: &str,
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    if task_id.contains('/') {
        return Err(decode_error("unsupported route"));
    }
    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let worker = store.worker.lock();
    let Some(task) = worker.tasks.get(task_id) else {
        return Err(task_not_found_error());
    };
    if !actor_has_resource_access(&actor, &store.tenant.lock().tenant_tasks, task_id) {
        return Err(auth_denied_error("task belongs to another tenant"));
    }
    Ok(WorkflowResponse::Json(200, render_task_json(task)))
}

pub fn handle_task_control(
    task_id: &str,
    action: &str,
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let mut store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if !actor_has_resource_access(&actor, &store.tenant.lock().tenant_tasks, task_id) {
        return Err(auth_denied_error("task belongs to another tenant"));
    }
    let worker = store.worker.lock();
    let Some(existing) = worker.tasks.get(task_id) else {
        return Err(task_not_found_error());
    };
    let target_status = match action {
        "start" => TaskStatus::InProgress,
        "pause" => TaskStatus::OnHold,
        "resume" => TaskStatus::InProgress,
        "complete" => TaskStatus::Completed,
        "review" => TaskStatus::Reviewed,
        "commit" => TaskStatus::Committed,
        "cancel" => TaskStatus::Discontinued,
        _ => return Err(decode_error("unsupported task action")),
    };
    if !existing.status.can_transition_to(target_status) {
        return Err(task_status_invalid_transition_error());
    }
    let previous_status = existing.status;
    drop(worker);
    if previous_status == target_status {
        return Ok(WorkflowResponse::Json(
            200,
            format!(
                "{{\"outcome\":\"ok\",\"task_id\":\"{}\",\"status\":\"{}\"}}",
                escape_json(task_id),
                previous_status.as_label()
            ),
        ));
    }
    let (event_id, sequence) = next_hl7_event_id_with_sequence(&mut store);
    let correlation_id = request_id_from_headers(&request.headers);
    let worker = request
        .headers
        .get("x-task-worker")
        .or_else(|| request.headers.get("x-sr-principal"))
        .map(ToString::to_string);
    if let Some(task) = store.worker.lock().tasks.get_mut(task_id) {
        task.status = target_status;
        if let Some(worker) = worker {
            task.worker = Some(worker);
        }
        task.updated_at_ms = now_epoch_millis();
    }
    let mut payload = BTreeMap::new();
    payload.insert("event".to_string(), "task.transition".to_string());
    payload.insert("action".to_string(), action.to_string());
    payload.insert("task_id".to_string(), task_id.to_string());
    payload.insert(
        "from_status".to_string(),
        previous_status.as_label().to_string(),
    );
    payload.insert(
        "to_status".to_string(),
        target_status.as_label().to_string(),
    );
    payload.insert("tenant".to_string(), actor.tenant.clone());
    if let Some(actor_name) = actor.principal.clone() {
        payload.insert("actor".to_string(), actor_name);
    }
    publish_hl7_event(
        &mut store,
        &workflow_event_source(&actor),
        "task",
        &payload,
        &event_id,
        sequence,
        &correlation_id,
    );
    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"outcome\":\"ok\",\"task_id\":\"{}\",\"status\":\"{}\"}}",
            escape_json(task_id),
            target_status.as_label()
        ),
    ))
}

pub fn completion_outcome_label(outcome: CompletionOutcome) -> &'static str {
    match outcome {
        CompletionOutcome::Success => "success",
        CompletionOutcome::Failure => "failure",
    }
}

pub fn parse_completion_outcome(value: &str) -> Result<CompletionOutcome, Box<Error>> {
    match value.trim().to_ascii_lowercase().as_str() {
        "success" | "ok" | "completed" | "delivered" => Ok(CompletionOutcome::Success),
        "failure" | "failed" | "error" | "timedout" | "canceled" => Ok(CompletionOutcome::Failure),
        _ => Err(decode_error("invalid completion outcome")),
    }
}

pub fn ups_state_label(state: UpsState) -> &'static str {
    match state {
        UpsState::Scheduled => "scheduled",
        UpsState::InProgress => "in_progress",
        UpsState::Canceled => "canceled",
        UpsState::Completed => "completed",
        UpsState::Failed => "failed",
    }
}

pub fn parse_ups_transition(value: &str) -> Result<UpsTransition, Box<Error>> {
    match value.trim().to_ascii_lowercase().as_str() {
        "start" => Ok(UpsTransition::Start),
        "cancel" => Ok(UpsTransition::Cancel),
        "complete" => Ok(UpsTransition::Complete),
        "fail" => Ok(UpsTransition::Fail),
        _ => Err(decode_error("unsupported ups transition action")),
    }
}

pub fn normalize_ups_uid(raw: &str) -> Result<String, Box<Error>> {
    let uid = normalize_route_identifier(raw)?;
    dicom_core::validate_uid_strict(TAG_SOP_INSTANCE_UID, &uid)?;
    Ok(uid)
}

pub fn tenant_scoped_key(tenant: &str, id: &str) -> String {
    format!("{tenant}|{id}")
}

pub fn tenant_scoped_suffix<'a>(key: &'a str, tenant: &str) -> Option<&'a str> {
    let prefix = format!("{tenant}|");
    key.strip_prefix(&prefix)
}

pub fn render_ups_workitem_json(item: &dicom_ups::UpsWorkitem) -> String {
    format!(
        "{{\"ups_instance_uid\":\"{}\",\"procedure_step_label\":\"{}\",\"state\":\"{}\",\"revision\":{}}}",
        escape_json(&item.ups_instance_uid),
        escape_json(&item.procedure_step_label),
        ups_state_label(item.state),
        item.revision,
    )
}

pub fn handle_workitem_routes(
    request_path: &str,
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
    _actor: &WorkflowActorContext,
) -> Result<WorkflowResponse, Box<Error>> {
    if request_path == WORKITEM_COLLECTION_PATH {
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_workitem_list(request, state, limits),
            "POST" => handle_workitem_create(request, state, limits),
            _ => Err(decode_error("unsupported route")),
        };
    }
    let Some(rest) = request_path.strip_prefix(WORKITEM_ITEM_PREFIX) else {
        return Err(decode_error("unsupported route"));
    };
    if rest.is_empty() {
        return Err(decode_error("unsupported route"));
    }
    if rest == "search" {
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_workitem_list(request, state, limits),
            _ => Err(decode_error("unsupported route")),
        };
    }
    if let Some((workitem_uid, suffix)) = rest.split_once('/') {
        let workitem_uid = normalize_ups_uid(workitem_uid)?;
        return match (request.method.as_str(), suffix) {
            ("GET", "state") | ("HEAD", "state") => {
                handle_workitem_state_get(&workitem_uid, request, state)
            }
            ("POST", "state") => {
                handle_workitem_state_update(&workitem_uid, request, state, limits)
            }
            ("POST", "cancel") => handle_workitem_cancel(&workitem_uid, request, state),
            _ => Err(decode_error("unsupported route")),
        };
    }
    let workitem_uid = normalize_ups_uid(rest)?;
    match request.method.as_str() {
        "GET" | "HEAD" => handle_workitem_get(&workitem_uid, request, state),
        "POST" => handle_workitem_update(&workitem_uid, request, state, limits),
        _ => Err(decode_error("unsupported route")),
    }
}

pub fn handle_workitem_list(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let query = merge_query_parameters(&request.query, limits)?;
    let state_filter = query
        .get("state")
        .map(|value| value.trim().to_ascii_lowercase());

    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let mut items: Vec<String> = Vec::new();
    for item in store.hl7.lock().ups.store().workitems() {
        if !actor_has_resource_access(&actor, &store.tenant.lock().tenant_tasks, &item.ups_instance_uid) {
            continue;
        }
        if let Some(filter) = &state_filter {
            if ups_state_label(item.state) != filter {
                continue;
            }
        }
        items.push(render_ups_workitem_json(item));
    }

    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"count\":{},\"items\":[{}]}}",
            items.len(),
            items.join(",")
        ),
    ))
}

pub fn handle_workitem_create(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let params = parse_form_map(&request.body, limits)?;
    let ups_instance_uid = normalize_ups_uid(required_param(&params, "ups_instance_uid")?)?;
    let procedure_step_label = required_param(&params, "procedure_step_label")?.to_string();
    let correlation_id = params.get("hl7_correlation_id").cloned();

    let request_signature = build_request_signature(&params);
    let idempotency_key = request
        .headers
        .get("x-idempotency-key")
        .cloned()
        .unwrap_or_default();
    let cache_key = format!("ups-create:{ups_instance_uid}:{idempotency_key}");

    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if let Some(entry) = store.worker.lock().task_idempotency.get(&cache_key) {
        if entry.signature == request_signature {
            return Ok(WorkflowResponse::Json(200, entry.response.clone()));
        }
        return Err(decode_error("idempotency key replay conflict"));
    }

    let created = store
        .hl7
        .lock()
        .ups
        .create(ups_instance_uid.clone(), procedure_step_label)?;
    route_id_to_tenant_index(&mut store.tenant.lock().tenant_tasks, &actor.tenant, &ups_instance_uid);
    if let Some(correlation_id) = correlation_id {
        let key = tenant_scoped_key(&actor.tenant, &correlation_id);
        let _ = store
            .hl7
            .lock()
            .hl7_ups_correlation
            .insert(key, ups_instance_uid.clone());
    }

    let body = render_ups_workitem_json(&created);
    let _ = store.worker.lock().task_idempotency.insert(
        cache_key,
        CachedMppsRequest {
            signature: request_signature,
            response: body.clone(),
        },
    );
    task_idempotency_limit(&mut store.worker.lock().task_idempotency);
    Ok(WorkflowResponse::Json(200, body))
}

pub fn handle_workitem_get(
    workitem_uid: &str,
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if !actor_has_resource_access(&actor, &store.tenant.lock().tenant_tasks, workitem_uid) {
        return Err(auth_denied_error("ups workitem belongs to another tenant"));
    }
    let item = store.hl7.lock().ups.get(workitem_uid)?;
    Ok(WorkflowResponse::Json(200, render_ups_workitem_json(&item)))
}

pub fn handle_workitem_update(
    workitem_uid: &str,
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let params = parse_form_map(&request.body, limits)?;
    let new_label = required_param(&params, "procedure_step_label")?.to_string();

    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if !actor_has_resource_access(&actor, &store.tenant.lock().tenant_tasks, workitem_uid) {
        return Err(auth_denied_error("ups workitem belongs to another tenant"));
    }
    let item = store.hl7.lock().ups.update(workitem_uid, new_label)?;
    Ok(WorkflowResponse::Json(200, render_ups_workitem_json(&item)))
}

pub fn handle_workitem_state_get(
    workitem_uid: &str,
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if !actor_has_resource_access(&actor, &store.tenant.lock().tenant_tasks, workitem_uid) {
        return Err(auth_denied_error("ups workitem belongs to another tenant"));
    }
    let item = store.hl7.lock().ups.get(workitem_uid)?;
    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"ups_instance_uid\":\"{}\",\"state\":\"{}\",\"revision\":{}}}",
            escape_json(workitem_uid),
            ups_state_label(item.state),
            item.revision,
        ),
    ))
}

pub fn handle_workitem_state_update(
    workitem_uid: &str,
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let params = parse_form_map(&request.body, limits)?;
    let action = required_param(&params, "action")?;
    let transition = parse_ups_transition(action)?;

    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if !actor_has_resource_access(&actor, &store.tenant.lock().tenant_tasks, workitem_uid) {
        return Err(auth_denied_error("ups workitem belongs to another tenant"));
    }
    let item = match transition {
        UpsTransition::Start => store.hl7.lock().ups.start(workitem_uid)?,
        UpsTransition::Cancel => store.hl7.lock().ups.cancel(workitem_uid)?,
        UpsTransition::Complete => store.hl7.lock().ups.complete(workitem_uid)?,
        UpsTransition::Fail => store.hl7.lock().ups.fail(workitem_uid)?,
    };
    Ok(WorkflowResponse::Json(200, render_ups_workitem_json(&item)))
}

pub fn handle_workitem_cancel(
    workitem_uid: &str,
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if !actor_has_resource_access(&actor, &store.tenant.lock().tenant_tasks, workitem_uid) {
        return Err(auth_denied_error("ups workitem belongs to another tenant"));
    }
    let item = store.hl7.lock().ups.cancel(workitem_uid)?;
    Ok(WorkflowResponse::Json(200, render_ups_workitem_json(&item)))
}

pub fn handle_ian_list(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let mut payload = String::from("[");
    let mut first = true;
    for (key, record) in &store.hl7.lock().ian_events {
        let Some(event_id) = tenant_scoped_suffix(key, &actor.tenant) else {
            continue;
        };
        if !first {
            payload.push(',');
        }
        first = false;
        payload.push_str(&format!(
            "{{\"event_id\":\"{}\",\"sop_instance_uid\":\"{}\",\"outcome\":\"{}\",\"ingested_at_ms\":{}}}",
            escape_json(event_id),
            escape_json(&record.sop_instance_uid),
            completion_outcome_label(record.outcome),
            record.ingested_at_ms,
        ));
    }
    payload.push(']');
    Ok(WorkflowResponse::Json(200, payload))
}

pub fn handle_ian_ingest(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let params = parse_form_map(&request.body, limits)?;
    let event_id = required_param(&params, "event_id")?.to_string();
    let sop_instance_uid = normalize_ups_uid(required_param(&params, "sop_instance_uid")?)?;
    let outcome = parse_completion_outcome(required_param(&params, "outcome")?)?;

    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let inserted =
        store
            .hl7
            .lock()
            .completion
            .ingest_ian(event_id.clone(), sop_instance_uid.clone(), outcome);
    if inserted {
        let key = tenant_scoped_key(&actor.tenant, &event_id);
        let _ = store.hl7.lock().ian_events.insert(
            key,
            IanEventRecord {
                event_id,
                sop_instance_uid,
                outcome,
                ingested_at_ms: now_epoch_millis(),
            },
        );
    }
    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"inserted\":{},\"outcome\":\"{}\"}}",
            inserted,
            completion_outcome_label(outcome)
        ),
    ))
}

pub fn handle_storage_commitment_status_list(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let mut payload = String::from("[");
    let mut first = true;
    for (key, record) in &store.hl7.lock().storage_commitment_status {
        let Some(transaction_uid) = tenant_scoped_suffix(key, &actor.tenant) else {
            continue;
        };
        if !first {
            payload.push(',');
        }
        first = false;
        payload.push_str(&format!(
            "{{\"transaction_uid\":\"{}\",\"outcome\":\"{}\",\"updated_at_ms\":{}}}",
            escape_json(transaction_uid),
            completion_outcome_label(record.outcome),
            record.updated_at_ms,
        ));
    }
    payload.push(']');
    Ok(WorkflowResponse::Json(200, payload))
}

pub fn handle_storage_commitment_status_get(
    transaction_uid: &str,
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    dicom_core::validate_uid_strict(TAG_SOP_INSTANCE_UID, transaction_uid)?;
    let actor = workflow_actor_context(&request.headers);
    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let key = tenant_scoped_key(&actor.tenant, transaction_uid);
    let hl7 = store.hl7.lock();
    let Some(record) = hl7.storage_commitment_status.get(&key) else {
        return Err(decode_error("storage commitment status not found"));
    };
    let outcome = record.outcome;
    let updated_at_ms = record.updated_at_ms;
    drop(hl7);
    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"transaction_uid\":\"{}\",\"outcome\":\"{}\",\"updated_at_ms\":{}}}",
            escape_json(transaction_uid),
            completion_outcome_label(outcome),
            updated_at_ms,
        ),
    ))
}

pub fn handle_storage_commitment_status_upsert(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let params = parse_form_map(&request.body, limits)?;
    let transaction_uid = normalize_ups_uid(required_param(&params, "transaction_uid")?)?;
    let outcome = parse_completion_outcome(required_param(&params, "outcome")?)?;

    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let event_id = format!("stgc:{}", transaction_uid);
    let _ =
        store
            .hl7
            .lock()
            .completion
            .ingest_storage_commitment(event_id, transaction_uid.clone(), outcome);
    let key = tenant_scoped_key(&actor.tenant, &transaction_uid);
    let _ = store.hl7.lock().storage_commitment_status.insert(
        key,
        StorageCommitmentStatusRecord {
            transaction_uid: transaction_uid.clone(),
            outcome,
            updated_at_ms: now_epoch_millis(),
        },
    );
    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"transaction_uid\":\"{}\",\"outcome\":\"{}\"}}",
            escape_json(&transaction_uid),
            completion_outcome_label(outcome),
        ),
    ))
}

pub fn handle_hl7_ups_correlation_list(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let mut payload = String::from("[");
    let mut first = true;
    for (key, ups_uid) in &store.hl7.lock().hl7_ups_correlation {
        let Some(correlation_id) = tenant_scoped_suffix(key, &actor.tenant) else {
            continue;
        };
        if !first {
            payload.push(',');
        }
        first = false;
        payload.push_str(&format!(
            "{{\"correlation_id\":\"{}\",\"ups_instance_uid\":\"{}\"}}",
            escape_json(correlation_id),
            escape_json(ups_uid),
        ));
    }
    payload.push(']');
    Ok(WorkflowResponse::Json(200, payload))
}

pub fn handle_hl7_ups_correlation_upsert(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let params = parse_form_map(&request.body, limits)?;
    let correlation_id = required_param(&params, "correlation_id")?.to_string();
    let ups_instance_uid = normalize_ups_uid(required_param(&params, "ups_instance_uid")?)?;

    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let key = tenant_scoped_key(&actor.tenant, &correlation_id);
    let _ = store
        .hl7
        .lock()
        .hl7_ups_correlation
        .insert(key, ups_instance_uid.clone());
    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"correlation_id\":\"{}\",\"ups_instance_uid\":\"{}\"}}",
            escape_json(&correlation_id),
            escape_json(&ups_instance_uid),
        ),
    ))
}

pub fn handle_task_create(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let params = parse_form_map(&request.body, limits)?;
    let scheduled_step_id = required_param(&params, "scheduled_step_id")?.to_string();
    let requested_procedure_id = params.get("requested_procedure_id").cloned();
    let worker = params.get("worker").cloned();
    let status = params
        .get("status")
        .map(|status| task_status_from_label(status, "invalid task status"))
        .transpose()?
        .unwrap_or(TaskStatus::Scheduled);
    let provided_task_id = params.get("task_id").cloned();

    let request_signature = build_request_signature(&params);
    let idempotency_key = request
        .headers
        .get("x-idempotency-key")
        .cloned()
        .unwrap_or_default();
    let cache_key = if idempotency_key.is_empty() {
        None
    } else {
        Some(format!("task-create:{idempotency_key}"))
    };

    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if let Some(key) = &cache_key {
        if let Some(entry) = store.worker.lock().task_idempotency.get(key) {
            if entry.signature == request_signature {
                return Ok(WorkflowResponse::Json(200, entry.response.clone()));
            }
            return Err(decode_error("idempotency key replay conflict"));
        }
    }

    let task_id = provided_task_id.unwrap_or_else(|| task_next_id(&mut store.worker.lock().task_id_sequence));
    let now = now_epoch_millis();
    let policy = tenant_policy(&store, &actor.tenant);

    if let Some(_task) = store.worker.lock().tasks.get(&task_id) {
        if !actor_has_resource_access(&actor, &store.tenant.lock().tenant_tasks, &task_id) {
            return Err(auth_denied_error("task belongs to another tenant"));
        }
    } else {
        let existing = store
            .tenant.lock().tenant_tasks
            .get(&actor.tenant)
            .map(|ids| ids.len())
            .unwrap_or_default();
        if existing >= policy.task_quota {
            return Err(limit_exceeded(
                "workflow_tenant_task_quota",
                existing as u64,
                policy.task_quota as u64,
            ));
        }
    }

    let outcome = if let Some(task) = store.worker.lock().tasks.get_mut(&task_id) {
        task.scheduled_step_id = scheduled_step_id.clone();
        task.requested_procedure_id = requested_procedure_id.clone();
        task.status = status;
        task.worker = worker.clone();
        task.updated_at_ms = now;
        "updated"
    } else {
        store.worker.lock().tasks.insert(
            task_id.clone(),
            ProcedureTask {
                task_id: task_id.clone(),
                scheduled_step_id,
                requested_procedure_id,
                status,
                worker,
                tenant: actor.tenant.clone(),
                created_at_ms: now,
                updated_at_ms: now,
            },
        );
        route_id_to_tenant_index(&mut store.tenant.lock().tenant_tasks, &actor.tenant, &task_id);
        "inserted"
    };

    let body = format!(
        "{{\"outcome\":\"{}\",\"task_id\":\"{}\"}}",
        outcome, task_id,
    );

    if let Some(key) = cache_key {
        store.worker.lock().task_idempotency.insert(
            key,
            CachedMppsRequest {
                signature: request_signature,
                response: body.clone(),
            },
        );
        task_idempotency_limit(&mut store.worker.lock().task_idempotency);
    }

    Ok(WorkflowResponse::Json(200, body))
}

pub fn handle_worklist_query(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let query_params = merge_query_parameters(&request.query, limits)?;
    let worklist_query = WorklistQuery {
        modality: query_params.get("modality").cloned(),
        scheduled_step_id: query_params.get("scheduled_step_id").cloned(),
        patient_id: lookup_identifier_from_map(
            &query_params,
            &["patient_id", "PatientID", "patientId", "Patient_Id"],
        ),
        requested_procedure_id: query_params.get("requested_procedure_id").cloned(),
    };
    let status_filter = parse_worklist_status_filter(query_params.get("status"))?;

    let (rows, status_by_step) = {
        let store = state
            .lock()
            .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
        let mpps = store.mpps.all_updates();
        let mut status_by_step = BTreeMap::new();
        for update in mpps {
            if !actor_has_resource_access(&actor, &store.tenant.lock().tenant_mpps, &update.sop_instance_uid) {
                continue;
            }
            let status = mpps_status_label(update.status).to_string();
            match status_by_step.get_mut(&update.performed_step_id) {
                Some((current_uid, current_status)) => {
                    if update.sop_instance_uid > *current_uid {
                        *current_uid = update.sop_instance_uid.clone();
                        *current_status = status;
                    }
                }
                None => {
                    status_by_step.insert(
                        update.performed_step_id.clone(),
                        (update.sop_instance_uid.clone(), status),
                    );
                }
            }
        }
        let rows = store.worklist.query(&worklist_query)?;
        (rows, status_by_step)
    };

    let mut items: Vec<WorklistItemWithStatus> = Vec::new();
    for row in rows {
        let item = validate_worklist_item(&row, &Limits::default())?;
        {
            let store = state
                .lock()
                .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
            if !actor_has_resource_access(&actor, &store.tenant.lock().tenant_worklist, &item.scheduled_step_id) {
                continue;
            }
        }
        let status = status_by_step
            .get(&item.scheduled_step_id)
            .map(|(_, status)| status.clone())
            .unwrap_or_else(|| "SCHEDULED".to_string());
        if let Some(filter) = &status_filter {
            if status != *filter {
                continue;
            }
        }
        items.push(WorklistItemWithStatus { item, status });
    }

    apply_worklist_sort(
        &mut items,
        query_params.get("sort"),
        query_params.get("order"),
    )?;

    let (offset, limit) =
        parse_pagination(query_params.get("page"), query_params.get("page_size"))?;
    let end = items.len().min(offset.saturating_add(limit));

    if offset > items.len() {
        items.clear();
    } else {
        items = items[offset..end].to_vec();
    }

    let mut json = String::from("[");
    for (idx, item) in items.iter().enumerate() {
        if idx > 0 {
            json.push(',');
        }
        json.push('{');
        json.push_str("\"scheduled_step_id\":\"");
        json.push_str(&escape_json(&item.item.scheduled_step_id));
        json.push_str("\",\"modality\":\"");
        json.push_str(&escape_json(&item.item.modality));
        json.push_str("\",\"start_date\":\"");
        json.push_str(&escape_json(&item.item.start_date));
        json.push_str("\",\"start_time\":\"");
        json.push_str(&escape_json(&item.item.start_time));
        if let Some(value) = item.item.requested_procedure_id.as_deref() {
            json.push_str("\",\"requested_procedure_id\":\"");
            json.push_str(&escape_json(&value));
        }
        if let Some(value) = item.item.scheduled_station_ae_title.as_deref() {
            json.push_str("\",\"scheduled_station_ae_title\":\"");
            json.push_str(&escape_json(&value));
        }
        if let Some(value) = item.item.patient_id.as_deref() {
            json.push_str("\",\"patient_id\":\"");
            json.push_str(&escape_json(&value));
        }
        if let Some(value) = item.item.accession_number.as_deref() {
            json.push_str("\",\"accession_number\":\"");
            json.push_str(&escape_json(&value));
        }
        json.push_str("\",\"status\":\"");
        json.push_str(&escape_json(&item.status));
        json.push_str("\"}");
    }
    json.push(']');
    Ok(WorkflowResponse::Json(200, json))
}

pub fn apply_worklist_sort(
    items: &mut Vec<WorklistItemWithStatus>,
    sort: Option<&String>,
    order: Option<&String>,
) -> Result<(), Box<Error>> {
    let descending = matches!(order.map(|value| value.as_str()), Some("desc"));
    match sort.map(|value| value.as_str()) {
        None | Some("start_date,start_time,scheduled_step_id,modality") | Some("default") => {
            if descending {
                items.sort_by(|a, b| {
                    (
                        b.item.start_date.as_str(),
                        b.item.start_time.as_str(),
                        b.item.scheduled_step_id.as_str(),
                        b.item.modality.as_str(),
                    )
                        .cmp(&(
                            a.item.start_date.as_str(),
                            a.item.start_time.as_str(),
                            a.item.scheduled_step_id.as_str(),
                            a.item.modality.as_str(),
                        ))
                });
            } else {
                items.sort_by(|a, b| {
                    (
                        a.item.start_date.as_str(),
                        a.item.start_time.as_str(),
                        a.item.scheduled_step_id.as_str(),
                        a.item.modality.as_str(),
                    )
                        .cmp(&(
                            b.item.start_date.as_str(),
                            b.item.start_time.as_str(),
                            b.item.scheduled_step_id.as_str(),
                            b.item.modality.as_str(),
                        ))
                });
            }
        }
        Some("status") => {
            if descending {
                items.sort_by(|a, b| b.status.cmp(&a.status));
            } else {
                items.sort_by(|a, b| a.status.cmp(&b.status));
            }
        }
        Some("scheduled_step_id") => {
            if descending {
                items.sort_by(|a, b| b.item.scheduled_step_id.cmp(&a.item.scheduled_step_id));
            } else {
                items.sort_by(|a, b| a.item.scheduled_step_id.cmp(&b.item.scheduled_step_id));
            }
        }
        Some("modality") => {
            if descending {
                items.sort_by(|a, b| b.item.modality.cmp(&a.item.modality));
            } else {
                items.sort_by(|a, b| a.item.modality.cmp(&b.item.modality));
            }
        }
        Some("patient_id") => {
            if descending {
                items.sort_by(|a, b| b.item.patient_id.cmp(&a.item.patient_id));
            } else {
                items.sort_by(|a, b| a.item.patient_id.cmp(&b.item.patient_id));
            }
        }
        Some("accession_number") => {
            if descending {
                items.sort_by(|a, b| b.item.accession_number.cmp(&a.item.accession_number));
            } else {
                items.sort_by(|a, b| a.item.accession_number.cmp(&b.item.accession_number));
            }
        }
        Some(_) => Err(decode_error("unsupported worklist sort"))?,
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct WorklistItemWithStatus {
    pub item: dicom_worklist::WorklistItem,
    pub status: String,
}

pub fn parse_worklist_status_filter(status: Option<&String>) -> Result<Option<String>, Box<Error>> {
    let Some(raw) = status else {
        return Ok(None);
    };
    if raw.is_empty() {
        return Err(decode_error("invalid worklist status filter"));
    }
    let normalized = raw.trim().replace('_', " ").to_ascii_uppercase();
    let allowed = match normalized.as_str() {
        "IN PROGRESS" | "INPROGRESS" => "IN PROGRESS",
        "COMPLETED" => "COMPLETED",
        "DISCONTINUED" => "DISCONTINUED",
        "SCHEDULED" | "PENDING" | "UNCLAIMED" => "SCHEDULED",
        _ => return Err(decode_error("invalid worklist status filter")),
    };
    Ok(Some(allowed.to_string()))
}

pub fn parse_pagination(
    page: Option<&String>,
    page_size: Option<&String>,
) -> Result<(usize, usize), Box<Error>> {
    let size = match page_size {
        Some(value) => {
            parse_usize_param("page_size", value, 1, Some(MAX_WORKFLOW_PAGE_SIZE), false)?
        }
        None => DEFAULT_WORKFLOW_PAGE_SIZE,
    };
    let page = match page {
        Some(value) => parse_usize_param("page", value, 1, None, true)?,
        None => 1,
    };
    let offset = page.saturating_sub(1).saturating_mul(size);
    Ok((offset, size))
}

pub fn parse_usize_param(
    name: &'static str,
    value: &str,
    min: usize,
    max: Option<usize>,
    _allow_unbounded: bool,
) -> Result<usize, Box<Error>> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| decode_error(format!("invalid {name} parameter")))?;
    if parsed < min {
        return Err(decode_error(format!("invalid {name} parameter")));
    }
    if let Some(max) = max {
        if parsed > max {
            return Err(limit_exceeded(name, parsed as u64, max as u64));
        }
    }
    Ok(parsed)
}

pub fn mpps_status_label(status: MppsStatus) -> &'static str {
    match status {
        MppsStatus::InProgress => "IN PROGRESS",
        MppsStatus::Completed => "COMPLETED",
        MppsStatus::Discontinued => "DISCONTINUED",
    }
}

pub fn handle_worklist_upsert(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let params = parse_form_map(&request.body, limits)?;
    let scheduled_step_id = required_param(&params, "scheduled_step_id")?.to_string();
    let dataset = build_worklist_dataset(&params)?;
    let mut store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if let Some(owner_tenant) = tenant_of_id(&store.tenant.lock().tenant_worklist, &scheduled_step_id) {
        if owner_tenant != actor.tenant {
            return Err(auth_denied_error("worklist item belongs to another tenant"));
        }
    }
    let outcome = store.worklist.upsert_dataset(&dataset)?;
    route_id_to_tenant_index(
        &mut store.tenant.lock().tenant_worklist,
        &actor.tenant,
        &scheduled_step_id,
    );
    let body = format!(
        "{{\"outcome\":\"{}\"}}",
        match outcome {
            dicom_worklist::UpsertOutcome::Inserted => "inserted",
            dicom_worklist::UpsertOutcome::Updated => "updated",
            dicom_worklist::UpsertOutcome::Duplicate => "duplicate",
        }
    );
    Ok(WorkflowResponse::Json(200, body))
}

pub fn handle_mpps_get(
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    handle_mpps_list(request, actor, state, limits)
}

pub fn handle_mpps_get_single(
    sop_uid: &str,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let Some(update) = store.mpps.get(sop_uid) else {
        return Err(mpps_not_found_error());
    };
    if !actor_has_resource_access(actor, &store.tenant.lock().tenant_mpps, sop_uid) {
        return Err(auth_denied_error("mpps belongs to another tenant"));
    }
    Ok(WorkflowResponse::Json(200, render_mpps_update_json(update)))
}

pub fn handle_mpps_get_status(
    sop_uid: &str,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let Some(update) = store.mpps.get(sop_uid) else {
        return Err(mpps_not_found_error());
    };
    if !actor_has_resource_access(actor, &store.tenant.lock().tenant_mpps, sop_uid) {
        return Err(auth_denied_error("mpps belongs to another tenant"));
    }
    Ok(WorkflowResponse::Json(200, render_mpps_update_json(update)))
}

pub fn handle_mpps_update_status(
    sop_uid: &str,
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let params = parse_form_map(&request.body, &Limits::default())?;
    let requested_status = parse_mpps_status(
        required_param(&params, "status")?,
        "invalid MPPS status transition status",
    )?;
    let payload_signature = build_request_signature(&params);
    let idempotency_key = request
        .headers
        .get("x-idempotency-key")
        .cloned()
        .unwrap_or_default();
    let cache_key = if idempotency_key.is_empty() {
        None
    } else {
        Some(format!("mpps-status:{sop_uid}:{idempotency_key}"))
    };

    let mut store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let Some(current) = store.mpps.get(sop_uid).cloned() else {
        return Err(mpps_not_found_error());
    };
    if !actor_has_resource_access(actor, &store.tenant.lock().tenant_mpps, sop_uid) {
        return Err(auth_denied_error("mpps belongs to another tenant"));
    }
    if let Some(key) = &cache_key {
        if let Some(entry) = store.worker.lock().mpps_idempotency.get(key) {
            if entry.signature == payload_signature {
                return Ok(WorkflowResponse::Json(200, entry.response.clone()));
            }
            return Err(mpps_idempotency_conflict_error());
        }
    }

    let mut update_params = BTreeMap::new();
    update_params.insert("sop_instance_uid".to_string(), sop_uid.to_string());
    update_params.insert(
        "performed_step_id".to_string(),
        current.performed_step_id.clone(),
    );
    update_params.insert("start_date".to_string(), current.start_date.clone());
    update_params.insert("start_time".to_string(), current.start_time.clone());
    update_params.insert(
        "status".to_string(),
        mpps_status_label(requested_status).to_string(),
    );
    if let Some(end_date) = params.get("end_date") {
        update_params.insert("end_date".to_string(), end_date.to_string());
    } else if let Some(end_date) = current.end_date {
        update_params.insert("end_date".to_string(), end_date);
    }
    if let Some(end_time) = params.get("end_time") {
        update_params.insert("end_time".to_string(), end_time.to_string());
    } else if let Some(end_time) = current.end_time {
        update_params.insert("end_time".to_string(), end_time);
    }

    let outcome = store.mpps.ingest(&build_mpps_dataset(&update_params)?)?;
    if current.status != requested_status {
        let mut payload = BTreeMap::new();
        payload.insert("event".to_string(), "mpps.transition".to_string());
        payload.insert("sop_instance_uid".to_string(), sop_uid.to_string());
        payload.insert(
            "from_status".to_string(),
            mpps_status_label(current.status).to_string(),
        );
        payload.insert(
            "to_status".to_string(),
            mpps_status_label(requested_status).to_string(),
        );
        payload.insert(
            "performed_step_id".to_string(),
            current.performed_step_id.clone(),
        );
        payload.insert("tenant".to_string(), actor.tenant.clone());
        if let Some(actor_name) = actor.principal.clone() {
            payload.insert("actor".to_string(), actor_name);
        }
        let (event_id, sequence) = next_hl7_event_id_with_sequence(&mut store);
        let correlation_id = request_id_from_headers(&request.headers);
        publish_hl7_event(
            &mut store,
            &workflow_event_source(&actor),
            "mpps",
            &payload,
            &event_id,
            sequence,
            &correlation_id,
        );
    }
    route_id_to_tenant_index(&mut store.tenant.lock().tenant_mpps, &actor.tenant, sop_uid);
    let body = render_mpps_ingest_outcome_json(&outcome);
    if let Some(key) = cache_key {
        store.worker.lock().mpps_idempotency.insert(
            key,
            CachedMppsRequest {
                signature: payload_signature,
                response: body.clone(),
            },
        );
        enforce_mpps_idempotency_capacity(&mut store.worker.lock().mpps_idempotency);
    }
    Ok(WorkflowResponse::Json(200, body))
}

pub fn handle_mpps_list(
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let query = merge_query_parameters(&request.query, limits)?;
    let mut updates = {
        let store = state
            .lock()
            .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
        let mut updates = if let Some(sop_uid) = request.query.get("sop_instance_uid") {
            match store.mpps.get(sop_uid) {
                Some(update) => vec![update.clone()],
                None => Vec::new(),
            }
        } else {
            store.mpps.all_updates()
        };
        updates.retain(|update| {
            actor_has_resource_access(actor, &store.tenant.lock().tenant_mpps, &update.sop_instance_uid)
        });
        updates
    };

    if let Some(status) = parse_mpps_status_filter(query.get("status"))? {
        updates.retain(|update| mpps_status_label(update.status) == status);
    }
    if let Some(performed_step_id) = query.get("performed_step_id") {
        updates.retain(|update| update.performed_step_id == *performed_step_id);
    }

    apply_mpps_sort(&mut updates, query.get("sort"), query.get("order"))?;

    let (offset, limit) = parse_pagination(query.get("page"), query.get("page_size"))?;
    let end = updates.len().min(offset.saturating_add(limit));
    if offset > updates.len() {
        updates.clear();
    } else {
        updates = updates[offset..end].to_vec();
    }

    let mut json = String::from("[");
    for (idx, update) in updates.iter().enumerate() {
        if idx > 0 {
            json.push(',');
        }
        json.push('{');
        json.push_str("\"sop_instance_uid\":\"");
        json.push_str(&escape_json(&update.sop_instance_uid));
        json.push_str("\",\"status\":\"");
        json.push_str(match update.status {
            MppsStatus::InProgress => "IN PROGRESS",
            MppsStatus::Completed => "COMPLETED",
            MppsStatus::Discontinued => "DISCONTINUED",
        });
        json.push_str("\",\"performed_step_id\":\"");
        json.push_str(&escape_json(&update.performed_step_id));
        json.push_str("\",\"start_date\":\"");
        json.push_str(&escape_json(&update.start_date));
        json.push_str("\",\"start_time\":\"");
        json.push_str(&escape_json(&update.start_time));
        if let Some(end_date) = &update.end_date {
            json.push_str("\",\"end_date\":\"");
            json.push_str(&escape_json(end_date));
        }
        if let Some(end_time) = &update.end_time {
            json.push_str("\",\"end_time\":\"");
            json.push_str(&escape_json(end_time));
        }
        json.push_str("\"}");
    }
    json.push(']');
    Ok(WorkflowResponse::Json(200, json))
}

pub fn apply_mpps_sort(
    updates: &mut Vec<dicom_mpps::MppsUpdate>,
    sort: Option<&String>,
    order: Option<&String>,
) -> Result<(), Box<Error>> {
    let descending = matches!(order.map(|value| value.as_str()), Some("desc"));
    match sort.map(|value| value.as_str()) {
        None | Some("default") | Some("sop_instance_uid") => {
            updates.sort_by(|a, b| a.sop_instance_uid.cmp(&b.sop_instance_uid));
            if descending {
                updates.reverse();
            }
        }
        Some("status") => {
            updates.sort_by(|a, b| mpps_status_label(a.status).cmp(mpps_status_label(b.status)));
            if descending {
                updates.reverse();
            }
        }
        Some("performed_step_id") => {
            updates.sort_by(|a, b| a.performed_step_id.cmp(&b.performed_step_id));
            if descending {
                updates.reverse();
            }
        }
        Some("start_date") => {
            updates.sort_by(|a, b| a.start_date.cmp(&b.start_date));
            if descending {
                updates.reverse();
            }
        }
        Some("start_time") => {
            updates.sort_by(|a, b| a.start_time.cmp(&b.start_time));
            if descending {
                updates.reverse();
            }
        }
        Some("end_date") => {
            updates.sort_by(|a, b| {
                (a.end_date.as_deref(), a.end_time.as_deref())
                    .cmp(&(b.end_date.as_deref(), b.end_time.as_deref()))
            });
            if descending {
                updates.reverse();
            }
        }
        Some(_) => Err(decode_error("unsupported MPPS sort"))?,
    }
    Ok(())
}

