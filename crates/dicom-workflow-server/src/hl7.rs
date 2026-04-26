//! HL7 transport, event processing, subscriptions, and connector management.

use super::*;

pub fn handle_hl7_ingest(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let idempotency_key = request
        .headers
        .get("x-idempotency-key")
        .map(std::string::String::as_str);
    let correlation_id = request_id_from_headers(&request.headers);
    let response = run_hl7_ingest_payload(
        &request.body,
        idempotency_key,
        &correlation_id,
        state,
        limits,
    )?;
    Ok(WorkflowResponse::Json(200, response))
}

pub fn fhir_ingest_enabled() -> Result<bool, Box<Error>> {
    parse_bool(
        WORKFLOW_SERVICE_NAME,
        "DICOM_WORKFLOW_FHIR_INGEST_ENABLED",
        false,
    )
    .map_err(|err| {
        io_error(
            "invalid DICOM_WORKFLOW_FHIR_INGEST_ENABLED",
            err.to_string(),
        )
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FhirIngestRequest {
    pub tenant: String,
    pub resource_type: String,
    pub source_system: String,
    pub content_sha256: Option<String>,
    pub dry_run: bool,
    pub validated_required_fields: Vec<String>,
}

pub fn is_supported_fhir_resource_type(resource_type: &str) -> bool {
    matches!(
        resource_type,
        "Bundle" | "Patient" | "Encounter" | "Observation" | "DiagnosticReport"
    )
}

pub fn parse_fhir_ingest_request(
    params: &BTreeMap<String, String>,
) -> Result<FhirIngestRequest, Box<Error>> {
    let tenant = params
        .get("tenant")
        .map(|value| normalize_identifier(value))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| TENANT_ID_DEFAULT.to_string());
    let resource_type = params
        .get("resource_type")
        .map(|value| normalize_identifier(value))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Bundle".to_string());
    if !is_supported_fhir_resource_type(resource_type.as_str()) {
        return Err(decode_error("unsupported fhir resource_type"));
    }
    let source_system = params
        .get("source_system")
        .or_else(|| params.get("source"))
        .map(|value| normalize_identifier(value))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let content_sha256 = params
        .get("content_sha256")
        .map(|value| normalize_identifier(value))
        .filter(|value| !value.is_empty());
    if let Some(sha256) = content_sha256.as_deref() {
        let is_hex = sha256.chars().all(|ch| ch.is_ascii_hexdigit());
        if sha256.len() != 64 || !is_hex {
            return Err(decode_error(
                "content_sha256 must be 64 lowercase/uppercase hex characters",
            ));
        }
    }
    let dry_run = match params
        .get("dry_run")
        .map(|value| value.to_ascii_lowercase())
    {
        Some(value) if value == "true" || value == "1" => true,
        Some(value) if value == "false" || value == "0" => false,
        Some(_) => return Err(decode_error("invalid dry_run flag")),
        None => false,
    };
    let validated_required_fields =
        validate_fhir_resource_specific_required_fields(resource_type.as_str(), params)?;
    Ok(FhirIngestRequest {
        tenant,
        resource_type,
        source_system,
        content_sha256,
        dry_run,
        validated_required_fields,
    })
}

pub fn render_fhir_ingest_response(request: &FhirIngestRequest) -> String {
    format!(
        "{{\"status\":\"accepted\",\"endpoint\":\"/interop/fhir\",\"tenant\":\"{}\",\"resource_type\":\"{}\",\"source_system\":\"{}\",\"accepted\":true,\"dry_run\":{},\"validation\":{{\"profile\":\"fhir-minimal-v1\",\"content_sha256_present\":{},\"required_fields\":{}}},\"mode\":\"typed\"}}",
        escape_json(&request.tenant),
        escape_json(&request.resource_type),
        escape_json(&request.source_system),
        if request.dry_run { "true" } else { "false" },
        if request.content_sha256.is_some() {
            "true"
        } else {
            "false"
        },
        render_string_array(&request.validated_required_fields),
    )
}

pub fn required_fhir_field(
    params: &BTreeMap<String, String>,
    key: &str,
    label: &str,
) -> Result<String, Box<Error>> {
    params
        .get(key)
        .map(|value| normalize_identifier(value))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| decode_error(format!("missing required fhir field: {label}")))
}

pub fn required_fhir_positive_integer(
    params: &BTreeMap<String, String>,
    key: &str,
    label: &str,
) -> Result<u64, Box<Error>> {
    let value = required_fhir_field(params, key, label)?;
    let parsed = value
        .parse::<u64>()
        .map_err(|_| decode_error(format!("invalid fhir numeric field: {label}")))?;
    if parsed == 0 {
        return Err(decode_error(format!(
            "invalid fhir numeric field: {label} must be > 0"
        )));
    }
    Ok(parsed)
}

pub fn validate_fhir_resource_specific_required_fields(
    resource_type: &str,
    params: &BTreeMap<String, String>,
) -> Result<Vec<String>, Box<Error>> {
    let mut required = Vec::new();
    match resource_type {
        "Bundle" => {
            let _ = required_fhir_field(params, "bundle_type", "bundle_type")?;
            let _ = required_fhir_positive_integer(params, "entry_count", "entry_count")?;
            required.push("bundle_type".to_string());
            required.push("entry_count".to_string());
        }
        "Patient" => {
            let _ = required_fhir_field(params, "patient_id", "patient_id")?;
            let _ = required_fhir_field(params, "patient_name", "patient_name")?;
            required.push("patient_id".to_string());
            required.push("patient_name".to_string());
        }
        "Encounter" => {
            let _ = required_fhir_field(params, "encounter_id", "encounter_id")?;
            let _ = required_fhir_field(params, "subject_id", "subject_id")?;
            required.push("encounter_id".to_string());
            required.push("subject_id".to_string());
        }
        "Observation" => {
            let _ = required_fhir_field(params, "observation_code", "observation_code")?;
            let _ = required_fhir_field(params, "subject_id", "subject_id")?;
            let _ = required_fhir_field(params, "observed_at", "observed_at")?;
            required.push("observation_code".to_string());
            required.push("subject_id".to_string());
            required.push("observed_at".to_string());
        }
        "DiagnosticReport" => {
            let _ = required_fhir_field(params, "report_code", "report_code")?;
            let _ = required_fhir_field(params, "subject_id", "subject_id")?;
            let _ = required_fhir_field(params, "issued_at", "issued_at")?;
            required.push("report_code".to_string());
            required.push("subject_id".to_string());
            required.push("issued_at".to_string());
        }
        _ => {}
    }
    Ok(required)
}

pub fn handle_fhir_ingest(
    request: &HttpRequest,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    if !fhir_ingest_enabled()? {
        return Ok(WorkflowResponse::Json(
            404,
            render_interop_error_json(
                "DVF.WORKFLOW.FHIR.DISABLED",
                "FHIR ingestion is disabled",
                "set DICOM_WORKFLOW_FHIR_INGEST_ENABLED=true to enable the typed ingest endpoint",
            ),
        ));
    }
    let params = parse_form_map(&request.body, limits)?;
    let typed_request = parse_fhir_ingest_request(&params)?;
    Ok(WorkflowResponse::Json(
        202,
        render_fhir_ingest_response(&typed_request),
    ))
}

pub fn run_hl7_mllp_listener(
    bind: &str,
    state: Arc<Mutex<RuntimeState>>,
    limits: Arc<Limits>,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(bind)?;
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let state = Arc::clone(&state);
                let limits = Arc::clone(&limits);
                thread::spawn(move || {
                    if let Err(err) =
                        run_hl7_mllp_connection(stream, Arc::clone(&state), Arc::clone(&limits))
                    {
                        eprintln!("dicom-workflow-server MLLP connection failed: {err}");
                    }
                });
            }
            Err(err) => {
                eprintln!("dicom-workflow-server MLLP accept failed: {err}");
            }
        }
    }
    Ok(())
}

