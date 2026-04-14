//! Deterministic route-matrix exporter for workflow endpoints.

use dicom_workflow_server::workflow_route_contract;

fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn main() {
    let mut rows_json = String::from("[");
    for (index, route) in workflow_route_contract().iter().enumerate() {
        if index > 0 {
            rows_json.push(',');
        }
        rows_json.push_str(&format!(
            "{{\"service\":\"dicom-workflow-server\",\"method\":\"{}\",\"path\":\"{}\",\"operation\":\"{}\",\"requires_writer_role\":{},\"requires_idempotency_key\":{},\"content_type\":{}}}",
            escape_json(route.method),
            escape_json(route.path_template),
            escape_json(route.operation),
            route.requires_writer_role,
            route.requires_idempotency_key,
            match route.content_type {
                Some(content_type) => format!("\"{}\"", escape_json(content_type)),
                None => "null".to_string(),
            }
        ));
    }
    rows_json.push(']');
    println!("{rows_json}");
}
