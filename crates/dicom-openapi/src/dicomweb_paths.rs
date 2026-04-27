//! DICOMweb endpoint definitions (QIDO-RS, WADO-RS, STOW-RS, WADO-URI).
//!
//! This module builds the `paths` section of the OpenAPI spec by iterating
//! over every `DicomWebRoute` variant and constructing the corresponding
//! OpenAPI path items and operations.

use crate::spec::*;
use std::collections::BTreeMap;

/// Build the full DICOMweb paths map with all route definitions.
pub fn build_dicomweb_paths() -> BTreeMap<String, PathItem> {
    let mut paths = BTreeMap::new();

    // --- QIDO-RS: /studies ---
    insert_qido_studies(&mut paths);
    insert_qido_all_series(&mut paths);
    insert_qido_all_instances(&mut paths);

    // --- QIDO-RS: scoped ---
    insert_qido_series_by_study(&mut paths);
    insert_qido_study_instances(&mut paths);
    insert_qido_instances_by_study_series(&mut paths);

    // --- STOW-RS ---
    insert_stow_studies(&mut paths);
    insert_stow_study_scoped(&mut paths);

    // --- WADO-RS ---
    insert_wado_study_retrieve(&mut paths);
    insert_wado_series_retrieve(&mut paths);
    insert_wado_instance_retrieve(&mut paths);
    insert_wado_study_metadata(&mut paths);
    insert_wado_series_metadata(&mut paths);
    insert_wado_instance_metadata(&mut paths);
    insert_wado_frame_retrieve(&mut paths);
    insert_wado_rendered(&mut paths);
    insert_wado_rendered_frame(&mut paths);
    insert_wado_bulkdata(&mut paths);

    // --- WADO-URI ---
    insert_wado_uri(&mut paths);

    // --- DELETE ---
    insert_delete_study(&mut paths);
    insert_delete_series(&mut paths);
    insert_delete_instance(&mut paths);

    paths
}

// Helper to get or create a path item.
fn ensure_path<'a>(paths: &'a mut BTreeMap<String, PathItem>, path: &str) -> &'a mut PathItem {
    paths.entry(path.to_string()).or_insert_with(|| PathItem {
        summary: None,
        description: None,
        get: None,
        post: None,
        delete: None,
        head: None,
        options: None,
        parameters: Vec::new(),
    })
}

fn uid_param(name: &str, desc: &str) -> Parameter {
    Parameter {
        name: name.to_string(),
        location: "path".to_string(),
        description: Some(desc.to_string()),
        required: Some(true),
        schema: Some(Schema {
            schema_type: Some("string".to_string()),
            format: Some("uri-component".to_string()),
            description: Some("DICOM UID".to_string()),
            ..Default::default()
        }),
    }
}

fn study_uid_param() -> Parameter {
    uid_param("StudyUID", "Study Instance UID")
}

fn series_uid_param() -> Parameter {
    uid_param("SeriesUID", "Series Instance UID")
}

fn instance_uid_param() -> Parameter {
    uid_param("InstanceUID", "SOP Instance UID")
}

fn frame_param() -> Parameter {
    Parameter {
        name: "FrameNumber".to_string(),
        location: "path".to_string(),
        description: Some("1-based frame number".to_string()),
        required: Some(true),
        schema: Some(Schema {
            schema_type: Some("integer".to_string()),
            format: Some("int32".to_string()),
            ..Default::default()
        }),
    }
}

fn qido_query_params() -> Vec<ParameterOrRef> {
    vec![
        ParameterOrRef::Ref(Ref {
            ref_path: "#/components/parameters/PatientID".to_string(),
        }),
        ParameterOrRef::Ref(Ref {
            ref_path: "#/components/parameters/StudyInstanceUID".to_string(),
        }),
        ParameterOrRef::Ref(Ref {
            ref_path: "#/components/parameters/AccessionNumber".to_string(),
        }),
        ParameterOrRef::Ref(Ref {
            ref_path: "#/components/parameters/Modality".to_string(),
        }),
        ParameterOrRef::Ref(Ref {
            ref_path: "#/components/parameters/limit".to_string(),
        }),
        ParameterOrRef::Ref(Ref {
            ref_path: "#/components/parameters/offset".to_string(),
        }),
    ]
}