pub fn run_hl7_mllp_connection(
    mut stream: TcpStream,
    state: Arc<Mutex<RuntimeState>>,
    limits: Arc<Limits>,
) -> std::io::Result<()> {
    let mut read_buffer = Vec::new();
    let mut incoming = [0u8; 8192];
    loop {
        let size = stream.read(&mut incoming)?;
        if size == 0 {
            return Ok(());
        }
        read_buffer.extend_from_slice(&incoming[..size]);
        while let Some(frame) = take_next_hl7_mllp_frame(&mut read_buffer, limits.max_input_bytes())?
        {
            let correlation_id = fallback_request_id();
            let (ack, error_code): (&str, Option<String>) =
                match run_hl7_ingest_payload(&frame, None, &correlation_id, &state, &limits) {
                    Ok(_) => ("AA", None),
                    Err(err) => {
                        let (ack, code) = classify_hl7_mllp_nack(err.as_ref());
                        (ack, Some(code))
                    }
                };
            let response = build_mllp_ack(ack, error_code.as_deref());
            stream.write_all(&response)?;
        }
    }
}

pub fn take_next_hl7_mllp_frame(
    read_buffer: &mut Vec<u8>,
    max_input_bytes: u64,
) -> std::io::Result<Option<Vec<u8>>> {
    let Some(start_idx) = read_buffer
        .iter()
        .position(|value| *value == HL7_MLLP_START_BYTE)
    else {
        if read_buffer.len() as u64 > max_input_bytes {
            return Err(IoError::new(
                IoErrorKind::InvalidInput,
                "HL7 MLLP frame exceeded max_input_bytes",
            ));
        }
        return Ok(None);
    };

    if start_idx > 0 {
        read_buffer.drain(0..start_idx);
    }

    let mut end_idx = None;
    let mut cursor = 1;
    while cursor + 1 < read_buffer.len() {
        if read_buffer[cursor] == HL7_MLLP_END_BYTES[0]
            && read_buffer[cursor + 1] == HL7_MLLP_END_BYTES[1]
        {
            end_idx = Some(cursor);
            break;
        }
        cursor += 1;
    }

    match end_idx {
        Some(cursor) => {
            if cursor < 1 {
                read_buffer.drain(0..2);
                return Ok(Some(Vec::new()));
            }
            if (cursor as u64 - 1) > max_input_bytes {
                return Err(IoError::new(
                    IoErrorKind::InvalidInput,
                    "HL7 MLLP frame exceeded max_input_bytes",
                ));
            }

            let frame = read_buffer[1..cursor].to_vec();
            read_buffer.drain(0..(cursor + HL7_MLLP_END_BYTES.len()));
            Ok(Some(frame))
        }
        None => {
            if (read_buffer.len() as u64 - 1) > max_input_bytes {
                return Err(IoError::new(
                    IoErrorKind::InvalidInput,
                    "HL7 MLLP frame exceeded max_input_bytes",
                ));
            }
            Ok(None)
        }
    }
}

pub fn classify_hl7_mllp_nack(err: &Error) -> (&'static str, String) {
    if matches!(&err.kind(), ErrorKind::DecodeError { stage, .. } if stage.contains("auth")) {
        ("AR", err.code().to_string())
    } else {
        ("AE", err.code().to_string())
    }
}

pub fn build_mllp_ack(ack: &str, error_code: Option<&str>) -> Vec<u8> {
    let mut frame = Vec::new();
    frame.push(HL7_MLLP_START_BYTE);
    frame.extend_from_slice(format!("MSA|{ack}|ACK\r").as_bytes());
    if let Some(code) = error_code.map(str::trim).filter(|value| !value.is_empty()) {
        frame.extend_from_slice(format!("ERR|{code}\r").as_bytes());
    }
    frame.extend_from_slice(&HL7_MLLP_END_BYTES);
    frame
}

pub fn run_hl7_file_drop_worker(
    drop_dir: PathBuf,
    done_dir: PathBuf,
    error_dir: PathBuf,
    poll_interval_ms: u64,
    state: Arc<Mutex<RuntimeState>>,
    limits: Arc<Limits>,
) {
    loop {
        if let Err(err) = run_hl7_file_drop_once(&drop_dir, &done_dir, &error_dir, &state, &limits)
        {
            eprintln!("dicom-workflow-server HL7 file-drop worker error: {err}");
        }
        thread::sleep(Duration::from_millis(poll_interval_ms.max(1)));
    }
}

pub fn run_hl7_file_drop_once(
    drop_dir: &Path,
    done_dir: &Path,
    error_dir: &Path,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Arc<Limits>,
) -> std::io::Result<()> {
    let mut files: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(drop_dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.is_file() {
            files.push(entry.path());
        }
    }
    files.sort();

    for path in files {
        let body = fs::read(&path)?;
        let correlation_id = fallback_request_id();
        match run_hl7_ingest_payload(&body, None, &correlation_id, state, limits) {
            Ok(_) => move_hl7_drop_file(&path, done_dir)?,
            Err(_) => move_hl7_drop_file(&path, error_dir)?,
        };
    }

    Ok(())
}

pub fn move_hl7_drop_file(source_path: &Path, destination_dir: &Path) -> std::io::Result<()> {
    let source_file_name = source_path
        .file_name()
        .ok_or_else(|| IoError::new(IoErrorKind::InvalidInput, "invalid hl7 drop file name"))?;
    let file_name = source_file_name.to_string_lossy().to_string();
    let mut destination = destination_dir.join(&file_name);
    if destination.exists() {
        let mut index = 0usize;
        loop {
            index = index.saturating_add(1);
            let staged = if let Some(stem) = Path::new(&file_name).file_stem() {
                if let Some(ext) = Path::new(&file_name).extension() {
                    destination_dir.join(format!(
                        "{}.{index}.{}",
                        stem.to_string_lossy(),
                        ext.to_string_lossy()
                    ))
                } else {
                    destination_dir.join(format!("{file_name}.{index}"))
                }
            } else {
                destination_dir.join(format!("{file_name}.{index}"))
            };
            if !staged.exists() {
                destination = staged;
                break;
            }
        }
    }
    if fs::rename(source_path, &destination).is_err() {
        fs::copy(source_path, &destination)?;
        fs::remove_file(source_path)?;
    }
    Ok(())
}

pub fn hl7_replay_cache_key(
    idempotency_key: Option<&str>,
    source: &str,
    message_type: &Hl7MessageClass,
    params: &BTreeMap<String, String>,
    signature: &str,
) -> String {
    if let Some(value) = idempotency_key
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        return format!("{}|{}:idempotency:{value}", source, message_type.as_label());
    }

    if let Some(message_control_id) = params
        .get("message_control_id")
        .or_else(|| params.get("message_id"))
        .filter(|value| !value.trim().is_empty())
    {
        return format!(
            "{}|{}:control:{}",
            source,
            message_type.as_label(),
            message_control_id
        );
    }

    if let Some(task_id) = params
        .get("task_id")
        .filter(|value| !value.trim().is_empty())
    {
        return format!("{}|{}:task:{task_id}", source, message_type.as_label());
    }

    if let Some(sop_instance_uid) = params
        .get("sop_instance_uid")
        .or_else(|| params.get("sop_uid"))
        .or_else(|| params.get("SOPInstanceUID"))
        .filter(|value| !value.trim().is_empty())
    {
        return format!(
            "{}|{}:sop:{sop_instance_uid}",
            source,
            message_type.as_label()
        );
    }

    format!("{}|{}:sig:{signature}", source, message_type.as_label())
}

