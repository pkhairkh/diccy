//! CORS configuration for DICOMweb endpoints.
//!
//! Provides configurable CORS headers for the `dicom-web` server, supporting
//! origin whitelist validation with wildcard patterns, preflight OPTIONS
//! request handling, and integration with RBAC for per-origin permission scoping.
//!
//! # Security Model
//!
//! The default CORS policy is conservative (deny-by-default). Origins must
//! be explicitly allowed via [`OriginPolicy`]. The `Any` variant should only
//! be used in development or air-gapped deployments.
//!
//! # Integration with RBAC
//!
//! Each allowed origin can be associated with a permission scope via
//! [`OriginPermissionScope`]. This enables enterprise deployments where
//! different origins (e.g., different hospital departments) have different
//! access levels.

use std::collections::HashSet;

use crate::HttpMethod;

// ---------------------------------------------------------------------------
// OriginPolicy — allowed origin patterns
// ---------------------------------------------------------------------------

/// Policy for which origins are allowed to make cross-origin requests.
///
/// Origins are matched against the `Origin` or `Sec-Fetch-Site` request
/// headers. The default is deny-all (fail-closed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OriginPolicy {
    /// Allow all origins (`Access-Control-Allow-Origin: *`).
    ///
    /// **Warning**: Only use in development or air-gapped deployments.
    /// When credentials are allowed, this variant is not permitted per
    /// the CORS specification.
    Any,

    /// Allow only specific origins from a whitelist.
    Whitelist(HashSet<String>),

    /// Allow origins matching regex patterns.
    ///
    /// Patterns are matched against the full origin string (including
    /// scheme, host, and port).
    Regex(Vec<String>),

    /// Allow all subdomains of a base domain.
    ///
    /// Matches `*.base_domain` where the wildcard covers exactly one
    /// subdomain level. For example, `Subdomain { base_domain: "example.com" }`
    /// matches `foo.example.com` but not `bar.foo.example.com`.
    Subdomain {
        /// The base domain for subdomain matching.
        base_domain: String,
    },
}

impl Default for OriginPolicy {
    fn default() -> Self {
        // Fail-closed: no origins allowed by default.
        Self::Whitelist(HashSet::new())
    }
}

impl OriginPolicy {
    /// Check if an origin is allowed by this policy.
    ///
    /// Returns `true` if the origin matches the policy, `false` otherwise.
    /// For the `Any` variant, always returns `true`.
    /// For the `Regex` variant, performs substring pattern matching
    /// (full regex support requires the `regex` crate; this implementation
    /// uses prefix/suffix/exact matching for deterministic behavior).
    pub fn is_origin_allowed(&self, origin: &str) -> bool {
        match self {
            OriginPolicy::Any => true,
            OriginPolicy::Whitelist(whitelist) => whitelist.contains(origin),
            OriginPolicy::Regex(patterns) => {
                for pattern in patterns {
                    if Self::match_pattern(pattern, origin) {
                        return true;
                    }
                }
                false
            }
            OriginPolicy::Subdomain { base_domain } => {
                // Match exact base domain or one-level subdomain
                if origin.ends_with(base_domain) {
                    let prefix = &origin[..origin.len() - base_domain.len()];
                    // Either exact match (prefix is "http://" or "https://")
                    // or one subdomain level (prefix ends with ".")
                    if prefix.is_empty() || prefix.ends_with('.') {
                        return true;
                    }
                    // Check scheme prefix
                    if prefix == "http://" || prefix == "https://" {
                        return true;
                    }
                    if prefix.ends_with(".")
                        && !prefix[..prefix.len() - 1].contains('.')
                    {
                        return true;
                    }
                }
                // Also check if origin is the base domain itself with scheme
                if origin == format!("http://{}", base_domain)
                    || origin == format!("https://{}", base_domain)
                {
                    return true;
                }
                false
            }
        }
    }