fn success_json_response(desc: &str) -> BTreeMap<String, Response> {
    let mut m = BTreeMap::new();
    m.insert(
        "200".to_string(),
        Response {
            description: desc.to_string(),
            content: {
                let mut c = BTreeMap::new();
                c.insert(
                    "application/dicom+json".to_string(),
                    MediaType {
                        schema: Some(Schema {
                            schema_type: Some("array".to_string()),
                            items: Some(Box::new(Schema {
                                ref_path: Some("#/components/schemas/DicomDataset".to_string()),
                                ..Default::default()
                            })),
                            ..Default::default()
                        }),
                    },
                );
                c
            },
        },
    );
    m
}

fn error_responses() -> BTreeMap<String, Response> {
    let mut m = BTreeMap::new();
    m.insert(
        "400".to_string(),
        Response {
            description: "Bad request – invalid or malformed parameters".to_string(),
            content: BTreeMap::new(),
        },
    );
    m.insert(
        "403".to_string(),
        Response {
            description: "Forbidden – authentication or authorisation failure".to_string(),
            content: BTreeMap::new(),
        },
    );
    m.insert(
        "404".to_string(),
        Response {
            description: "Not found".to_string(),
            content: BTreeMap::new(),
        },
    );
    m.insert(
        "413".to_string(),
        Response {
            description: "Payload Too Large".to_string(),
            content: BTreeMap::new(),
        },
    );
    m
}

fn merged_responses(success: BTreeMap<String, Response>) -> BTreeMap<String, Response> {
    let mut m = success;
    m.extend(error_responses());
    m
}

// ---- QIDO-RS ----

fn insert_qido_studies(paths: &mut BTreeMap<String, PathItem>) {
    let path = ensure_path(paths, "/studies");
    path.summary = Some("QIDO-RS study search".to_string());
    path.get = Some(Operation {
        operation_id: Some("qidoStudiesGet".to_string()),
        summary: Some("Search for studies".to_string()),
        description: Some(
            "QIDO-RS: Search for DICOM studies matching the given query parameters.".to_string(),
        ),
        tags: vec!["QIDO-RS".to_string()],
        parameters: qido_query_params(),
        request_body: None,
        responses: merged_responses(success_json_response("Array of matching study datasets")),
        security: vec![],
        deprecated: None,
    });
    path.head = Some(Operation {
        operation_id: Some("qidoStudiesHead".to_string()),
        summary: Some("Search for studies (headers only)".to_string()),
        description: Some(
            "QIDO-RS: Return headers for a study search without response bodies.".to_string(),
        ),
        tags: vec!["QIDO-RS".to_string()],
        parameters: qido_query_params(),
        request_body: None,
        responses: merged_responses(BTreeMap::new()),
        security: vec![],
        deprecated: None,
    });
}

fn insert_qido_all_series(paths: &mut BTreeMap<String, PathItem>) {
    let path = ensure_path(paths, "/series");
    path.summary = Some("QIDO-RS global series search".to_string());
    path.get = Some(Operation {
        operation_id: Some("qidoAllSeriesGet".to_string()),
        summary: Some("Search all series".to_string()),
        description: Some("QIDO-RS: Search for series across all studies.".to_string()),
        tags: vec!["QIDO-RS".to_string()],
        parameters: qido_query_params(),
        request_body: None,
        responses: merged_responses(success_json_response("Array of matching series datasets")),
        security: vec![],
        deprecated: None,
    });
}

fn insert_qido_all_instances(paths: &mut BTreeMap<String, PathItem>) {
    let path = ensure_path(paths, "/instances");
    path.summary = Some("QIDO-RS global instance search".to_string());
    path.get = Some(Operation {
        operation_id: Some("qidoAllInstancesGet".to_string()),
        summary: Some("Search all instances".to_string()),
        description: Some("QIDO-RS: Search for instances across all studies.".to_string()),
        tags: vec!["QIDO-RS".to_string()],
        parameters: qido_query_params(),
        request_body: None,
        responses: merged_responses(success_json_response("Array of matching instance datasets")),
        security: vec![],
        deprecated: None,
    });
}