pub fn run_hl7_ingest_payload(
    raw_body: &[u8],
    idempotency_key: Option<&str>,
    correlation_id: &str,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<String, Box<Error>> {
    let params = parse_hl7_payload(raw_body, limits)?;
    let source = normalize_identifier(params.get("source").unwrap_or(&"interop".to_string()));
    let message_type_raw = required_param(&params, "message_type")?;
    let message_class = parse_hl7_message_class(message_type_raw)
        .ok_or_else(|| decode_error("unsupported hl7 message type"))?;
    let payload_signature = build_request_signature(&params);
    let replay_key = hl7_replay_cache_key(
        idempotency_key,
        &source,
        &message_class,
        &params,
        &payload_signature,
    );

    let correlation_id = {
        let value = correlation_id.trim();
        if value.is_empty() {
            fallback_request_id()
        } else {
            value.to_string()
        }
    };

    let mut state = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if let Some(entry) = state.hl7.lock().replay_cache.get(&replay_key) {
        if entry.signature == payload_signature.as_str() {
            return Ok(entry.response.clone());
        }
        return Err(decode_error("hl7 replay payload mismatch"));
    }

    let (event_id, sequence) = next_hl7_event_id_with_sequence(&mut state);
    let tenant = params
        .get("tenant")
        .map(|value| normalize_identifier(value))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| TENANT_ID_DEFAULT.to_string());
    let ack = match message_class {
        Hl7MessageClass::Adt => handle_hl7_adt_event(
            &source,
            &tenant,
            &event_id,
            &payload_signature,
            &params,
            &mut state,
        ),
        Hl7MessageClass::Orm => handle_hl7_orm_event(
            &source,
            &tenant,
            &event_id,
            &payload_signature,
            &params,
            &mut state,
        ),
        Hl7MessageClass::Oru => handle_hl7_oru_event(
            &source,
            &tenant,
            &event_id,
            &payload_signature,
            &params,
            &mut state,
        ),
        Hl7MessageClass::Siu => handle_hl7_siu_event(
            &source,
            &tenant,
            &event_id,
            &payload_signature,
            &params,
            &mut state,
        ),
    };
    let ack = match ack {
        Ok(response) => {
            publish_hl7_event(
                &mut state,
                &source,
                &message_class.as_label(),
                &params,
                &event_id,
                sequence,
                &correlation_id,
            );
            state.hl7.lock().replay_cache.insert(
                replay_key,
                CachedMppsRequest {
                    signature: payload_signature,
                    response: response.clone(),
                },
            );
            hl7_replay_cache_limit(&mut state.hl7.lock().replay_cache);
            response
        }
        Err(err) => {
            let failed_reason = err.to_string();
            let raw_body = String::from_utf8(raw_body.to_vec())
                .unwrap_or_else(|_| "[invalid utf8]".to_string());
            publish_hl7_failure(
                &mut state,
                &source,
                &message_class.as_label(),
                &raw_body,
                &failed_reason,
                &correlation_id,
                &event_id,
                sequence,
            );
            return Err(err);
        }
    };
    Ok(ack)
}

pub fn parse_hl7_payload(body: &[u8], limits: &Limits) -> Result<BTreeMap<String, String>, Box<Error>> {
    if body.len() as u64 > limits.max_input_bytes() {
        return Err(limit_exceeded(
            "max_input_bytes",
            body.len() as u64,
            limits.max_input_bytes(),
        ));
    }

    if let Ok(params) = parse_form_map(body, limits) {
        if let Some(raw_type) = params.get("message_type") {
            if parse_hl7_message_class(raw_type).is_some() {
                return Ok(params);
            }
        }
    }
    parse_hl7_raw_message_payload(body, limits)
}

pub fn parse_hl7_raw_message_payload(
    body: &[u8],
    limits: &Limits,
) -> Result<BTreeMap<String, String>, Box<Error>> {
    let body =
        std::str::from_utf8(body).map_err(|_| decode_error("hl7 payload is not valid UTF-8"))?;
    let mut source = String::new();
    let mut message_type = String::new();
    let mut message_control = String::new();
    let mut patient_id = String::new();
    let mut order_id = String::new();
    let mut performer = String::new();
    let mut status = String::new();
    let mut study = String::new();
    let mut series = String::new();
    let sop = String::new();
    let mut report = String::new();

    for raw_segment in body.split(|c| c == '\r' || c == '\n') {
        let segment = raw_segment.trim();
        if segment.is_empty() {
            continue;
        }
        if let Some(rest) = segment.strip_prefix("MSH|") {
            let fields: Vec<&str> = rest.split('|').collect();
            message_type = hl7_field(fields.get(8)).unwrap_or("ADT").to_string();
            source = hl7_field(fields.get(2))
                .or_else(|| hl7_field(fields.get(0)))
                .unwrap_or("interop")
                .to_string();
            message_control = hl7_field(fields.get(9)).unwrap_or("MSG1").to_string();
        } else if let Some(raw) = segment.strip_prefix("PID|") {
            let fields: Vec<&str> = raw.split('|').collect();
            patient_id = hl7_field(fields.get(2)).unwrap_or("").to_string();
        } else if let Some(raw) = segment.strip_prefix("ORC|") {
            let fields: Vec<&str> = raw.split('|').collect();
            order_id = hl7_field(fields.get(1)).unwrap_or("").to_string();
            status = hl7_field(fields.get(4)).unwrap_or("").to_string();
            performer = hl7_field(fields.get(5)).unwrap_or("").to_string();
        } else if let Some(raw) = segment.strip_prefix("OBR|") {
            let fields: Vec<&str> = raw.split('|').collect();
            if order_id.is_empty() {
                order_id = hl7_field(fields.get(2)).unwrap_or("").to_string();
            }
            if study.is_empty() {
                study = hl7_field(fields.get(0)).unwrap_or("").to_string();
            }
            if series.is_empty() {
                series = hl7_field(fields.get(1)).unwrap_or("").to_string();
            }
            if status.is_empty() {
                status = hl7_field(fields.get(4)).unwrap_or("").to_string();
            }
            if report.is_empty() {
                report = hl7_field(fields.get(7)).unwrap_or("").to_string();
            }
            if report.is_empty() {
                report = hl7_field(fields.get(5)).unwrap_or("").to_string();
            }
        } else if let Some(raw) = segment.strip_prefix("OBX|") {
            if report.is_empty() {
                let fields: Vec<&str> = raw.split('|').collect();
                report = hl7_field(fields.get(4)).unwrap_or("").to_string();
            }
        }
    }

    let message_class = parse_hl7_message_class(&message_type)
        .ok_or_else(|| decode_error("unsupported hl7 message type"))?;
    let source = normalize_identifier(&source);
    let message_type = message_class.as_label().to_string();
    let mut payload = BTreeMap::new();
    let seed = if !order_id.is_empty() {
        normalize_identifier(&order_id)
    } else if !patient_id.is_empty() {
        normalize_identifier(&patient_id)
    } else if !message_control.is_empty() {
        normalize_identifier(&message_control)
    } else {
        "MSG-0001".to_string()
    };
    if source.is_empty() {
        return Err(decode_error("unsupported hl7 payload source"));
    }
    enforce_ascii_and_limits(&source, limits)?;
    payload.insert("source".to_string(), source);
    enforce_ascii_and_limits(&message_type, limits)?;
    payload.insert("message_type".to_string(), message_type);

    let scheduled_step_id = if !seed.is_empty() {
        format!("STEP-{seed}")
    } else {
        format!("STEP-{}", normalize_identifier(&message_control))
    };
    enforce_ascii_and_limits(&scheduled_step_id, limits)?;
    payload.insert("scheduled_step_id".to_string(), scheduled_step_id);

    let status_label = normalize_hl7_status(&status)
        .unwrap_or(TaskStatus::Scheduled)
        .as_label()
        .to_string();
    if !message_control.is_empty() {
        let message_control_id = normalize_identifier(&message_control);
        enforce_ascii_and_limits(&message_control_id, limits)?;
        payload.insert("message_control_id".to_string(), message_control_id);
    }
    enforce_ascii_and_limits(&status_label, limits)?;
    payload.insert("status".to_string(), status_label);

    if !patient_id.is_empty() {
        let patient_id = normalize_identifier(&patient_id);
        enforce_ascii_and_limits(&patient_id, limits)?;
        payload.insert("patient_id".to_string(), patient_id);
    }

    if !order_id.is_empty() {
        let requested_procedure_id = normalize_identifier(&order_id);
        enforce_ascii_and_limits(&requested_procedure_id, limits)?;
        payload.insert("requested_procedure_id".to_string(), requested_procedure_id);
    }

    if !performer.is_empty() {
        let worker = normalize_identifier(&performer);
        enforce_ascii_and_limits(&worker, limits)?;
        payload.insert("worker".to_string(), worker);
    }

    if !study.is_empty() {
        let study_instance_uid = normalize_identifier(&study);
        enforce_ascii_and_limits(&study_instance_uid, limits)?;
        payload.insert("study_instance_uid".to_string(), study_instance_uid);
    }

    if !series.is_empty() {
        let series_instance_uid = normalize_identifier(&series);
        enforce_ascii_and_limits(&series_instance_uid, limits)?;
        payload.insert("series_instance_uid".to_string(), series_instance_uid);
    }

    if !sop.is_empty() {
        let sop_instance_uid = normalize_identifier(&sop);
        enforce_ascii_and_limits(&sop_instance_uid, limits)?;
        payload.insert("sop_instance_uid".to_string(), sop_instance_uid);
    }

    if !report.is_empty() {
        let report_text = normalize_identifier(&report);
        enforce_ascii_and_limits(&report_text, limits)?;
        payload.insert("report_text".to_string(), report_text);
    }

    Ok(payload)
}
pub fn handle_hl7_failures(state: &Arc<Mutex<RuntimeState>>) -> Result<WorkflowResponse, Box<Error>> {
    let state = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let records: Vec<Hl7FailureRecord> = state.hl7.lock().failures.iter().cloned().collect();
    let mut records = records;
    records.reverse();
    let mut json = String::from("[");
    for (index, record) in records.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            "{{\"id\":\"{}\",\"source\":\"{}\",\"message_type\":\"{}\",\"reason\":\"{}\",\"payload_excerpt\":\"{}\",\"created_at_ms\":{},\"scope\":\"{}\",\"subscription_id\":\"{}\",\"event_id\":\"{}\",\"correlation_id\":\"{}\",\"sequence\":{},\"attempt\":{},\"max_attempts\":{}}}",
            escape_json(&record.id),
            escape_json(&record.source),
            escape_json(&record.message_type),
            escape_json(&record.reason),
            escape_json(&record.payload_excerpt),
            record.created_at_ms,
            escape_json(&record.scope),
            escape_json(&record.subscription_id),
            escape_json(&record.event_id),
            escape_json(&record.correlation_id),
            record.sequence,
            record.attempt,
            record.max_attempts,
        ));
    }
    json.push(']');
    Ok(WorkflowResponse::Json(200, json))
}