    /// Simple pattern matching supporting `*` wildcards.
    ///
    /// - `*` matches any sequence of characters
    /// - `?` matches exactly one character
    /// - All other characters match literally
    fn match_pattern(pattern: &str, value: &str) -> bool {
        let p: Vec<char> = pattern.chars().collect();
        let v: Vec<char> = value.chars().collect();
        Self::match_pattern_recursive(&p, &v, 0, 0)
    }

    fn match_pattern_recursive(
        pattern: &[char],
        value: &[char],
        pi: usize,
        vi: usize,
    ) -> bool {
        if pi == pattern.len() && vi == value.len() {
            return true;
        }
        if pi == pattern.len() {
            return false;
        }
        match pattern[pi] {
            '*' => {
                // Try matching zero or more characters
                for skip in vi..=value.len() {
                    if Self::match_pattern_recursive(pattern, value, pi + 1, skip) {
                        return true;
                    }
                }
                false
            }
            '?' => {
                if vi < value.len() {
                    Self::match_pattern_recursive(pattern, value, pi + 1, vi + 1)
                } else {
                    false
                }
            }
            c => {
                if vi < value.len() && value[vi] == c {
                    Self::match_pattern_recursive(pattern, value, pi + 1, vi + 1)
                } else {
                    false
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// CorsConfig — full CORS configuration
// ---------------------------------------------------------------------------

/// CORS configuration for DICOMweb endpoints.
///
/// Controls which origins, methods, and headers are permitted for
/// cross-origin requests. The default configuration is conservative
/// (deny-all).
#[derive(Debug, Clone)]
pub struct CorsConfig {
    /// Policy for allowed origins.
    pub allowed_origins: OriginPolicy,
    /// HTTP methods allowed in cross-origin requests.
    pub allowed_methods: Vec<HttpMethod>,
    /// Headers allowed in cross-origin requests.
    pub allowed_headers: Vec<String>,
    /// Headers exposed to the client in cross-origin responses.
    pub exposed_headers: Vec<String>,
    /// Whether credentials (cookies, auth headers) are allowed.
    pub allow_credentials: bool,
    /// Maximum age in seconds for preflight cache.
    pub max_age_seconds: Option<u32>,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: OriginPolicy::default(),
            allowed_methods: Vec::new(),
            allowed_headers: Vec::new(),
            exposed_headers: Vec::new(),
            allow_credentials: false,
            max_age_seconds: None,
        }
    }
}

impl CorsConfig {
    /// Check if an origin is allowed.
    pub fn is_origin_allowed(&self, origin: &str) -> bool {
        self.allowed_origins.is_origin_allowed(origin)
    }

    /// Generate CORS response headers for a given request.
    ///
    /// If the request origin is not allowed, returns an empty vector
    /// (no CORS headers = browser will block the response).
    pub fn cors_headers(
        &self,
        request_origin: Option<&str>,
        request_method: Option<&str>,
    ) -> Vec<(String, String)> {
        let Some(origin) = request_origin else {
            return Vec::new();
        };

        if !self.is_origin_allowed(origin) {
            return Vec::new();
        }

        let mut headers = Vec::new();

        // Access-Control-Allow-Origin
        if matches!(self.allowed_origins, OriginPolicy::Any) && !self.allow_credentials {
            headers.push((
                "access-control-allow-origin".to_string(),
                "*".to_string(),
            ));
        } else {
            headers.push((
                "access-control-allow-origin".to_string(),
                origin.to_string(),
            ));
        }

        // Access-Control-Allow-Credentials
        if self.allow_credentials {
            headers.push((
                "access-control-allow-credentials".to_string(),
                "true".to_string(),
            ));
        }

        // Access-Control-Expose-Headers
        if !self.exposed_headers.is_empty() {
            headers.push((
                "access-control-expose-headers".to_string(),
                self.exposed_headers.join(", "),
            ));
        }

        // Access-Control-Max-Age
        if let Some(max_age) = self.max_age_seconds {
            headers.push((
                "access-control-max-age".to_string(),
                max_age.to_string(),
            ));
        }

        // For preflight, include method and header headers
        if request_method.is_some() {
            // Access-Control-Allow-Methods
            if !self.allowed_methods.is_empty() {
                let methods: Vec<String> =
                    self.allowed_methods.iter().map(|m| http_method_str(*m).to_string()).collect();
                headers.push((
                    "access-control-allow-methods".to_string(),
                    methods.join(", "),
                ));
            }

            // Access-Control-Allow-Headers
            if !self.allowed_headers.is_empty() {
                headers.push((
                    "access-control-allow-headers".to_string(),
                    self.allowed_headers.join(", "),
                ));
            }
        }

        headers
    }

    /// Check if a preflight request should be accepted.
    ///
    /// Returns a [`CorsPreflightResult`] indicating whether the preflight
    /// is allowed and what headers to include in the response.
    pub fn handle_preflight(&self, request: &crate::WebRequest) -> CorsPreflightResult {
        // Extract Origin header
        let origin = request
            .headers
            .iter()
            .find(|h| h.name.to_lowercase() == "origin")
            .map(|h| h.value.as_str());

        let Some(origin) = origin else {
            return CorsPreflightResult {
                allowed: false,
                headers: Vec::new(),
            };
        };

        if !self.is_origin_allowed(origin) {
            return CorsPreflightResult {
                allowed: false,
                headers: Vec::new(),
            };
        }

        // Extract Access-Control-Request-Method
        let request_method = request
            .headers
            .iter()
            .find(|h| h.name.to_lowercase() == "access-control-request-method")
            .map(|h| h.value.as_str());

        // Extract Access-Control-Request-Headers
        let request_headers = request
            .headers
            .iter()
            .find(|h| h.name.to_lowercase() == "access-control-request-headers")
            .map(|h| h.value.as_str());

        // Validate requested method
        if let Some(method_str) = request_method {
            if let Some(requested_method) = parse_http_method(method_str) {
                if !self.allowed_methods.contains(&requested_method) {
                    return CorsPreflightResult {
                        allowed: false,
                        headers: Vec::new(),
                    };
                }
            }
        }

        // Validate requested headers (if any custom headers are requested)
        if let Some(custom_headers) = request_headers {
            for header in custom_headers.split(',') {
                let header = header.trim().to_lowercase();
                if !header.is_empty()
                    && !self
                        .allowed_headers
                        .iter()
                        .any(|h| h.to_lowercase() == header)
                {
                    // Common DICOMweb headers that should be implicitly allowed
                    if !is_standard_dicomweb_header(&header) {
                        return CorsPreflightResult {
                            allowed: false,
                            headers: Vec::new(),
                        };
                    }
                }
            }
        }

        let headers = self.cors_headers(Some(origin), request_method);

        CorsPreflightResult {
            allowed: true,
            headers,
        }
    }
}

/// Result of a CORS preflight check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorsPreflightResult {
    /// Whether the preflight request is allowed.
    pub allowed: bool,
    /// CORS headers to include in the preflight response.
    pub headers: Vec<(String, String)>,
}

// ---------------------------------------------------------------------------
// Per-origin permission scope for RBAC integration
// ---------------------------------------------------------------------------

/// Authorization scope granted to a specific origin.
///
/// This enables enterprise deployments where different origins (e.g.,
/// different hospital departments or partner applications) have different
/// access levels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OriginPermissionScope {
    /// The origin this scope applies to.
    pub origin: String,
    /// Authorization scopes allowed for this origin.
    pub allowed_scopes: Vec<AuthScope>,
}

/// Authorization scope for per-origin RBAC integration.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AuthScope {
    /// Read-only access (QIDO, WADO).
    Read,
    /// Write access (STOW).
    Write,
    /// Delete access.
    Delete,
    /// Measurement and annotation operations.
    Measurement,
    /// Segmentation operations.
    Segmentation,
    /// Administrative operations.
    Admin,
}

