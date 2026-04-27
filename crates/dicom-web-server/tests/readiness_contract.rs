use std::io::{Read, Write};
use std::net::Shutdown;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind free port");
    listener.local_addr().expect("local addr").port()
}

fn temp_wal_path() -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("diccy_web_readyz_{nonce}.wal"))
}

fn send_http(port: u16, method: &str, path: &str) -> std::io::Result<(u16, String)> {
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port))?;
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
    );
    stream.write_all(request.as_bytes())?;
    stream.shutdown(Shutdown::Write)?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    let status_code = response
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .expect("status code");
    let body = response
        .split("\r\n\r\n")
        .nth(1)
        .or_else(|| response.split("\n\n").nth(1))
        .unwrap_or_default()
        .to_string();
    Ok((status_code, body))
}

fn spawn_web_server(port: u16, wal_path: &std::path::Path) -> Child {
    Command::new(env!("CARGO_BIN_EXE_dicom-web-server"))
        .env("DICOM_WEB_BIND", format!("127.0.0.1:{port}"))
        .env("DICOM_WEB_AUTH_MODE", "allow_all")
        .env("DICOM_WEB_STORAGE_WAL", wal_path.to_string_lossy().as_ref())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn web server")
}

fn wait_for_web_health(child: &mut Child, port: u16) {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().expect("poll web child status") {
            let mut stdout = String::new();
            let mut stderr = String::new();
            if let Some(out) = child.stdout.as_mut() {
                let _ = out.read_to_string(&mut stdout);
            }
            if let Some(err) = child.stderr.as_mut() {
                let _ = err.read_to_string(&mut stderr);
            }
            panic!(
                "web server exited before /healthz became ready: status={status}, stdout={stdout:?}, stderr={stderr:?}"
            );
        }
        if std::time::Instant::now() > deadline {
            panic!("web server did not expose /healthz in time");
        }
        if let Ok((status, _)) = send_http(port, "GET", "/healthz") {
            if status == 200 {
                return;
            }
        }
        thread::sleep(Duration::from_millis(40));
    }
}

#[test]
fn web_health_and_readiness_contracts_are_deterministic() {
    let port = free_port();
    let wal_path = temp_wal_path();
    let mut child = spawn_web_server(port, &wal_path);
    wait_for_web_health(&mut child, port);

    let (health_status_1, health_body_1) = send_http(port, "GET", "/healthz").expect("healthz");
    let (health_status_2, health_body_2) =
        send_http(port, "GET", "/healthz").expect("healthz repeat");
    assert_eq!(health_status_1, 200);
    assert_eq!(health_status_2, 200);
    assert_eq!(health_body_1, health_body_2);
    assert!(health_body_1.contains("\"status\":\"ok\""));

    let (ready_status_1, ready_body_1) = send_http(port, "GET", "/readyz").expect("readyz");
    let (ready_status_2, ready_body_2) = send_http(port, "GET", "/readyz").expect("readyz repeat");
    assert_eq!(ready_status_1, 200);
    assert_eq!(ready_status_2, 200);
    assert_eq!(ready_body_1, ready_body_2);
    assert!(ready_body_1.contains("\"status\":\"ready\""));

    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_file(&wal_path);
}
