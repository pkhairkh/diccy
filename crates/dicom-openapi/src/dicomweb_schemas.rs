//! DICOM data model schemas for the OpenAPI specification.
//!
//! Defines the JSON Schema objects that describe DICOM datasets, STOW
//! responses, and other data structures used in the DICOMweb API.

use crate::spec::*;
use std::collections::BTreeMap;

/// Build the schemas section of the OpenAPI components.
pub fn build_dicomweb_schemas() -> BTreeMap<String, Schema> {
    let mut schemas = BTreeMap::new();

    schemas.insert(
        "DicomDataset".to_string(),
        Schema {
            schema_type: Some("object".to_string()),
            description: Some(
                "A DICOM dataset serialised as DICOM JSON (application/dicom+json). \
                 Each key is a DICOM tag in the form ggggeeee; each value follows \
                 the DICOM JSON attribute object model."
                    .to_string(),
            ),
            properties: {
                let mut props = BTreeMap::new();
                props.insert(
                    "vr".to_string(),
                    Schema {
                        schema_type: Some("string".to_string()),
                        description: Some("Value Representation two-letter code".to_string()),
                        ..Default::default()
                    },
                );
                props
            },
            required: vec!["vr".to_string()],
            additional_properties: Some(Box::new(Schema {
                description: Some("DICOM attribute value (varies by VR)".to_string()),
                ..Default::default()
            })),
            ..Default::default()
        },
    );

    schemas.insert(
        "StowResponse".to_string(),
        Schema {
            schema_type: Some("object".to_string()),
            description: Some("STOW-RS store response".to_string()),
            properties: {
                let mut props = BTreeMap::new();
                props.insert(
                    "0002,0010".to_string(),
                    Schema {
                        schema_type: Some("string".to_string()),
                        description: Some("Transfer Syntax UID used for storage".to_string()),
                        ..Default::default()
                    },
                );
                props
            },
            additional_properties: Some(Box::new(Schema {
                ref_path: Some("#/components/schemas/DicomDataset".to_string()),
                ..Default::default()
            })),
            ..Default::default()
        },
    );

    schemas.insert(
        "MultipartDicomPayload".to_string(),
        Schema {
            schema_type: Some("string".to_string()),
            format: Some("binary".to_string()),
            description: Some(
                "Multipart/related payload with type=application/dicom. \
                 Each part is a DICOM Part 10 file."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    schemas.insert(
        "DicomJsonPayload".to_string(),
        Schema {
            schema_type: Some("array".to_string()),
            description: Some(
                "Array of DICOM datasets in DICOM JSON representation."
                    .to_string(),
            ),
            items: Some(Box::new(Schema {
                ref_path: Some("#/components/schemas/DicomDataset".to_string()),
                ..Default::default()
            })),
            ..Default::default()
        },
    );

    schemas.insert(
        "Error".to_string(),
        Schema {
            schema_type: Some("object".to_string()),
            description: Some("Structured error response".to_string()),
            properties: {
                let mut props = BTreeMap::new();
                props.insert(
                    "code".to_string(),
                    Schema {
                        schema_type: Some("string".to_string()),
                        description: Some("Stable error code".to_string()),
                        ..Default::default()
                    },
                );
                props.insert(
                    "message".to_string(),
                    Schema {
                        schema_type: Some("string".to_string()),
                        description: Some("Human-readable error message".to_string()),
                        ..Default::default()
                    },
                );
                props.insert(
                    "detail".to_string(),
                    Schema {
                        schema_type: Some("string".to_string()),
                        description: Some("Additional error detail".to_string()),
                        ..Default::default()
                    },
                );
                props
            },
            required: vec!["code".to_string(), "message".to_string()],
            ..Default::default()
        },
    );

    schemas
}

/// Build the reusable parameters section of the OpenAPI components.
pub fn build_dicomweb_parameters() -> BTreeMap<String, Parameter> {
    let mut params = BTreeMap::new();

    params.insert(
        "PatientID".to_string(),
        Parameter {
            name: "PatientID".to_string(),
            location: "query".to_string(),
            description: Some("Filter by Patient ID".to_string()),
            required: Some(false),
            schema: Some(Schema {
                schema_type: Some("string".to_string()),
                ..Default::default()
            }),
        },
    );

    params.insert(
        "StudyInstanceUID".to_string(),
        Parameter {
            name: "StudyInstanceUID".to_string(),
            location: "query".to_string(),
            description: Some("Filter by Study Instance UID".to_string()),
            required: Some(false),
            schema: Some(Schema {
                schema_type: Some("string".to_string()),
                ..Default::default()
            }),
        },
    );

    params.insert(
        "AccessionNumber".to_string(),
        Parameter {
            name: "AccessionNumber".to_string(),
            location: "query".to_string(),
            description: Some("Filter by Accession Number".to_string()),
            required: Some(false),
            schema: Some(Schema {
                schema_type: Some("string".to_string()),
                ..Default::default()
            }),
        },
    );

    params.insert(
        "Modality".to_string(),
        Parameter {
            name: "Modality".to_string(),
            location: "query".to_string(),
            description: Some("Filter by Modality (e.g. CT, MR, MG)".to_string()),
            required: Some(false),
            schema: Some(Schema {
                schema_type: Some("string".to_string()),
                enum_values: vec![
                    "CT".to_string(),
                    "MR".to_string(),
                    "MG".to_string(),
                    "US".to_string(),
                    "XR".to_string(),
                    "NM".to_string(),
                    "PT".to_string(),
                    "XA".to_string(),
                    "SR".to_string(),
                ],
                ..Default::default()
            }),
        },
    );

    params.insert(
        "limit".to_string(),
        Parameter {
            name: "limit".to_string(),
            location: "query".to_string(),
            description: Some("Maximum number of results to return".to_string()),
            required: Some(false),
            schema: Some(Schema {
                schema_type: Some("integer".to_string()),
                format: Some("int32".to_string()),
                ..Default::default()
            }),
        },
    );

    params.insert(
        "offset".to_string(),
        Parameter {
            name: "offset".to_string(),
            location: "query".to_string(),
            description: Some("Zero-based offset into the result set".to_string()),
            required: Some(false),
            schema: Some(Schema {
                schema_type: Some("integer".to_string()),
                format: Some("int32".to_string()),
                ..Default::default()
            }),
        },
    );

    params
}
