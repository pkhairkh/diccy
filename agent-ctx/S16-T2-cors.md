# S16-T2: CORS and Multi-Origin DICOMweb Support

## Task: Add configurable CORS headers to dicom-web-server

### Completed Implementation

1. **Created `/home/z/diccy/crates/dicom-web/src/cors.rs`**
   - `OriginPolicy` enum: Any, Whitelist(HashSet), Regex(Vec<String>), Subdomain { base_domain }
   - `CorsConfig` struct with allowed_origins, allowed_methods, allowed_headers, exposed_headers, allow_credentials, max_age_seconds
   - `OriginPolicy::is_origin_allowed()` — wildcard pattern matching with `*` and `?`
   - `CorsConfig::cors_headers()` — generates CORS response headers
   - `CorsConfig::handle_preflight()` — handles OPTIONS preflight requests
   - `OriginPermissionScope` — per-origin RBAC permission scoping
   - `AuthScope` enum: Read, Write, Delete, Measurement, Segmentation, Admin
   - `MultiOriginCorsConfig` — multi-origin CORS with per-origin scopes
   - `default_dicomweb_cors()` — conservative default (deny-all, GET/HEAD/POST allowed)
   - `development_cors()` — permissive development config

2. **Updated `/home/z/diccy/crates/dicom-web/src/lib.rs`**
   - Added `pub mod cors;`
   - Added `cors: CorsConfig` field to `DicomWebServiceConfig`
   - Default config uses `default_dicomweb_cors()`

### Key Design Decisions
- Fail-closed: default CORS policy denies all origins
- Wildcard pattern matching for regex-style origin matching without external regex dependency
- Per-origin RBAC integration via `OriginPermissionScope`
- OHIF cross-origin scenario explicitly tested

### Test Results
- 20 CORS tests passing
- Tests cover: OriginPolicy variants, CorsConfig header generation, preflight handling, multi-origin scopes, OHIF integration scenario, auth scope roundtrip, pattern matching
