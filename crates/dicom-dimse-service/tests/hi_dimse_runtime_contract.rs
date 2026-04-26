use dicom_auth::{AuthAction, AuthDecision, AuthDenyReason, AuthRequest, Authorizer};
use dicom_core::Result;
use dicom_core::{ErrorKind, Limits, Tag};
use dicom_dimse_service::{
    CEchoRequest, CStoreRequest, DimseAuthConfig, DimseClient, DimseClientConfig, DimseRoleConfig,
    DimseServer, DimseServerConfig, DimseService, DimseStatus, StorageBackedDimseService,
    TlsPolicy, TransportSecurity,
};
use dicom_net::AssociationPolicy;
use dicom_storage::Storage;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
struct TimeoutService;

impl DimseService for TimeoutService {
    fn on_c_echo(&mut self, _: CEchoRequest) -> dicom_core::Result<DimseStatus> {
        std::thread::sleep(Duration::from_millis(250));
        Ok(DimseStatus::success())
    }

    fn on_c_store(&mut self, _: CStoreRequest) -> dicom_core::Result<DimseStatus> {
        Ok(DimseStatus::processing_failure())
    }
}

#[derive(Debug, Clone)]
struct NoopService;

impl DimseService for NoopService {
    fn on_c_echo(&mut self, _request: CEchoRequest) -> dicom_core::Result<DimseStatus> {
        Ok(DimseStatus::success())
    }

    fn on_c_store(&mut self, _request: CStoreRequest) -> dicom_core::Result<DimseStatus> {
        Ok(DimseStatus::processing_failure())
    }
}

#[derive(Debug, Clone)]
struct SuccessService;

impl DimseService for SuccessService {
    fn on_c_echo(&mut self, _request: CEchoRequest) -> dicom_core::Result<DimseStatus> {
        Ok(DimseStatus::success())
    }

    fn on_c_store(&mut self, _request: CStoreRequest) -> dicom_core::Result<DimseStatus> {
        Ok(DimseStatus::success())
    }
}

#[derive(Debug, Clone)]
struct AssociateButDenyDimseRequest;

impl Authorizer for AssociateButDenyDimseRequest {
    fn authorize(&self, request: &AuthRequest<'_>) -> Result<AuthDecision> {
        match request.action {
            AuthAction::Associate => Ok(AuthDecision::Allow),
            _ => Ok(AuthDecision::Deny(AuthDenyReason::Unauthorized)),
        }
    }
}

#[test]
fn dimse_server_starts_and_accepts_insecure_transport_when_configured() {
    let config = DimseServerConfig {
        auth: DimseAuthConfig::allow_all(),
        tls_policy: TlsPolicy::AllowInsecure,
        transport_security: TransportSecurity::Insecure,
        ..DimseServerConfig::default()
    };
    let server = DimseServer::bind("127.0.0.1:0".parse().unwrap(), config, NoopService)
        .expect("server bind");
    let addr = server.local_addr().expect("server addr");
    let handle = thread::spawn(move || {
        let _ = server.run_once();
    });

    let mut client_config = DimseClientConfig::default();
    client_config.tls_policy = TlsPolicy::AllowInsecure;
    client_config.transport_security = TransportSecurity::Insecure;
    let mut client = DimseClient::connect(
        addr,
        "CALLED_AE",
        "CALLING_AE",
        &AssociationPolicy::verification_default(),
        client_config,
    )
    .expect("client connect");
    let status = client.c_echo(1).expect("c-echo");
    assert_eq!(status.code, DimseStatus::success().code);
    let _ = client.release();
    handle.join().expect("run_once thread");
}

#[test]
fn dimse_server_rejects_double_bind_on_same_address() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind probe listener");
    let addr = listener.local_addr().expect("listener addr");

    let err = match DimseServer::bind(addr, DimseServerConfig::default(), NoopService) {
        Ok(_server) => panic!("expected double-bind failure"),
        Err(err) => err,
    };
    assert!(matches!(err.kind(), ErrorKind::IoError { .. }));

    drop(listener);
}

