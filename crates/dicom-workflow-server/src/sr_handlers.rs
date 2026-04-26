//! MPPS ingest and SR workflow HTTP handler functions.

use super::*;

pub fn render_string_array(values: &[String]) -> String {
    let mut out = String::from("[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push('"');
        out.push_str(&escape_json(value));
        out.push('"');
    }
    out.push(']');
    out
}

pub fn normalize_hl7_status(raw: &str) -> Option<TaskStatus> {
    match raw.trim().replace('_', " ").to_ascii_uppercase().as_str() {
        "SCHEDULED" | "PENDING" | "ADMIT" | "REGISTER" | "CREATE" => Some(TaskStatus::Scheduled),
        "IN PROGRESS" | "INPROGRESS" | "START" | "RUN" => Some(TaskStatus::InProgress),
        "ON HOLD" | "ONHOLD" | "HOLD" => Some(TaskStatus::OnHold),
        "COMPLETED" => Some(TaskStatus::Completed),
        "REVIEWED" => Some(TaskStatus::Reviewed),
        "COMMITTED" => Some(TaskStatus::Committed),
        "DISCONTINUED" | "CANCELED" | "CANCELLED" | "DISCHARGE" => Some(TaskStatus::Discontinued),
        _ => None,
    }
}

pub fn parse_hl7_task_id(
    params: &BTreeMap<String, String>,
    source: &str,
) -> Result<String, Box<Error>> {
    if let Some(value) = params
        .get("task_id")
        .or_else(|| params.get("task"))
        .cloned()
    {
        let value = normalize_identifier(&value);
        if !value.is_empty() {
            return Ok(value);
        }
    }
    if let Some(step) = params.get("scheduled_step_id").cloned() {
        let sanitized = normalize_identifier(&step);
        if !sanitized.is_empty() {
            return Ok(format!("STEP-{sanitized}"));
        }
    }
    if let Some(patient) = params.get("patient_id") {
        let value = normalize_identifier(patient);
        if !value.is_empty() {
            return Ok(format!("PAT-{value}"));
        }
    }
    if let Some(accession) = params.get("accession_number") {
        let value = normalize_identifier(accession);
        if !value.is_empty() {
            return Ok(format!("ACC-{value}"));
        }
    }
    Ok(format!("HL7-{source}"))
}

pub fn handle_hl7_adt_event(
    source: &str,
    tenant: &str,
    event_id: &str,
    payload_signature: &str,
    params: &BTreeMap<String, String>,
    state: &mut RuntimeState,
) -> Result<String, Box<Error>> {
    let task_id = parse_hl7_task_id(params, source)?;
    let scheduled_step_id = required_identifier_from_map(
        params,
        &["scheduled_step_id", "ScheduledStepID", "step_id"],
        "scheduled_step_id",
    )?;
    let requested_procedure_id = params.get("requested_procedure_id").cloned();
    let worker = params.get("worker").cloned();
    let now = now_epoch_millis();
    let requested_status = params
        .get("status")
        .and_then(|raw| normalize_hl7_status(raw))
        .unwrap_or(TaskStatus::Scheduled);

    let created = match state.tasks.entry(task_id.clone()) {
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            let task = entry.get_mut();
            if task.tenant != tenant {
                return Err(auth_denied_error(
                    "hl7 task update denied because tenant is not owner",
                ));
            }
            if task.status != requested_status {
                if !task.status.can_transition_to(requested_status) {
                    return Err(task_status_invalid_transition_error());
                }
                task.status = requested_status;
            }
            task.scheduled_step_id = scheduled_step_id;
            task.requested_procedure_id = requested_procedure_id;
            if worker.is_some() {
                task.worker = worker;
            }
            task.updated_at_ms = now;
            false
        }
        std::collections::btree_map::Entry::Vacant(v) => {
            let _ = v.insert(ProcedureTask {
                task_id: task_id.clone(),
                scheduled_step_id,
                requested_procedure_id,
                status: requested_status,
                worker,
                tenant: tenant.to_string(),
                created_at_ms: now,
                updated_at_ms: now,
            });
            route_id_to_tenant_index(&mut state.tenant_tasks, tenant, &task_id);
            true
        }
    };
    let status = state.tasks[&task_id].status.as_label().to_string();
    Ok(format!(
        "{{\"ack\":\"AA\",\"event_id\":\"{}\",\"message_type\":\"ADT\",\"task_id\":\"{}\",\"task_status\":\"{}\",\"outcome\":\"{}\",\"signature\":\"{}\"}}",
        escape_json(event_id),
        escape_json(&task_id),
        escape_json(&status),
        if created { "inserted" } else { "updated" },
        escape_json(payload_signature)
    ))
}

