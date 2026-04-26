use dicom_web::{
    dicomweb_route_capability_matrix, service_interface_capability_matrix, DicomWebRouteState,
    HttpMethod, ServiceAvailability, ServiceInterface, ServiceInterfacePolicy,
};

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
use dicom_core::{ErrorKind, Limits, Tag};
#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
use dicom_storage::Storage;
#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
use dicom_web::stow_preflight_compatibility;
#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
use dicom_web::{DicomWebService, DicomWebServiceConfig, TransportSecurity, WebAuthConfig};

#[test]
fn service_interface_matrix_lists_dimse_dicomweb_mwl_and_mpps_states() {
    // REQ-HI-255, REQ-HI-256, REQ-HI-320
    let matrix = service_interface_capability_matrix(ServiceInterfacePolicy::default());
    assert_eq!(matrix.len(), 4);
    assert_eq!(matrix[0].interface, ServiceInterface::Dimse);
    assert_eq!(matrix[1].interface, ServiceInterface::Dicomweb);
    assert_eq!(matrix[2].interface, ServiceInterface::Mwl);
    assert_eq!(matrix[3].interface, ServiceInterface::Mpps);

    let dicomweb = matrix
        .iter()
        .find(|row| row.interface == ServiceInterface::Dicomweb)
        .expect("dicomweb row");
    if cfg!(any(feature = "qido", feature = "wado", feature = "stow")) {
        assert_eq!(dicomweb.availability, ServiceAvailability::Active);
    } else {
        assert_eq!(dicomweb.availability, ServiceAvailability::UnsupportedBuild);
    }
}

#[test]
fn service_interface_matrix_surfaces_runtime_policy_disablement() {
    // REQ-HI-255, REQ-HI-262, REQ-HI-320
    let policy = ServiceInterfacePolicy {
        dimse_enabled: true,
        dicomweb_enabled: false,
        mwl_enabled: true,
        mpps_enabled: true,
    };
    let matrix = service_interface_capability_matrix(policy);
    let dicomweb = matrix
        .iter()
        .find(|row| row.interface == ServiceInterface::Dicomweb)
        .expect("dicomweb row");
    if cfg!(any(feature = "qido", feature = "wado", feature = "stow")) {
        assert_eq!(dicomweb.availability, ServiceAvailability::DisabledByPolicy);
        assert_eq!(dicomweb.reason, "disabled by runtime policy");
    } else {
        assert_eq!(dicomweb.availability, ServiceAvailability::UnsupportedBuild);
    }
}