#[test]
fn dimse_server_auth_failure_rejects_association() {
    let config = DimseServerConfig {
        tls_policy: TlsPolicy::AllowInsecure,
        transport_security: TransportSecurity::Insecure,
        ..DimseServerConfig::default()
    };
    let server = DimseServer::bind("127.0.0.1:0".parse().unwrap(), config, NoopService)
        .expect("server bind");
    let addr = server.local_addr().expect("server addr");
    let handle = thread::spawn(move || {
        let _ = server.run_once();
    });

    let mut client_config = DimseClientConfig::default();
    client_config.tls_policy = TlsPolicy::AllowInsecure;
    client_config.transport_security = TransportSecurity::Insecure;
    let err = match DimseClient::connect(
        addr,
        "CALLED_AE",
        "CALLING_AE",
        &AssociationPolicy::verification_default(),
        client_config,
    ) {
        Ok(_client) => panic!("expected auth failure"),
        Err(err) => err,
    };
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));

    handle.join().expect("run_once thread");
}

#[test]
fn dimse_server_auth_failure_rejects_request_after_association() {
    let config = DimseServerConfig {
        auth: DimseAuthConfig {
            authorizer: Arc::new(AssociateButDenyDimseRequest),
            audit: None,
        },
        tls_policy: TlsPolicy::AllowInsecure,
        transport_security: TransportSecurity::Insecure,
        ..DimseServerConfig::default()
    };
    let server = DimseServer::bind("127.0.0.1:0".parse().unwrap(), config, NoopService)
        .expect("server bind");
    let addr = server.local_addr().expect("server addr");
    let handle = thread::spawn(move || {
        let _ = server.run_once();
    });

    let mut client_config = DimseClientConfig::default();
    client_config.tls_policy = TlsPolicy::AllowInsecure;
    client_config.transport_security = TransportSecurity::Insecure;
    let mut client = DimseClient::connect(
        addr,
        "CALLED_AE",
        "CALLING_AE",
        &AssociationPolicy::verification_default(),
        client_config,
    )
    .expect("association");
    let status = client.c_echo(1).expect("c-echo status response");
    assert_eq!(status.code, DimseStatus::processing_failure().code);

    let _ = client.release();
    handle.join().expect("run_once thread");
}

#[test]
fn dimse_server_read_timeout_path_is_exercised_for_slow_handlers() {
    let config = DimseServerConfig {
        auth: DimseAuthConfig::allow_all(),
        tls_policy: TlsPolicy::AllowInsecure,
        transport_security: TransportSecurity::Insecure,
        read_timeout: Duration::from_secs(5),
        write_timeout: Duration::from_secs(5),
        ..DimseServerConfig::default()
    };
    let server = DimseServer::bind("127.0.0.1:0".parse().unwrap(), config, TimeoutService)
        .expect("server bind");
    let addr = server.local_addr().expect("server addr");
    let handle = thread::spawn(move || {
        let _ = server.run_once();
    });

    let mut client_config = DimseClientConfig::default();
    client_config.tls_policy = TlsPolicy::AllowInsecure;
    client_config.transport_security = TransportSecurity::Insecure;
    client_config.read_timeout = Duration::from_millis(25);

    let mut client = DimseClient::connect(
        addr,
        "CALLED_AE",
        "CALLING_AE",
        &AssociationPolicy::verification_default(),
        client_config,
    )
    .expect("association");
    let err = client
        .c_echo(1)
        .expect_err("expected read timeout path behavior");
    assert!(matches!(err.kind(), ErrorKind::IoError { .. }));
    handle.join().expect("run_once thread");
}