pub fn handle_hl7_orm_event(
    source: &str,
    tenant: &str,
    event_id: &str,
    payload_signature: &str,
    params: &BTreeMap<String, String>,
    state: &mut RuntimeState,
) -> Result<String, Box<Error>> {
    let task_id = parse_hl7_task_id(params, source)?;
    let scheduled_step_id = required_identifier_from_map(
        params,
        &["scheduled_step_id", "ScheduledStepID", "step_id"],
        "scheduled_step_id",
    )?;
    let requested_procedure_id = params.get("requested_procedure_id").cloned();
    let worker = params.get("worker").cloned();
    let now = now_epoch_millis();
    let requested_status = params
        .get("status")
        .and_then(|raw| normalize_hl7_status(raw))
        .unwrap_or(TaskStatus::Scheduled);

    let created = match state.tasks.entry(task_id.clone()) {
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            let task = entry.get_mut();
            if task.tenant != tenant {
                return Err(auth_denied_error(
                    "hl7 task update denied because tenant is not owner",
                ));
            }
            if task.status != requested_status {
                if !task.status.can_transition_to(requested_status) {
                    return Err(task_status_invalid_transition_error());
                }
                task.status = requested_status;
            }
            task.scheduled_step_id = scheduled_step_id;
            task.requested_procedure_id = requested_procedure_id;
            if worker.is_some() {
                task.worker = worker;
            }
            task.updated_at_ms = now;
            false
        }
        std::collections::btree_map::Entry::Vacant(v) => {
            let _ = v.insert(ProcedureTask {
                task_id: task_id.clone(),
                scheduled_step_id,
                requested_procedure_id,
                status: requested_status,
                worker,
                tenant: tenant.to_string(),
                created_at_ms: now,
                updated_at_ms: now,
            });
            route_id_to_tenant_index(&mut state.tenant_tasks, tenant, &task_id);
            true
        }
    };
    let status = state.tasks[&task_id].status.as_label().to_string();

    let mut mpps_outcome = None;
    if let (Some(sop_instance_uid), Some(performed_step_id), Some(start_date), Some(start_time)) = (
        params.get("sop_instance_uid"),
        params.get("performed_step_id"),
        params.get("start_date"),
        params.get("start_time"),
    ) {
        let status = normalize_hl7_status(
            params
                .get("mpps_status")
                .unwrap_or(&"IN_PROGRESS".to_string()),
        )
        .unwrap_or(TaskStatus::InProgress);
        let mpps_status = if status == TaskStatus::Completed {
            MppsStatus::Completed
        } else if status == TaskStatus::Discontinued {
            MppsStatus::Discontinued
        } else {
            MppsStatus::InProgress
        };
        let mut update = BTreeMap::new();
        update.insert("sop_instance_uid".to_string(), sop_instance_uid.to_string());
        update.insert(
            "performed_step_id".to_string(),
            performed_step_id.to_string(),
        );
        update.insert(
            "status".to_string(),
            mpps_status_label(mpps_status).to_string(),
        );
        update.insert("start_date".to_string(), start_date.to_string());
        update.insert("start_time".to_string(), start_time.to_string());
        let dataset = build_mpps_dataset(&update)?;
        let outcome = state.mpps.ingest(&dataset)?;
        route_id_to_tenant_index(&mut state.tenant_mpps, tenant, sop_instance_uid);
        mpps_outcome = Some(render_mpps_ingest_outcome_json(&outcome));
    }

    Ok(format!(
        "{{\"ack\":\"AA\",\"event_id\":\"{}\",\"message_type\":\"ORM\",\"task_id\":\"{}\",\"task_status\":\"{}\",\"mpps_outcome\":{},\"outcome\":\"{}\",\"signature\":\"{}\"}}",
        escape_json(event_id),
        escape_json(&task_id),
        escape_json(&status),
        mpps_outcome.unwrap_or_else(|| "\"not_configured\"".to_string()),
        if created { "inserted" } else { "updated" },
        escape_json(payload_signature)
    ))
}