fn insert_qido_series_by_study(paths: &mut BTreeMap<String, PathItem>) {
    let path = ensure_path(paths, "/studies/{StudyUID}/series");
    path.summary = Some("QIDO-RS study-level series search".to_string());
    path.parameters = vec![ParameterOrRef::Inline(study_uid_param())];
    path.get = Some(Operation {
        operation_id: Some("qidoSeriesByStudyGet".to_string()),
        summary: Some("Search series within a study".to_string()),
        description: Some(
            "QIDO-RS: Search for series within the specified study.".to_string(),
        ),
        tags: vec!["QIDO-RS".to_string()],
        parameters: qido_query_params(),
        request_body: None,
        responses: merged_responses(success_json_response("Array of matching series datasets")),
        security: vec![],
        deprecated: None,
    });
}

fn insert_qido_study_instances(paths: &mut BTreeMap<String, PathItem>) {
    let path = ensure_path(paths, "/studies/{StudyUID}/instances");
    path.summary = Some("QIDO-RS study-level instance search".to_string());
    path.parameters = vec![ParameterOrRef::Inline(study_uid_param())];
    path.get = Some(Operation {
        operation_id: Some("qidoStudyInstancesGet".to_string()),
        summary: Some("Search instances within a study".to_string()),
        description: Some(
            "QIDO-RS: Search for instances within the specified study.".to_string(),
        ),
        tags: vec!["QIDO-RS".to_string()],
        parameters: qido_query_params(),
        request_body: None,
        responses: merged_responses(success_json_response("Array of matching instance datasets")),
        security: vec![],
        deprecated: None,
    });
}

fn insert_qido_instances_by_study_series(paths: &mut BTreeMap<String, PathItem>) {
    let path = ensure_path(paths, "/studies/{StudyUID}/series/{SeriesUID}/instances");
    path.summary = Some("QIDO-RS series-level instance search".to_string());
    path.parameters = vec![
        ParameterOrRef::Inline(study_uid_param()),
        ParameterOrRef::Inline(series_uid_param()),
    ];
    path.get = Some(Operation {
        operation_id: Some("qidoInstancesByStudySeriesGet".to_string()),
        summary: Some("Search instances within a series".to_string()),
        description: Some(
            "QIDO-RS: Search for instances within the specified series.".to_string(),
        ),
        tags: vec!["QIDO-RS".to_string()],
        parameters: qido_query_params(),
        request_body: None,
        responses: merged_responses(success_json_response("Array of matching instance datasets")),
        security: vec![],
        deprecated: None,
    });
}

// ---- STOW-RS ----

fn insert_stow_studies(paths: &mut BTreeMap<String, PathItem>) {
    let path = ensure_path(paths, "/studies");
    let mut success = BTreeMap::new();
    success.insert(
        "200".to_string(),
        Response {
            description: "STOW-RS store response".to_string(),
            content: {
                let mut c = BTreeMap::new();
                c.insert(
                    "application/dicom+json".to_string(),
                    MediaType {
                        schema: Some(Schema {
                            ref_path: Some("#/components/schemas/StowResponse".to_string()),
                            ..Default::default()
                        }),
                    },
                );
                c
            },
        },
    );
    path.post = Some(Operation {
        operation_id: Some("stowStudiesPost".to_string()),
        summary: Some("Store instances".to_string()),
        description: Some(
            "STOW-RS: Store DICOM instances without a study scope.".to_string(),
        ),
        tags: vec!["STOW-RS".to_string()],
        parameters: vec![],
        request_body: Some(RequestBody {
            description: Some("DICOM instances to store".to_string()),
            content: {
                let mut c = BTreeMap::new();
                c.insert(
                    "multipart/related; type=\"application/dicom\"".to_string(),
                    MediaType {
                        schema: Some(Schema {
                            ref_path: Some("#/components/schemas/MultipartDicomPayload".to_string()),
                            ..Default::default()
                        }),
                    },
                );
                c.insert(
                    "application/dicom+json".to_string(),
                    MediaType {
                        schema: Some(Schema {
                            ref_path: Some("#/components/schemas/DicomJsonPayload".to_string()),
                            ..Default::default()
                        }),
                    },
                );
                c
            },
            required: Some(true),
        }),
        responses: merged_responses(success),
        security: vec![],
        deprecated: None,
    });
}

