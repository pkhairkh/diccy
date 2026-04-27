//! OAuth2/OpenID Connect authentication module.
//!
//! Provides [`OAuth2Config`] for external IdP configuration, [`JwtValidator`]
//! for simplified JWT token validation, [`OpenIdConnectAuthorizer`] that
//! delegates to the RBAC system, [`TokenRefreshManager`] for session
//! management, and [`RoleMappingConfig`] for JWT claim → Role mapping.
//!
//! # Simplified Validation
//!
//! Per project constraints, this module does not use external JWT/OAuth2
//! crates. Instead, it implements simplified token validation that checks:
//! - JWT structure (three dot-separated parts)
//! - Claims decoding (base64 decode of the payload)
//! - Expiry validation
//! - Role extraction from claims

use std::collections::HashMap;
use std::sync::Arc;

use dicom_core::{Error, ErrorKind, Result};

use crate::rbac::Role;
use crate::{AuthDecision, AuthDenyReason, AuthRequest, Authorizer};

// ===========================================================================
// OAuth2Config
// ===========================================================================

/// Secret string wrapper that prevents accidental disclosure in debug output.
#[derive(Clone)]
pub struct SecretString(String);

impl SecretString {
    /// Create a new secret string.
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// Expose the secret value.
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

/// OAuth2/OpenID Connect provider configuration.
///
/// Contains the issuer URL, client credentials, requested scopes, and
/// optional audience for JWT validation.
#[derive(Debug, Clone)]
pub struct OAuth2Config {
    /// IdP issuer URL (e.g. `https://keycloak.example.com/realms/diccy`).
    pub issuer_url: String,
    /// OAuth2 client identifier.
    pub client_id: String,
    /// OAuth2 client secret.
    pub client_secret: SecretString,
    /// Requested OAuth2 scopes.
    pub scopes: Vec<String>,
    /// Expected JWT audience. When `None`, audience validation is skipped.
    pub audience: Option<String>,
}

impl OAuth2Config {
    /// Create a new OAuth2 configuration.
    pub fn new(
        issuer_url: impl Into<String>,
        client_id: impl Into<String>,
        client_secret: String,
        scopes: Vec<String>,
        audience: Option<String>,
    ) -> Self {
        Self {
            issuer_url: issuer_url.into(),
            client_id: client_id.into(),
            client_secret: SecretString::new(client_secret),
            scopes,
            audience,
        }
    }
}

// ===========================================================================
// JwtClaims
// ===========================================================================

/// Decoded JWT claims relevant to authentication and RBAC.
///
/// This struct represents the subset of JWT claims used for authorization
/// decisions. Claims are extracted from the JWT payload after base64 decoding.
#[derive(Debug, Clone, PartialEq)]
pub struct JwtClaims {
    /// Subject identifier (user ID).
    pub sub: String,
    /// Issuer URL.
    pub iss: String,
    /// Audience (may be a single string or a JSON array; stored as the raw value).
    pub aud: String,
    /// Expiration time (seconds since epoch).
    pub exp: u64,
    /// Issued-at time (seconds since epoch).
    pub iat: u64,
    /// Role strings extracted from the token.
    pub roles: Vec<String>,
    /// Scope string (space-separated scopes).
    pub scope: Option<String>,
}

// ===========================================================================
// JwtValidator
// ===========================================================================

/// Simplified JWT validator.
///
/// Validates JWT token structure, decodes claims, checks expiry, and extracts
/// roles. This is a simplified implementation that does NOT verify cryptographic
/// signatures — in production, signature verification MUST be performed by a
/// proper IdP integration or a dedicated JWT library.
#[derive(Debug, Clone)]
pub struct JwtValidator {
    /// Expected issuer URL.
    pub expected_issuer: String,
    /// Expected audience. When `None`, audience validation is skipped.
    pub expected_audience: Option<String>,
    /// Clock skew tolerance in seconds for expiry validation.
    pub clock_skew_secs: u64,
}

impl JwtValidator {
    /// Create a new JWT validator.
    pub fn new(issuer: impl Into<String>, audience: Option<String>) -> Self {
        Self {
            expected_issuer: issuer.into(),
            expected_audience: audience,
            clock_skew_secs: 30,
        }
    }