pub fn handle_hl7_connector_status_dashboard(
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let state = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;

    let mut connector_subscriptions: BTreeMap<String, u64> = BTreeMap::new();
    let mut connector_delivered_events: BTreeMap<String, u64> = BTreeMap::new();
    let mut connector_last_event_ms: BTreeMap<String, u64> = BTreeMap::new();
    let mut connector_callback_failures: BTreeMap<String, u64> = BTreeMap::new();
    let mut custom_connector_subscriptions = 0u64;
    let mut webhook_connector_subscriptions = 0u64;
    let mut message_bus_connector_subscriptions = 0u64;
    let mut wildcard_subscriptions = 0u64;
    let mut subscription_to_connector: BTreeMap<String, String> = BTreeMap::new();

    for subscription in state.hl7.lock().subscriptions.values() {
        match &subscription.sink.kind {
            Hl7SinkKind::Custom(connector_alias) => {
                custom_connector_subscriptions = custom_connector_subscriptions.saturating_add(1);
                let subscription_count = connector_subscriptions
                    .entry(connector_alias.clone())
                    .or_insert(0);
                *subscription_count = subscription_count.saturating_add(1);

                let delivered_events = connector_delivered_events
                    .entry(connector_alias.clone())
                    .or_insert(0);
                *delivered_events = delivered_events.saturating_add(subscription.delivered_events);

                let last_event = connector_last_event_ms
                    .entry(connector_alias.clone())
                    .or_insert(0);
                *last_event = (*last_event).max(subscription.last_event_ms);
                let _ = subscription_to_connector
                    .insert(subscription.id.clone(), connector_alias.clone());
            }
            Hl7SinkKind::Webhook => {
                webhook_connector_subscriptions = webhook_connector_subscriptions.saturating_add(1);
            }
            Hl7SinkKind::MessageBus => {
                message_bus_connector_subscriptions =
                    message_bus_connector_subscriptions.saturating_add(1);
            }
        }

        if subscription.source == "*" {
            wildcard_subscriptions = wildcard_subscriptions.saturating_add(1);
        }
    }

    let mut failure_total = 0u64;
    let mut failure_ingest = 0u64;
    let mut failure_callback = 0u64;
    for failure in state.hl7.lock().failures.iter() {
        failure_total = failure_total.saturating_add(1);
        match failure.scope.as_str() {
            "ingest" => {
                failure_ingest = failure_ingest.saturating_add(1);
            }
            "callback" => {
                failure_callback = failure_callback.saturating_add(1);
                if let Some(connector_alias) =
                    subscription_to_connector.get(&failure.subscription_id)
                {
                    let failures = connector_callback_failures
                        .entry(connector_alias.clone())
                        .or_insert(0);
                    *failures = failures.saturating_add(1);
                }
            }
            _ => {}
        }
    }

    let mut connectors = Vec::new();
    for descriptor in normalized_connector_descriptors(&state) {
        let alias = &descriptor.alias;
        let target = &descriptor.target;
        let subscription_count = connector_subscriptions
            .get(alias)
            .copied()
            .unwrap_or_default();
        let delivered_events = connector_delivered_events
            .get(alias)
            .copied()
            .unwrap_or_default();
        let last_event_ms = connector_last_event_ms
            .get(alias)
            .copied()
            .unwrap_or_default();
        let callback_failures = connector_callback_failures
            .get(alias)
            .copied()
            .unwrap_or_default();
        let plugin_json = descriptor.plugin.as_ref().map_or_else(
            || "null".to_string(),
            |plugin| {
                format!(
                    "{{\"path\":\"{}\",\"version\":\"{}\",\"compatible_min\":\"{}\",\"compatible_max\":\"{}\"}}",
                    escape_json(&plugin.plugin_path),
                    escape_json(&plugin.adapter_version),
                    escape_json(&plugin.compatible_min),
                    escape_json(&plugin.compatible_max),
                )
            },
        );
        connectors.push(format!(
            "{{\"alias\":\"{}\",\"target\":\"{}\",\"adapter_kind\":\"{}\",\"subscription_count\":{},\"delivered_events\":{},\"last_event_ms\":{},\"callback_failures\":{},\"plugin\":{}}}",
            escape_json(alias),
            escape_json(target),
            descriptor.adapter_kind.as_label(),
            subscription_count,
            delivered_events,
            last_event_ms,
            callback_failures,
            plugin_json,
        ));
    }

    let mut json = String::new();
    json.push('{');
    json.push_str(&format!(
        "\"generated_at_ms\":{},\"connector_count\":{},",
        now_epoch_millis(),
        state.hl7.lock().connector_registry.len()
    ));
    json.push_str("\"connectors\":[");
    for (index, connector) in connectors.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str(connector);
    }
    json.push_str("]");
    json.push_str(&format!(
        "\"summary\":{{\"subscription_count\":{},\"custom_connector_subscriptions\":{},\"webhook_subscriptions\":{},\"message_bus_subscriptions\":{},\"wildcard_subscriptions\":{},\"failures\":{{\"total\":{},\"ingest\":{},\"callback\":{}}}}}",
        state.hl7.lock().subscriptions.len(),
        custom_connector_subscriptions,
        webhook_connector_subscriptions,
        message_bus_connector_subscriptions,
        wildcard_subscriptions,
        failure_total,
        failure_ingest,
        failure_callback
    ));
    json.push('}');
    Ok(WorkflowResponse::Json(200, json))
}