#[cfg(feature = "dimse-c-get")]
#[test]
fn c_get_disabled_in_roles_fails_closed_at_association_time() {
    let config = DimseServerConfig {
        auth: DimseAuthConfig::allow_all(),
        tls_policy: TlsPolicy::AllowInsecure,
        transport_security: TransportSecurity::Insecure,
        roles: DimseRoleConfig {
            c_get_enabled: false,
            ..DimseRoleConfig::default()
        },
        ..DimseServerConfig::default()
    };
    let server = DimseServer::bind("127.0.0.1:0".parse().unwrap(), config, NoopService)
        .expect("server bind");
    let addr = server.local_addr().expect("server addr");
    let handle = thread::spawn(move || {
        let _ = server.run_once();
    });

    let mut client_config = DimseClientConfig::default();
    client_config.tls_policy = TlsPolicy::AllowInsecure;
    client_config.transport_security = TransportSecurity::Insecure;

    let c_get_only_policy = AssociationPolicy {
        called_ae: None,
        supported_abstract_syntaxes: vec!["1.2.840.10008.5.1.4.1.2.2.3".to_string()],
        supported_transfer_syntaxes: vec!["1.2.840.10008.1.2".to_string()],
        max_pdu_length: 16_384,
    };

    let result = DimseClient::connect(
        addr,
        "CALLED_AE",
        "CALLING_AE",
        &c_get_only_policy,
        client_config,
    );
    match result {
        Err(err) => {
            assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
        }
        Ok(mut client) => {
            let err = client
                .c_get(11, "1.2.840.10008.5.1.4.1.2.2.3", 0, &[])
                .expect_err("C-GET must fail closed when no C-GET context is negotiated");
            assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
            let _ = client.release();
        }
    }
    handle.join().expect("run_once thread");
}

#[cfg(feature = "dimse-c-get")]
#[test]
fn c_get_disabled_in_roles_rejects_operation_after_verification_association() {
    let config = DimseServerConfig {
        auth: DimseAuthConfig::allow_all(),
        tls_policy: TlsPolicy::AllowInsecure,
        transport_security: TransportSecurity::Insecure,
        roles: DimseRoleConfig {
            c_get_enabled: false,
            ..DimseRoleConfig::default()
        },
        ..DimseServerConfig::default()
    };
    let server = DimseServer::bind("127.0.0.1:0".parse().unwrap(), config, NoopService)
        .expect("server bind");
    let addr = server.local_addr().expect("server addr");
    let handle = thread::spawn(move || {
        let _ = server.run_once();
    });

    let mut client_config = DimseClientConfig::default();
    client_config.tls_policy = TlsPolicy::AllowInsecure;
    client_config.transport_security = TransportSecurity::Insecure;
    let mut client = DimseClient::connect(
        addr,
        "CALLED_AE",
        "CALLING_AE",
        &AssociationPolicy::verification_default(),
        client_config,
    )
    .expect("verification association");

    let err = client
        .c_get(9, "1.2.840.10008.5.1.4.1.2.2.3", 0, &[])
        .expect_err("C-GET must fail closed when role is disabled");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    let _ = client.release();
    handle.join().expect("run_once thread");
}

#[test]
fn malformed_dimse_transport_payload_fails_closed() {
    let config = DimseServerConfig {
        auth: DimseAuthConfig::allow_all(),
        tls_policy: TlsPolicy::AllowInsecure,
        transport_security: TransportSecurity::Insecure,
        ..DimseServerConfig::default()
    };
    let server = DimseServer::bind("127.0.0.1:0".parse().unwrap(), config, NoopService)
        .expect("server bind");
    let addr = server.local_addr().expect("server addr");
    let handle = thread::spawn(move || {
        let _ = server.run_once();
    });

    let mut stream = std::net::TcpStream::connect(addr).expect("connect");
    stream
        .write_all(b"NOT_A_VALID_DIMSE_PDU")
        .expect("write malformed payload");
    stream
        .shutdown(std::net::Shutdown::Write)
        .expect("shutdown write");
    let mut buffer = [0u8; 16];
    let read = stream.read(&mut buffer);
    assert!(read.is_err() || matches!(read, Ok(0)));
    handle.join().expect("run_once thread");
}

