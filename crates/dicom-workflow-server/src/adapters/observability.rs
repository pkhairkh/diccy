use super::{
    json_escape, parse_bool, parse_log_level, redact_diagnostic_message, WORKFLOW_SERVICE_NAME,
};

#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub enum WorkflowLogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[allow(missing_docs)]
impl WorkflowLogLevel {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "off" => Some(Self::Off),
            "error" => Some(Self::Error),
            "warn" | "warning" => Some(Self::Warn),
            "info" => Some(Self::Info),
            "debug" => Some(Self::Debug),
            "trace" => Some(Self::Trace),
            _ => None,
        }
    }

    pub fn should_log(&self, level: Self) -> bool {
        (*self as u8) >= (level as u8)
    }
}

impl From<WorkflowLogLevel> for u8 {
    fn from(value: WorkflowLogLevel) -> Self {
        match value {
            WorkflowLogLevel::Off => 0,
            WorkflowLogLevel::Error => 1,
            WorkflowLogLevel::Warn => 2,
            WorkflowLogLevel::Info => 3,
            WorkflowLogLevel::Debug => 4,
            WorkflowLogLevel::Trace => 5,
        }
    }
}

#[allow(missing_docs)]
pub struct WorkflowObservability {
    service: &'static str,
    log_level: WorkflowLogLevel,
    telemetry_enabled: bool,
    telemetry_safe_subset: bool,
}

#[allow(missing_docs)]
impl WorkflowObservability {
    pub fn from_env(prefix: &str, service: &'static str) -> std::io::Result<Self> {
        let log_level_key = format!("{prefix}LOG_LEVEL");
        let log_level = parse_log_level(&log_level_key)?;
        let telemetry_enabled = parse_bool(
            WORKFLOW_SERVICE_NAME,
            &format!("{prefix}TELEMETRY_ENABLED"),
            false,
        )?;
        let telemetry_safe_subset = parse_bool(
            WORKFLOW_SERVICE_NAME,
            &format!("{prefix}TELEMETRY_SAFE_SUBSET"),
            true,
        )?;

        Ok(Self {
            service,
            log_level,
            telemetry_enabled,
            telemetry_safe_subset,
        })
    }

    pub fn log(&self, level: WorkflowLogLevel, message: &str) {
        if self.log_level.should_log(level) {
            eprintln!("[{level:?}] {}: {message}", self.service);
        }
    }

    pub fn emit_telemetry(&self, event: &str, fields: &[(&str, &str)]) {
        if !self.telemetry_enabled {
            return;
        }
        let rendered_fields: String = fields
            .iter()
            .map(|(k, v)| {
                let sanitized = redact_diagnostic_message(v);
                format!("{k}={}", json_escape(&sanitized))
            })
            .collect::<Vec<_>>()
            .join(" ");
        eprintln!(
            "telemetry safe_subset={} service={} event={} {}",
            self.telemetry_safe_subset, self.service, event, rendered_fields,
        );
    }
}
