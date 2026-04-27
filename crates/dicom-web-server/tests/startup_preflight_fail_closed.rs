use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn temp_wal_path() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("diccy_web_startup_preflight_{nonce}.wal"))
}

fn run_failed_web_startup(envs: &[(&str, String)]) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_dicom-web-server"));
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    for (key, value) in envs {
        command.env(key, value);
    }

    let mut child = command.spawn().expect("spawn dicom-web-server");
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        if child.try_wait().expect("poll web server exit").is_some() {
            let output = child.wait_with_output().expect("collect web output");
            assert!(
                !output.status.success(),
                "web server unexpectedly started: stdout={:?}, stderr={:?}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return format!(
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("expected dicom-web-server to fail startup within timeout");
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn web_startup_preflight_rejects_invalid_bind_address() {
    let wal_path = temp_wal_path();
    let output = run_failed_web_startup(&[
        ("DICOM_WEB_BIND", "bad:bind".to_string()),
        (
            "DICOM_WEB_STORAGE_WAL",
            wal_path.to_string_lossy().into_owned(),
        ),
    ]);
    assert!(
        output.contains("invalid bind address")
            || output.contains("bad:bind")
            || output.contains("invalid port value"),
        "expected invalid bind diagnostics, got: {output}"
    );
    let _ = std::fs::remove_file(wal_path);
}

#[test]
fn web_startup_preflight_rejects_unwritable_wal_path() {
    let output = run_failed_web_startup(&[
        ("DICOM_WEB_BIND", "127.0.0.1:0".to_string()),
        (
            "DICOM_WEB_STORAGE_WAL",
            "/dev/null/dicom-web-server.wal".to_string(),
        ),
    ]);
    assert!(
        output.contains("storage WAL")
            || output.contains("Not a directory")
            || output.contains("failed")
            || output.contains("File exists"),
        "expected unwritable WAL diagnostics, got: {output}"
    );
}

#[test]
fn web_startup_preflight_rejects_invalid_tls_cert_path_variables() {
    let wal_path = temp_wal_path();
    let output = run_failed_web_startup(&[
        ("DICOM_WEB_BIND", "127.0.0.1:0".to_string()),
        (
            "DICOM_WEB_STORAGE_WAL",
            wal_path.to_string_lossy().into_owned(),
        ),
        (
            "DICOM_WEB_TLS_CERT_PATH",
            "/dev/null/unsupported-web-cert.pem".to_string(),
        ),
    ]);
    assert!(
        output.contains("DICOM_WEB_TLS_CERT_PATH")
            || output.contains("unknown")
            || output.contains("unsupported"),
        "expected invalid cert-path diagnostics, got: {output}"
    );
    let _ = std::fs::remove_file(wal_path);
}