#[test]
fn c_store_disabled_in_roles_returns_fail_closed_status() {
    let config = DimseServerConfig {
        auth: DimseAuthConfig::allow_all(),
        tls_policy: TlsPolicy::AllowInsecure,
        transport_security: TransportSecurity::Insecure,
        roles: DimseRoleConfig {
            c_store_enabled: false,
            ..DimseRoleConfig::default()
        },
        ..DimseServerConfig::default()
    };
    let server = DimseServer::bind("127.0.0.1:0".parse().unwrap(), config, NoopService)
        .expect("server bind");
    let addr = server.local_addr().expect("server addr");
    let handle = thread::spawn(move || {
        let _ = server.run_once();
    });

    let mut client_config = DimseClientConfig::default();
    client_config.tls_policy = TlsPolicy::AllowInsecure;
    client_config.transport_security = TransportSecurity::Insecure;
    match DimseClient::connect(
        addr,
        "CALLED_AE",
        "CALLING_AE",
        &c_store_association_policy(),
        client_config,
    ) {
        Ok(mut client) => {
            match client.c_store(
                5,
                "1.2.840.10008.5.1.4.1.1.7",
                "1.2.840.10008.5.1.4.1.1.7.55",
                &sample_c_store_dataset("1.2.3", "1.2.3.4", "1.2.3.4.5"),
            ) {
                Ok(status) => assert_eq!(status.code, 0x0122),
                Err(err) => assert!(matches!(err.kind(), ErrorKind::DecodeError { .. })),
            }
            let _ = client.release();
        }
        Err(err) => assert!(matches!(err.kind(), ErrorKind::DecodeError { .. })),
    }
    handle.join().expect("run_once thread");
}

fn free_tcp_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("allocate port");
    let port = listener.local_addr().expect("allocated port").port();
    drop(listener);
    port
}

fn service_binary_path() -> PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_dicom-dimse-service") {
        return PathBuf::from(path);
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_dir = std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| {
        manifest_dir
            .parent()
            .expect("parent")
            .parent()
            .expect("workspace root")
            .join("target")
            .to_string_lossy()
            .into_owned()
    });
    PathBuf::from(target_dir)
        .join("debug")
        .join("dicom-dimse-service")
}