pub fn handle_hl7_connector_features(
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let state = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;

    let mut connectors = Vec::new();
    for descriptor in normalized_connector_descriptors(&state) {
        let alias = &descriptor.alias;
        let target = &descriptor.target;
        let feature_enabled =
            resolve_hl7_connector_feature_flag(&state.hl7.lock().connector_feature_flags, alias);
        let rollout_percent =
            resolve_hl7_connector_rollout_percent(&state.hl7.lock().connector_rollout_percent, alias);
        let plugin_json = descriptor.plugin.as_ref().map_or_else(
            || "null".to_string(),
            |plugin| {
                format!(
                    "{{\"path\":\"{}\",\"version\":\"{}\",\"compatible_min\":\"{}\",\"compatible_max\":\"{}\"}}",
                    escape_json(&plugin.plugin_path),
                    escape_json(&plugin.adapter_version),
                    escape_json(&plugin.compatible_min),
                    escape_json(&plugin.compatible_max),
                )
            },
        );
        connectors.push(format!(
            "{{\"alias\":\"{}\",\"target\":\"{}\",\"adapter_kind\":\"{}\",\"feature_enabled\":{},\"rollout_percent\":{},\"plugin\":{}}}",
            escape_json(alias),
            escape_json(target),
            descriptor.adapter_kind.as_label(),
            feature_enabled,
            rollout_percent,
            plugin_json,
        ));
    }

    let mut json = String::new();
    json.push('{');
    json.push_str(&format!(
        "\"generated_at_ms\":{},\"connector_count\":{},",
        now_epoch_millis(),
        state.hl7.lock().connector_registry.len()
    ));
    json.push_str("\"connectors\":[");
    for (index, connector) in connectors.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str(connector);
    }
    json.push(']');
    json.push('}');
    Ok(WorkflowResponse::Json(200, json))
}

pub fn handle_hl7_connector_rollout_list(
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let state = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let mut connectors = Vec::new();
    for descriptor in normalized_connector_descriptors(&state) {
        let rollout_percent = resolve_hl7_connector_rollout_percent(
            &state.hl7.lock().connector_rollout_percent,
            &descriptor.alias,
        );
        connectors.push(format!(
            "{{\"alias\":\"{}\",\"rollout_percent\":{},\"percent\":{}}}",
            escape_json(&descriptor.alias),
            rollout_percent,
            rollout_percent,
        ));
    }
    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"generated_at_ms\":{},\"connector_count\":{},\"connectors\":[{}]}}",
            now_epoch_millis(),
            state.hl7.lock().connector_registry.len(),
            connectors.join(","),
        ),
    ))
}

pub fn handle_hl7_connector_rollout_update(
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    enforce_admin_api_version(request)?;
    if actor.role.as_deref() != Some("admin") {
        return Err(auth_denied_error(
            "connector rollout update requires admin role",
        ));
    }
    let params = parse_form_map(&request.body, limits)?;
    let alias = params
        .get("alias")
        .map(|value| normalize_connector_alias(value))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| decode_error("missing required parameter: alias"))?;
    let rollout_percent = params
        .get("rollout_percent")
        .or_else(|| params.get("percent"))
        .ok_or_else(|| decode_error("missing required parameter: rollout_percent (or percent)"))?
        .trim()
        .parse::<u64>()
        .map_err(|_| decode_error("rollout_percent must be an integer between 0 and 100"))?;
    if rollout_percent > 100 {
        return Err(decode_error("rollout_percent must be within 0..=100"));
    }

    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if !store.hl7.lock().connector_registry.contains_key(&alias) {
        return Err(decode_error("unknown connector alias"));
    }
    store.hl7.lock().connector_rollout_percent.insert(alias.clone(), rollout_percent);
    let rollout_snapshot_path = connector_rollout_snapshot_path(&store.health.read().audit_path);
    persist_hl7_connector_rollout_state(
        &rollout_snapshot_path,
        &store.hl7.lock().connector_rollout_percent,
    );
    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"status\":\"updated\",\"alias\":\"{}\",\"rollout_percent\":{},\"percent\":{},\"updated_at_ms\":{}}}",
            escape_json(&alias),
            rollout_percent,
            rollout_percent,
            now_epoch_millis(),
        ),
    ))
}

pub fn handle_hl7_connector_capabilities(
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let state = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;

    let mut connectors = Vec::new();
    for descriptor in normalized_connector_descriptors(&state) {
        let version = descriptor
            .plugin
            .as_ref()
            .map(|plugin| plugin.adapter_version.as_str())
            .unwrap_or("1.0.0");
        let supported_message_classes = match descriptor.adapter_kind {
            NormalizedConnectorAdapterKind::HisRis => "[\"ADT\",\"ORM\",\"ORU\",\"SIU\"]",
            NormalizedConnectorAdapterKind::Dimse => "[\"C-FIND\",\"N-CREATE\",\"N-SET\"]",
            NormalizedConnectorAdapterKind::Generic => "[\"ADT\",\"ORU\"]",
        };
        connectors.push(format!(
            "{{\"alias\":\"{}\",\"adapter_kind\":\"{}\",\"version\":\"{}\",\"adapter_version\":\"{}\",\"supported_message_classes\":{}}}",
            escape_json(&descriptor.alias),
            descriptor.adapter_kind.as_label(),
            escape_json(version),
            escape_json(version),
            supported_message_classes,
        ));
    }

    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"generated_at_ms\":{},\"connector_count\":{},\"connectors\":[{}]}}",
            now_epoch_millis(),
            state.hl7.lock().connector_registry.len(),
            connectors.join(","),
        ),
    ))
}

pub fn render_interop_error_json(code: &str, message: &str, detail: &str) -> String {
    format!(
        "{{\"error\":{{\"code\":\"{}\",\"message\":\"{}\",\"detail\":\"{}\"}}}}",
        escape_json(code),
        escape_json(message),
        escape_json(detail),
    )
}

pub fn handle_hl7_connector_health(
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let state = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let descriptors = normalized_connector_descriptors(&state);
    if descriptors.is_empty() {
        return Ok(WorkflowResponse::Json(
            503,
            render_interop_error_json(
                "DVF.WORKFLOW.CONNECTOR_HEALTH.EMPTY",
                "connector health unavailable",
                "no connectors are configured",
            ),
        ));
    }
    let mut subscription_to_connector: BTreeMap<String, String> = BTreeMap::new();
    for subscription in state.hl7.lock().subscriptions.values() {
        if let Hl7SinkKind::Custom(connector_alias) = &subscription.sink.kind {
            let _ =
                subscription_to_connector.insert(subscription.id.clone(), connector_alias.clone());
        }
    }
    let mut downstream_timeout_connectors: BTreeMap<String, bool> = BTreeMap::new();
    for failure in &state.hl7.lock().failures {
        if failure.scope != "callback" {
            continue;
        }
        if !failure.reason.to_ascii_lowercase().contains("timeout") {
            continue;
        }
        if let Some(connector_alias) = subscription_to_connector.get(&failure.subscription_id) {
            let _ = downstream_timeout_connectors.insert(connector_alias.clone(), true);
        }
    }

    let mut rows = Vec::new();
    for descriptor in descriptors {
        let is_timeout = downstream_timeout_connectors
            .get(&descriptor.alias)
            .copied()
            .unwrap_or(false);
        let (status, reason) = if is_timeout {
            ("downstream_timeout", "recent callback timeout failures")
        } else if descriptor.target.trim().is_empty() {
            ("degraded", "connector target is empty")
        } else {
            ("healthy", "ok")
        };
        rows.push(format!(
            "{{\"alias\":\"{}\",\"adapter_kind\":\"{}\",\"target\":\"{}\",\"status\":\"{}\",\"reason\":\"{}\"}}",
            escape_json(&descriptor.alias),
            descriptor.adapter_kind.as_label(),
            escape_json(&descriptor.target),
            status,
            reason,
        ));
    }
    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"generated_at_ms\":{},\"connectors\":[{}]}}",
            now_epoch_millis(),
            rows.join(","),
        ),
    ))
}