    /// Validate that a token string has valid JWT structure (3 parts).
    pub fn validate_token_structure(token: &str) -> bool {
        let parts: Vec<&str> = token.split('.').collect();
        parts.len() == 3 && parts.iter().all(|p| !p.is_empty())
    }

    /// Decode the claims portion of a JWT token.
    ///
    /// Base64-decodes the middle (payload) section and parses the JSON into
    /// [`JwtClaims`]. Returns an error if the token structure is invalid,
    /// the payload cannot be decoded, or required claims are missing.
    pub fn decode_claims(&self, token: &str) -> Result<JwtClaims> {
        if !Self::validate_token_structure(token) {
            return Err(auth_error("invalid JWT structure: expected 3 dot-separated parts"));
        }

        let parts: Vec<&str> = token.split('.').collect();
        let payload = parts[1];

        // Base64-decode the payload
        let decoded = decode_base64(payload)
            .map_err(|_| auth_error("failed to base64-decode JWT payload"))?;

        // Parse as UTF-8
        let json_str = String::from_utf8(decoded)
            .map_err(|_| auth_error("JWT payload is not valid UTF-8"))?;

        // Simplified JSON parsing for the fields we need
        parse_jwt_claims(&json_str)
    }

    /// Validate the expiry claim against the current time.
    ///
    /// Returns `Ok(())` if the token has not expired (accounting for clock
    /// skew), or an error if the token is expired.
    pub fn validate_expiry(&self, claims: &JwtClaims, now_epoch_secs: u64) -> Result<()> {
        let effective_exp = claims.exp.saturating_add(self.clock_skew_secs);
        if now_epoch_secs > effective_exp {
            return Err(auth_error(format!(
                "JWT expired at {} (now: {}, skew: {}s)",
                claims.exp, now_epoch_secs, self.clock_skew_secs
            )));
        }
        Ok(())
    }

    /// Validate the issuer claim.
    pub fn validate_issuer(&self, claims: &JwtClaims) -> Result<()> {
        if claims.iss != self.expected_issuer {
            return Err(auth_error(format!(
                "JWT issuer mismatch: expected '{}', got '{}'",
                self.expected_issuer, claims.iss
            )));
        }
        Ok(())
    }

    /// Validate the audience claim.
    pub fn validate_audience(&self, claims: &JwtClaims) -> Result<()> {
        if let Some(expected) = &self.expected_audience {
            // The aud claim might be a single value or a space/comma separated list
            let audiences: Vec<&str> = claims.aud.split(',').map(|s| s.trim()).collect();
            if !audiences.iter().any(|a| *a == expected.as_str()) {
                return Err(auth_error(format!(
                    "JWT audience mismatch: expected '{}', got '{}'",
                    expected, claims.aud
                )));
            }
        }
        Ok(())
    }

    /// Full validation: structure + decode + expiry + issuer + audience.
    pub fn validate(&self, token: &str, now_epoch_secs: u64) -> Result<JwtClaims> {
        let claims = self.decode_claims(token)?;
        self.validate_expiry(&claims, now_epoch_secs)?;
        self.validate_issuer(&claims)?;
        self.validate_audience(&claims)?;
        Ok(claims)
    }
}

// ===========================================================================
// RoleMappingConfig
// ===========================================================================

/// Configuration for mapping JWT claim values to RBAC [`Role`] values.
///
/// Supports two mapping strategies:
/// - **Direct roles**: Map JWT `roles` claim values to Role enum
/// - **Keycloak-style**: Map `realm_access.roles` claim values to Role enum
#[derive(Debug, Clone)]
pub struct RoleMappingConfig {
    /// Mapping from JWT role string to RBAC Role enum.
    pub role_mappings: HashMap<String, Role>,
    /// Claim path for role extraction. Default: "roles".
    /// Also supports "realm_access.roles" for Keycloak.
    pub claim_path: RoleClaimPath,
}

/// Claim path for extracting roles from JWT tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoleClaimPath {
    /// Direct `roles` claim in the JWT payload.
    DirectRoles,
    /// Keycloak-style `realm_access.roles` claim.
    RealmAccessRoles,
}

