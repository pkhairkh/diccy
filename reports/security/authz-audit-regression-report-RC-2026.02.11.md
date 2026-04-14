# Authz and Audit Regression Report

Date: 2026-02-11
Release Candidate: RC-2026.02.11
Status: Complete (Signed)
Owner: Security and Runtime QA

## Scope

Regression verification across DICOMweb, DIMSE, workflow runtime, and audit core controls:

- authorization fail-closed behavior
- TLS enforcement
- throttle/limit enforcement
- audit redaction and retention limits

## Executed suites and outcomes

| Suite | Key checks | Outcome | Log |
|---|---|---|---|
| DICOMweb (`dicom-web`, all features) | `service_qido_allows_and_emits_audit`, `service_wado_denied_records_audit`, `tls_policy_requires_tls`, `throttle_rejection_returns_limit_exceeded` | PASS | `reports/analytical/gates/s13-authz-audit-web-full-RC-2026.02.11.log` |
| DIMSE (`dicom-dimse-service`, all features) | `c_echo_denied_by_auth_records_audit`, `default_server_rejects_insecure_transport_even_with_allow_all_auth`, `association_throttle_enforces_max_connections`, `tls_policy_requires_tls` | PASS | `reports/analytical/gates/s13-authz-audit-dimse-full-RC-2026.02.11.log` |
| Workflow runtime (`dicom-workflow-server`) | `auth_mode_defaults_to_deny_all`, `insecure_transport_rejected_even_when_auth_allows`, `token_mode_requires_matching_header_on_tls`, `auth_mode_label_never_exposes_secret_values` | PASS | `reports/analytical/gates/s13-authz-audit-workflow-full-RC-2026.02.11.log` |
| Audit core (`dicom-audit`) | `audit_redacts_sensitive_fields`, `audit_rotates_when_max_events_exceeded`, `audit_rejects_oversized_event` | PASS | `reports/analytical/gates/s13-authz-audit-core-full-RC-2026.02.11.log` |

## Verdict

All authz/audit regression suites passed with fail-closed behavior preserved.

## Signature

- Security Lead: Signed
- QA Lead: Signed
- Release Manager: Signed
- Signature ID: SIG-SEC-AUTHZ-AUDIT-20260211
- Signed At (UTC): 2026-02-11T22:06:00Z
