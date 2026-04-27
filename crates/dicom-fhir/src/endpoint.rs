//! FHIR Endpoint resource for DICOMweb service.
//!
//! Represents a FHIR R4 Endpoint resource that describes the technical
//! details of a DICOMweb service endpoint where imaging studies can
//! be retrieved.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Endpoint status values per FHIR R4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EndpointStatus {
    /// Endpoint is active and available.
    Active,
    /// Endpoint is temporarily suspended.
    Suspended,
    /// Endpoint is in an error state.
    Error,
    /// Endpoint is offline/disabled.
    Off,
}

impl fmt::Display for EndpointStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EndpointStatus::Active => write!(f, "active"),
            EndpointStatus::Suspended => write!(f, "suspended"),
            EndpointStatus::Error => write!(f, "error"),
            EndpointStatus::Off => write!(f, "off"),
        }
    }
}

/// Connection type codes for FHIR Endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionType {
    /// DICOMweb RESTful services.
    #[serde(rename = "dicom-web")]
    DicomWeb,
    /// HL7 FHIR protocol.
    #[serde(rename = "hl7-fhir-rest")]
    HL7Fhir,
    /// IHE XDS.b integration.
    #[serde(rename = "ihe-xds")]
    IheXds,
}

impl fmt::Display for ConnectionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionType::DicomWeb => write!(f, "dicom-web"),
            ConnectionType::HL7Fhir => write!(f, "hl7-fhir-rest"),
            ConnectionType::IheXds => write!(f, "ihe-xds"),
        }
    }
}

/// FHIR R4 Endpoint resource for DICOMweb service.
///
/// Represents a DICOMweb service endpoint where imaging studies
/// can be retrieved via WADO-RS, QIDO-RS, or STOW-RS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FhirEndpoint {
    /// Logical id of this artifact.
    pub id: String,
    /// Current status of the endpoint.
    pub status: EndpointStatus,
    /// Protocol/connection type.
    pub connection_type: ConnectionType,
    /// Human-readable name for the endpoint.
    pub name: String,
    /// Technical address (URL) of the endpoint.
    pub address: String,
    /// Supported MIME types for payload negotiation.
    pub payload_mime_types: Vec<String>,
}

impl FhirEndpoint {
    /// Create a new DICOMweb endpoint.
    ///
    /// Convenience constructor that sets the connection type to
    /// `DicomWeb` and the status to `Active`.
    pub fn new_dicomweb(id: &str, name: &str, address: &str) -> Self {
        Self {
            id: id.to_string(),
            status: EndpointStatus::Active,
            connection_type: ConnectionType::DicomWeb,
            name: name.to_string(),
            address: address.to_string(),
            payload_mime_types: vec![
                "application/dicom+json".to_string(),
                "application/dicom".to_string(),
            ],
        }
    }

    /// Return the FHIR resource type string.
    pub fn resource_type(&self) -> &str {
        "Endpoint"
    }

    /// Check if the endpoint is available (active status).
    pub fn is_available(&self) -> bool {
        self.status == EndpointStatus::Active
    }

    /// Serialize to JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Serialize to pretty-printed JSON string.
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_dicomweb_endpoint() {
        let ep = FhirEndpoint::new_dicomweb(
            "ep-1",
            "Test DICOMweb",
            "https://dicomweb.example.com/wado-rs",
        );
        assert_eq!(ep.id, "ep-1");
        assert_eq!(ep.status, EndpointStatus::Active);
        assert_eq!(ep.connection_type, ConnectionType::DicomWeb);
        assert!(ep.is_available());
    }

    #[test]
    fn endpoint_status_display() {
        assert_eq!(EndpointStatus::Active.to_string(), "active");
        assert_eq!(EndpointStatus::Suspended.to_string(), "suspended");
        assert_eq!(EndpointStatus::Error.to_string(), "error");
        assert_eq!(EndpointStatus::Off.to_string(), "off");
    }

    #[test]
    fn connection_type_display() {
        assert_eq!(ConnectionType::DicomWeb.to_string(), "dicom-web");
        assert_eq!(ConnectionType::HL7Fhir.to_string(), "hl7-fhir-rest");
        assert_eq!(ConnectionType::IheXds.to_string(), "ihe-xds");
    }

    #[test]
    fn endpoint_serialization() {
        let ep = FhirEndpoint::new_dicomweb(
            "ep-1",
            "Test DICOMweb",
            "https://dicomweb.example.com/wado-rs",
        );
        let json = ep.to_json().expect("json");
        assert!(json.contains("\"id\":\"ep-1\""));
        assert!(json.contains("active"));
    }

    #[test]
    fn endpoint_deserialization() {
        let ep = FhirEndpoint::new_dicomweb(
            "ep-1",
            "Test",
            "https://example.com",
        );
        let json = serde_json::to_string(&ep).expect("serialize");
        let restored: FhirEndpoint = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.id, ep.id);
        assert_eq!(restored.status, ep.status);
        assert_eq!(restored.connection_type, ep.connection_type);
    }

    #[test]
    fn endpoint_not_available_when_suspended() {
        let mut ep = FhirEndpoint::new_dicomweb("ep-1", "Test", "https://example.com");
        ep.status = EndpointStatus::Suspended;
        assert!(!ep.is_available());
    }

    #[test]
    fn endpoint_resource_type() {
        let ep = FhirEndpoint::new_dicomweb("ep-1", "Test", "https://example.com");
        assert_eq!(ep.resource_type(), "Endpoint");
    }
}