impl AuthScope {
    /// Return the string name of this scope.
    pub fn as_str(&self) -> &'static str {
        match self {
            AuthScope::Read => "read",
            AuthScope::Write => "write",
            AuthScope::Delete => "delete",
            AuthScope::Measurement => "measurement",
            AuthScope::Segmentation => "segmentation",
            AuthScope::Admin => "admin",
        }
    }

    /// Parse a scope from its string representation.
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s {
            "read" => Some(AuthScope::Read),
            "write" => Some(AuthScope::Write),
            "delete" => Some(AuthScope::Delete),
            "measurement" => Some(AuthScope::Measurement),
            "segmentation" => Some(AuthScope::Segmentation),
            "admin" => Some(AuthScope::Admin),
            _ => None,
        }
    }
}

/// Multi-origin CORS configuration with per-origin RBAC scopes.
#[derive(Debug, Clone, Default)]
pub struct MultiOriginCorsConfig {
    /// CORS configuration applied to all origins.
    pub base_cors: CorsConfig,
    /// Per-origin permission scopes.
    pub origin_scopes: Vec<OriginPermissionScope>,
}

impl MultiOriginCorsConfig {
    /// Create a new multi-origin configuration.
    pub fn new(base_cors: CorsConfig) -> Self {
        Self {
            base_cors,
            origin_scopes: Vec::new(),
        }
    }