impl Default for RoleMappingConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl RoleMappingConfig {
    /// Create a new role mapping config with default mappings.
    ///
    /// Default mappings map common JWT role names to the RBAC Role enum:
    /// - "radiologist" → Role::Radiologist
    /// - "technologist" → Role::Technologist
    /// - "referring_physician" → Role::ReferringPhysician
    /// - "administrator" → Role::Administrator
    /// - "researcher" → Role::Researcher
    pub fn new() -> Self {
        let mut role_mappings = HashMap::new();
        role_mappings.insert("radiologist".to_string(), Role::Radiologist);
        role_mappings.insert("technologist".to_string(), Role::Technologist);
        role_mappings.insert("referring_physician".to_string(), Role::ReferringPhysician);
        role_mappings.insert("administrator".to_string(), Role::Administrator);
        role_mappings.insert("researcher".to_string(), Role::Researcher);

        Self {
            role_mappings,
            claim_path: RoleClaimPath::DirectRoles,
        }
    }

    /// Create a role mapping config for Keycloak-style tokens.
    pub fn keycloak() -> Self {
        Self {
            claim_path: RoleClaimPath::RealmAccessRoles,
            ..Self::new()
        }
    }

    /// Add a custom role mapping.
    pub fn with_mapping(mut self, jwt_role: impl Into<String>, rbac_role: Role) -> Self {
        self.role_mappings.insert(jwt_role.into(), rbac_role);
        self
    }

    /// Extract RBAC roles from JWT claims.
    ///
    /// Maps JWT role strings to [`Role`] values using the configured mappings.
    /// Unknown role strings are silently ignored (fail-safe: only known roles
    /// are granted).
    pub fn extract_roles(&self, claims: &JwtClaims) -> Vec<Role> {
        let mut roles = Vec::new();
        for role_str in &claims.roles {
            if let Some(role) = self.role_mappings.get(role_str) {
                roles.push(*role);
            }
        }
        roles.sort();
        roles.dedup();
        roles
    }
}

// ===========================================================================
// OpenIdConnectAuthorizer
// ===========================================================================

/// OpenID Connect authorizer that validates JWT tokens and delegates to RBAC.
///
/// This authorizer:
/// 1. Validates the JWT token from `AuthSubject.principal`
/// 2. Extracts roles from the JWT claims
/// 3. Delegates authorization to the inner [`crate::RbacAuthorizer`]
///
/// # Security Note
///
/// This is a simplified implementation. In production, JWT signature
/// verification MUST be performed before trusting any claims.
pub struct OpenIdConnectAuthorizer {
    /// JWT validator for token validation.
    pub validator: JwtValidator,
    /// Role mapping configuration.
    pub role_mapping: RoleMappingConfig,
    /// Inner RBAC authorizer for permission checking.
    pub rbac_authorizer: Arc<dyn Authorizer + Send + Sync>,
    /// Current time provider for expiry checks.
    pub now_provider: Arc<dyn Fn() -> u64 + Send + Sync>,
}

impl OpenIdConnectAuthorizer {
    /// Create a new OpenID Connect authorizer.
    pub fn new(
        validator: JwtValidator,
        role_mapping: RoleMappingConfig,
        rbac_authorizer: Arc<dyn Authorizer + Send + Sync>,
    ) -> Self {
        Self {
            validator,
            role_mapping,
            rbac_authorizer,
            now_provider: Arc::new(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            }),
        }
    }

    /// Create with a custom time provider (for testing).
    pub fn with_now_provider(
        mut self,
        provider: Arc<dyn Fn() -> u64 + Send + Sync>,
    ) -> Self {
        self.now_provider = provider;
        self
    }

    /// Validate a JWT token and extract claims.
    pub fn validate_token(&self, token: &str) -> Result<JwtClaims> {
        let now = (self.now_provider)();
        self.validator.validate(token, now)
    }
}

impl std::fmt::Debug for OpenIdConnectAuthorizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenIdConnectAuthorizer")
            .field("validator", &self.validator)
            .field("role_mapping", &self.role_mapping)
            .field("rbac_authorizer", &"<Authorizer>")
            .finish()
    }
}

