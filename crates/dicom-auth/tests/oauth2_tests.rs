//! Integration tests for the S14-T3 OAuth2/OpenID Connect authentication module.

use dicom_auth::*;
use std::sync::Arc;

/// Simple base64 encoder for test purposes.

// ===========================================================================
// JWT structure validation tests
// ===========================================================================

#[test]
fn jwt_structure_valid() {
    assert!(oauth2::JwtValidator::validate_token_structure("a.b.c"));
    assert!(oauth2::JwtValidator::validate_token_structure(
        "header.payload.sig"
    ));
}

#[test]
fn jwt_structure_invalid_too_few_parts() {
    assert!(!oauth2::JwtValidator::validate_token_structure("a.b"));
    assert!(!oauth2::JwtValidator::validate_token_structure("a"));
    assert!(!oauth2::JwtValidator::validate_token_structure(""));
}

#[test]
fn jwt_structure_invalid_too_many_parts() {
    assert!(!oauth2::JwtValidator::validate_token_structure("a.b.c.d"));
}

#[test]
fn jwt_structure_invalid_empty_parts() {
    assert!(!oauth2::JwtValidator::validate_token_structure("a..c"));
    assert!(!oauth2::JwtValidator::validate_token_structure(".b.c"));
    assert!(!oauth2::JwtValidator::validate_token_structure("a.b."));
}

// ===========================================================================
// JWT claims decoding tests
// ===========================================================================

