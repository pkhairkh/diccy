//! Core OpenAPI 3.1 specification types.
//!
//! This module defines the Rust data model that maps directly to the
//! OpenAPI 3.1.0 JSON Schema. Every field uses `Option` wrappers so
//! that only explicitly-set fields appear in the serialised output.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Top-level document
// ---------------------------------------------------------------------------

/// OpenAPI 3.1 specification document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenApiSpec {
    /// OpenAPI version string (always `"3.1.0"`).
    pub openapi: String,
    /// API metadata.
    pub info: Info,
    /// Server endpoint list.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub servers: Vec<Server>,
    /// Path → path-item map.
    pub paths: BTreeMap<String, PathItem>,
    /// Reusable schemas and security schemes.
    #[serde(skip_serializing_if = "Components::is_empty")]
    pub components: Components,
    /// Global security requirements.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub security: Vec<SecurityRequirement>,
    /// External documentation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_docs: Option<ExternalDocs>,
}

// ---------------------------------------------------------------------------
// Info / metadata
// ---------------------------------------------------------------------------

/// API metadata (title, version, description, …).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Info {
    /// API title.
    pub title: String,
    /// API version.
    pub version: String,
    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Terms of service URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terms_of_service: Option<String>,
    /// Contact information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact: Option<Contact>,
    /// Licence information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<License>,
}

/// Contact information for the API.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Contact {
    /// Contact name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Contact email.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Contact URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// Licence information.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct License {
    /// Licence name.
    pub name: String,
    /// Licence URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

/// Server endpoint definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Server {
    /// Server URL (may contain `{variable}` placeholders).
    pub url: String,
    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

// ---------------------------------------------------------------------------
// Path item / operation
// ---------------------------------------------------------------------------

/// A path item containing one operation per HTTP method.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PathItem {
    /// Short summary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Detailed description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// GET operation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub get: Option<Operation>,
    /// POST operation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post: Option<Operation>,
    /// DELETE operation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delete: Option<Operation>,
    /// HEAD operation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head: Option<Operation>,
    /// OPTIONS operation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Operation>,
    /// Shared parameters for all operations on this path.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<ParameterOrRef>,
}

/// A single API operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Operation {
    /// Operation identifier (camelCase).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    /// Short summary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Detailed description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Tags for grouping.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Operation-specific parameters.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<ParameterOrRef>,
    /// Request body.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_body: Option<RequestBody>,
    /// Response map (status code → response).
    pub responses: BTreeMap<String, Response>,
    /// Security requirements for this operation.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub security: Vec<SecurityRequirement>,
    /// Whether the operation is deprecated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<bool>,
}

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

/// A parameter that is either inline or a `$ref`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ParameterOrRef {
    /// Inline parameter definition.
    Inline(Parameter),
    /// Reference to a component parameter.
    Ref(Ref),
}

/// An inline OpenAPI parameter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Parameter {
    /// Parameter name.
    pub name: String,
    /// Location of the parameter (`path`, `query`, `header`, `cookie`).
    #[serde(rename = "in")]
    pub location: String,
    /// Short description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether the parameter is required.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    /// Schema for the parameter value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<Schema>,
}

/// A `$ref` pointer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Ref {
    /// Reference path, e.g. `"#/components/parameters/StudyUID"`.
    #[serde(rename = "$ref")]
    pub ref_path: String,
}

// ---------------------------------------------------------------------------
// Request body
// ---------------------------------------------------------------------------

/// Request body definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RequestBody {
    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Media type → schema map.
    pub content: BTreeMap<String, MediaType>,
    /// Whether the request body is required.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
}

/// Media type object (content map value).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MediaType {
    /// Schema for the media type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<Schema>,
}

// ---------------------------------------------------------------------------
// Responses
// ---------------------------------------------------------------------------

/// A single response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Response {
    /// Human-readable description.
    pub description: String,
    /// Media type → schema map.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub content: BTreeMap<String, MediaType>,
}

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

/// An OpenAPI 3.1 / JSON Schema object.
///
/// Only the most commonly-used fields are modelled; additional
/// properties can be added via the `extra` map.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Schema {
    /// Schema `$ref` pointer.
    #[serde(rename = "$ref", skip_serializing_if = "Option::is_none")]
    pub ref_path: Option<String>,
    /// `type` keyword.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub schema_type: Option<String>,
    /// `format` keyword.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// `description` keyword.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// `enum` keyword.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub enum_values: Vec<String>,
    /// `items` schema (for array types).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<Schema>>,
    /// `properties` map (for object types).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, Schema>,
    /// `required` property list.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub required: Vec<String>,
    /// `additionalProperties` schema (for map types).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_properties: Option<Box<Schema>>,
    /// `allOf` composition.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub all_of: Vec<Schema>,
    /// `oneOf` composition.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub one_of: Vec<Schema>,
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Reusable components (schemas, security schemes, parameters).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Components {
    /// Named schemas.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub schemas: BTreeMap<String, Schema>,
    /// Named security schemes.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub security_schemes: BTreeMap<String, SecurityScheme>,
    /// Named parameters.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub parameters: BTreeMap<String, Parameter>,
}

impl Components {
    /// Return true if all component maps are empty.
    pub fn is_empty(&self) -> bool {
        self.schemas.is_empty() && self.security_schemes.is_empty() && self.parameters.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Security
// ---------------------------------------------------------------------------

/// A single security requirement (map of scheme name → scope list).
pub type SecurityRequirement = BTreeMap<String, Vec<String>>;

/// Security scheme definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SecurityScheme {
    /// Scheme type (`http`, `oauth2`, `openIdConnect`).
    #[serde(rename = "type")]
    pub scheme_type: String,
    /// HTTP authentication scheme (`bearer`, `basic`, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheme: Option<String>,
    /// Bearer token format hint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bearer_format: Option<String>,
    /// OAuth2 flow configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flows: Option<OAuthFlows>,
    /// OpenID Connect discovery URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_id_connect_url: Option<String>,
    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// OAuth2 flows.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OAuthFlows {
    /// Authorisation code flow.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_code: Option<OAuthFlow>,
    /// Client credentials flow.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_credentials: Option<OAuthFlow>,
    /// Implicit flow.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implicit: Option<OAuthFlow>,
    /// Resource owner password flow.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<OAuthFlow>,
}

/// A single OAuth2 flow.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OAuthFlow {
    /// Authorisation URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_url: Option<String>,
    /// Token URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_url: Option<String>,
    /// Refresh URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_url: Option<String>,
    /// Scope name → description map.
    pub scopes: BTreeMap<String, String>,
}

// ---------------------------------------------------------------------------
// External docs
// ---------------------------------------------------------------------------

/// External documentation link.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExternalDocs {
    /// Documentation URL.
    pub url: String,
    /// Description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}