pub fn handle_hl7_oru_event(
    source: &str,
    tenant: &str,
    event_id: &str,
    payload_signature: &str,
    params: &BTreeMap<String, String>,
    state: &mut RuntimeState,
) -> Result<String, Box<Error>> {
    let study_instance_uid = required_identifier_from_map(
        params,
        &["study_instance_uid", "study_uid", "StudyInstanceUID"],
        "study_instance_uid",
    )?;
    let series_instance_uid = required_identifier_from_map(
        params,
        &["series_instance_uid", "series_uid", "SeriesInstanceUID"],
        "series_instance_uid",
    )?;
    let sop_instance_uid = required_identifier_from_map(
        params,
        &["sop_instance_uid", "sop_uid", "SOPInstanceUID"],
        "sop_instance_uid",
    )?;
    let report = params
        .get("report_text")
        .or_else(|| params.get("text"))
        .unwrap_or(&"Interop generated report".to_string())
        .to_string();
    let observed = parse_u64_or_default(params.get("authored_epoch_ms"));
    let auth = SrAuthContext {
        principal: Some(source.to_string()),
        can_write: true,
    };
    let item = SrAuthoringContentItem::Text {
        concept: Code {
            code_value: "121071".to_string(),
            scheme: "DCM".to_string(),
            meaning: "Finding".to_string(),
        },
        text: report.clone(),
        referenced_sop_instance_uid: None,
    };
    let outcome = if let Some(current) = state.sr.get(&sop_instance_uid) {
        state.sr.update(
            SrUpdateEnvelope {
                sop_instance_uid: sop_instance_uid.to_string(),
                expected_version: current.version,
                item,
                observer: Some(source.to_string()),
                known_referenced_sop_instance_uids: Vec::new(),
                idempotency_key: event_id.to_string(),
                request_id: Some(format!("hl7:{event_id}")),
            },
            &auth,
        )?
    } else {
        state.sr.create(
            SrCreateRequest {
                study_instance_uid: study_instance_uid.clone(),
                series_instance_uid: series_instance_uid.clone(),
                sop_instance_uid: sop_instance_uid.to_string(),
                observer: source.to_string(),
                authored_epoch_ms: observed.max(1),
                item,
                known_referenced_sop_instance_uids: parse_known_refs(params.get("known_refs")),
                idempotency_key: event_id.to_string(),
                request_id: Some(format!("hl7:{event_id}")),
            },
            &auth,
        )?
    };
    let outcome_label = match outcome.kind {
        SrWriteOutcomeKind::Created => "created",
        SrWriteOutcomeKind::Updated => "updated",
        SrWriteOutcomeKind::Duplicate => "duplicate",
    };
    route_id_to_tenant_index(&mut state.tenant_sr, tenant, &sop_instance_uid);
    Ok(format!(
        "{{\"ack\":\"AA\",\"event_id\":\"{}\",\"message_type\":\"ORU\",\"sop_instance_uid\":\"{}\",\"study_instance_uid\":\"{}\",\"series_instance_uid\":\"{}\",\"outcome\":\"{}\",\"version\":{},\"idempotency_replay\":{},\"signature\":\"{}\"}}",
        escape_json(event_id),
        escape_json(&sop_instance_uid),
        escape_json(&study_instance_uid),
        escape_json(&series_instance_uid),
        outcome_label,
        outcome.version,
        if outcome.idempotency_replay { "true" } else { "false" },
        escape_json(payload_signature),
    ))
}

pub fn handle_hl7_siu_event(
    source: &str,
    tenant: &str,
    event_id: &str,
    payload_signature: &str,
    params: &BTreeMap<String, String>,
    state: &mut RuntimeState,
) -> Result<String, Box<Error>> {
    let adt_ack = handle_hl7_adt_event(source, tenant, event_id, payload_signature, params, state)?;
    Ok(adt_ack.replace("\"message_type\":\"ADT\"", "\"message_type\":\"SIU\""))
}

pub fn parse_u64_or_default(raw: Option<&String>) -> u64 {
    raw.and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_else(now_epoch_millis)
}

pub fn handle_mpps_ingest(
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let params = parse_form_map(&request.body, limits)?;
    let sop_instance_uid = required_identifier_from_map(
        &params,
        &["sop_instance_uid", "sop_uid", "SOPInstanceUID", "SOP_UID"],
        "sop_instance_uid",
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
        Some(format!("mpps-update:{idempotency_key}"))
    };
    let mut store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if let Some(owner_tenant) = tenant_of_id(&store.tenant_mpps, &sop_instance_uid) {
        if owner_tenant != actor.tenant {
            return Err(auth_denied_error("mpps belongs to another tenant"));
        }
    }
    let dataset = build_mpps_dataset(&params)?;
    if let Some(key) = &cache_key {
        if let Some(entry) = store.mpps_idempotency.get(key) {
            if entry.signature == payload_signature {
                return Ok(WorkflowResponse::Json(200, entry.response.clone()));
            }
            return Err(mpps_idempotency_conflict_error());
        }
    }
    let outcome = store.mpps.ingest(&dataset)?;
    let body = render_mpps_ingest_outcome_json(&outcome);
    route_id_to_tenant_index(&mut store.tenant_mpps, &actor.tenant, &sop_instance_uid);
    if let Some(key) = cache_key {
        store.mpps_idempotency.insert(
            key,
            CachedMppsRequest {
                signature: payload_signature,
                response: body.clone(),
            },
        );
        enforce_mpps_idempotency_capacity(&mut store.mpps_idempotency);
    }
    Ok(WorkflowResponse::Json(200, body))
}