pub fn handle_hl7_subscriptions_list(
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let state = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let subscriptions: Vec<Hl7Subscription> = state.hl7.lock().subscriptions.values().cloned().collect();
    let mut subscriptions = subscriptions;
    subscriptions.sort_by(|a, b| a.id.cmp(&b.id));
    let mut json = String::from("[");
    for (index, subscription) in subscriptions.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push('{');
        json.push_str(&format!(
            "\"id\":\"{}\",\"source\":\"{}\",\"event_filter\":{},\"sink_kind\":\"{}\",\"sink_target\":\"{}\",\"created_at_ms\":{},\"delivered_events\":{},\"last_event_ms\":{}",
            escape_json(&subscription.id),
            escape_json(&subscription.source),
            render_string_set(&subscription.event_filter),
            escape_json(subscription.sink.kind.as_label()),
            escape_json(&subscription.sink.target),
            subscription.created_at_ms,
            subscription.delivered_events,
            subscription.last_event_ms,
        ));
        json.push('}');
    }
    json.push(']');
    Ok(WorkflowResponse::Json(200, json))
}

pub fn handle_hl7_subscriptions_create(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let params = parse_form_map(&request.body, &Limits::default())?;
    let source = normalize_identifier(required_param(&params, "source")?);
    let raw_kind = normalize_identifier(
        params
            .get("sink_kind")
            .or_else(|| params.get("sink"))
            .ok_or_else(|| decode_error("missing required parameter: sink_kind"))?,
    );
    let sink_kind = match raw_kind.to_ascii_lowercase().as_str() {
        "webhook" | "http" => Hl7SinkKind::Webhook,
        "message_bus" | "bus" | "messagebus" => Hl7SinkKind::MessageBus,
        "custom" | "connector" => {
            let connector = normalize_identifier(
                params
                    .get("sink_connector")
                    .or_else(|| params.get("connector"))
                    .ok_or_else(|| decode_error("missing required parameter: sink_connector"))?,
            );
            if connector.is_empty() {
                return Err(decode_error("missing required parameter: sink_connector"));
            }
            let registered = {
                let state = state
                    .lock()
                    .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
                let registry = state.hl7.lock().connector_registry.clone();
                resolve_hl7_connector_alias(&registry, &connector).is_some()
            };
            if !registered {
                return Err(decode_error("unknown sink connector"));
            }
            Hl7SinkKind::Custom(connector.to_ascii_lowercase())
        }
        _ => return Err(decode_error("unsupported sink kind")),
    };
    let sink_target = normalize_identifier(
        params
            .get("sink_target")
            .or_else(|| params.get("target"))
            .ok_or_else(|| decode_error("missing required parameter: sink_target"))?,
    );
    if sink_target.is_empty() {
        return Err(decode_error("missing required parameter: sink_target"));
    }

    let event_filter = parse_hl7_event_filter(
        params
            .get("event_filter")
            .map(|value| value.as_str())
            .or_else(|| params.get("event").map(|value| value.as_str()))
            .unwrap_or("all"),
    )?;
    if event_filter.is_empty() {
        return Err(decode_error("missing required hl7 event filter"));
    }

    let state = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let policy = tenant_policy(&state, TENANT_ID_DEFAULT);
    if state.hl7.lock().subscriptions.len() >= MAX_HL7_SUBSCRIPTIONS {
        return Err(decode_error("subscription capacity exceeded"));
    }
    let source_subscriptions = state
        .hl7
        .lock()
        .subscriptions
        .values()
        .filter(|entry| entry.source == source || source == "*")
        .count();
    if source_subscriptions >= policy.subscription_quota {
        return Err(limit_exceeded(
            "workflow_subscription_quota",
            source_subscriptions as u64,
            policy.subscription_quota as u64,
        ));
    }
    let id = {
        let mut hl7 = state.hl7.lock();
        hl7.subscription_seq = hl7.subscription_seq.saturating_add(1);
        format!("sub-{0:05}", hl7.subscription_seq)
    };
    let created_at_ms = now_epoch_millis();
    let subscription = Hl7Subscription {
        id: id.clone(),
        source,
        event_filter,
        sink: Hl7Sink {
            kind: sink_kind,
            target: sink_target,
        },
        delivered_events: 0,
        created_at_ms,
        last_event_ms: 0,
    };
    state
        .hl7
        .lock()
        .subscriptions
        .insert(id.clone(), subscription.clone());

    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"id\":\"{}\",\"source\":\"{}\",\"event_filter\":{},\"sink_kind\":\"{}\",\"sink_target\":\"{}\",\"created_at_ms\":{}}}",
            escape_json(&id),
            escape_json(&subscription.source),
            render_string_set(&subscription.event_filter),
            escape_json(subscription.sink.kind.as_label()),
            escape_json(&subscription.sink.target),
            created_at_ms
        ),
    ))
}

pub fn handle_reconciliation_jobs_list(
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
            "reconciliation tenant scope outside caller tenant",
        ));
    }

    let state = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let jobs: Vec<StudyReconciliationJob> = state.hl7.lock().reconciliation_jobs.values().cloned().filter(|job| {
        if tenant_filter == "*" || tenant_filter == "all" {
            return is_admin;
        }
        job.tenant == tenant_filter
    }).collect();
    let mut jobs = jobs;
    jobs.sort_by(|a, b| a.id.cmp(&b.id));
    let mut json = String::from("[");
    for (index, job) in jobs.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            "{{\"id\":\"{}\",\"tenant\":\"{}\",\"source\":\"{}\",\"target_endpoint\":\"{}\",\"interval_seconds\":{},\"enabled\":{},\"runs_enqueued\":{},\"runs_completed\":{},\"created_at_ms\":{},\"last_run_at_ms\":{}}}",
            escape_json(&job.id),
            escape_json(&job.tenant),
            escape_json(&job.source),
            escape_json(&job.target_endpoint),
            job.interval_seconds,
            if job.enabled { "true" } else { "false" },
            job.runs_enqueued,
            job.runs_completed,
            job.created_at_ms,
            job.last_run_at_ms,
        ));
    }
    json.push(']');
    Ok(WorkflowResponse::Json(200, json))
}

pub fn parse_reconciliation_interval(raw: Option<&str>) -> Result<u64, Box<Error>> {
    let interval_seconds = raw
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(300);
    if !(RECONCILIATION_INTERVAL_FLOOR_SECONDS..=RECONCILIATION_INTERVAL_CEILING_SECONDS)
        .contains(&interval_seconds)
    {
        return Err(decode_error(format!(
            "interval_seconds must be within {}..={}",
            RECONCILIATION_INTERVAL_FLOOR_SECONDS, RECONCILIATION_INTERVAL_CEILING_SECONDS
        )));
    }
    Ok(interval_seconds)
}

pub fn handle_reconciliation_jobs_create(
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let params = parse_form_map(&request.body, limits)?;
    let source = normalize_identifier(required_param(&params, "source")?);
    let target_endpoint = normalize_identifier(
        params
            .get("target_endpoint")
            .or_else(|| params.get("target"))
            .ok_or_else(|| decode_error("missing required parameter: target_endpoint"))?,
    );
    if target_endpoint.is_empty() {
        return Err(decode_error("missing required parameter: target_endpoint"));
    }
    let interval_seconds =
        parse_reconciliation_interval(params.get("interval_seconds").map(String::as_str))?;
    let state = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if state.hl7.lock().reconciliation_jobs.len() >= MAX_RECONCILIATION_JOBS {
        return Err(limit_exceeded(
            "workflow_reconciliation_jobs",
            state.hl7.lock().reconciliation_jobs.len() as u64,
            MAX_RECONCILIATION_JOBS as u64,
        ));
    }
    let tenant_jobs = state
        .hl7
        .lock()
        .reconciliation_jobs
        .values()
        .filter(|job| job.tenant == actor.tenant)
        .count();
    if tenant_jobs >= MAX_RECONCILIATION_JOBS_PER_TENANT {
        return Err(limit_exceeded(
            "workflow_reconciliation_jobs_per_tenant",
            tenant_jobs as u64,
            MAX_RECONCILIATION_JOBS_PER_TENANT as u64,
        ));
    }
    let id = {
        let mut hl7 = state.hl7.lock();
        hl7.reconciliation_seq = hl7.reconciliation_seq.saturating_add(1);
        format!("recon-{0:05}", hl7.reconciliation_seq)
    };
    let now_ms = now_epoch_millis();
    let job = StudyReconciliationJob {
        id: id.clone(),
        tenant: actor.tenant.clone(),
        source,
        target_endpoint: target_endpoint.clone(),
        interval_seconds,
        runs_enqueued: 0,
        runs_completed: 0,
        last_run_at_ms: 0,
        created_at_ms: now_ms,
        enabled: true,
    };
    state
        .hl7
        .lock()
        .reconciliation_jobs
        .insert(id.clone(), job.clone());
    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"id\":\"{}\",\"tenant\":\"{}\",\"source\":\"{}\",\"target_endpoint\":\"{}\",\"interval_seconds\":{},\"enabled\":{}}}",
            escape_json(&job.id),
            escape_json(&job.tenant),
            escape_json(&job.source),
            escape_json(&job.target_endpoint),
            job.interval_seconds,
            if job.enabled { "true" } else { "false" },
        ),
    ))
}

