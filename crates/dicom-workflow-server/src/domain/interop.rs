//! Interop bounded-context route constants.

/// Base HL7 ingest route.
pub(crate) const INTEROP_HL7_PATH: &str = "/interop/hl7";
/// Prefix for HL7 sub-routes.
pub(crate) const INTEROP_HL7_PREFIX: &str = "/interop/hl7/";
/// HL7 failures suffix.
pub(crate) const INTEROP_HL7_FAILURES_SUFFIX: &str = "failures";
/// HL7 failures route.
pub(crate) const INTEROP_HL7_FAILURES_PATH: &str = "/interop/hl7/failures";
/// Connector status dashboard route.
pub(crate) const INTEROP_CONNECTORS_STATUS_PATH: &str = "/interop/connectors/status";
/// Connector features dashboard route.
pub(crate) const INTEROP_CONNECTORS_FEATURES_PATH: &str = "/interop/connectors/features";
/// Connector rollout administration route.
pub(crate) const INTEROP_CONNECTORS_ROLLOUT_PATH: &str = "/interop/connectors/rollout";
/// Connector health dashboard route.
pub(crate) const INTEROP_CONNECTORS_HEALTH_PATH: &str = "/interop/connectors/health";
/// Connector capabilities discovery route.
pub(crate) const INTEROP_CONNECTORS_CAPABILITIES_PATH: &str = "/interop/connectors/capabilities";
/// HL7 subscription route.
pub(crate) const INTEROP_SUBSCRIPTIONS_PATH: &str = "/interop/subscriptions";
/// FHIR ingest route.
pub(crate) const INTEROP_FHIR_PATH: &str = "/interop/fhir";
/// Reconciliation jobs route prefix.
pub(crate) const INTEROP_RECONCILIATION_JOBS_PREFIX: &str = "/interop/reconciliation/jobs";

/// Parsed interop route classification for orchestration-only routing layers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InteropRoute<'a> {
    Hl7Ingest,
    Hl7Failures,
    ConnectorStatus,
    ConnectorFeatures,
    ConnectorRollout,
    ConnectorCapabilities,
    ConnectorHealth,
    Subscriptions,
    FhirIngest,
    ReconciliationJobsCollection,
    ReconciliationJobRun { job_id: &'a str },
}

/// Classify a normalized request path into an interop bounded-context route.
pub(crate) fn classify_interop_route(path: &str) -> Option<InteropRoute<'_>> {
    if path == INTEROP_HL7_PATH {
        return Some(InteropRoute::Hl7Ingest);
    }
    if let Some(rest) = path.strip_prefix(INTEROP_HL7_PREFIX) {
        if rest == INTEROP_HL7_FAILURES_SUFFIX {
            return Some(InteropRoute::Hl7Failures);
        }
        return None;
    }
    if path == INTEROP_CONNECTORS_STATUS_PATH {
        return Some(InteropRoute::ConnectorStatus);
    }
    if path == INTEROP_CONNECTORS_FEATURES_PATH {
        return Some(InteropRoute::ConnectorFeatures);
    }
    if path == INTEROP_CONNECTORS_ROLLOUT_PATH {
        return Some(InteropRoute::ConnectorRollout);
    }
    if path == INTEROP_CONNECTORS_CAPABILITIES_PATH {
        return Some(InteropRoute::ConnectorCapabilities);
    }
    if path == INTEROP_CONNECTORS_HEALTH_PATH {
        return Some(InteropRoute::ConnectorHealth);
    }
    if path == INTEROP_SUBSCRIPTIONS_PATH {
        return Some(InteropRoute::Subscriptions);
    }
    if path == INTEROP_FHIR_PATH {
        return Some(InteropRoute::FhirIngest);
    }
    if let Some(rest) = path.strip_prefix(INTEROP_RECONCILIATION_JOBS_PREFIX) {
        if rest.is_empty() {
            return Some(InteropRoute::ReconciliationJobsCollection);
        }
        let trimmed = rest.trim_start_matches('/');
        if trimmed.is_empty() {
            return None;
        }
        let Some((job_id, action)) = trimmed.split_once('/') else {
            return None;
        };
        if job_id.is_empty() || action != "run" {
            return None;
        }
        return Some(InteropRoute::ReconciliationJobRun { job_id });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{classify_interop_route, InteropRoute};

    #[test]
    fn classify_interop_route_parses_reconciliation_run_paths() {
        let parsed = classify_interop_route("/interop/reconciliation/jobs/recon-001/run");
        assert_eq!(
            parsed,
            Some(InteropRoute::ReconciliationJobRun {
                job_id: "recon-001"
            })
        );
    }

    #[test]
    fn classify_interop_route_rejects_unsupported_suffixes() {
        assert_eq!(
            classify_interop_route("/interop/reconciliation/jobs/recon-001/invalid"),
            None
        );
        assert_eq!(classify_interop_route("/interop/hl7/failures/extra"), None);
    }
}