pub fn render_mpps_ingest_outcome_json(outcome: &dicom_mpps::IngestOutcome) -> String {
    let outcome = match outcome {
        MppsIngestOutcome::Inserted => "inserted",
        MppsIngestOutcome::Updated => "updated",
        MppsIngestOutcome::Duplicate => "duplicate",
    };
    format!("{{\"outcome\":\"{outcome}\"}}")
}

pub fn render_mpps_update_json(update: &dicom_mpps::MppsUpdate) -> String {
    let mut json = String::new();
    json.push_str("{\"sop_instance_uid\":\"");
    json.push_str(&escape_json(&update.sop_instance_uid));
    json.push_str("\",\"status\":\"");
    json.push_str(mpps_status_label(update.status));
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
    json.push('}');
    json
}

pub fn parse_mpps_status(raw: &str, message: &str) -> Result<MppsStatus, Box<Error>> {
    match raw.trim().replace('_', " ").to_ascii_uppercase().as_str() {
        "IN PROGRESS" => Ok(MppsStatus::InProgress),
        "COMPLETED" => Ok(MppsStatus::Completed),
        "DISCONTINUED" => Ok(MppsStatus::Discontinued),
        "INPROGRESS" => Ok(MppsStatus::InProgress),
        _ => Err(decode_error(message)),
    }
}

pub fn parse_mpps_status_filter(raw: Option<&String>) -> Result<Option<&'static str>, Box<Error>> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let normalized = raw.trim().replace('_', " ").to_ascii_uppercase();
    let status = match normalized.as_str() {
        "IN PROGRESS" | "INPROGRESS" => "IN PROGRESS",
        "COMPLETED" => "COMPLETED",
        "DISCONTINUED" => "DISCONTINUED",
        "SCHEDULED" | "PENDING" | "UNCLAIMED" => "SCHEDULED",
        _ => return Err(decode_error("invalid MPPS status filter")),
    };
    Ok(Some(status))
}

pub fn mpps_not_found_error() -> Box<Error> {
    Error::from_kind(
        ErrorKind::NotFound {
            detail: "MPPS instance not found".to_string(),
        },
        "MPPS instance not found",
    )
    .into()
}

pub fn mpps_idempotency_conflict_error() -> Box<Error> {
    Error::from_kind(
        ErrorKind::IntegrityError {
            detail: "idempotency key replay conflict".to_string(),
        },
        "idempotency key replay conflict",
    )
    .into()
}

pub fn build_request_signature(params: &BTreeMap<String, String>) -> String {
    let mut output = String::new();
    let mut first = true;
    for (key, value) in params {
        if !first {
            output.push('&');
        }
        first = false;
        output.push_str(key);
        output.push('=');
        output.push_str(value);
    }
    output
}

pub fn enforce_mpps_idempotency_capacity(cache: &mut BTreeMap<String, CachedMppsRequest>) {
    while cache.len() > MAX_MPPS_IDEMPOTENCY_ENTRIES {
        let Some(oldest) = cache.keys().next().cloned() else {
            break;
        };
        let _ = cache.remove(&oldest);
    }
}

