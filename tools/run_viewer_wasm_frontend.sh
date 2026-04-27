#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: ./tools/run_viewer_wasm_frontend.sh [options]

Builds viewer-wasm for the browser using wasm-pack, then serves the host app.

Options:
  --port <N>        HTTP port (default: 4173)
  --https           Serve over HTTPS (TLS) instead of HTTP
  --cert <PATH>     TLS certificate path (PEM). Requires --key.
  --key <PATH>      TLS private key path (PEM). Requires --cert.
  --dicomweb-upstream <URL>
                    Upstream DICOMweb base URL for /dicomweb proxy
                    (default: http://127.0.0.1:8080)
  --workflow-upstream <URL>
                    Upstream workflow base URL for /workflow proxy
                    (default: http://127.0.0.1:8082)
  --release         Build in release mode (default: dev mode)
  --no-build        Skip wasm-pack build and only serve existing files
  --features <CSV>  Extra Cargo features for viewer-wasm (for example: network)
                    Note: webgpu-backend is enabled by default by this script.
  -h, --help        Show this help text
EOF
}

PORT=4173
BUILD_MODE=dev
NO_BUILD=0
FEATURES=""
USE_HTTPS=0
TLS_CERT=""
TLS_KEY=""
DICOMWEB_UPSTREAM="http://127.0.0.1:8080"
WORKFLOW_UPSTREAM="http://127.0.0.1:8082"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --port)
      PORT="${2:-}"
      if [[ -z "$PORT" ]]; then
        echo "error: --port requires a value" >&2
        exit 1
      fi
      shift 2
      ;;
    --https)
      USE_HTTPS=1
      shift
      ;;
    --cert)
      TLS_CERT="${2:-}"
      if [[ -z "$TLS_CERT" ]]; then
        echo "error: --cert requires a value" >&2
        exit 1
      fi
      shift 2
      ;;
    --key)
      TLS_KEY="${2:-}"
      if [[ -z "$TLS_KEY" ]]; then
        echo "error: --key requires a value" >&2
        exit 1
      fi
      shift 2
      ;;
    --release)
      BUILD_MODE=release
      shift
      ;;
    --dicomweb-upstream)
      DICOMWEB_UPSTREAM="${2:-}"
      if [[ -z "$DICOMWEB_UPSTREAM" ]]; then
        echo "error: --dicomweb-upstream requires a value" >&2
        exit 1
      fi
      shift 2
      ;;
    --workflow-upstream)
      WORKFLOW_UPSTREAM="${2:-}"
      if [[ -z "$WORKFLOW_UPSTREAM" ]]; then
        echo "error: --workflow-upstream requires a value" >&2
        exit 1
      fi
      shift 2
      ;;
    --no-build)
      NO_BUILD=1
      shift
      ;;
    --features)
      FEATURES="${2:-}"
      if [[ -z "$FEATURES" ]]; then
        echo "error: --features requires a value" >&2
        exit 1
      fi
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown option '$1'" >&2
      usage
      exit 1
      ;;
  esac
done

if [[ -n "$TLS_CERT" && -z "$TLS_KEY" ]]; then
  echo "error: --cert requires --key" >&2
  exit 1
fi
if [[ -n "$TLS_KEY" && -z "$TLS_CERT" ]]; then
  echo "error: --key requires --cert" >&2
  exit 1
fi
if [[ -n "$TLS_CERT" || -n "$TLS_KEY" ]]; then
  USE_HTTPS=1
fi

FEATURE_SET="webgpu-backend"
if [[ -n "$FEATURES" ]]; then
  case ",${FEATURES}," in
    *",webgpu-backend,"*) FEATURE_SET="${FEATURES}" ;;
    *) FEATURE_SET="${FEATURES},webgpu-backend" ;;
  esac
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CRATE_DIR="${ROOT_DIR}/crates/viewer-wasm"
WEB_DIR="${CRATE_DIR}/web"
PKG_DIR="${WEB_DIR}/pkg"
TLS_DIR="${ROOT_DIR}/state/viewer-wasm/tls"

# Prefer rustup-managed toolchains when available so wasm target detection and
# build behavior are deterministic across machines that also have system Rust.
if command -v rustup >/dev/null 2>&1; then
  RUSTUP_CARGO="$(rustup which cargo 2>/dev/null || true)"
  if [[ -n "${RUSTUP_CARGO}" ]]; then
    export PATH="$(dirname "${RUSTUP_CARGO}"):${PATH}"
  fi
fi

