//! OpenAPI 3.1 specification generator for the DiCCY DICOMweb API.
//!
//! This crate auto-generates an OpenAPI 3.1 specification from the
//! DICOMweb route definitions, types, and authentication model.  The
//! generated spec covers all endpoints: QIDO-RS (search), WADO-RS
//! (retrieve), STOW-RS (store), and WADO-URI (legacy).
//!
//! # Quick start
//!
//! ```
//! use dicom_openapi::generate_dicomweb_openapi;
//!
//! let spec = generate_dicomweb_openapi();
//! let json = spec.to_json();
//! ```

#![deny(missing_docs)]

pub mod spec;
pub mod dicomweb_paths;
pub mod dicomweb_schemas;
pub mod security_schemes;

pub use spec::*;

use std::collections::BTreeMap;

/// Generate the complete DICOMweb OpenAPI 3.1 specification.
pub fn generate_dicomweb_openapi() -> OpenApiSpec {
    let paths = dicomweb_paths::build_dicomweb_paths();
    let schemas = dicomweb_schemas::build_dicomweb_schemas();
    let parameters = dicomweb_schemas::build_dicomweb_parameters();
    let security_schemes = security_schemes::build_security_schemes();
    let security = security_schemes::build_default_security();

    OpenApiSpec {
        openapi: "3.1.0".to_string(),
        info: Info {
            title: "DiCCY DICOMweb API".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: Some(
                "DiCCY PACS Workstation DICOMweb API. \
                 Implements QIDO-RS (search), WADO-RS (retrieve), STOW-RS (store), \
                 and WADO-URI (legacy retrieve) per DICOM PS3.18."
                    .to_string(),
            ),
            terms_of_service: None,
            contact: Some(Contact {
                name: Some("DiCCY Engineering".to_string()),
                email: Some("engineering@diccy.example".to_string()),
                url: Some("https://diccy.example".to_string()),
            }),
            license: Some(License {
                name: "Apache-2.0".to_string(),
                url: Some("https://www.apache.org/licenses/LICENSE-2.0".to_string()),
            }),
        },
        servers: vec![
            Server {
                url: "https://diccy.example/api".to_string(),
                description: Some("Production server".to_string()),
            },
            Server {
                url: "http://localhost:8080/api".to_string(),
                description: Some("Local development server".to_string()),
            },
        ],
        paths,
        components: Components {
            schemas,
            security_schemes,
            parameters,
        },
        security,
        external_docs: Some(ExternalDocs {
            url: "https://dicom.nema.org/medical/dicom/current/output/chtml/part18/".to_string(),
            description: Some("DICOM PS3.18 — DICOMweb".to_string()),
        }),
    }
}

impl OpenApiSpec {
    /// Serialise the specification to a JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|e| {
            format!("{{\"error\": \"serialization failed: {e}\"}}")
        })
    }

    /// Validate that the specification has the minimum required fields
    /// and that all paths contain at least one operation.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.openapi != "3.1.0" {
            errors.push(format!("invalid openapi version: {}", self.openapi));
        }

        if self.info.title.is_empty() {
            errors.push("info.title must not be empty".to_string());
        }

        if self.info.version.is_empty() {
            errors.push("info.version must not be empty".to_string());
        }

        if self.paths.is_empty() {
            errors.push("paths must not be empty".to_string());
        }

        for (path, item) in &self.paths {
            let has_operation = item.get.is_some()
                || item.post.is_some()
                || item.delete.is_some()
                || item.head.is_some()
                || item.options.is_some();
            if !has_operation {
                errors.push(format!("path '{path}' has no operations defined"));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_spec_validates() {
        let spec = generate_dicomweb_openapi();
        spec.validate().expect("generated spec should validate");
    }

    #[test]
    fn spec_has_all_dicomweb_paths() {
        let spec = generate_dicomweb_openapi();
        // QIDO
        assert!(spec.paths.contains_key("/studies"));
        assert!(spec.paths.contains_key("/series"));
        assert!(spec.paths.contains_key("/instances"));
        assert!(spec.paths.contains_key("/studies/{StudyUID}/series"));
        assert!(spec.paths.contains_key("/studies/{StudyUID}/instances"));
        assert!(
            spec.paths
                .contains_key("/studies/{StudyUID}/series/{SeriesUID}/instances")
        );

        // WADO
        assert!(spec.paths.contains_key("/studies/{StudyUID}/metadata"));
        assert!(
            spec.paths
                .contains_key("/studies/{StudyUID}/series/{SeriesUID}/metadata")
        );
        assert!(spec.paths.contains_key("/wado"));

        // Rendered / bulkdata
        assert!(
            spec.paths.contains_key(
                "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/rendered"
            )
        );
        assert!(
            spec.paths.contains_key(
                "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/bulkdata"
            )
        );

        // Frames
        assert!(spec.paths.contains_key(
            "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/frames/{FrameNumber}"
        ));
    }

    #[test]
    fn spec_includes_bearer_and_oauth2() {
        let spec = generate_dicomweb_openapi();
        assert!(spec.components.security_schemes.contains_key("BearerAuth"));
        assert!(spec.components.security_schemes.contains_key("OAuth2"));
    }

    #[test]
    fn spec_serializes_to_valid_json() {
        let spec = generate_dicomweb_openapi();
        let json = spec.to_json();
        // Must be valid JSON
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("spec JSON should be parseable");
        assert_eq!(parsed["openapi"], "3.1.0");
        assert!(parsed["paths"].is_object());
        assert!(parsed["components"]["schemas"].is_object());
    }

    #[test]
    fn spec_has_schemas_for_core_types() {
        let spec = generate_dicomweb_openapi();
        assert!(spec.components.schemas.contains_key("DicomDataset"));
        assert!(spec.components.schemas.contains_key("StowResponse"));
        assert!(spec.components.schemas.contains_key("Error"));
    }

    #[test]
    fn spec_has_reusable_parameters() {
        let spec = generate_dicomweb_openapi();
        assert!(spec.components.parameters.contains_key("PatientID"));
        assert!(spec.components.parameters.contains_key("Modality"));
        assert!(spec.components.parameters.contains_key("limit"));
    }

    #[test]
    fn spec_qido_studies_has_get_and_head() {
        let spec = generate_dicomweb_openapi();
        let studies = spec.paths.get("/studies").expect("/studies path");
        assert!(studies.get.is_some(), "/studies must have GET");
        assert!(studies.head.is_some(), "/studies must have HEAD");
    }

    #[test]
    fn spec_stow_studies_has_post() {
        let spec = generate_dicomweb_openapi();
        let studies = spec.paths.get("/studies").expect("/studies path");
        assert!(studies.post.is_some(), "/studies must have POST for STOW");
    }

    #[test]
    fn spec_wado_uri_is_deprecated() {
        let spec = generate_dicomweb_openapi();
        let wado = spec.paths.get("/wado").expect("/wado path");
        let get_op = wado.get.as_ref().expect("/wado must have GET");
        assert_eq!(get_op.deprecated, Some(true));
    }

    #[test]
    fn empty_spec_fails_validation() {
        let spec = OpenApiSpec {
            openapi: "3.1.0".to_string(),
            info: Info {
                title: String::new(),
                version: String::new(),
                description: None,
                terms_of_service: None,
                contact: None,
                license: None,
            },
            servers: vec![],
            paths: BTreeMap::new(),
            components: Components::default(),
            security: vec![],
            external_docs: None,
        };
        let result = spec.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("title")));
        assert!(errors.iter().any(|e| e.contains("version")));
        assert!(errors.iter().any(|e| e.contains("paths")));
    }
}