pub fn handle_sr_list(
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let query = merge_query_parameters(&request.query, limits)?;
    let mut docs: Vec<_> = {
        let store = state
            .lock()
            .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
        let mut docs: Vec<_> = Vec::new();
        for doc in store.sr.documents() {
            if !actor_has_resource_access(actor, &store.tenant_sr, &doc.provenance.sop_instance_uid)
            {
                continue;
            }
            let status = store
                .sr
                .lifecycle_status(&doc.provenance.sop_instance_uid)
                .unwrap_or(SrLifecycleStatus::Draft);
            docs.push((doc.clone(), status));
        }
        docs
    };
    if let Some(study_uid) = lookup_identifier_from_map(
        &query,
        &[
            "study_uid",
            "study_instance_uid",
            "StudyInstanceUID",
            "StudyUID",
            "study_uid",
        ],
    ) {
        docs.retain(|(doc, _status)| doc.provenance.study_instance_uid == *study_uid);
    }
    if let Some(series_uid) = lookup_identifier_from_map(
        &query,
        &[
            "series_uid",
            "series_instance_uid",
            "SeriesInstanceUID",
            "SeriesUID",
        ],
    ) {
        docs.retain(|(doc, _status)| doc.provenance.series_instance_uid == *series_uid);
    }
    if let Some(observer) = query.get("observer") {
        docs.retain(|(doc, _status)| doc.provenance.observer == *observer);
    }
    apply_sr_sort(&mut docs, query.get("sort"), query.get("order"))?;

    let (offset, limit) = parse_pagination(query.get("page"), query.get("page_size"))?;
    let end = docs.len().min(offset.saturating_add(limit));
    if offset > docs.len() {
        docs.clear();
    } else {
        docs = docs[offset..end].to_vec();
    }

    let mut json = String::from("[");
    for (idx, (doc, status)) in docs.iter().enumerate() {
        if idx > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            "{{\"study_instance_uid\":\"{}\",\"series_instance_uid\":\"{}\",\"sop_instance_uid\":\"{}\",\"observer\":\"{}\",\"version\":{},\"item_count\":{},\"lifecycle_status\":\"{}\"}}",
            escape_json(&doc.provenance.study_instance_uid),
            escape_json(&doc.provenance.series_instance_uid),
            escape_json(&doc.provenance.sop_instance_uid),
            escape_json(&doc.provenance.observer),
            doc.version,
            doc.items.len(),
            status.as_label()
        ));
    }
    json.push(']');
    Ok(WorkflowResponse::Json(200, json))
}

pub fn apply_sr_sort(
    docs: &mut Vec<(pack_sr::SrAuthoredDocument, SrLifecycleStatus)>,
    sort: Option<&String>,
    order: Option<&String>,
) -> Result<(), Box<Error>> {
    let descending = matches!(order.map(|value| value.as_str()), Some("desc"));
    match sort.map(|value| value.as_str()) {
        None
        | Some("default")
        | Some("study_instance_uid,series_instance_uid,sop_instance_uid") => {
            docs.sort_by(|a, b| {
                (
                    a.0.provenance.study_instance_uid.as_str(),
                    a.0.provenance.series_instance_uid.as_str(),
                    a.0.provenance.sop_instance_uid.as_str(),
                )
                    .cmp(&(
                        b.0.provenance.study_instance_uid.as_str(),
                        b.0.provenance.series_instance_uid.as_str(),
                        b.0.provenance.sop_instance_uid.as_str(),
                    ))
            });
            if descending {
                docs.reverse();
            }
        }
        Some("study_instance_uid") => {
            docs.sort_by(|a, b| {
                a.0.provenance
                    .study_instance_uid
                    .cmp(&b.0.provenance.study_instance_uid)
            });
            if descending {
                docs.reverse();
            }
        }
        Some("series_instance_uid") => {
            docs.sort_by(|a, b| {
                a.0.provenance
                    .series_instance_uid
                    .cmp(&b.0.provenance.series_instance_uid)
            });
            if descending {
                docs.reverse();
            }
        }
        Some("sop_instance_uid") => {
            docs.sort_by(|a, b| {
                a.0.provenance
                    .sop_instance_uid
                    .cmp(&b.0.provenance.sop_instance_uid)
            });
            if descending {
                docs.reverse();
            }
        }
        Some("observer") => {
            docs.sort_by(|a, b| a.0.provenance.observer.cmp(&b.0.provenance.observer));
            if descending {
                docs.reverse();
            }
        }
        Some("version") => {
            docs.sort_by(|a, b| a.0.version.cmp(&b.0.version));
            if descending {
                docs.reverse();
            }
        }
        Some("item_count") => {
            docs.sort_by(|a, b| a.0.items.len().cmp(&b.0.items.len()));
            if descending {
                docs.reverse();
            }
        }
        Some(_) => Err(decode_error("unsupported SR sort"))?,
    }
    Ok(())
}

pub fn handle_sr_get(
    sop_instance_uid: &str,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let Some(doc) = store.sr.get(sop_instance_uid).cloned() else {
        return Err(sr_not_found_error());
    };
    if !actor_has_resource_access(actor, &store.tenant_sr, sop_instance_uid) {
        return Err(auth_denied_error("sr belongs to another tenant"));
    }
    let status = store
        .sr
        .lifecycle_status(sop_instance_uid)
        .unwrap_or(SrLifecycleStatus::Draft);
    Ok(WorkflowResponse::Json(
        200,
        render_sr_document_json(&doc, status),
    ))
}