pub fn handle_reconciliation_jobs_run(
    job_id: &str,
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let id = normalize_route_identifier(job_id)?;
    let idempotency_key = request
        .headers
        .get("x-idempotency-key")
        .map(|value| normalize_identifier(value))
        .unwrap_or_default();
    if idempotency_key.is_empty() {
        return Err(decode_error("missing required x-idempotency-key header"));
    }
    let state = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let idempotency_cache_key = format!(
        "{RECONCILIATION_RUN_IDEMPOTENCY_PREFIX}{}:{}:{}",
        actor.tenant, id, idempotency_key
    );
    if let Some(cached) = state.worker.lock().task_idempotency.get(&idempotency_cache_key) {
        let replay = cached.response.replacen(
            "\"idempotency_replay\":false",
            "\"idempotency_replay\":true",
            1,
        );
        return Ok(WorkflowResponse::Json(200, replay));
    }
    let mut hl7 = state.hl7.lock();
    let job = hl7
        .reconciliation_jobs
        .get_mut(&id)
        .ok_or_else(|| decode_error("reconciliation job not found"))?;
    if actor.role.as_deref() != Some("admin") && job.tenant != actor.tenant {
        return Err(auth_denied_error(
            "reconciliation job belongs to another tenant",
        ));
    }
    if !job.enabled {
        return Err(decode_error("reconciliation job is disabled"));
    }
    job.runs_enqueued = job.runs_enqueued.saturating_add(1);
    job.last_run_at_ms = now_epoch_millis();
    job.runs_completed = job.runs_completed.saturating_add(1);
    let response = format!(
        "{{\"id\":\"{}\",\"run_status\":\"ok\",\"runs_enqueued\":{},\"runs_completed\":{},\"last_run_at_ms\":{},\"idempotency_replay\":false}}",
        escape_json(&job.id),
        job.runs_enqueued,
        job.runs_completed,
        job.last_run_at_ms,
    );
    let response_signature = hash_text(&format!(
        "{}|{}|{}|{}|{}",
        job.id, job.runs_enqueued, job.runs_completed, job.last_run_at_ms, actor.tenant
    ));
    let _ = state.worker.lock().task_idempotency.insert(
        idempotency_cache_key,
        CachedMppsRequest {
            signature: response_signature,
            response: response.clone(),
        },
    );
    task_idempotency_limit(&mut state.worker.lock().task_idempotency);
    let reconciliation_idempotency_path =
        reconciliation_run_idempotency_snapshot_path(&state.health.read().audit_path);
    persist_reconciliation_run_idempotency_cache(
        &reconciliation_idempotency_path,
        &state.worker.lock().task_idempotency,
    );
    Ok(WorkflowResponse::Json(200, response))
}

pub fn publish_hl7_failure(
    state: &mut RuntimeState,
    source: &str,
    message_type: &str,
    raw_body: &str,
    reason: &str,
    correlation_id: &str,
    event_id: &str,
    sequence: u64,
) {
    publish_hl7_failure_record(
        state,
        source,
        message_type,
        raw_body,
        reason,
        "ingest",
        "",
        0,
        0,
        event_id,
        sequence,
        correlation_id,
    );
}

pub fn publish_hl7_failure_record(
    state: &mut RuntimeState,
    source: &str,
    message_type: &str,
    raw_body: &str,
    reason: &str,
    scope: &str,
    subscription_id: &str,
    attempt: u32,
    max_attempts: u32,
    event_id: &str,
    sequence: u64,
    correlation_id: &str,
) {
    let excerpt = if raw_body.len() > MAX_HL7_FAILURE_EXCERPT_BYTES {
        &raw_body[..MAX_HL7_FAILURE_EXCERPT_BYTES]
    } else {
        raw_body
    };
    {
        let mut hl7 = state.hl7.lock();
        hl7.failure_seq = hl7.failure_seq.saturating_add(1);
        let failure_seq = hl7.failure_seq;
        hl7.failures.push_front(Hl7FailureRecord {
            id: format!("hl7-fail-{0:06}", failure_seq),
            source: normalize_identifier(source),
            message_type: normalize_identifier(message_type),
            reason: normalize_identifier(reason),
            payload_excerpt: excerpt.to_string(),
            created_at_ms: now_epoch_millis(),
            scope: normalize_identifier(scope),
            subscription_id: normalize_identifier(subscription_id),
            event_id: normalize_identifier(event_id),
            correlation_id: normalize_identifier(correlation_id),
            sequence,
            attempt,
            max_attempts,
        });
        while hl7.failures.len() > MAX_HL7_FAILURES {
            let _ = hl7.failures.pop_back();
        }
        let audit_path = state.health.read().audit_path.clone();
        let failure_queue_path = hl7_failure_queue_snapshot_path(&audit_path);
        persist_hl7_failure_queue(&failure_queue_path, &hl7.failures);
    }
}

pub fn next_hl7_event_id_with_sequence(state: &mut RuntimeState) -> (String, u64) {
    let next = state.hl7.lock().event_seq.saturating_add(1);
    state.hl7.lock().event_seq = next;
    (format!("HL7-{0:06}", next), next)
}

pub fn render_callback_payload_excerpt(
    message_type: &str,
    source: &str,
    payload: &BTreeMap<String, String>,
) -> String {
    let mut excerpt = format!("event_type={message_type},source={source}");
    for (key, value) in payload.iter().take(4) {
        excerpt.push(',');
        excerpt.push_str(key);
        excerpt.push('=');
        excerpt.push_str(value);
    }
    if excerpt.len() > MAX_HL7_FAILURE_EXCERPT_BYTES {
        excerpt[..MAX_HL7_FAILURE_EXCERPT_BYTES].to_string()
    } else {
        excerpt
    }
}

pub fn publish_hl7_callback_failure(
    state: &mut RuntimeState,
    subscription: &Hl7Subscription,
    message_type: &str,
    payload: &BTreeMap<String, String>,
    event_id: &str,
    correlation_id: &str,
    sequence: u64,
    attempt: u32,
    max_attempts: u32,
) {
    let reason = format!("callback delivery failed after attempt {attempt} of {max_attempts}");
    let raw_body = render_callback_payload_excerpt(message_type, &subscription.source, payload);
    publish_hl7_failure_record(
        state,
        &subscription.source,
        message_type,
        &raw_body,
        &reason,
        "callback",
        &subscription.id,
        attempt,
        max_attempts,
        event_id,
        sequence,
        correlation_id,
    );
}

pub fn webhook_auth_strategy_from_env() -> Result<WebhookAuthStrategy, &'static str> {
    let raw = env::var(WEBHOOK_AUTH_STRATEGY_ENV).unwrap_or_else(|_| "none".to_string());
    match raw.trim().to_ascii_lowercase().as_str() {
        "" | "none" => Ok(WebhookAuthStrategy::None),
        "hmac-sha256" | "hmac_sha256" => Ok(WebhookAuthStrategy::HmacSha256),
        _ => Err("unsupported webhook auth strategy"),
    }
}