fn insert_stow_study_scoped(paths: &mut BTreeMap<String, PathItem>) {
    let path = ensure_path(paths, "/studies/{StudyUID}");
    path.parameters = vec![ParameterOrRef::Inline(study_uid_param())];
    path.summary = Some("STOW-RS scoped store / WADO study retrieve".to_string());

    // POST for STOW scoped
    let mut success = BTreeMap::new();
    success.insert(
        "200".to_string(),
        Response {
            description: "STOW-RS scoped store response".to_string(),
            content: {
                let mut c = BTreeMap::new();
                c.insert(
                    "application/dicom+json".to_string(),
                    MediaType {
                        schema: Some(Schema {
                            ref_path: Some("#/components/schemas/StowResponse".to_string()),
                            ..Default::default()
                        }),
                    },
                );
                c
            },
        },
    );
    path.post = Some(Operation {
        operation_id: Some("stowStudyScopedPost".to_string()),
        summary: Some("Store instances scoped by study".to_string()),
        description: Some(
            "STOW-RS: Store DICOM instances within the specified study context.".to_string(),
        ),
        tags: vec!["STOW-RS".to_string()],
        parameters: vec![],
        request_body: Some(RequestBody {
            description: Some("DICOM instances to store".to_string()),
            content: {
                let mut c = BTreeMap::new();
                c.insert(
                    "multipart/related; type=\"application/dicom\"".to_string(),
                    MediaType {
                        schema: Some(Schema {
                            ref_path: Some("#/components/schemas/MultipartDicomPayload".to_string()),
                            ..Default::default()
                        }),
                    },
                );
                c
            },
            required: Some(true),
        }),
        responses: merged_responses(success),
        security: vec![],
        deprecated: None,
    });
}

// ---- WADO-RS ----

fn multipart_dicom_response(desc: &str) -> BTreeMap<String, Response> {
    let mut m = BTreeMap::new();
    m.insert(
        "200".to_string(),
        Response {
            description: desc.to_string(),
            content: {
                let mut c = BTreeMap::new();
                c.insert(
                    "multipart/related; type=\"application/dicom\"".to_string(),
                    MediaType {
                        schema: Some(Schema {
                            schema_type: Some("string".to_string()),
                            format: Some("binary".to_string()),
                            ..Default::default()
                        }),
                    },
                );
                c
            },
        },
    );
    m
}

fn single_dicom_response(desc: &str) -> BTreeMap<String, Response> {
    let mut m = BTreeMap::new();
    m.insert(
        "200".to_string(),
        Response {
            description: desc.to_string(),
            content: {
                let mut c = BTreeMap::new();
                c.insert(
                    "application/dicom".to_string(),
                    MediaType {
                        schema: Some(Schema {
                            schema_type: Some("string".to_string()),
                            format: Some("binary".to_string()),
                            ..Default::default()
                        }),
                    },
                );
                c
            },
        },
    );
    m
}

fn metadata_json_response(desc: &str) -> BTreeMap<String, Response> {
    let mut m = BTreeMap::new();
    m.insert(
        "200".to_string(),
        Response {
            description: desc.to_string(),
            content: {
                let mut c = BTreeMap::new();
                c.insert(
                    "application/dicom+json".to_string(),
                    MediaType {
                        schema: Some(Schema {
                            schema_type: Some("array".to_string()),
                            items: Some(Box::new(Schema {
                                ref_path: Some("#/components/schemas/DicomDataset".to_string()),
                                ..Default::default()
                            })),
                            ..Default::default()
                        }),
                    },
                );
                c
            },
        },
    );
    m
}