#[test]
fn dicomweb_route_matrix_is_deterministic_and_feature_gated() {
    // REQ-HI-256, REQ-HI-275, REQ-HI-280, REQ-HI-326
    let matrix = dicomweb_route_capability_matrix();
    assert_eq!(matrix.len(), 34);
    assert_eq!(matrix[0].path, "/studies");
    assert_eq!(matrix[0].method, HttpMethod::Get);
    assert_eq!(
        matrix[33].path,
        "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}"
    );
    assert_eq!(matrix[33].method, HttpMethod::Delete);

    let stow_studies = matrix
        .iter()
        .find(|row| row.path == "/studies" && row.method == HttpMethod::Post)
        .expect("stow /studies route");
    assert_eq!(
        stow_studies.content_type,
        Some("application/dicom, application/dicom+xml, application/dicom+json, or multipart/related")
    );
    if cfg!(feature = "stow") {
        assert_eq!(stow_studies.state, DicomWebRouteState::Implemented);
    } else {
        assert_eq!(stow_studies.state, DicomWebRouteState::Blocked);
    }

    let wado_instance = matrix
        .iter()
        .find(|row| {
            row.path == "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}"
                && row.method == HttpMethod::Get
        })
        .expect("wado instance route");
    if cfg!(feature = "wado") {
        assert_eq!(wado_instance.state, DicomWebRouteState::Implemented);
    } else {
        assert_eq!(wado_instance.state, DicomWebRouteState::Blocked);
    }

    let wado_study = matrix
        .iter()
        .find(|row| row.path == "/studies/{StudyUID}" && row.method == HttpMethod::Get)
        .expect("wado study retrieve route");
    if cfg!(feature = "wado") {
        assert_eq!(wado_study.state, DicomWebRouteState::Implemented);
    } else {
        assert_eq!(wado_study.state, DicomWebRouteState::Blocked);
    }

    let wado_series = matrix
        .iter()
        .find(|row| {
            row.path == "/studies/{StudyUID}/series/{SeriesUID}" && row.method == HttpMethod::Get
        })
        .expect("wado series retrieve route");
    if cfg!(feature = "wado") {
        assert_eq!(wado_series.state, DicomWebRouteState::Implemented);
    } else {
        assert_eq!(wado_series.state, DicomWebRouteState::Blocked);
    }

    let wado_rendered = matrix
        .iter()
        .find(|row| {
            row.path == "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/rendered"
                && row.method == HttpMethod::Get
        })
        .expect("wado rendered route");
    if cfg!(feature = "wado") {
        assert_eq!(wado_rendered.state, DicomWebRouteState::Implemented);
    } else {
        assert_eq!(wado_rendered.state, DicomWebRouteState::Blocked);
    }

    let wado_bulkdata = matrix
        .iter()
        .find(|row| {
            row.path == "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/bulkdata"
                && row.method == HttpMethod::Get
        })
        .expect("wado bulkdata route");
    if cfg!(feature = "wado") {
        assert_eq!(wado_bulkdata.state, DicomWebRouteState::Implemented);
    } else {
        assert_eq!(wado_bulkdata.state, DicomWebRouteState::Blocked);
    }
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
fn meta_element_ui(tag: Tag, value: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&tag.0.to_le_bytes());
    buf.extend_from_slice(&tag.1.to_le_bytes());
    buf.extend_from_slice(b"UI");
    let mut bytes = value.as_bytes().to_vec();
    if bytes.len() % 2 == 1 {
        bytes.push(0);
    }
    buf.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
    buf.extend_from_slice(&bytes);
    buf
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
fn dataset_element_explicit(tag: Tag, vr: [u8; 2], value: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&tag.0.to_le_bytes());
    buf.extend_from_slice(&tag.1.to_le_bytes());
    buf.extend_from_slice(&vr);
    let mut bytes = value.to_vec();
    if bytes.len() % 2 == 1 {
        bytes.push(0);
    }
    match &vr {
        b"OB" | b"OW" | b"SQ" | b"UN" | b"UT" => {
            buf.extend_from_slice(&0u16.to_le_bytes());
            buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        }
        _ => {
            buf.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
        }
    }
    buf.extend_from_slice(&bytes);
    buf
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
fn sample_p10(
    meta_ts: &str,
    study_uid: &str,
    series_uid: &str,
    instance_uid: &str,
    sop_uid: &str,
) -> Vec<u8> {
    let mut dataset = Vec::new();
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0008, 0x0016),
        *b"UI",
        sop_uid.as_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0008, 0x0018),
        *b"UI",
        instance_uid.as_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0020, 0x000D),
        *b"UI",
        study_uid.as_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0020, 0x000E),
        *b"UI",
        series_uid.as_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0002),
        *b"US",
        &1u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0004),
        *b"CS",
        b"MONOCHROME2",
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0010),
        *b"US",
        &1u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0011),
        *b"US",
        &1u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0100),
        *b"US",
        &16u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0101),
        *b"US",
        &12u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0102),
        *b"US",
        &11u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0103),
        *b"US",
        &0u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x7FE0, 0x0010),
        *b"OB",
        &[0u8],
    ));

    let mut bytes = vec![0u8; 128];
    bytes.extend_from_slice(b"DICM");
    bytes.extend_from_slice(&meta_element_ui(Tag(0x0002, 0x0010), meta_ts));
    bytes.extend_from_slice(&dataset);
    bytes
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
fn sample_ct(meta_ts: &str, study_uid: &str, series_uid: &str, instance_uid: &str) -> Vec<u8> {
    sample_p10(
        meta_ts,
        study_uid,
        series_uid,
        instance_uid,
        "1.2.840.10008.5.1.4.1.1.7",
    )
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
fn sample_unsupported_sop(
    meta_ts: &str,
    study_uid: &str,
    series_uid: &str,
    _instance_uid: &str,
) -> Vec<u8> {
    sample_p10(
        meta_ts,
        study_uid,
        series_uid,
        _instance_uid,
        "1.2.840.10008.9999.1.1.1",
    )
}

#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
fn multipart_body(boundary: &str, payloads: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    for payload in payloads {
        out.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        out.extend_from_slice(b"content-type: application/dicom\r\n\r\n");
        out.extend_from_slice(payload);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    out
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
#[test]
fn stow_preflight_validates_study_uid_and_sop_transfer_compatibility() {
    // REQ-HI-259, REQ-HI-283, REQ-HI-316
    let payload = sample_ct(
        "1.2.840.10008.1.2.1",
        "1.2.840.1000.1",
        "1.2.840.1000.1.1",
        "1.2.840.1000.1.1.1",
    );
    let preflight = stow_preflight_compatibility(&payload, &Limits::default()).expect("preflight");
    assert_eq!(preflight.study_uid, "1.2.840.1000.1");
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
#[test]
fn stow_preflight_fails_closed_on_unsupported_transfer_syntax() {
    // REQ-HI-259, REQ-HI-316
    let payload = sample_ct(
        "1.2.840.10008.1.2.2",
        "1.2.840.2000.1",
        "1.2.840.2000.1.1",
        "1.2.840.2000.1.1.1",
    );
    let err = stow_preflight_compatibility(&payload, &Limits::default()).expect_err("error");
    assert!(matches!(
        err.kind(),
        ErrorKind::UnsupportedTransferSyntax { .. } | ErrorKind::DecodeError { .. }
    ));
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
#[test]
fn stow_preflight_fails_closed_on_unsupported_sop_class() {
    // REQ-HI-259, REQ-HI-316
    let payload = sample_unsupported_sop(
        "1.2.840.10008.1.2.1",
        "1.2.840.2200.1",
        "1.2.840.2200.1.1",
        "1.2.840.2200.1.1.1",
    );
    let err = stow_preflight_compatibility(&payload, &Limits::default()).expect_err("error");
    assert!(matches!(err.kind(), ErrorKind::UnsupportedSopClass { .. }));
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
#[test]
fn stow_preflight_rejects_malformed_dataset() {
    // REQ-HI-259, REQ-HI-316
    let payload = vec![0x00, 0x01, 0x02, 0x03];
    let err = stow_preflight_compatibility(&payload, &Limits::default()).expect_err("error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
#[test]
fn service_stow_rejects_unsupported_sop_and_keeps_storage_empty() {
    // REQ-HI-316
    let mut storage = Storage::new(Limits::default());
    let mut config = DicomWebServiceConfig::default();
    config.auth = WebAuthConfig::allow_all();
    let service = DicomWebService::new(config);
    let first = sample_ct(
        "1.2.840.10008.1.2.1",
        "1.2.840.3300.1",
        "1.2.840.3300.1.1",
        "1.2.840.3300.1.1.1",
    );
    let bad = sample_unsupported_sop(
        "1.2.840.10008.1.2.1",
        "1.2.840.4400.1",
        "1.2.840.4400.1.1",
        "1.2.840.4400.1.1.1",
    );
    let boundary = "batch-deny";
    let mut stow_http = format!(
        "POST /studies\r\ncontent-type: multipart/related; type=\"application/dicom\"; boundary={boundary}\r\n\r\n"
    )
    .into_bytes();
    stow_http.extend_from_slice(&multipart_body(&boundary, &[first, bad]));
    let err = service
        .handle_http(&stow_http, TransportSecurity::Insecure, &mut storage)
        .expect_err("unsupported sop class");
    assert!(matches!(
        err.kind(),
        ErrorKind::UnsupportedSopClass { .. } | ErrorKind::DecodeError { .. }
    ));
    assert_eq!(storage.index().total_instances(), 0);
}