if [[ "$NO_BUILD" -eq 0 ]]; then
  if ! command -v wasm-pack >/dev/null 2>&1; then
    echo "error: wasm-pack is required but not installed." >&2
    exit 1
  fi
  if ! command -v wasm-bindgen >/dev/null 2>&1; then
    cat <<'EOF' >&2
error: wasm-bindgen-cli is required but not installed.
Install once, then retry:
  cargo install wasm-bindgen-cli
EOF
    exit 1
  fi
  if ! command -v rustc >/dev/null 2>&1; then
    echo "error: rustc is required to build viewer-wasm." >&2
    exit 1
  fi
  TARGET_LIBDIR="$(rustc --print target-libdir --target wasm32-unknown-unknown 2>/dev/null || true)"
  if [[ -z "${TARGET_LIBDIR}" || ! -d "${TARGET_LIBDIR}" ]]; then
    cat <<'EOF' >&2
error: missing Rust target wasm32-unknown-unknown
If you use rustup:
  rustup target add wasm32-unknown-unknown

If your rustc comes from a non-rustup toolchain (for example Homebrew),
install the wasm32 target for that toolchain as described by:
  https://rustwasm.github.io/wasm-pack/book/prerequisites/non-rustup-setups.html
EOF
    exit 1
  fi

  echo "[1/2] Building viewer-wasm for web (${BUILD_MODE})"
  build_cmd=(
    wasm-pack build "${CRATE_DIR}"
    --mode no-install
    --target web
    --out-dir "${PKG_DIR}"
    --out-name viewer_wasm
  )
  if [[ "$BUILD_MODE" == "release" ]]; then
    build_cmd+=(--release)
  else
    build_cmd+=(--dev)
  fi
  build_cmd+=(-- --features "$FEATURE_SET")
  echo "[1/2] viewer-wasm features: ${FEATURE_SET}"
  "${build_cmd[@]}"
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "error: python3 is required to run the local static server." >&2
  exit 1
fi

echo "[2/2] Serving ${WEB_DIR}"
HOST_IP="$(hostname -I 2>/dev/null | awk '{print $1}')"
if [[ "$USE_HTTPS" -eq 1 ]]; then
  if [[ -z "$TLS_CERT" || -z "$TLS_KEY" ]]; then
    if ! command -v openssl >/dev/null 2>&1; then
      echo "error: openssl is required for autogenerated HTTPS certificates." >&2
      echo "Provide --cert/--key or install openssl." >&2
      exit 1
    fi
    mkdir -p "${TLS_DIR}"
    TLS_CERT="${TLS_DIR}/viewer-wasm-local.crt"
    TLS_KEY="${TLS_DIR}/viewer-wasm-local.key"
    if [[ ! -f "${TLS_CERT}" || ! -f "${TLS_KEY}" ]]; then
      SAN="DNS:localhost,IP:127.0.0.1"
      if [[ -n "${HOST_IP}" ]]; then
        SAN="${SAN},IP:${HOST_IP}"
      fi
      openssl req -x509 -newkey rsa:2048 -sha256 -days 365 -nodes \
        -keyout "${TLS_KEY}" \
        -out "${TLS_CERT}" \
        -subj "/CN=localhost" \
        -addext "subjectAltName=${SAN}" >/dev/null 2>&1
    fi
  fi

  if [[ ! -f "${TLS_CERT}" ]]; then
    echo "error: TLS certificate not found: ${TLS_CERT}" >&2
    exit 1
  fi
  if [[ ! -f "${TLS_KEY}" ]]; then
    echo "error: TLS key not found: ${TLS_KEY}" >&2
    exit 1
  fi
fi

if [[ "$USE_HTTPS" -eq 1 ]]; then
  echo "Open: https://127.0.0.1:${PORT}"
  if [[ -n "${HOST_IP:-}" ]]; then
    echo "LAN:  https://${HOST_IP}:${PORT}"
  fi
else
  echo "Open: http://127.0.0.1:${PORT}"
  if [[ -n "${HOST_IP:-}" ]]; then
    echo "LAN:  http://${HOST_IP}:${PORT}"
  fi
fi
echo "Proxy routes:"
echo "  /dicomweb -> ${DICOMWEB_UPSTREAM}"
echo "  /workflow -> ${WORKFLOW_UPSTREAM}"

export DICCY_WEB_DIR="${WEB_DIR}"
export DICCY_TLS_CERT="${TLS_CERT}"
export DICCY_TLS_KEY="${TLS_KEY}"
export DICCY_PORT="${PORT}"
export DICCY_USE_HTTPS="${USE_HTTPS}"
export DICCY_DICOMWEB_UPSTREAM="${DICOMWEB_UPSTREAM}"
export DICCY_WORKFLOW_UPSTREAM="${WORKFLOW_UPSTREAM}"
python3 - <<'PY'
import http.server
import os
import ssl
import urllib.error
import urllib.parse
import urllib.request