fn rendered_response(desc: &str) -> BTreeMap<String, Response> {
    let mut m = BTreeMap::new();
    m.insert(
        "200".to_string(),
        Response {
            description: desc.to_string(),
            content: {
                let mut c = BTreeMap::new();
                c.insert(
                    "image/png".to_string(),
                    MediaType {
                        schema: Some(Schema {
                            schema_type: Some("string".to_string()),
                            format: Some("binary".to_string()),
                            ..Default::default()
                        }),
                    },
                );
                c.insert(
                    "image/jpeg".to_string(),
                    MediaType {
                        schema: Some(Schema {
                            schema_type: Some("string".to_string()),
                            format: Some("binary".to_string()),
                            ..Default::default()
                        }),
                    },
                );
                c
            },
        },
    );
    m
}

fn insert_wado_study_retrieve(paths: &mut BTreeMap<String, PathItem>) {
    let path = ensure_path(paths, "/studies/{StudyUID}");
    path.get = Some(Operation {
        operation_id: Some("wadoStudyRetrieveGet".to_string()),
        summary: Some("Retrieve all instances in a study".to_string()),
        description: Some(
            "WADO-RS: Retrieve all DICOM instances for the specified study as multipart.".to_string(),
        ),
        tags: vec!["WADO-RS".to_string()],
        parameters: vec![],
        request_body: None,
        responses: merged_responses(multipart_dicom_response("Multipart DICOM instances for the study")),
        security: vec![],
        deprecated: None,
    });
    path.head = Some(Operation {
        operation_id: Some("wadoStudyRetrieveHead".to_string()),
        summary: Some("Retrieve study retrieve headers".to_string()),
        description: Some("WADO-RS: HEAD for study-level retrieve.".to_string()),
        tags: vec!["WADO-RS".to_string()],
        parameters: vec![],
        request_body: None,
        responses: merged_responses(BTreeMap::new()),
        security: vec![],
        deprecated: None,
    });
}

fn insert_wado_series_retrieve(paths: &mut BTreeMap<String, PathItem>) {
    let path = ensure_path(paths, "/studies/{StudyUID}/series/{SeriesUID}");
    path.summary = Some("WADO-RS series retrieve / QIDO series".to_string());
    path.parameters = vec![
        ParameterOrRef::Inline(study_uid_param()),
        ParameterOrRef::Inline(series_uid_param()),
    ];
    path.get = Some(Operation {
        operation_id: Some("wadoSeriesRetrieveGet".to_string()),
        summary: Some("Retrieve all instances in a series".to_string()),
        description: Some(
            "WADO-RS: Retrieve all DICOM instances for the specified series as multipart.".to_string(),
        ),
        tags: vec!["WADO-RS".to_string()],
        parameters: vec![],
        request_body: None,
        responses: merged_responses(multipart_dicom_response("Multipart DICOM instances for the series")),
        security: vec![],
        deprecated: None,
    });
}

fn insert_wado_instance_retrieve(paths: &mut BTreeMap<String, PathItem>) {
    let path = ensure_path(paths, "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}");
    path.summary = Some("WADO-RS instance retrieve / delete instance".to_string());
    path.parameters = vec![
        ParameterOrRef::Inline(study_uid_param()),
        ParameterOrRef::Inline(series_uid_param()),
        ParameterOrRef::Inline(instance_uid_param()),
    ];
    path.get = Some(Operation {
        operation_id: Some("wadoInstanceRetrieveGet".to_string()),
        summary: Some("Retrieve a single DICOM instance".to_string()),
        description: Some(
            "WADO-RS: Retrieve a single DICOM SOP Instance.".to_string(),
        ),
        tags: vec!["WADO-RS".to_string()],
        parameters: vec![],
        request_body: None,
        responses: merged_responses(single_dicom_response("DICOM Part 10 instance")),
        security: vec![],
        deprecated: None,
    });
}