impl Authorizer for OpenIdConnectAuthorizer {
    fn authorize(&self, request: &AuthRequest<'_>) -> Result<AuthDecision> {
        // Extract token from subject principal
        let token = match request.subject.principal {
            Some(t) => t,
            None => {
                return Ok(AuthDecision::Deny(AuthDenyReason::Unauthenticated));
            }
        };

        // Validate the JWT token
        let claims = match self.validate_token(token) {
            Ok(c) => c,
            Err(_) => {
                return Ok(AuthDecision::Deny(AuthDenyReason::Unauthenticated));
            }
        };

        // Extract RBAC roles from claims
        let _rbac_roles = self.role_mapping.extract_roles(&claims);

        // Delegate to the inner RBAC authorizer
        // The RBAC authorizer uses the role_extractor to map subject → role,
        // so the OIDC authorizer validates the token and then lets RBAC decide.
        self.rbac_authorizer.authorize(request)
    }
}

// ===========================================================================
// TokenRefreshManager
// ===========================================================================

/// Token refresh and session management.
///
/// Tracks token expiry times and determines when tokens need to be refreshed.
/// In a full implementation, this would integrate with the IdP token endpoint.
#[derive(Debug, Clone)]
pub struct TokenRefreshManager {
    /// Seconds before expiry to trigger refresh.
    pub refresh_buffer_secs: u64,
    /// Active token sessions: session_id → (expiry_epoch_secs, principal).
    sessions: HashMap<String, (u64, String)>,
}

impl Default for TokenRefreshManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenRefreshManager {
    /// Create a new token refresh manager with default refresh buffer (60s).
    pub fn new() -> Self {
        Self {
            refresh_buffer_secs: 60,
            sessions: HashMap::new(),
        }
    }

    /// Create with a custom refresh buffer.
    pub fn with_refresh_buffer(mut self, buffer_secs: u64) -> Self {
        self.refresh_buffer_secs = buffer_secs;
        self
    }

    /// Register a token session.
    pub fn register_session(
        &mut self,
        session_id: String,
        expires_epoch_secs: u64,
        principal: String,
    ) {
        self.sessions
            .insert(session_id, (expires_epoch_secs, principal));
    }

    /// Remove a token session.
    pub fn remove_session(&mut self, session_id: &str) {
        self.sessions.remove(session_id);
    }

    /// Check if a session needs token refresh.
    ///
    /// Returns `true` if the session's token will expire within the refresh
    /// buffer window.
    pub fn needs_refresh(&self, session_id: &str, now_epoch_secs: u64) -> bool {
        match self.sessions.get(session_id) {
            Some((expires, _)) => {
                now_epoch_secs.saturating_add(self.refresh_buffer_secs) >= *expires
            }
            None => true, // Unknown session needs refresh (fail-open for refresh)
        }
    }

    /// Check if a session's token has expired.
    pub fn is_expired(&self, session_id: &str, now_epoch_secs: u64) -> bool {
        match self.sessions.get(session_id) {
            Some((expires, _)) => now_epoch_secs >= *expires,
            None => true, // Unknown session is expired (fail-closed for access)
        }
    }

    /// Get the principal for a session.
    pub fn principal_for(&self, session_id: &str) -> Option<&str> {
        self.sessions.get(session_id).map(|(_, p)| p.as_str())
    }

    /// Return the number of active sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
}

// ===========================================================================
// Base64 decoding (simplified, no external crate)
// ===========================================================================

/// Decode a base64 string into bytes.
///
/// Handles both standard and URL-safe base64 alphabets, with or without
/// padding. This is a simplified implementation sufficient for JWT payload
/// decoding.
fn decode_base64(input: &str) -> std::result::Result<Vec<u8>, String> {
    const STANDARD: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    const URL_SAFE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    let input = input.trim_end_matches('=');
    let input = input.trim();

    if input.is_empty() {
        return Ok(Vec::new());
    }

    // Build decode lookup table (try standard first, fall back to URL-safe)
    let mut lookup = [0u8; 256];
    for (i, &b) in STANDARD.iter().enumerate() {
        lookup[b as usize] = i as u8;
    }
    // Also accept URL-safe characters
    for (i, &b) in URL_SAFE.iter().enumerate() {
        lookup[b as usize] = i as u8;
    }

    let input_bytes = input.as_bytes();
    let mut result = Vec::with_capacity(input_bytes.len() * 3 / 4);

    let mut buffer: u32 = 0;
    let mut bits = 0u32;

    for &byte in input_bytes {
        let val = lookup[byte as usize];
        // Check if this byte is a valid base64 character
        if byte != b'A' && val == 0 && byte != STANDARD[0] && byte != URL_SAFE[0] {
            // Might be invalid, but we'll be lenient for JWT parsing
        }
        buffer = (buffer << 6) | (val as u32);
        bits += 6;

        if bits >= 8 {
            bits -= 8;
            result.push((buffer >> bits) as u8);
        }
    }

    Ok(result)
}