pub fn resolve_webhook_sink_signature(
    subscription: &Hl7Subscription,
    message_type: &str,
    source: &str,
    payload: &BTreeMap<String, String>,
    correlation_id: &str,
    sequence: u64,
) -> Result<Option<String>, &'static str> {
    let strategy = webhook_auth_strategy_from_env()?;
    match strategy {
        WebhookAuthStrategy::None => Ok(None),
        WebhookAuthStrategy::HmacSha256 => {
            let secret = env::var(WEBHOOK_AUTH_SECRET_ENV)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .ok_or("webhook auth strategy hmac-sha256 requires shared secret")?;
            let excerpt = render_callback_payload_excerpt(message_type, source, payload);
            Ok(Some(hash_text(&format!(
                "{}|{}|{}|{}|{}|{}|{}",
                secret,
                subscription.sink.target,
                message_type,
                source,
                correlation_id,
                sequence,
                excerpt
            ))))
        }
    }
}

pub fn publish_hl7_event_attempt(
    subscription: &Hl7Subscription,
    message_type: &str,
    source: &str,
    payload: &BTreeMap<String, String>,
    correlation_id: &str,
    sequence: u64,
) -> Result<(), &'static str> {
    if matches!(subscription.sink.kind, Hl7SinkKind::Webhook)
        && (subscription.sink.target.starts_with("http://")
            || subscription.sink.target.starts_with("https://"))
    {
        let _ = resolve_webhook_sink_signature(
            subscription,
            message_type,
            source,
            payload,
            correlation_id,
            sequence,
        )?;
    }
    if subscription
        .sink
        .target
        .to_ascii_lowercase()
        .contains("callback-fail")
    {
        return Err("simulated callback failure");
    }
    Ok(())
}

pub fn callback_circuit_breaker_backoff_ms(
    consecutive_failures: u32,
    failure_threshold: u32,
    base_backoff_ms: u64,
    max_backoff_ms: u64,
) -> u64 {
    let exponent = consecutive_failures.saturating_sub(failure_threshold);
    let shifted = base_backoff_ms
        .checked_shl(exponent.min(8))
        .unwrap_or(max_backoff_ms);
    shifted.min(max_backoff_ms)
}

pub fn publish_hl7_event(
    state: &mut RuntimeState,
    source: &str,
    message_type: &str,
    params: &BTreeMap<String, String>,
    event_id: &str,
    sequence: u64,
    correlation_id: &str,
) {
    let source = normalize_identifier(source);
    let message_type = normalize_identifier(message_type).to_ascii_lowercase();
    let now_ms = now_epoch_millis();
    hl7_callback_idempotency_prune(
        &mut state.hl7.lock().callback_delivery_idempotency,
        now_ms,
        state.hl7.lock().callback_idempotency_ttl_ms,
    );
    persist_hl7_callback_idempotency_cache(
        &state.hl7.lock().callback_idempotency_path,
        &state.hl7.lock().callback_delivery_idempotency,
    );
    let subscriptions: Vec<Hl7Subscription> = state.hl7.lock().subscriptions.values().cloned().collect();
    for subscription in subscriptions {
        if !subscription.event_filter.iter().any(|candidate| {
            candidate == "all"
                || candidate == "*"
                || candidate == message_type.as_str()
                || (candidate == "workflow"
                    && matches!(message_type.as_str(), "task" | "mpps" | "sr"))
        }) {
            continue;
        }
        if subscription.source != source && subscription.source != "*" {
            continue;
        }
        let callback_idempotency_key = format!("{event_id}|{}", subscription.id);
        if state
            .hl7
            .lock()
            .callback_delivery_idempotency
            .contains_key(&callback_idempotency_key)
        {
            continue;
        }
        let connector_alias = match &subscription.sink.kind {
            Hl7SinkKind::Custom(alias) => Some(normalize_connector_alias(alias)),
            _ => None,
        };
        if let Some(alias) = connector_alias.as_deref() {
            if let Some(open_until_ms) = state
                .hl7
                .lock()
                .connector_circuit_open_until_ms
                .get(alias)
                .copied()
            {
                if open_until_ms > now_ms {
                    continue;
                }
                state.hl7.lock().connector_circuit_open_until_ms.remove(alias);
            }
            let feature_enabled =
                resolve_hl7_connector_feature_flag(&state.hl7.lock().connector_feature_flags, alias);
            if !feature_enabled {
                continue;
            }
            let rollout_percent =
                resolve_hl7_connector_rollout_percent(&state.hl7.lock().connector_rollout_percent, alias);
            if !hl7_connector_rollout_allows(event_id, alias, rollout_percent) {
                continue;
            }
        }
        let callback_max_attempts = state.hl7.lock().callback_max_attempts;
        let mut delivered = false;
        for attempt in 1..=callback_max_attempts {
            if publish_hl7_event_attempt(
                &subscription,
                message_type.as_str(),
                source.as_str(),
                params,
                correlation_id,
                sequence,
            )
            .is_ok()
            {
                delivered = true;
                break;
            }
            if attempt == callback_max_attempts {
                publish_hl7_callback_failure(
                    state,
                    &subscription,
                    message_type.as_str(),
                    params,
                    event_id,
                    correlation_id,
                    sequence,
                    attempt,
                    callback_max_attempts,
                );
                if let Some(alias) = connector_alias.as_deref() {
                    let mut hl7 = state.hl7.lock();
                    let threshold = hl7.callback_circuit_breaker_failure_threshold;
                    let base_backoff = hl7.callback_circuit_breaker_base_backoff_ms;
                    let max_backoff = hl7.callback_circuit_breaker_max_backoff_ms;
                    let current_streak = hl7.connector_callback_failure_streak.get(alias).copied().unwrap_or(0);
                    let new_streak = current_streak.saturating_add(1);
                    hl7.connector_callback_failure_streak.insert(alias.to_string(), new_streak);
                    if new_streak >= threshold {
                        let backoff_ms = callback_circuit_breaker_backoff_ms(
                            new_streak,
                            threshold,
                            base_backoff,
                            max_backoff,
                        );
                        let _ = hl7
                            .connector_circuit_open_until_ms
                            .insert(alias.to_string(), now_ms.saturating_add(backoff_ms));
                    }
                }
            }
        }
        if delivered {
            if let Some(alias) = connector_alias.as_deref() {
                state.hl7.lock().connector_callback_failure_streak.remove(alias);
                state.hl7.lock().connector_circuit_open_until_ms.remove(alias);
            }
            if let Some(subscription_state) = state.hl7.lock().subscriptions.get_mut(&subscription.id) {
                subscription_state.delivered_events =
                    subscription_state.delivered_events.saturating_add(1);
                subscription_state.last_event_ms = now_ms;
            }
            state
                .hl7
                .lock()
                .callback_delivery_idempotency
                .insert(callback_idempotency_key, now_ms);
            hl7_callback_idempotency_prune(
                &mut state.hl7.lock().callback_delivery_idempotency,
                now_ms,
                state.hl7.lock().callback_idempotency_ttl_ms,
            );
            persist_hl7_callback_idempotency_cache(
                &state.hl7.lock().callback_idempotency_path,
                &state.hl7.lock().callback_delivery_idempotency,
            );
        }
    }
}

/// Parse HL7 event filter string into a `BTreeSet<String>` (S13-T6).
pub fn parse_hl7_event_filter(raw: &str) -> Result<BTreeSet<String>, Box<Error>> {
    let mut out = BTreeSet::new();
    for candidate in raw.split(',') {
        let token = normalize_identifier(candidate);
        if token.is_empty() {
            continue;
        }
        let token = token.to_ascii_lowercase();
        match token.as_str() {
            "adt" | "orm" | "oru" | "siu" | "task" | "mpps" | "sr" | "workflow" | "all" | "*" => {
                let _ = out.insert(token);
            }
            _ => return Err(decode_error("unsupported hl7 event filter")),
        }
    }
    if out.is_empty() {
        let _ = out.insert("all".to_string());
    }
    Ok(out)
}

