use std::fs;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn temp_dir(prefix: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("rdvf_{prefix}_{nonce}"))
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind free port");
    listener.local_addr().expect("local addr").port()
}

fn send_http(port: u16, method: &str, path: &str, body: &str) -> std::io::Result<(u16, String)> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nx-sr-principal: integration-test\r\nx-sr-role: admin\r\nx-workflow-tenant: tenant-default\r\nx-idempotency-key: it-fixed-key\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes())?;
    stream.shutdown(Shutdown::Write)?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;

    let mut lines = response.lines();
    let status_line = lines.next().expect("status line");
    let code = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|raw| raw.parse::<u16>().ok())
        .expect("status code");
    let body = response
        .split("\r\n\r\n")
        .nth(1)
        .or_else(|| response.split("\n\n").nth(1))
        .unwrap_or_default()
        .to_string();
    Ok((code, body))
}

fn wait_for_health(child: &mut Child, port: u16) {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().expect("poll child status") {
            let mut stdout = String::new();
            let mut stderr = String::new();
            if let Some(out) = child.stdout.as_mut() {
                let _ = out.read_to_string(&mut stdout);
            }
            if let Some(err) = child.stderr.as_mut() {
                let _ = err.read_to_string(&mut stderr);
            }
            panic!(
                "workflow server exited before health check: status={status}, stdout={stdout:?}, stderr={stderr:?}"
            );
        }
        if std::time::Instant::now() > deadline {
            panic!("workflow server did not become healthy in time");
        }
        if send_http(port, "GET", "/healthz", "").is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn spawn_server(work_dir: &PathBuf, port: u16) -> Child {
    let worklist = work_dir.join("worklist.snapshot");
    let mpps = work_dir.join("mpps.snapshot");
    let sr = work_dir.join("sr.snapshot");
    let sr_audit = work_dir.join("sr.audit.log");
    let workflow_audit = work_dir.join("workflow.audit.log");
    let tls_cert = work_dir.join("tls-cert.pem");
    let tls_key = work_dir.join("tls-key.pem");
    fs::write(&tls_cert, "test-cert").expect("write tls cert");
    fs::write(&tls_key, "test-key").expect("write tls key");

    Command::new(env!("CARGO_BIN_EXE_dicom-workflow-server"))
        .env("DICOM_WORKFLOW_BIND", format!("127.0.0.1:{port}"))
        .env("DICOM_WORKFLOW_AUTH_MODE", "allow_all")
        .env("DICOM_WORKFLOW_TRANSPORT_SECURITY", "tls")
        .env("DICOM_WORKFLOW_TLS_CERT_PATH", tls_cert)
        .env("DICOM_WORKFLOW_TLS_KEY_PATH", tls_key)
        .env("DICOM_WORKFLOW_WORKLIST_STATE_PATH", worklist)
        .env("DICOM_WORKFLOW_MPPS_STATE_PATH", mpps)
        .env("DICOM_WORKFLOW_SR_STATE_PATH", sr)
        .env("DICOM_WORKFLOW_SR_AUDIT_PATH", sr_audit)
        .env("DICOM_WORKFLOW_AUDIT_PATH", workflow_audit)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn workflow server")
}

fn run_failed_workflow_startup(envs: &[(&str, String)]) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_dicom-workflow-server"));
    for (key, value) in envs {
        command.env(key, value);
    }
    let output = command.output().expect("run workflow server");
    assert!(
        !output.status.success(),
        "workflow server unexpectedly started: stdout={:?}, stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn interop_reconciliation_jobs_create_list_run() {
    let dir = temp_dir("interop_recon_jobs_it");
    fs::create_dir_all(&dir).expect("create temp dir");
    let port = free_port();
    let mut child = spawn_server(&dir, port);
    wait_for_health(&mut child, port);

    let create_body =
        "source=his&target_endpoint=https%3A%2F%2Frecon.example%2Fsync&interval_seconds=120";
    let (create_status, create_response) =
        send_http(port, "POST", "/interop/reconciliation/jobs", create_body)
            .expect("create reconciliation job");
    assert_eq!(
        create_status, 200,
        "unexpected create response: {create_response}"
    );
    assert!(create_response.contains("\"interval_seconds\":120"));

    let marker = "\"id\":\"";
    let start = create_response
        .find(marker)
        .map(|index| index + marker.len())
        .expect("job id start");
    let end = create_response[start..]
        .find('"')
        .map(|offset| start + offset)
        .expect("job id end");
    let job_id = create_response[start..end].to_string();

    let (list_status, list_response) = send_http(port, "GET", "/interop/reconciliation/jobs", "")
        .expect("list reconciliation jobs");
    assert_eq!(list_status, 200);
    assert!(list_response.contains(&job_id));
    assert!(list_response.contains("runs_enqueued"));

    let run_path = format!("/interop/reconciliation/jobs/{job_id}/run");
    let (run_status, run_response) =
        send_http(port, "POST", &run_path, "").expect("run reconciliation job");
    assert_eq!(run_status, 200);
    assert!(run_response.contains("\"run_status\":\"ok\""));

    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn workflow_startup_preflight_rejects_invalid_bind_address() {
    let output = run_failed_workflow_startup(&[("DICOM_WORKFLOW_BIND", "bad:bind".to_string())]);
    assert!(
        output.contains("invalid bind address")
            || output.contains("invalid port value")
            || output.contains("bad:bind"),
        "expected invalid bind diagnostics, got: {output}"
    );
}

#[test]
fn workflow_startup_preflight_rejects_invalid_tls_cert_paths() {
    let dir = temp_dir("workflow_preflight_invalid_cert");
    fs::create_dir_all(&dir).expect("create temp dir");
    let worklist = dir.join("worklist.snapshot");
    let mpps = dir.join("mpps.snapshot");
    let sr = dir.join("sr.snapshot");
    let sr_audit = dir.join("sr.audit.log");
    let workflow_audit = dir.join("workflow.audit.log");

    let output = run_failed_workflow_startup(&[
        ("DICOM_WORKFLOW_BIND", "127.0.0.1:0".to_string()),
        ("DICOM_WORKFLOW_AUTH_MODE", "allow_all".to_string()),
        ("DICOM_WORKFLOW_TRANSPORT_SECURITY", "tls".to_string()),
        (
            "DICOM_WORKFLOW_TLS_CERT_PATH",
            "/dev/null/missing-workflow-cert.pem".to_string(),
        ),
        (
            "DICOM_WORKFLOW_TLS_KEY_PATH",
            "/dev/null/missing-workflow-key.pem".to_string(),
        ),
        (
            "DICOM_WORKFLOW_WORKLIST_STATE_PATH",
            worklist.to_string_lossy().into_owned(),
        ),
        (
            "DICOM_WORKFLOW_MPPS_STATE_PATH",
            mpps.to_string_lossy().into_owned(),
        ),
        (
            "DICOM_WORKFLOW_SR_STATE_PATH",
            sr.to_string_lossy().into_owned(),
        ),
        (
            "DICOM_WORKFLOW_SR_AUDIT_PATH",
            sr_audit.to_string_lossy().into_owned(),
        ),
        (
            "DICOM_WORKFLOW_AUDIT_PATH",
            workflow_audit.to_string_lossy().into_owned(),
        ),
    ]);

    assert!(
        output.contains("TLS cert path is not a file")
            || output.contains("transport requires TLS")
            || output.contains("TLS key path is not a file")
            || output.contains("Not a directory"),
        "expected invalid TLS material diagnostics, got: {output}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn workflow_startup_preflight_rejects_unwritable_state_paths() {
    let output = run_failed_workflow_startup(&[
        ("DICOM_WORKFLOW_BIND", "127.0.0.1:0".to_string()),
        ("DICOM_WORKFLOW_AUTH_MODE", "allow_all".to_string()),
        (
            "DICOM_WORKFLOW_WORKLIST_STATE_PATH",
            "/dev/null/workflow-worklist.snapshot".to_string(),
        ),
        (
            "DICOM_WORKFLOW_MPPS_STATE_PATH",
            "/dev/null/workflow-mpps.snapshot".to_string(),
        ),
        (
            "DICOM_WORKFLOW_SR_STATE_PATH",
            "/dev/null/workflow-sr.snapshot".to_string(),
        ),
        (
            "DICOM_WORKFLOW_SR_AUDIT_PATH",
            "/dev/null/workflow-sr.audit.log".to_string(),
        ),
        (
            "DICOM_WORKFLOW_AUDIT_PATH",
            "/dev/null/workflow.audit.log".to_string(),
        ),
    ]);
    assert!(
        output.contains("failed to prepare")
            || output.contains("Not a directory")
            || output.contains("state")
            || output.contains("File exists"),
        "expected unwritable path diagnostics, got: {output}"
    );
}

#[test]
fn workflow_health_and_readiness_contracts_are_deterministic() {
    let dir = temp_dir("workflow_readyz_contract_it");
    fs::create_dir_all(&dir).expect("create temp dir");
    let port = free_port();
    let mut child = spawn_server(&dir, port);
    wait_for_health(&mut child, port);

    let (health_status_1, health_body_1) =
        send_http(port, "GET", "/healthz", "").expect("healthz first");
    let (health_status_2, health_body_2) =
        send_http(port, "GET", "/healthz", "").expect("healthz second");
    assert_eq!(health_status_1, 200);
    assert_eq!(health_status_2, 200);
    assert_eq!(health_body_1, health_body_2);
    assert!(health_body_1.contains("\"status\":\"ok\""));

    let (ready_status_1, ready_body_1) =
        send_http(port, "GET", "/readyz", "").expect("readyz first");
    let (ready_status_2, ready_body_2) =
        send_http(port, "GET", "/readyz", "").expect("readyz second");
    assert_eq!(ready_status_1, 200);
    assert_eq!(ready_status_2, 200);
    assert_eq!(ready_body_1, ready_body_2);
    assert!(ready_body_1.contains("\"status\":\"ready\""));

    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_dir_all(&dir);
}
