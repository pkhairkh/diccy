//! Security scheme definitions (Bearer, OAuth2) for the DICOMweb API.

use crate::spec::*;
use std::collections::BTreeMap;

/// Build the security schemes section of the OpenAPI components.
pub fn build_security_schemes() -> BTreeMap<String, SecurityScheme> {
    let mut schemes = BTreeMap::new();

    schemes.insert(
        "BearerAuth".to_string(),
        SecurityScheme {
            scheme_type: "http".to_string(),
            scheme: Some("bearer".to_string()),
            bearer_format: Some("JWT".to_string()),
            description: Some(
                "JWT Bearer token issued by the DiCCY identity provider. \
                 The token must include the `dicom-web` scope."
                    .to_string(),
            ),
            flows: None,
            open_id_connect_url: None,
        },
    );

    schemes.insert(
        "OAuth2".to_string(),
        SecurityScheme {
            scheme_type: "oauth2".to_string(),
            scheme: None,
            bearer_format: None,
            description: Some("OAuth2 authorisation for DICOMweb access.".to_string()),
            flows: Some(OAuthFlows {
                authorization_code: Some(OAuthFlow {
                    authorization_url: Some(
                        "https://auth.diccy.example/oauth2/authorize".to_string(),
                    ),
                    token_url: Some("https://auth.diccy.example/oauth2/token".to_string()),
                    refresh_url: Some("https://auth.diccy.example/oauth2/refresh".to_string()),
                    scopes: {
                        let mut s = BTreeMap::new();
                        s.insert(
                            "dicom-web".to_string(),
                            "Read and write DICOMweb resources".to_string(),
                        );
                        s.insert(
                            "dicom-web.read".to_string(),
                            "Read-only DICOMweb access (QIDO, WADO)".to_string(),
                        );
                        s.insert(
                            "dicom-web.write".to_string(),
                            "Write DICOMweb access (STOW, DELETE)".to_string(),
                        );
                        s
                    },
                }),
                client_credentials: Some(OAuthFlow {
                    authorization_url: None,
                    token_url: Some("https://auth.diccy.example/oauth2/token".to_string()),
                    refresh_url: None,
                    scopes: {
                        let mut s = BTreeMap::new();
                        s.insert(
                            "dicom-web".to_string(),
                            "Machine-to-machine DICOMweb access".to_string(),
                        );
                        s
                    },
                }),
                implicit: None,
                password: None,
            }),
            open_id_connect_url: None,
        },
    );

    schemes
}

/// Build the default global security requirements.
pub fn build_default_security() -> Vec<SecurityRequirement> {
    vec![
        {
            let mut req = BTreeMap::new();
            req.insert("BearerAuth".to_string(), vec![]);
            req
        },
        {
            let mut req = BTreeMap::new();
            req.insert("OAuth2".to_string(), vec!["dicom-web".to_string()]);
            req
        },
    ]
}