fn temp_wal_path() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push("dicom-dimse-service");
    path.push(format!(
        "runtime-contract-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ));
    path.set_extension("wal");
    path
}

fn get_http_response(port: u16, path: &str) -> std::io::Result<String> {
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(Duration::from_millis(500)))?;
    stream.set_write_timeout(Some(Duration::from_millis(500)))?;
    let request = format!("GET {path} HTTP/1.1\r\nhost: 127.0.0.1\r\nconnection: close\r\n\r\n");
    stream.write_all(request.as_bytes())?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

fn wait_for_http(port: u16, path: &str, timeout: Duration, expected_status: &str) -> String {
    let deadline = std::time::Instant::now() + timeout;
    let mut last = None;
    while std::time::Instant::now() < deadline {
        match get_http_response(port, path) {
            Ok(response) => {
                if response.starts_with(expected_status) {
                    return response;
                }
                last = Some(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("unexpected response: {response}"),
                ));
            }
            Err(err) => {
                last = Some(err);
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
    panic!("expected http endpoint {path} to become available with {expected_status}: {last:?}");
}

fn run_failed_dimse_startup(envs: &[(&str, String)]) -> String {
    let mut command = Command::new(service_binary_path());
    for (key, value) in envs {
        command.env(key, value);
    }
    let output = command.output().expect("run dimse service");
    assert!(
        !output.status.success(),
        "dimse service unexpectedly started: stdout={:?}, stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn duration_percentile(mut latencies: Vec<Duration>, percentile: u32) -> Duration {
    assert!(
        (1..=100).contains(&percentile),
        "percentile must be in 1..=100"
    );
    if latencies.is_empty() {
        return Duration::from_nanos(0);
    }
    latencies.sort_unstable();
    let idx = latencies.len().saturating_sub(1) * (percentile as usize) / 100;
    latencies[idx]
}

fn c_store_association_policy() -> AssociationPolicy {
    AssociationPolicy {
        called_ae: None,
        supported_abstract_syntaxes: vec![
            "1.2.840.10008.1.1".to_string(),
            "1.2.840.10008.5.1.4.1.1.2".to_string(),
            "1.2.840.10008.5.1.4.1.1.7".to_string(),
        ],
        supported_transfer_syntaxes: vec!["1.2.840.10008.1.2.1".to_string()],
        max_pdu_length: 16_384,
    }
}

fn dataset_element_explicit(tag: Tag, vr: [u8; 2], value: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&tag.0.to_le_bytes());
    buf.extend_from_slice(&tag.1.to_le_bytes());
    buf.extend_from_slice(&vr);
    let mut bytes = value.to_vec();
    if bytes.len() % 2 == 1 {
        bytes.push(0);
    }
    match &vr {
        b"OB" | b"OW" | b"SQ" | b"UN" | b"UT" => {
            buf.extend_from_slice(&0u16.to_le_bytes());
            buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        }
        _ => {
            buf.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
        }
    }
    buf.extend_from_slice(&bytes);
    buf
}

fn sample_c_store_dataset(study_uid: &str, series_uid: &str, instance_uid: &str) -> Vec<u8> {
    let mut dataset = Vec::new();
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0008, 0x0016),
        *b"UI",
        b"1.2.840.10008.5.1.4.1.1.7",
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0008, 0x0018),
        *b"UI",
        instance_uid.as_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0020, 0x000D),
        *b"UI",
        study_uid.as_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0020, 0x000E),
        *b"UI",
        series_uid.as_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0002),
        *b"US",
        &1u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0004),
        *b"CS",
        b"MONOCHROME2",
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0010),
        *b"US",
        &1u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0011),
        *b"US",
        &1u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0100),
        *b"US",
        &16u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0101),
        *b"US",
        &12u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0102),
        *b"US",
        &11u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0103),
        *b"US",
        &0u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x7FE0, 0x0010),
        *b"OB",
        &[0u8],
    ));
    dataset
}