/// Helper to create a simple JWT with base64-encoded payload.
fn make_test_jwt(payload_json: &str) -> String {
    let header = b64_encode(r#"{"alg":"RS256","typ":"JWT"}"#);
    let payload = b64_encode(payload_json);
    let signature = b64_encode("fake-signature");
    format!("{header}.{payload}.{signature}")
}

/// Simple base64 encoder for test purposes.
fn b64_encode(input: &str) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    let mut result = String::new();
    let mut i = 0;

    while i < bytes.len() {
        // Read up to 3 bytes
        let b0 = bytes[i] as u32;
        let b1 = if i + 1 < bytes.len() { bytes[i + 1] as u32 } else { 0 };
        let b2 = if i + 2 < bytes.len() { bytes[i + 2] as u32 } else { 0 };

        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(CHARSET[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARSET[((triple >> 12) & 0x3F) as usize] as char);

        if i + 1 < bytes.len() {
            result.push(CHARSET[((triple >> 6) & 0x3F) as usize] as char);
        }
        if i + 2 < bytes.len() {
            result.push(CHARSET[(triple & 0x3F) as usize] as char);
        }

        i += 3;
    }

    // No padding for JWT compatibility
    result
}

#[test]
fn decode_claims_basic() {
    let payload = r#"{"sub":"user1","iss":"https://idp.example.com","aud":"diccy","exp":9999999999,"iat":1000000,"roles":["radiologist"]}"#;
    let token = make_test_jwt(payload);

    let validator = oauth2::JwtValidator::new("https://idp.example.com", Some("diccy".to_string()));
    let claims = validator.decode_claims(&token).expect("decode claims");

    assert_eq!(claims.sub, "user1");
    assert_eq!(claims.iss, "https://idp.example.com");
    assert_eq!(claims.aud, "diccy");
    assert_eq!(claims.exp, 9999999999);
    assert_eq!(claims.iat, 1000000);
    assert_eq!(claims.roles, vec!["radiologist"]);
}

#[test]
fn decode_claims_with_scope() {
    let payload = r#"{"sub":"user2","iss":"https://idp.example.com","aud":"diccy","exp":9999999999,"iat":1000000,"scope":"openid profile email"}"#;
    let token = make_test_jwt(payload);

    let validator = oauth2::JwtValidator::new("https://idp.example.com", None);
    let claims = validator.decode_claims(&token).expect("decode claims");

    assert_eq!(claims.sub, "user2");
    assert_eq!(claims.scope.as_deref(), Some("openid profile email"));
}

#[test]
fn decode_claims_invalid_structure() {
    let validator = oauth2::JwtValidator::new("https://idp.example.com", None);
    assert!(validator.decode_claims("not-a-jwt").is_err());
    assert!(validator.decode_claims("a.b").is_err());
}

// ===========================================================================
// Expiry validation tests
// ===========================================================================

#[test]
fn validate_expiry_not_expired() {
    let claims = oauth2::JwtClaims {
        sub: "user1".to_string(),
        iss: "https://idp.example.com".to_string(),
        aud: "diccy".to_string(),
        exp: 9999999999,
        iat: 1000000,
        roles: vec![],
        scope: None,
    };

    let validator = oauth2::JwtValidator::new("https://idp.example.com", None);
    assert!(validator.validate_expiry(&claims, 1000000).is_ok());
}

#[test]
fn validate_expired_token() {
    let claims = oauth2::JwtClaims {
        sub: "user1".to_string(),
        iss: "https://idp.example.com".to_string(),
        aud: "diccy".to_string(),
        exp: 1000, // Expired
        iat: 500,
        roles: vec![],
        scope: None,
    };

    let validator = oauth2::JwtValidator::new("https://idp.example.com", None);
    assert!(validator.validate_expiry(&claims, 2000).is_err());
}

#[test]
fn validate_expiry_with_clock_skew() {
    let claims = oauth2::JwtClaims {
        sub: "user1".to_string(),
        iss: "https://idp.example.com".to_string(),
        aud: "diccy".to_string(),
        exp: 1000,
        iat: 500,
        roles: vec![],
        scope: None,
    };

    let validator = oauth2::JwtValidator::new("https://idp.example.com", None);
    // Default clock skew is 30s, so exp=1000 is valid until 1030
    assert!(validator.validate_expiry(&claims, 1020).is_ok());
    assert!(validator.validate_expiry(&claims, 1040).is_err());
}

// ===========================================================================
// Role extraction from claims tests
// ===========================================================================

#[test]
fn role_mapping_extract_single_role() {
    let config = oauth2::RoleMappingConfig::new();
    let claims = oauth2::JwtClaims {
        sub: "user1".to_string(),
        iss: "https://idp.example.com".to_string(),
        aud: "diccy".to_string(),
        exp: 9999999999,
        iat: 1000000,
        roles: vec!["radiologist".to_string()],
        scope: None,
    };

    let roles = config.extract_roles(&claims);
    assert_eq!(roles, vec![Role::Radiologist]);
}

#[test]
fn role_mapping_extract_multiple_roles() {
    let config = oauth2::RoleMappingConfig::new();
    let claims = oauth2::JwtClaims {
        sub: "user1".to_string(),
        iss: "https://idp.example.com".to_string(),
        aud: "diccy".to_string(),
        exp: 9999999999,
        iat: 1000000,
        roles: vec!["radiologist".to_string(), "administrator".to_string()],
        scope: None,
    };

    let roles = config.extract_roles(&claims);
    assert!(roles.contains(&Role::Radiologist));
    assert!(roles.contains(&Role::Administrator));
}

#[test]
fn role_mapping_ignores_unknown_roles() {
    let config = oauth2::RoleMappingConfig::new();
    let claims = oauth2::JwtClaims {
        sub: "user1".to_string(),
        iss: "https://idp.example.com".to_string(),
        aud: "diccy".to_string(),
        exp: 9999999999,
        iat: 1000000,
        roles: vec!["unknown_role".to_string(), "radiologist".to_string()],
        scope: None,
    };

    let roles = config.extract_roles(&claims);
    assert_eq!(roles, vec![Role::Radiologist]);
}

#[test]
fn role_mapping_custom_mapping() {
    let config = oauth2::RoleMappingConfig::new()
        .with_mapping("custom_rad", Role::Radiologist);

    let claims = oauth2::JwtClaims {
        sub: "user1".to_string(),
        iss: "https://idp.example.com".to_string(),
        aud: "diccy".to_string(),
        exp: 9999999999,
        iat: 1000000,
        roles: vec!["custom_rad".to_string()],
        scope: None,
    };

    let roles = config.extract_roles(&claims);
    assert_eq!(roles, vec![Role::Radiologist]);
}

// ===========================================================================
// OpenIdConnectAuthorizer tests
// ===========================================================================

#[test]
fn openid_connect_authorizer_valid_token_delegates_to_rbac() {
    let rbac = RbacAuthorizer::new()
        .with_rbac_policy(RbacPolicy::new())
        .with_role_extractor(Arc::new(|_subject| Some(Role::Radiologist)));

    let validator = oauth2::JwtValidator::new("https://idp.example.com", Some("diccy".to_string()));
    let role_mapping = oauth2::RoleMappingConfig::new();

    let oidc = oauth2::OpenIdConnectAuthorizer::new(
        validator,
        role_mapping,
        Arc::new(rbac),
    )
    .with_now_provider(Arc::new(|| 1000000)); // Fixed time for testing

    let payload = r#"{"sub":"user1","iss":"https://idp.example.com","aud":"diccy","exp":9999999999,"iat":1000000,"roles":["radiologist"]}"#;
    let token = make_test_jwt(payload);

    // Radiologist can query (ReadStudy)
    let request = AuthRequest {
        scope: AuthScope::Viewer,
        action: AuthAction::Query,
        subject: AuthSubject {
            principal: Some(&token),
            peer: None,
        },
        resource: AuthResource::none(),
    };

    let decision = oidc.authorize(&request).expect("decision");
    assert!(decision.is_allowed());
}

#[test]
fn openid_connect_authorizer_no_principal_denies() {
    let rbac = RbacAuthorizer::new()
        .with_rbac_policy(RbacPolicy::new())
        .with_role_extractor(Arc::new(|_subject| Some(Role::Radiologist)));

    let validator = oauth2::JwtValidator::new("https://idp.example.com", None);
    let role_mapping = oauth2::RoleMappingConfig::new();

    let oidc = oauth2::OpenIdConnectAuthorizer::new(
        validator,
        role_mapping,
        Arc::new(rbac),
    );

    let request = AuthRequest {
        scope: AuthScope::Viewer,
        action: AuthAction::Query,
        subject: AuthSubject::anonymous(),
        resource: AuthResource::none(),
    };

    let decision = oidc.authorize(&request).expect("decision");
    assert!(!decision.is_allowed());
    assert!(matches!(
        decision,
        AuthDecision::Deny(AuthDenyReason::Unauthenticated)
    ));
}

#[test]
fn openid_connect_authorizer_invalid_token_denies() {
    let rbac = RbacAuthorizer::new()
        .with_rbac_policy(RbacPolicy::new())
        .with_role_extractor(Arc::new(|_subject| Some(Role::Radiologist)));

    let validator = oauth2::JwtValidator::new("https://idp.example.com", None);
    let role_mapping = oauth2::RoleMappingConfig::new();

    let oidc = oauth2::OpenIdConnectAuthorizer::new(
        validator,
        role_mapping,
        Arc::new(rbac),
    );

    let request = AuthRequest {
        scope: AuthScope::Viewer,
        action: AuthAction::Query,
        subject: AuthSubject {
            principal: Some("not-a-valid-jwt"),
            peer: None,
        },
        resource: AuthResource::none(),
    };

    let decision = oidc.authorize(&request).expect("decision");
    assert!(!decision.is_allowed());
}

// ===========================================================================
// RoleMappingConfig tests
// ===========================================================================

#[test]
fn role_mapping_config_default_has_five_roles() {
    let config = oauth2::RoleMappingConfig::new();
    assert_eq!(config.role_mappings.len(), 5);
}

#[test]
fn role_mapping_config_keycloak_style() {
    let config = oauth2::RoleMappingConfig::keycloak();
    assert_eq!(config.claim_path, oauth2::RoleClaimPath::RealmAccessRoles);
    // Should still have the standard mappings
    assert_eq!(
        config.role_mappings.get("radiologist"),
        Some(&Role::Radiologist)
    );
}

// ===========================================================================
// OAuth2Config tests
// ===========================================================================

#[test]
fn oauth2_config_creation() {
    let config = oauth2::OAuth2Config::new(
        "https://keycloak.example.com/realms/diccy",
        "diccy-client",
        "super-secret".to_string(),
        vec!["openid".to_string(), "profile".to_string()],
        Some("diccy-audience".to_string()),
    );

    assert_eq!(config.issuer_url, "https://keycloak.example.com/realms/diccy");
    assert_eq!(config.client_id, "diccy-client");
    assert_eq!(config.client_secret.expose_secret(), "super-secret");
    assert_eq!(config.scopes, vec!["openid", "profile"]);
    assert_eq!(config.audience.as_deref(), Some("diccy-audience"));
}

#[test]
fn oauth2_config_secret_redacted_in_debug() {
    let config = oauth2::OAuth2Config::new(
        "https://idp.example.com",
        "client",
        "my-secret".to_string(),
        vec![],
        None,
    );

    let debug_output = format!("{config:?}");
    assert!(!debug_output.contains("my-secret"));
    assert!(debug_output.contains("[REDACTED]"));
}

// ===========================================================================
// TokenRefreshManager tests
// ===========================================================================

#[test]
fn token_refresh_manager_basic_flow() {
    let mut mgr = oauth2::TokenRefreshManager::new();
    mgr.register_session("sess-1".to_string(), 1000, "alice".to_string());

    assert!(!mgr.is_expired("sess-1", 999));
    assert!(mgr.is_expired("sess-1", 1000));
    assert_eq!(mgr.principal_for("sess-1"), Some("alice"));
    assert_eq!(mgr.session_count(), 1);
}

#[test]
fn token_refresh_manager_needs_refresh() {
    let mut mgr = oauth2::TokenRefreshManager::new().with_refresh_buffer(60);
    mgr.register_session("sess-1".to_string(), 1000, "alice".to_string());

    // At t=939: 939 + 60 = 999 < 1000 → no refresh needed
    assert!(!mgr.needs_refresh("sess-1", 939));
    // At t=940: 940 + 60 = 1000 >= 1000 → refresh needed
    assert!(mgr.needs_refresh("sess-1", 940));
}

#[test]
fn token_refresh_manager_unknown_session() {
    let mgr = oauth2::TokenRefreshManager::new();

    // Unknown session: needs_refresh returns true (fail-open for refresh)
    assert!(mgr.needs_refresh("unknown", 0));
    // Unknown session: is_expired returns true (fail-closed for access)
    assert!(mgr.is_expired("unknown", 0));
    assert_eq!(mgr.principal_for("unknown"), None);
}

#[test]
fn token_refresh_manager_remove_session() {
    let mut mgr = oauth2::TokenRefreshManager::new();
    mgr.register_session("sess-1".to_string(), 1000, "alice".to_string());
    assert_eq!(mgr.session_count(), 1);

    mgr.remove_session("sess-1");
    assert_eq!(mgr.session_count(), 0);
    assert!(mgr.is_expired("sess-1", 0));
}

// ===========================================================================
// JwtValidator full validation tests
// ===========================================================================

#[test]
fn jwt_validator_full_validation_success() {
    let payload = r#"{"sub":"user1","iss":"https://idp.example.com","aud":"diccy","exp":9999999999,"iat":1000000,"roles":["radiologist"]}"#;
    let token = make_test_jwt(payload);

    let validator = oauth2::JwtValidator::new("https://idp.example.com", Some("diccy".to_string()));
    let claims = validator.validate(&token, 1000000).expect("validate");

    assert_eq!(claims.sub, "user1");
    assert_eq!(claims.roles, vec!["radiologist"]);
}

#[test]
fn jwt_validator_issuer_mismatch() {
    let payload = r#"{"sub":"user1","iss":"https://wrong-issuer.com","aud":"diccy","exp":9999999999,"iat":1000000}"#;
    let token = make_test_jwt(payload);

    let validator = oauth2::JwtValidator::new("https://idp.example.com", Some("diccy".to_string()));
    assert!(validator.validate(&token, 1000000).is_err());
}

#[test]
fn jwt_validator_audience_mismatch() {
    let payload = r#"{"sub":"user1","iss":"https://idp.example.com","aud":"wrong-audience","exp":9999999999,"iat":1000000}"#;
    let token = make_test_jwt(payload);

    let validator = oauth2::JwtValidator::new("https://idp.example.com", Some("diccy".to_string()));
    assert!(validator.validate(&token, 1000000).is_err());
}

#[test]
fn jwt_validator_expired() {
    let payload = r#"{"sub":"user1","iss":"https://idp.example.com","aud":"diccy","exp":1000,"iat":500}"#;
    let token = make_test_jwt(payload);

    let validator = oauth2::JwtValidator::new("https://idp.example.com", Some("diccy".to_string()));
    assert!(validator.validate(&token, 2000).is_err());
}

#[test]
fn jwt_validator_no_audience_check_when_none() {
    let payload = r#"{"sub":"user1","iss":"https://idp.example.com","aud":"any-audience","exp":9999999999,"iat":1000000}"#;
    let token = make_test_jwt(payload);

    let validator = oauth2::JwtValidator::new("https://idp.example.com", None);
    assert!(validator.validate(&token, 1000000).is_ok());
}