fn insert_wado_study_metadata(paths: &mut BTreeMap<String, PathItem>) {
    let path = ensure_path(paths, "/studies/{StudyUID}/metadata");
    path.summary = Some("WADO-RS study metadata".to_string());
    path.parameters = vec![ParameterOrRef::Inline(study_uid_param())];
    path.get = Some(Operation {
        operation_id: Some("wadoStudyMetadataGet".to_string()),
        summary: Some("Retrieve study-level metadata".to_string()),
        description: Some(
            "WADO-RS: Retrieve metadata for all instances in the specified study.".to_string(),
        ),
        tags: vec!["WADO-RS".to_string()],
        parameters: vec![],
        request_body: None,
        responses: merged_responses(metadata_json_response("Study metadata as DICOM JSON")),
        security: vec![],
        deprecated: None,
    });
}

fn insert_wado_series_metadata(paths: &mut BTreeMap<String, PathItem>) {
    let path = ensure_path(paths, "/studies/{StudyUID}/series/{SeriesUID}/metadata");
    path.summary = Some("WADO-RS series metadata".to_string());
    path.parameters = vec![
        ParameterOrRef::Inline(study_uid_param()),
        ParameterOrRef::Inline(series_uid_param()),
    ];
    path.get = Some(Operation {
        operation_id: Some("wadoSeriesMetadataGet".to_string()),
        summary: Some("Retrieve series-level metadata".to_string()),
        description: Some(
            "WADO-RS: Retrieve metadata for all instances in the specified series.".to_string(),
        ),
        tags: vec!["WADO-RS".to_string()],
        parameters: vec![],
        request_body: None,
        responses: merged_responses(metadata_json_response("Series metadata as DICOM JSON")),
        security: vec![],
        deprecated: None,
    });
}

fn insert_wado_instance_metadata(paths: &mut BTreeMap<String, PathItem>) {
    let path = ensure_path(paths, "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/metadata");
    path.summary = Some("WADO-RS instance metadata".to_string());
    path.parameters = vec![
        ParameterOrRef::Inline(study_uid_param()),
        ParameterOrRef::Inline(series_uid_param()),
        ParameterOrRef::Inline(instance_uid_param()),
    ];
    path.get = Some(Operation {
        operation_id: Some("wadoInstanceMetadataGet".to_string()),
        summary: Some("Retrieve instance-level metadata".to_string()),
        description: Some(
            "WADO-RS: Retrieve metadata for a single SOP Instance.".to_string(),
        ),
        tags: vec!["WADO-RS".to_string()],
        parameters: vec![],
        request_body: None,
        responses: merged_responses(metadata_json_response("Instance metadata as DICOM JSON")),
        security: vec![],
        deprecated: None,
    });
}

fn insert_wado_frame_retrieve(paths: &mut BTreeMap<String, PathItem>) {
    let path = ensure_path(paths, "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/frames/{FrameNumber}");
    path.summary = Some("WADO-RS frame retrieve".to_string());
    path.parameters = vec![
        ParameterOrRef::Inline(study_uid_param()),
        ParameterOrRef::Inline(series_uid_param()),
        ParameterOrRef::Inline(instance_uid_param()),
        ParameterOrRef::Inline(frame_param()),
    ];
    path.get = Some(Operation {
        operation_id: Some("wadoFrameRetrieveGet".to_string()),
        summary: Some("Retrieve a single frame".to_string()),
        description: Some(
            "WADO-RS: Retrieve pixel data for a specific frame of a multi-frame instance.".to_string(),
        ),
        tags: vec!["WADO-RS".to_string()],
        parameters: vec![],
        request_body: None,
        responses: merged_responses(single_dicom_response("Frame pixel data")),
        security: vec![],
        deprecated: None,
    });
}

