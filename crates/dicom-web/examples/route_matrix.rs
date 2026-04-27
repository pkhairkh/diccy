//! Deterministic route-matrix exporter for DICOMweb operations.

use dicom_web::{dicomweb_route_capability_matrix, DicomWebRoute};

fn method_to_text(method: dicom_web::HttpMethod) -> &'static str {
    match method {
        dicom_web::HttpMethod::Get => "GET",
        dicom_web::HttpMethod::Head => "HEAD",
        dicom_web::HttpMethod::Post => "POST",
        dicom_web::HttpMethod::Delete => "DELETE",
    }
}

fn state_to_text(state: dicom_web::DicomWebRouteState) -> &'static str {
    match state {
        dicom_web::DicomWebRouteState::Implemented => "implemented",
        dicom_web::DicomWebRouteState::Partial => "partial",
        dicom_web::DicomWebRouteState::Blocked => "blocked",
        dicom_web::DicomWebRouteState::NotExposed => "not-exposed",
    }
}

fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn main() {
    let matrix = dicomweb_route_capability_matrix();
    let mut rows_json = String::from("[");
    for (index, route) in DicomWebRoute::all().enumerate() {
        if index > 0 {
            rows_json.push(',');
        }
        let state = matrix
            .get(&route)
            .copied()
            .unwrap_or(dicom_web::DicomWebRouteState::NotExposed);
        rows_json.push_str(&format!(
            "{{\"service\":\"dicom-web\",\"method\":\"{}\",\"path\":\"{}\",\"operation\":\"{}\",\"required_feature\":\"{}\",\"state\":\"{}\",\"content_type\":{}}}",
            method_to_text(route.method()),
            escape_json(route.path()),
            escape_json(route.operation()),
            escape_json(route.required_feature()),
            state_to_text(state),
            match route.content_type() {
                Some(content_type) => format!("\"{}\"", escape_json(content_type)),
                None => "null".to_string(),
            }
        ));
    }
    rows_json.push(']');
    println!("{rows_json}");
}