#[test]
fn dimse_service_exposes_health_and_readiness_endpoints_for_orchestration() {
    // Verify the binary-level orchestration endpoints.
    let server_port = free_tcp_port();
    let health_port = free_tcp_port();
    let wal_path = temp_wal_path();
    let mut command = Command::new(service_binary_path());
    let _ = std::fs::remove_file(&wal_path);
    let mut child = command
        .env("DICOM_DIMSE_AUTH_MODE", "allow_all")
        .env("DICOM_DIMSE_BIND", format!("127.0.0.1:{server_port}"))
        .env(
            "DICOM_DIMSE_HEALTH_BIND",
            format!("127.0.0.1:{health_port}"),
        )
        .env("DICOM_DIMSE_TLS_POLICY", "allow_insecure")
        .env("DICOM_DIMSE_TRANSPORT_SECURITY", "insecure")
        .env("DICOM_DIMSE_STORAGE_WAL", wal_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn service");

    let ready = wait_for_http(
        health_port,
        "/readyz",
        Duration::from_secs(5),
        "HTTP/1.1 200",
    );
    let health = wait_for_http(
        health_port,
        "/healthz",
        Duration::from_secs(2),
        "HTTP/1.1 200",
    );
    let ready_repeat = wait_for_http(
        health_port,
        "/readyz",
        Duration::from_secs(2),
        "HTTP/1.1 200",
    );
    let health_repeat = wait_for_http(
        health_port,
        "/healthz",
        Duration::from_secs(2),
        "HTTP/1.1 200",
    );
    assert!(ready.starts_with("HTTP/1.1 200"));
    assert!(health.starts_with("HTTP/1.1 200"));
    assert_eq!(ready, ready_repeat);
    assert_eq!(health, health_repeat);

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn dimse_startup_preflight_rejects_invalid_bind_address() {
    let output = run_failed_dimse_startup(&[("DICOM_DIMSE_BIND", "bad:bind".to_string())]);
    assert!(
        output.contains("invalid bind address") || output.contains("bad:bind"),
        "expected invalid bind diagnostics, got: {output}"
    );
}

#[test]
fn dimse_startup_preflight_rejects_invalid_tls_material_paths() {
    let wal_path = temp_wal_path();
    let output = run_failed_dimse_startup(&[
        ("DICOM_DIMSE_BIND", "127.0.0.1:0".to_string()),
        ("DICOM_DIMSE_AUTH_MODE", "allow_all".to_string()),
        ("DICOM_DIMSE_TRANSPORT_SECURITY", "tls".to_string()),
        (
            "DICOM_DIMSE_TLS_CERT_PATH",
            "/dev/null/missing-dimse-cert.pem".to_string(),
        ),
        (
            "DICOM_DIMSE_TLS_KEY_PATH",
            "/dev/null/missing-dimse-key.pem".to_string(),
        ),
        (
            "DICOM_DIMSE_STORAGE_WAL",
            wal_path.to_string_lossy().into_owned(),
        ),
    ]);
    assert!(
        output.contains("DICOM_DIMSE_TLS_CERT_PATH must reference an existing file")
            || output.contains("DICOM_DIMSE_TLS_KEY_PATH must reference an existing file")
            || output.contains("requires DICOM_DIMSE_TLS_CERT_PATH"),
        "expected invalid TLS material diagnostics, got: {output}"
    );
    let _ = std::fs::remove_file(&wal_path);
}

#[test]
fn dimse_startup_preflight_rejects_unwritable_wal_path() {
    let output = run_failed_dimse_startup(&[
        ("DICOM_DIMSE_BIND", "127.0.0.1:0".to_string()),
        ("DICOM_DIMSE_AUTH_MODE", "allow_all".to_string()),
        (
            "DICOM_DIMSE_STORAGE_WAL",
            "/dev/null/dicom-dimse-service.wal".to_string(),
        ),
    ]);
    assert!(
        output.contains("failed to open DIMSE storage WAL")
            || output.contains("Not a directory")
            || output.contains("storage WAL")
            || output.contains("failed to create DIMSE storage parent directory")
            || output.contains("File exists"),
        "expected unwritable WAL diagnostics, got: {output}"
    );
}

#[test]
fn dimse_graceful_shutdown_flushes_storage_wal_for_reopen() {
    let wal_path = temp_wal_path();
    let _ = std::fs::remove_file(&wal_path);

    let service = StorageBackedDimseService::open(Limits::default(), &wal_path)
        .expect("open storage-backed dimse service");
    let config = DimseServerConfig {
        auth: DimseAuthConfig::allow_all(),
        tls_policy: TlsPolicy::AllowInsecure,
        transport_security: TransportSecurity::Insecure,
        ..DimseServerConfig::default()
    };
    let server =
        DimseServer::bind("127.0.0.1:0".parse().unwrap(), config, service).expect("server bind");
    let addr = server.local_addr().expect("server addr");
    let handle = thread::spawn(move || {
        let _ = server.run_once();
    });

    let mut client_config = DimseClientConfig::default();
    client_config.tls_policy = TlsPolicy::AllowInsecure;
    client_config.transport_security = TransportSecurity::Insecure;
    let association_policy = c_store_association_policy();
    let mut client = DimseClient::connect(
        addr,
        "CALLED_AE",
        "CALLING_AE",
        &association_policy,
        client_config,
    )
    .expect("client connect");

    let sop_instance_uid = "1.2.840.10008.9.9.9.1";
    let payload =
        sample_c_store_dataset("1.2.840.10008.9.9", "1.2.840.10008.9.9.1", sop_instance_uid);
    let status = client
        .c_store(1, "1.2.840.10008.5.1.4.1.1.7", sop_instance_uid, &payload)
        .expect("c-store status");
    assert_eq!(status.code, DimseStatus::success().code);

    let _ = client.release();
    handle.join().expect("run_once thread");

    let wal_size = std::fs::metadata(&wal_path).expect("wal metadata").len();
    assert!(
        wal_size > 0,
        "expected non-empty WAL after graceful shutdown"
    );

    let reopened = Storage::open(Limits::default(), &wal_path).expect("reopen wal");
    let datasets = reopened.datasets().expect("datasets");
    assert_eq!(datasets.len(), 1);

    let _ = std::fs::remove_file(&wal_path);
}

#[test]
fn dimse_server_sustained_c_store_load_stays_within_budget() {
    // REQ-DIMSE-302, REQ-DIMSE-303, REQ-HI-290
    const C_STORE_PAYLOAD_BYTES: usize = 32 * 1024;
    const C_STORE_OP_COUNT: usize = 240;
    const MIN_THROUGHPUT_OPS_PER_SEC: f64 = 4.0;
    const MAX_P95_LATENCY: Duration = Duration::from_millis(220);
    const MAX_TOTAL_DURATION: Duration = Duration::from_secs(30);

    let payload = vec![0xA5u8; C_STORE_PAYLOAD_BYTES];
    let config = DimseServerConfig {
        auth: DimseAuthConfig::allow_all(),
        tls_policy: TlsPolicy::AllowInsecure,
        transport_security: TransportSecurity::Insecure,
        ..DimseServerConfig::default()
    };
    let server = DimseServer::bind("127.0.0.1:0".parse().unwrap(), config, SuccessService)
        .expect("server bind");
    let addr = server.local_addr().expect("server addr");
    let handle = thread::spawn(move || {
        let _ = server.run_once();
    });

    let mut client_config = DimseClientConfig::default();
    client_config.tls_policy = TlsPolicy::AllowInsecure;
    client_config.transport_security = TransportSecurity::Insecure;
    let association_policy = c_store_association_policy();
    let mut client = DimseClient::connect(
        addr,
        "CALLED_AE",
        "CALLING_AE",
        &association_policy,
        client_config,
    )
    .expect("client connect");

    let mut latencies = Vec::with_capacity(C_STORE_OP_COUNT);
    let started = std::time::Instant::now();
    for i in 0..C_STORE_OP_COUNT {
        let message_id = u16::try_from(i + 1).expect("small message id");
        let sop_instance_uid = format!("1.2.840.10008.1.2.3.4.5.6.7.8.{i}");
        let op_start = std::time::Instant::now();
        let status = client
            .c_store(
                message_id,
                "1.2.840.10008.5.1.4.1.1.2",
                &sop_instance_uid,
                &payload,
            )
            .expect("c-store status");
        latencies.push(op_start.elapsed());
        assert_eq!(status.code, DimseStatus::success().code);
    }
    let total = started.elapsed();
    let p95 = duration_percentile(latencies, 95);

    let throughput = C_STORE_OP_COUNT as f64 / total.as_secs_f64();
    assert!(
        total <= MAX_TOTAL_DURATION,
        "sustained C-STORE run took {total:?}, above budget {MAX_TOTAL_DURATION:?}"
    );
    assert!(
        p95 <= MAX_P95_LATENCY,
        "95th percentile C-STORE latency {p95:?} above budget {MAX_P95_LATENCY:?}"
    );
    assert!(
        throughput >= MIN_THROUGHPUT_OPS_PER_SEC,
        "throughput {throughput:.2} ops/s below budget {MIN_THROUGHPUT_OPS_PER_SEC:.2} ops/s"
    );

    let _ = client.release();
    handle.join().expect("run_once thread");
}