fn insert_wado_rendered(paths: &mut BTreeMap<String, PathItem>) {
    let path = ensure_path(paths, "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/rendered");
    path.summary = Some("WADO-RS rendered retrieve".to_string());
    path.parameters = vec![
        ParameterOrRef::Inline(study_uid_param()),
        ParameterOrRef::Inline(series_uid_param()),
        ParameterOrRef::Inline(instance_uid_param()),
    ];
    path.get = Some(Operation {
        operation_id: Some("wadoRenderedInstanceGet".to_string()),
        summary: Some("Retrieve a rendered image".to_string()),
        description: Some(
            "WADO-RS: Retrieve a rendered (decompressed, windowed) image for the instance.".to_string(),
        ),
        tags: vec!["WADO-RS".to_string()],
        parameters: vec![ParameterOrRef::Inline(Parameter {
            name: "accept".to_string(),
            location: "query".to_string(),
            description: Some("Desired rendered media type (image/png or image/jpeg)".to_string()),
            required: Some(false),
            schema: Some(Schema {
                schema_type: Some("string".to_string()),
                enum_values: vec!["image/png".to_string(), "image/jpeg".to_string()],
                ..Default::default()
            }),
        })],
        request_body: None,
        responses: merged_responses(rendered_response("Rendered image")),
        security: vec![],
        deprecated: None,
    });
}

fn insert_wado_rendered_frame(paths: &mut BTreeMap<String, PathItem>) {
    let path = ensure_path(paths, "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/frames/{FrameNumber}/rendered");
    path.summary = Some("WADO-RS rendered frame retrieve".to_string());
    path.parameters = vec![
        ParameterOrRef::Inline(study_uid_param()),
        ParameterOrRef::Inline(series_uid_param()),
        ParameterOrRef::Inline(instance_uid_param()),
        ParameterOrRef::Inline(frame_param()),
    ];
    path.get = Some(Operation {
        operation_id: Some("wadoRenderedFrameGet".to_string()),
        summary: Some("Retrieve a rendered frame".to_string()),
        description: Some(
            "WADO-RS: Retrieve a rendered image for a specific frame.".to_string(),
        ),
        tags: vec!["WADO-RS".to_string()],
        parameters: vec![],
        request_body: None,
        responses: merged_responses(rendered_response("Rendered frame image")),
        security: vec![],
        deprecated: None,
    });
}

fn insert_wado_bulkdata(paths: &mut BTreeMap<String, PathItem>) {
    let path = ensure_path(paths, "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/bulkdata");
    path.summary = Some("WADO-RS bulkdata retrieve".to_string());
    path.parameters = vec![
        ParameterOrRef::Inline(study_uid_param()),
        ParameterOrRef::Inline(series_uid_param()),
        ParameterOrRef::Inline(instance_uid_param()),
    ];
    let mut resp = BTreeMap::new();
    resp.insert(
        "200".to_string(),
        Response {
            description: "Bulkdata octet stream".to_string(),
            content: {
                let mut c = BTreeMap::new();
                c.insert(
                    "application/octet-stream".to_string(),
                    MediaType {
                        schema: Some(Schema {
                            schema_type: Some("string".to_string()),
                            format: Some("binary".to_string()),
                            ..Default::default()
                        }),
                    },
                );
                c
            },
        },
    );
    path.get = Some(Operation {
        operation_id: Some("wadoBulkDataGet".to_string()),
        summary: Some("Retrieve bulkdata".to_string()),
        description: Some(
            "WADO-RS: Retrieve bulk (raw) data for the specified instance.".to_string(),
        ),
        tags: vec!["WADO-RS".to_string()],
        parameters: vec![],
        request_body: None,
        responses: merged_responses(resp),
        security: vec![],
        deprecated: None,
    });
}

// ---- WADO-URI ----