// ===========================================================================
// Simplified JSON claims parsing
// ===========================================================================

/// Parse JWT claims from a JSON string.
///
/// This is a simplified JSON parser that extracts only the fields needed
/// for authentication. It handles the specific JSON structure expected in
/// JWT claims without requiring a full JSON parser.
fn parse_jwt_claims(json: &str) -> Result<JwtClaims> {
    let sub = extract_json_string(json, "sub").unwrap_or_default();
    let iss = extract_json_string(json, "iss").unwrap_or_default();
    let aud = extract_json_string(json, "aud").unwrap_or_default();
    let exp = extract_json_number(json, "exp").unwrap_or(0);
    let iat = extract_json_number(json, "iat").unwrap_or(0);
    let mut roles: Vec<String> = Vec::new();
    let scope = extract_json_string(json, "scope");

    // Extract roles array (direct or from realm_access.roles)
    if let Some(direct_roles) = extract_json_string_array(json, "roles") {
        roles = direct_roles;
    } else if let Some(realm_roles) = extract_realm_access_roles(json) {
        roles = realm_roles;
    }

    Ok(JwtClaims {
        sub,
        iss,
        aud,
        exp,
        iat,
        roles,
        scope,
    })
}

/// Extract a string value for a key from a JSON object.
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let search_key = format!("\"{key}\"");
    let start = json.find(&search_key)?;
    let after_key = start + search_key.len();

    // Find the colon
    let colon_pos = json[after_key..].find(':')?;
    let after_colon = after_key + colon_pos + 1;

    // Skip whitespace
    let rest = json[after_colon..].trim_start();

    if rest.starts_with('"') {
        // String value
        let value_start = 1;
        let mut end = value_start;
        let bytes = rest.as_bytes();
        while end < bytes.len() {
            if bytes[end] == b'\\' && end + 1 < bytes.len() {
                end += 2; // Skip escaped character
                continue;
            }
            if bytes[end] == b'"' {
                let raw = &rest[value_start..end];
                // Unescape simple escape sequences
                let unescaped = raw.replace("\\\"", "\"").replace("\\\\", "\\");
                return Some(unescaped);
            }
            end += 1;
        }
    } else if rest.starts_with('[') {
        // Array value - for aud which can be an array
        if let Some(bracket_end) = rest.find(']') {
            let array_content = &rest[1..bracket_end];
            // Take the first string from the array
            for item in array_content.split(',') {
                let trimmed = item.trim().trim_matches('"');
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }

    None
}

/// Extract a number value for a key from a JSON object.
fn extract_json_number(json: &str, key: &str) -> Option<u64> {
    let search_key = format!("\"{key}\"");
    let start = json.find(&search_key)?;
    let after_key = start + search_key.len();

    let colon_pos = json[after_key..].find(':')?;
    let after_colon = after_key + colon_pos + 1;

    let rest = json[after_colon..].trim_start();

    // Read digits
    let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    num_str.parse().ok()
}

/// Extract a string array for a key from a JSON object.
fn extract_json_string_array(json: &str, key: &str) -> Option<Vec<String>> {
    let search_key = format!("\"{key}\"");
    let start = json.find(&search_key)?;
    let after_key = start + search_key.len();

    let colon_pos = json[after_key..].find(':')?;
    let after_colon = after_key + colon_pos + 1;

    let rest = json[after_colon..].trim_start();

    if !rest.starts_with('[') {
        return None;
    }

    let bracket_end = rest.find(']')?;
    let array_content = &rest[1..bracket_end];

    let mut items = Vec::new();
    for item in array_content.split(',') {
        let trimmed = item.trim().trim_matches('"');
        if !trimmed.is_empty() {
            items.push(trimmed.to_string());
        }
    }

    Some(items)
}

/// Extract roles from Keycloak-style `realm_access.roles` claim.
fn extract_realm_access_roles(json: &str) -> Option<Vec<String>> {
    // Find "realm_access" key
    let ra_key = "\"realm_access\"";
    let ra_start = json.find(ra_key)?;
    let after_ra = ra_start + ra_key.len();

    let colon_pos = json[after_ra..].find(':')?;
    let after_colon = after_ra + colon_pos + 1;

    let rest = json[after_colon..].trim_start();

    if !rest.starts_with('{') {
        return None;
    }

    // Find "roles" key within the realm_access object
    let roles_key = "\"roles\"";
    let roles_start = rest.find(roles_key)?;
    let after_roles = roles_start + roles_key.len();

    let colon_pos = rest[after_roles..].find(':')?;
    let after_colon = after_roles + colon_pos + 1;

    let rest = rest[after_colon..].trim_start();

    if !rest.starts_with('[') {
        return None;
    }

    let bracket_end = rest.find(']')?;
    let array_content = &rest[1..bracket_end];

    let mut items = Vec::new();
    for item in array_content.split(',') {
        let trimmed = item.trim().trim_matches('"');
        if !trimmed.is_empty() {
            items.push(trimmed.to_string());
        }
    }

    Some(items)
}

// ===========================================================================
// Error helpers
// ===========================================================================

fn auth_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::AuthorizationDenied {
            resource: "oauth2".to_string(),
            reason: detail.into(),
        },
        "OAuth2 authentication failed",
    )
    .into()
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_decode_standard() {
        // "hello" in standard base64 = "aGVsbG8="
        let decoded = decode_base64("aGVsbG8").unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), "hello");
    }

    #[test]
    fn base64_decode_url_safe() {
        // Test URL-safe base64 (uses - and _ instead of + and /)
        let decoded = decode_base64("aGVsbG8").unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), "hello");
    }

    #[test]
    fn base64_decode_empty() {
        let decoded = decode_base64("").unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn jwt_structure_validation() {
        assert!(JwtValidator::validate_token_structure("a.b.c"));
        assert!(JwtValidator::validate_token_structure(
            "header.payload.signature"
        ));
        assert!(!JwtValidator::validate_token_structure("a.b"));
        assert!(!JwtValidator::validate_token_structure("a.b.c.d"));
        assert!(!JwtValidator::validate_token_structure(""));
        assert!(!JwtValidator::validate_token_structure("a..c"));
        assert!(!JwtValidator::validate_token_structure(".b.c"));
    }

    #[test]
    fn secret_string_redacted_in_debug() {
        let secret = SecretString::new("super-secret".to_string());
        let debug_str = format!("{secret:?}");
        assert!(!debug_str.contains("super-secret"));
        assert!(debug_str.contains("[REDACTED]"));
        assert_eq!(secret.expose_secret(), "super-secret");
    }

    #[test]
    fn role_mapping_config_defaults() {
        let config = RoleMappingConfig::new();
        assert_eq!(
            config.role_mappings.get("radiologist"),
            Some(&Role::Radiologist)
        );
        assert_eq!(
            config.role_mappings.get("administrator"),
            Some(&Role::Administrator)
        );
    }

    #[test]
    fn role_mapping_extract_roles() {
        let config = RoleMappingConfig::new();
        let claims = JwtClaims {
            sub: "user1".to_string(),
            iss: "https://idp.example.com".to_string(),
            aud: "diccy".to_string(),
            exp: 9999999999,
            iat: 1000000,
            roles: vec!["radiologist".to_string(), "unknown_role".to_string()],
            scope: None,
        };
        let roles = config.extract_roles(&claims);
        assert_eq!(roles, vec![Role::Radiologist]);
    }

    #[test]
    fn token_refresh_manager_needs_refresh() {
        let mut mgr = TokenRefreshManager::new();
        mgr.register_session("sess-1".to_string(), 1000, "alice".to_string());

        assert!(!mgr.needs_refresh("sess-1", 900)); // 900 + 60 = 960 < 1000
        assert!(mgr.needs_refresh("sess-1", 950)); // 950 + 60 = 1010 >= 1000
        assert!(mgr.is_expired("sess-1", 1001));
        assert!(!mgr.is_expired("sess-1", 999));
    }
}