web_dir = os.environ["DICCY_WEB_DIR"]
cert_path = os.environ["DICCY_TLS_CERT"]
key_path = os.environ["DICCY_TLS_KEY"]
port = int(os.environ["DICCY_PORT"])
use_https = os.environ.get("DICCY_USE_HTTPS", "0") == "1"
dicomweb_upstream = os.environ["DICCY_DICOMWEB_UPSTREAM"].rstrip("/")
workflow_upstream = os.environ["DICCY_WORKFLOW_UPSTREAM"].rstrip("/")

HOP_BY_HOP_HEADERS = {
    "connection",
    "proxy-connection",
    "keep-alive",
    "transfer-encoding",
    "upgrade",
    "te",
    "trailers",
    "proxy-authenticate",
    "proxy-authorization",
}


class ViewerHostRequestHandler(http.server.SimpleHTTPRequestHandler):
    backend_routes = (
        ("/dicomweb", dicomweb_upstream),
        ("/workflow", workflow_upstream),
    )

    def _proxy_target_url(self):
        split = urllib.parse.urlsplit(self.path)
        path = split.path or "/"
        query = split.query
        for prefix, upstream in self.backend_routes:
            if path == prefix or path.startswith(prefix + "/"):
                suffix = path[len(prefix):] or "/"
                target = f"{upstream}{suffix}"
                if query:
                    target = f"{target}?{query}"
                return target
        return None

    def _forward_proxy_request(self):
        target_url = self._proxy_target_url()
        if not target_url:
            return False

        length = int(self.headers.get("Content-Length", "0") or "0")
        body = self.rfile.read(length) if length > 0 else None
        headers = {}
        for key, value in self.headers.items():
            lower = key.lower()
            if lower in HOP_BY_HOP_HEADERS or lower == "host" or lower == "content-length":
                continue
            headers[key] = value
        if body is not None:
            headers["Content-Length"] = str(len(body))

        request = urllib.request.Request(target_url, data=body, headers=headers, method=self.command)
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                payload = response.read()
                self.send_response(response.status)
                for key, value in response.headers.items():
                    lower = key.lower()
                    if lower in HOP_BY_HOP_HEADERS or lower == "content-length":
                        continue
                    self.send_header(key, value)
                self.send_header("Content-Length", str(len(payload)))
                self.end_headers()
                if self.command != "HEAD":
                    self.wfile.write(payload)
            return True
        except urllib.error.HTTPError as error:
            payload = error.read()
            self.send_response(error.code)
            for key, value in error.headers.items():
                lower = key.lower()
                if lower in HOP_BY_HOP_HEADERS or lower == "content-length":
                    continue
                self.send_header(key, value)
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            if self.command != "HEAD":
                self.wfile.write(payload)
            return True
        except urllib.error.URLError as error:
            self.log_error("proxy upstream unavailable for %s: %s", target_url, error.reason)
            message = f"upstream unavailable: {error.reason}\n".encode("utf-8")
            self.send_response(502)
            self.send_header("Content-Type", "text/plain; charset=utf-8")
            self.send_header("Content-Length", str(len(message)))
            self.end_headers()
            if self.command != "HEAD":
                self.wfile.write(message)
            return True

    def do_GET(self):
        if self._forward_proxy_request():
            return
        super().do_GET()

    def do_HEAD(self):
        if self._forward_proxy_request():
            return
        super().do_HEAD()

    def do_POST(self):
        if self._forward_proxy_request():
            return
        self.send_error(405, "Method not allowed")

    def do_PATCH(self):
        if self._forward_proxy_request():
            return
        self.send_error(405, "Method not allowed")

    def do_PUT(self):
        if self._forward_proxy_request():
            return
        self.send_error(405, "Method not allowed")

    def do_DELETE(self):
        if self._forward_proxy_request():
            return
        self.send_error(405, "Method not allowed")

    def do_OPTIONS(self):
        if self._forward_proxy_request():
            return
        self.send_response(204)
        self.send_header("Allow", "GET, HEAD, OPTIONS")
        self.end_headers()


handler = lambda *args, **kwargs: ViewerHostRequestHandler(*args, directory=web_dir, **kwargs)  # noqa: E731
httpd = http.server.ThreadingHTTPServer(("0.0.0.0", port), handler)
if use_https:
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    ctx.load_cert_chain(certfile=cert_path, keyfile=key_path)
    httpd.socket = ctx.wrap_socket(httpd.socket, server_side=True)
httpd.serve_forever()
PY