fn insert_wado_uri(paths: &mut BTreeMap<String, PathItem>) {
    let path = ensure_path(paths, "/wado");
    path.summary = Some("WADO-URI legacy retrieve".to_string());
    let mut resp = BTreeMap::new();
    resp.insert(
        "200".to_string(),
        Response {
            description: "DICOM object retrieved via WADO-URI".to_string(),
            content: {
                let mut c = BTreeMap::new();
                c.insert(
                    "application/dicom".to_string(),
                    MediaType {
                        schema: Some(Schema {
                            schema_type: Some("string".to_string()),
                            format: Some("binary".to_string()),
                            ..Default::default()
                        }),
                    },
                );
                c
            },
        },
    );
    path.get = Some(Operation {
        operation_id: Some("wadoUriGet".to_string()),
        summary: Some("Legacy WADO-URI retrieve".to_string()),
        description: Some(
            "WADO-URI: Legacy DICOM retrieve using query parameters (requestType, studyUID, seriesUID, objectUID).".to_string(),
        ),
        tags: vec!["WADO-URI".to_string()],
        parameters: vec![
            ParameterOrRef::Inline(Parameter {
                name: "requestType".to_string(),
                location: "query".to_string(),
                description: Some("Must be \"WADO\"".to_string()),
                required: Some(true),
                schema: Some(Schema {
                    schema_type: Some("string".to_string()),
                    enum_values: vec!["WADO".to_string()],
                    ..Default::default()
                }),
            }),
            ParameterOrRef::Inline(Parameter {
                name: "studyUID".to_string(),
                location: "query".to_string(),
                description: Some("Study Instance UID".to_string()),
                required: Some(true),
                schema: Some(Schema {
                    schema_type: Some("string".to_string()),
                    ..Default::default()
                }),
            }),
            ParameterOrRef::Inline(Parameter {
                name: "seriesUID".to_string(),
                location: "query".to_string(),
                description: Some("Series Instance UID".to_string()),
                required: Some(true),
                schema: Some(Schema {
                    schema_type: Some("string".to_string()),
                    ..Default::default()
                }),
            }),
            ParameterOrRef::Inline(Parameter {
                name: "objectUID".to_string(),
                location: "query".to_string(),
                description: Some("SOP Instance UID".to_string()),
                required: Some(true),
                schema: Some(Schema {
                    schema_type: Some("string".to_string()),
                    ..Default::default()
                }),
            }),
        ],
        request_body: None,
        responses: merged_responses(resp),
        security: vec![],
        deprecated: Some(true),
    });
}

// ---- DELETE ----

fn insert_delete_study(paths: &mut BTreeMap<String, PathItem>) {
    let p = "/studies/{StudyUID}";
    let path = ensure_path(paths, p);
    let mut resp = BTreeMap::new();
    resp.insert(
        "200".to_string(),
        Response {
            description: "Study deleted (soft-delete tombstone applied)".to_string(),
            content: BTreeMap::new(),
        },
    );
    path.delete = Some(Operation {
        operation_id: Some("deleteStudy".to_string()),
        summary: Some("Delete a study".to_string()),
        description: Some("Soft-delete a study (feature-gated by runtime policy).".to_string()),
        tags: vec!["Delete".to_string()],
        parameters: vec![],
        request_body: None,
        responses: merged_responses(resp),
        security: vec![],
        deprecated: None,
    });
}

fn insert_delete_series(paths: &mut BTreeMap<String, PathItem>) {
    let p = "/studies/{StudyUID}/series/{SeriesUID}";
    let path = ensure_path(paths, p);
    let mut resp = BTreeMap::new();
    resp.insert(
        "200".to_string(),
        Response {
            description: "Series deleted (soft-delete tombstone applied)".to_string(),
            content: BTreeMap::new(),
        },
    );
    path.delete = Some(Operation {
        operation_id: Some("deleteSeries".to_string()),
        summary: Some("Delete a series".to_string()),
        description: Some("Soft-delete a series within a study.".to_string()),
        tags: vec!["Delete".to_string()],
        parameters: vec![],
        request_body: None,
        responses: merged_responses(resp),
        security: vec![],
        deprecated: None,
    });
}

fn insert_delete_instance(paths: &mut BTreeMap<String, PathItem>) {
    let p = "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}";
    let path = ensure_path(paths, p);
    let mut resp = BTreeMap::new();
    resp.insert(
        "200".to_string(),
        Response {
            description: "Instance deleted (soft-delete tombstone applied)".to_string(),
            content: BTreeMap::new(),
        },
    );
    path.delete = Some(Operation {
        operation_id: Some("deleteInstance".to_string()),
        summary: Some("Delete an instance".to_string()),
        description: Some("Soft-delete a single SOP Instance.".to_string()),
        tags: vec!["Delete".to_string()],
        parameters: vec![],
        request_body: None,
        responses: merged_responses(resp),
        security: vec![],
        deprecated: None,
    });
}