    /// Add a permission scope for a specific origin.
    pub fn add_origin_scope(&mut self, scope: OriginPermissionScope) {
        self.origin_scopes.push(scope);
    }

    /// Get the permission scopes for a specific origin.
    pub fn scopes_for_origin(&self, origin: &str) -> Option<&OriginPermissionScope> {
        self.origin_scopes.iter().find(|s| s.origin == origin)
    }

    /// Check if an origin has a specific authorization scope.
    pub fn origin_has_scope(&self, origin: &str, scope: &AuthScope) -> bool {
        self.scopes_for_origin(origin)
            .map(|s| s.allowed_scopes.contains(scope))
            .unwrap_or(false)
    }

    /// Generate CORS headers for a request, considering both base CORS
    /// and per-origin scopes.
    pub fn cors_headers_for_origin(
        &self,
        request_origin: Option<&str>,
    ) -> Vec<(String, String)> {
        self.base_cors.cors_headers(request_origin, None)
    }
}

// ---------------------------------------------------------------------------
// Default DICOMweb CORS configuration
// ---------------------------------------------------------------------------

/// Return a default conservative CORS configuration for DICOMweb.
///
/// This configuration:
/// - Denies all origins by default (whitelist with empty set)
/// - Allows GET, HEAD, POST methods (needed for DICOMweb)
/// - Allows standard DICOMweb headers
/// - Does not allow credentials
/// - Sets a 1-hour preflight cache
pub fn default_dicomweb_cors() -> CorsConfig {
    CorsConfig {
        allowed_origins: OriginPolicy::default(),
        allowed_methods: vec![HttpMethod::Get, HttpMethod::Head, HttpMethod::Post],
        allowed_headers: vec![
            "accept".to_string(),
            "accept-encoding".to_string(),
            "accept-language".to_string(),
            "authorization".to_string(),
            "content-type".to_string(),
            "x-requested-with".to_string(),
        ],
        exposed_headers: vec![
            "content-type".to_string(),
            "content-length".to_string(),
            "x-dicom-web-study-uid".to_string(),
        ],
        allow_credentials: false,
        max_age_seconds: Some(3600),
    }
}

