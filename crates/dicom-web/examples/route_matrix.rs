//! Deterministic route-matrix exporter for DICOMweb operations.

use dicom_web::{dicomweb_route_capability_matrix, HttpMethod};

fn method_to_text(method: &HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Head => "HEAD",
        HttpMethod::Post => "POST",
        HttpMethod::Delete => "DELETE",
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
    let mut rows_json = String::from("[");
    for (index, route) in dicomweb_route_capability_matrix().iter().enumerate() {
        if index > 0 {
            rows_json.push(',');
        }
        rows_json.push_str(&format!(
            "{{\"service\":\"dicom-web\",\"method\":\"{}\",\"path\":\"{}\",\"operation\":\"{}\",\"required_feature\":\"{}\",\"state\":\"{}\",\"content_type\":{}}}",
            method_to_text(&route.method),
            escape_json(route.path),
            escape_json(route.operation),
            escape_json(route.required_feature),
            state_to_text(route.state),
            match route.content_type {
                Some(content_type) => format!("\"{}\"", escape_json(content_type)),
                None => "null".to_string(),
            }
        ));
    }
    rows_json.push(']');
    println!("{rows_json}");
}