pub fn handle_sr_create(
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let params = parse_form_map(&request.body, limits)?;
    let idempotency_key = request
        .headers
        .get("x-idempotency-key")
        .cloned()
        .unwrap_or_default();
    let item = parse_sr_item_from_params(&params)?;
    let auth = sr_auth_context(request);
    let mut store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let sop_instance_uid = required_identifier_from_map(
        &params,
        &["sop_instance_uid", "sop_uid", "SOPInstanceUID", "SOP_UID"],
        "sop_instance_uid",
    )?;
    if !actor_has_resource_access(actor, &store.tenant_sr, &sop_instance_uid)
        && tenant_of_id(&store.tenant_sr, &sop_instance_uid).is_some()
    {
        return Err(auth_denied_error("sr belongs to another tenant"));
    }
    let outcome = store.sr.create(
        SrCreateRequest {
            study_instance_uid: required_identifier_from_map(
                &params,
                &[
                    "study_instance_uid",
                    "study_uid",
                    "StudyInstanceUID",
                    "StudyUID",
                ],
                "study_instance_uid",
            )?,
            series_instance_uid: required_identifier_from_map(
                &params,
                &[
                    "series_instance_uid",
                    "series_uid",
                    "SeriesInstanceUID",
                    "SeriesUID",
                ],
                "series_instance_uid",
            )?,
            sop_instance_uid: sop_instance_uid.clone(),
            observer: required_param(&params, "observer")?.to_string(),
            authored_epoch_ms: required_param(&params, "authored_epoch_ms")?
                .parse::<u64>()
                .map_err(|_| decode_error("invalid authored_epoch_ms"))?,
            item,
            known_referenced_sop_instance_uids: parse_known_refs(params.get("known_refs")),
            idempotency_key,
            request_id: Some(request_id_from_headers(&request.headers)),
        },
        &auth,
    )?;
    route_id_to_tenant_index(&mut store.tenant_sr, &actor.tenant, &sop_instance_uid);
    Ok(WorkflowResponse::Json(
        200,
        render_sr_write_outcome_json(&outcome),
    ))
}

pub fn handle_sr_update(
    sop_instance_uid: &str,
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let params = parse_form_map(&request.body, limits)?;
    let idempotency_key = request
        .headers
        .get("x-idempotency-key")
        .cloned()
        .unwrap_or_default();
    let item = parse_sr_item_from_params(&params)?;
    let auth = sr_auth_context(request);
    let mut store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if !actor_has_resource_access(actor, &store.tenant_sr, sop_instance_uid)
        && tenant_of_id(&store.tenant_sr, sop_instance_uid).is_some()
    {
        return Err(auth_denied_error("sr belongs to another tenant"));
    }
    let outcome = store.sr.update(
        SrUpdateEnvelope {
            sop_instance_uid: sop_instance_uid.to_string(),
            expected_version: required_param(&params, "expected_version")?
                .parse::<u64>()
                .map_err(|_| decode_error("invalid expected_version"))?,
            item,
            observer: params.get("observer").cloned(),
            known_referenced_sop_instance_uids: parse_known_refs(params.get("known_refs")),
            idempotency_key,
            request_id: Some(request_id_from_headers(&request.headers)),
        },
        &auth,
    )?;
    route_id_to_tenant_index(&mut store.tenant_sr, &actor.tenant, sop_instance_uid);
    Ok(WorkflowResponse::Json(
        200,
        render_sr_write_outcome_json(&outcome),
    ))
}