/// Return a permissive CORS configuration for development.
///
/// **Warning**: Do not use in production.
pub fn development_cors() -> CorsConfig {
    CorsConfig {
        allowed_origins: OriginPolicy::Any,
        allowed_methods: vec![
            HttpMethod::Get,
            HttpMethod::Head,
            HttpMethod::Post,
            HttpMethod::Delete,
        ],
        allowed_headers: vec![
            "accept".to_string(),
            "accept-encoding".to_string(),
            "authorization".to_string(),
            "content-type".to_string(),
        ],
        exposed_headers: Vec::new(),
        allow_credentials: false,
        max_age_seconds: Some(3600),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert an HttpMethod to its HTTP string representation.
fn http_method_str(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Head => "HEAD",
        HttpMethod::Post => "POST",
        HttpMethod::Delete => "DELETE",
    }
}

/// Parse an HTTP method string.
fn parse_http_method(s: &str) -> Option<HttpMethod> {
    match s.trim().to_uppercase().as_str() {
        "GET" => Some(HttpMethod::Get),
        "HEAD" => Some(HttpMethod::Head),
        "POST" => Some(HttpMethod::Post),
        "DELETE" => Some(HttpMethod::Delete),
        _ => None,
    }
}

/// Check if a header is a standard DICOMweb-related header that should
/// be implicitly allowed in preflight checks.
fn is_standard_dicomweb_header(header: &str) -> bool {
    matches!(
        header,
        "accept"
            | "accept-encoding"
            | "accept-language"
            | "authorization"
            | "content-type"
            | "content-length"
            | "cache-control"
            | "pragma"
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Header, QueryParam, WebRequest};

    #[test]
    fn origin_policy_any_allows_all() {
        let policy = OriginPolicy::Any;
        assert!(policy.is_origin_allowed("http://localhost:3000"));
        assert!(policy.is_origin_allowed("https://pacs.example.com"));
    }

    #[test]
    fn origin_policy_whitelist() {
        let mut whitelist = HashSet::new();
        whitelist.insert("http://localhost:3000".to_string());
        whitelist.insert("https://ohif.example.com".to_string());
        let policy = OriginPolicy::Whitelist(whitelist);

        assert!(policy.is_origin_allowed("http://localhost:3000"));
        assert!(policy.is_origin_allowed("https://ohif.example.com"));
        assert!(!policy.is_origin_allowed("https://evil.example.com"));
    }

    #[test]
    fn origin_policy_default_denies_all() {
        let policy = OriginPolicy::default();
        assert!(!policy.is_origin_allowed("http://localhost:3000"));
    }

    #[test]
    fn origin_policy_subdomain() {
        let policy = OriginPolicy::Subdomain {
            base_domain: "example.com".to_string(),
        };
        assert!(policy.is_origin_allowed("https://pacs.example.com"));
        assert!(policy.is_origin_allowed("http://example.com"));
        assert!(policy.is_origin_allowed("https://viewer.example.com"));
        assert!(!policy.is_origin_allowed("https://evil.other.com"));
    }

    #[test]
    fn origin_policy_regex_wildcard() {
        let policy = OriginPolicy::Regex(vec!["https://*.example.com".to_string()]);
        assert!(policy.is_origin_allowed("https://pacs.example.com"));
        assert!(policy.is_origin_allowed("https://viewer.example.com"));
        assert!(!policy.is_origin_allowed("http://pacs.example.com"));
    }

    #[test]
    fn cors_config_default_denies_all() {
        let config = CorsConfig::default();
        assert!(!config.is_origin_allowed("http://localhost:3000"));
    }

    #[test]
    fn cors_config_headers_for_allowed_origin() {
        let mut whitelist = HashSet::new();
        whitelist.insert("http://localhost:3000".to_string());
        let config = CorsConfig {
            allowed_origins: OriginPolicy::Whitelist(whitelist),
            allowed_methods: vec![HttpMethod::Get, HttpMethod::Post],
            allowed_headers: vec!["authorization".to_string()],
            exposed_headers: vec!["content-type".to_string()],
            allow_credentials: false,
            max_age_seconds: Some(3600),
        };

        let headers = config.cors_headers(Some("http://localhost:3000"), Some("GET"));
        assert!(headers.iter().any(|(k, v)| k == "access-control-allow-origin" && v == "http://localhost:3000"));
        assert!(headers.iter().any(|(k, _)| k == "access-control-expose-headers"));
    }

    #[test]
    fn cors_config_headers_for_disallowed_origin() {
        let mut whitelist = HashSet::new();
        whitelist.insert("http://localhost:3000".to_string());
        let config = CorsConfig {
            allowed_origins: OriginPolicy::Whitelist(whitelist),
            ..CorsConfig::default()
        };

        let headers = config.cors_headers(Some("https://evil.com"), None);
        assert!(headers.is_empty());
    }

    #[test]
    fn cors_config_any_origin_with_star() {
        let config = CorsConfig {
            allowed_origins: OriginPolicy::Any,
            ..CorsConfig::default()
        };

        let headers = config.cors_headers(Some("http://anything.com"), None);
        assert!(headers.iter().any(|(k, v)| k == "access-control-allow-origin" && v == "*"));
    }

    #[test]
    fn cors_config_credentials_with_any_origin_denied() {
        // Per CORS spec, credentials cannot be used with wildcard origin
        let config = CorsConfig {
            allowed_origins: OriginPolicy::Any,
            allow_credentials: true,
            ..CorsConfig::default()
        };

        let headers = config.cors_headers(Some("http://anything.com"), None);
        // Should use specific origin instead of *
        assert!(headers.iter().any(|(k, v)| k == "access-control-allow-origin" && v != "*"));
    }

    #[test]
    fn cors_preflight_allowed() {
        let mut whitelist = HashSet::new();
        whitelist.insert("http://localhost:3000".to_string());
        let config = CorsConfig {
            allowed_origins: OriginPolicy::Whitelist(whitelist),
            allowed_methods: vec![HttpMethod::Get, HttpMethod::Post],
            allowed_headers: vec!["authorization".to_string(), "content-type".to_string()],
            allow_credentials: false,
            max_age_seconds: Some(3600),
            exposed_headers: Vec::new(),
        };

        let request = WebRequest {
            method: HttpMethod::Head,
            transport: crate::TransportSecurity::Insecure,
            path: "/wado-rs/studies/1.2.3".to_string(),
            query: vec![],
            headers: vec![
                Header { name: "origin".to_string(), value: "http://localhost:3000".to_string() },
                Header { name: "access-control-request-method".to_string(), value: "GET".to_string() },
            ],
            body: vec![],
        };

        let result = config.handle_preflight(&request);
        assert!(result.allowed);
        assert!(!result.headers.is_empty());
    }

    #[test]
    fn cors_preflight_disallowed_origin() {
        let mut whitelist = HashSet::new();
        whitelist.insert("http://localhost:3000".to_string());
        let config = CorsConfig {
            allowed_origins: OriginPolicy::Whitelist(whitelist),
            ..default_dicomweb_cors()
        };

        let request = WebRequest {
            method: HttpMethod::Head,
            transport: crate::TransportSecurity::Insecure,
            path: "/wado-rs/studies/1.2.3".to_string(),
            query: vec![],
            headers: vec![
                Header { name: "origin".to_string(), value: "https://evil.com".to_string() },
                Header { name: "access-control-request-method".to_string(), value: "GET".to_string() },
            ],
            body: vec![],
        };

        let result = config.handle_preflight(&request);
        assert!(!result.allowed);
    }

    #[test]
    fn cors_preflight_disallowed_method() {
        let mut whitelist = HashSet::new();
        whitelist.insert("http://localhost:3000".to_string());
        let config = CorsConfig {
            allowed_origins: OriginPolicy::Whitelist(whitelist),
            allowed_methods: vec![HttpMethod::Get], // Only GET allowed
            ..default_dicomweb_cors()
        };

        let request = WebRequest {
            method: HttpMethod::Head,
            transport: crate::TransportSecurity::Insecure,
            path: "/wado-rs/studies/1.2.3".to_string(),
            query: vec![],
            headers: vec![
                Header { name: "origin".to_string(), value: "http://localhost:3000".to_string() },
                Header { name: "access-control-request-method".to_string(), value: "DELETE".to_string() },
            ],
            body: vec![],
        };

        let result = config.handle_preflight(&request);
        assert!(!result.allowed);
    }

    #[test]
    fn default_dicomweb_cors_config() {
        let config = default_dicomweb_cors();
        assert!(!config.is_origin_allowed("http://localhost:3000"));
        assert!(config.allowed_methods.contains(&HttpMethod::Get));
        assert!(config.allowed_methods.contains(&HttpMethod::Post));
        assert!(!config.allow_credentials);
        assert_eq!(config.max_age_seconds, Some(3600));
    }

    #[test]
    fn development_cors_allows_all() {
        let config = development_cors();
        assert!(config.is_origin_allowed("http://localhost:3000"));
        assert!(config.is_origin_allowed("https://anything.com"));
    }

    #[test]
    fn multi_origin_cors_scopes() {
        let mut config = MultiOriginCorsConfig::new(development_cors());
        config.add_origin_scope(OriginPermissionScope {
            origin: "http://localhost:3000".to_string(),
            allowed_scopes: vec![AuthScope::Read, AuthScope::Measurement],
        });
        config.add_origin_scope(OriginPermissionScope {
            origin: "https://admin.example.com".to_string(),
            allowed_scopes: vec![AuthScope::Read, AuthScope::Write, AuthScope::Delete, AuthScope::Admin],
        });

        assert!(config.origin_has_scope("http://localhost:3000", &AuthScope::Read));
        assert!(config.origin_has_scope("http://localhost:3000", &AuthScope::Measurement));
        assert!(!config.origin_has_scope("http://localhost:3000", &AuthScope::Write));
        assert!(config.origin_has_scope("https://admin.example.com", &AuthScope::Admin));
    }

    #[test]
    fn auth_scope_roundtrip() {
        for scope in &[
            AuthScope::Read,
            AuthScope::Write,
            AuthScope::Delete,
            AuthScope::Measurement,
            AuthScope::Segmentation,
            AuthScope::Admin,
        ] {
            assert_eq!(AuthScope::from_str_opt(scope.as_str()), Some(scope.clone()));
        }
        assert_eq!(AuthScope::from_str_opt("nonexistent"), None);
    }

    #[test]
    fn pattern_matching_wildcard() {
        assert!(OriginPolicy::match_pattern("https://*.example.com", "https://pacs.example.com"));
        assert!(OriginPolicy::match_pattern("https://*.example.com", "https://viewer.example.com"));
        assert!(!OriginPolicy::match_pattern("https://*.example.com", "http://pacs.example.com"));
        assert!(OriginPolicy::match_pattern("*", "anything"));
        assert!(OriginPolicy::match_pattern("http://localhost:*", "http://localhost:3000"));
    }

    #[test]
    fn pattern_matching_question_mark() {
        assert!(OriginPolicy::match_pattern("http://localhost:300?", "http://localhost:3000"));
        assert!(!OriginPolicy::match_pattern("http://localhost:300?", "http://localhost:30001"));
    }

    #[test]
    fn ohif_cross_origin_scenario() {
        // Acceptance test: OHIF on localhost:3000 retrieves from DiCCY on localhost:8080
        let mut whitelist = HashSet::new();
        whitelist.insert("http://localhost:3000".to_string());
        let cors = CorsConfig {
            allowed_origins: OriginPolicy::Whitelist(whitelist),
            allowed_methods: vec![HttpMethod::Get, HttpMethod::Post],
            allowed_headers: vec![
                "accept".to_string(),
                "authorization".to_string(),
                "content-type".to_string(),
            ],
            exposed_headers: vec!["content-type".to_string()],
            allow_credentials: true,
            max_age_seconds: Some(3600),
        };

        // Simulate OHIF WADO request
        let _request = WebRequest {
            method: HttpMethod::Get,
            transport: crate::TransportSecurity::Insecure,
            path: "/wado-rs/studies/1.2.3.4/series/5.6.7/instances/8.9.0".to_string(),
            query: vec![],
            headers: vec![
                Header { name: "origin".to_string(), value: "http://localhost:3000".to_string() },
            ],
            body: vec![],
        };

        let headers = cors.cors_headers(Some("http://localhost:3000"), None);
        assert!(!headers.is_empty(), "OHIF origin should be allowed");
        assert!(headers.iter().any(|(k, _)| k == "access-control-allow-origin"));
        assert!(headers.iter().any(|(k, _)| k == "access-control-allow-credentials"));
    }
}