pub fn handle_sr_transition(
    action: &str,
    sop_instance_uid: &str,
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let idempotency_key = request
        .headers
        .get("x-idempotency-key")
        .cloned()
        .unwrap_or_default();
    let auth = sr_auth_context(request);
    let mut store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if !actor_has_resource_access(actor, &store.tenant_sr, sop_instance_uid)
        && tenant_of_id(&store.tenant_sr, sop_instance_uid).is_some()
    {
        return Err(auth_denied_error("sr belongs to another tenant"));
    }
    let previous_status = store.sr.lifecycle_status(sop_instance_uid);
    let correlation_id = request_id_from_headers(&request.headers);

    let transition = SrLifecycleTransitionRequest {
        sop_instance_uid: sop_instance_uid.to_string(),
        idempotency_key,
        request_id: Some(correlation_id.clone()),
    };
    let outcome = match action {
        "review" => store.sr.review(transition, &auth),
        "finalize" => store.sr.finalize(transition, &auth),
        "commit" => store.sr.commit(transition, &auth),
        "cancel" => store.sr.cancel(transition, &auth),
        _ => return Err(decode_error("unsupported sr transition")),
    }?;
    if !outcome.idempotency_replay {
        let from_status = previous_status
            .unwrap_or(SrLifecycleStatus::Draft)
            .as_label();
        let mut payload = BTreeMap::new();
        payload.insert("event".to_string(), "sr.transition".to_string());
        payload.insert("sop_instance_uid".to_string(), sop_instance_uid.to_string());
        payload.insert("action".to_string(), action.to_string());
        payload.insert("from_status".to_string(), from_status.to_string());
        payload.insert(
            "to_status".to_string(),
            outcome.status.as_label().to_string(),
        );
        payload.insert("version".to_string(), outcome.version.to_string());
        payload.insert("tenant".to_string(), actor.tenant.clone());
        if let Some(actor_name) = actor.principal.clone() {
            payload.insert("actor".to_string(), actor_name);
        }
        let (event_id, sequence) = next_hl7_event_id_with_sequence(&mut store);
        publish_hl7_event(
            &mut store,
            &workflow_event_source(&actor),
            "sr",
            &payload,
            &event_id,
            sequence,
            &correlation_id,
        );
    }
    route_id_to_tenant_index(&mut store.tenant_sr, &actor.tenant, sop_instance_uid);
    Ok(WorkflowResponse::Json(
        200,
        render_sr_transition_outcome_json(&outcome),
    ))
}

pub fn handle_sr_history(
    sop_instance_uid: &str,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if store.sr.get(sop_instance_uid).is_none() {
        return Err(sr_not_found_error());
    }
    if !actor_has_resource_access(actor, &store.tenant_sr, sop_instance_uid) {
        return Err(auth_denied_error("sr belongs to another tenant"));
    }
    let history = store.sr.lifecycle_history(sop_instance_uid);
    Ok(WorkflowResponse::Json(
        200,
        render_sr_history_json(&history),
    ))
}

pub fn parse_sr_item_from_params(
    params: &BTreeMap<String, String>,
) -> Result<SrAuthoringContentItem, Box<Error>> {
    let concept = Code {
        code_value: required_param(params, "concept_code_value")?.to_string(),
        scheme: required_param(params, "concept_scheme")?.to_string(),
        meaning: required_param(params, "concept_meaning")?.to_string(),
    };
    let referenced = params
        .get("referenced_sop_instance_uid")
        .filter(|value| !value.trim().is_empty())
        .cloned();
    match required_param(params, "item_kind")? {
        "num" => Ok(SrAuthoringContentItem::Num {
            concept,
            value: required_param(params, "num_value")?
                .parse::<f64>()
                .map_err(|_| decode_error("invalid num_value"))?,
            units: Code {
                code_value: required_param(params, "units_code_value")?.to_string(),
                scheme: required_param(params, "units_scheme")?.to_string(),
                meaning: required_param(params, "units_meaning")?.to_string(),
            },
            referenced_sop_instance_uid: referenced,
        }),
        "text" => Ok(SrAuthoringContentItem::Text {
            concept,
            text: required_param(params, "text_value")?.to_string(),
            referenced_sop_instance_uid: referenced,
        }),
        "code" => Ok(SrAuthoringContentItem::Code {
            concept,
            value: Code {
                code_value: required_param(params, "value_code_value")?.to_string(),
                scheme: required_param(params, "value_scheme")?.to_string(),
                meaning: required_param(params, "value_meaning")?.to_string(),
            },
            referenced_sop_instance_uid: referenced,
        }),
        _ => Err(decode_error("unsupported item_kind")),
    }
}

pub fn parse_known_refs(value: Option<&String>) -> Vec<String> {
    let mut refs = value
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    refs.sort();
    refs.dedup();
    refs
}

pub fn sr_auth_context(request: &HttpRequest) -> SrAuthContext {
    let principal = request
        .headers
        .get("x-sr-principal")
        .cloned()
        .or_else(|| request.headers.get("x-auth-principal").cloned())
        .filter(|value| !value.trim().is_empty());
    let role = request
        .headers
        .get("x-sr-role")
        .or_else(|| request.headers.get("x-auth-role"))
        .map(|value| value.as_str())
        .unwrap_or("");
    let can_write = workflow_role_is_writer(role);
    SrAuthContext {
        principal,
        can_write,
    }
}

pub fn render_sr_write_outcome_json(outcome: &dicom_workflow_server::SrWriteOutcome) -> String {
    let kind = match outcome.kind {
        SrWriteOutcomeKind::Created => "created",
        SrWriteOutcomeKind::Updated => "updated",
        SrWriteOutcomeKind::Duplicate => "duplicate",
    };
    format!(
        "{{\"outcome\":\"{}\",\"sop_instance_uid\":\"{}\",\"version\":{},\"idempotency_replay\":{}}}",
        kind,
        escape_json(&outcome.sop_instance_uid),
        outcome.version,
        if outcome.idempotency_replay {
            "true"
        } else {
            "false"
        }
    )
}

pub fn render_sr_transition_outcome_json(outcome: &SrLifecycleTransitionOutcome) -> String {
    let status = outcome.status.as_label();
    format!(
        "{{\"operation\":\"{}\",\"sop_instance_uid\":\"{}\",\"status\":\"{}\",\"version\":{},\"idempotency_replay\":{}}}",
        escape_json(outcome.operation),
        escape_json(&outcome.sop_instance_uid),
        status,
        outcome.version,
        if outcome.idempotency_replay { "true" } else { "false" }
    )
}

pub fn render_sr_history_json(history: &[SrLifecycleHistoryRecord]) -> String {
    let mut json = String::from("[");
    for (idx, entry) in history.iter().enumerate() {
        if idx > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            "{{\"at_epoch_ms\":{},\"action\":\"{}\",\"status\":\"{}\",\"version\":{}}}",
            entry.at_epoch_ms,
            escape_json(entry.action),
            entry.status.as_label(),
            entry.version
        ));
    }
    json.push(']');
    json
}

pub fn render_sr_document_json(
    document: &pack_sr::SrAuthoredDocument,
    status: SrLifecycleStatus,
) -> String {
    let mut json = String::new();
    json.push_str("{\"provenance\":{");
    json.push_str(&format!(
        "\"study_instance_uid\":\"{}\",\"series_instance_uid\":\"{}\",\"sop_instance_uid\":\"{}\",\"observer\":\"{}\",\"authored_epoch_ms\":{},\"lifecycle_status\":\"{}\"",
        escape_json(&document.provenance.study_instance_uid),
        escape_json(&document.provenance.series_instance_uid),
        escape_json(&document.provenance.sop_instance_uid),
        escape_json(&document.provenance.observer),
        document.provenance.authored_epoch_ms,
        status.as_label()
    ));
    json.push_str("},\"version\":");
    json.push_str(&document.version.to_string());
    json.push_str(",\"items\":[");
    for (idx, item) in document.items.iter().enumerate() {
        if idx > 0 {
            json.push(',');
        }
        json.push_str(&render_sr_item_json(item));
    }
    json.push_str("]}");
    json
}

pub fn render_sr_item_json(item: &SrAuthoringContentItem) -> String {
    match item {
        SrAuthoringContentItem::Num {
            concept,
            value,
            units,
            referenced_sop_instance_uid,
        } => format!(
            "{{\"kind\":\"num\",\"concept\":{},\"value\":{},\"units\":{},\"referenced_sop_instance_uid\":{}}}",
            render_code_json(concept),
            value,
            render_code_json(units),
            render_optional_str(referenced_sop_instance_uid.as_deref())
        ),
        SrAuthoringContentItem::Text {
            concept,
            text,
            referenced_sop_instance_uid,
        } => format!(
            "{{\"kind\":\"text\",\"concept\":{},\"text\":\"{}\",\"referenced_sop_instance_uid\":{}}}",
            render_code_json(concept),
            escape_json(text),
            render_optional_str(referenced_sop_instance_uid.as_deref())
        ),
        SrAuthoringContentItem::Code {
            concept,
            value,
            referenced_sop_instance_uid,
        } => format!(
            "{{\"kind\":\"code\",\"concept\":{},\"value\":{},\"referenced_sop_instance_uid\":{}}}",
            render_code_json(concept),
            render_code_json(value),
            render_optional_str(referenced_sop_instance_uid.as_deref())
        ),
    }
}

pub fn render_code_json(code: &Code) -> String {
    format!(
        "{{\"code_value\":\"{}\",\"scheme\":\"{}\",\"meaning\":\"{}\"}}",
        escape_json(&code.code_value),
        escape_json(&code.scheme),
        escape_json(&code.meaning)
    )
}


