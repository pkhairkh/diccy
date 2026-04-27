# Runtime Deployment Templates

This folder provides baseline deployment templates for packaged runtimes.

- `dicom-web-server.env.example`
- `dicom-workflow-server.env.example`
- `docker-compose.framework-core.yml`
- `docker-compose.workstation.yml`
- `docker-compose.backend-services.yml`
- `orthanc_adapter_example.sh`

These templates are security-oriented defaults (durable persistence + explicit transport/auth policy).
Use secret management for token values and avoid committing environment files with real credentials.

Security baseline notes:
- `deploy/docker-compose.backend-services.yml` and `deploy/docker-compose.workstation.yml` now default to fail-closed auth/transport (`deny_all` / `token`, `require_tls`, `tls`) unless explicitly overridden with environment variables.
- Kubernetes enterprise manifests require mounted TLS secret material for workflow and DIMSE services:
  - `workflow-tls` (`tls.crt`, `tls.key`)
  - `dimse-tls` (`tls.crt`, `tls.key`)
  - `workflow-auth` (`auth-token`)
- Kubernetes runtime manifests enforce least-privilege controls:
  - pod-level `runAsNonRoot: true`
  - `seccompProfile.type: RuntimeDefault`
  - container-level `allowPrivilegeEscalation: false` and capability drop-all.
- Kubernetes enterprise service pods include health-gated rollout checks:
  - web/workflow readiness+live probes on `/readyz` and `/healthz`
  - DIMSE readiness+live probes on health endpoint (`DICOM_DIMSE_HEALTH_BIND`)
- Kubernetes enterprise baseline now includes:
  - `deploy/k8s/dicom-enterprise-network-policy.yaml` for default-deny ingress + explicit service ingress allowlists.
  - `deploy/k8s/dicom-enterprise-pdb.yaml` for disruption budgets on web/workflow/DIMSE runtimes.
  - scheduler resilience defaults (`podAntiAffinity`, `topologySpreadConstraints`) in runtime deployment manifests.
- Treat these compose files as production-like security baselines; local convenience overrides should be explicit and short-lived.

## Profile-specific compose stacks

Run from repository root:

```bash
docker compose -f deploy/docker-compose.framework-core.yml up
```

```bash
docker compose -f deploy/docker-compose.backend-services.yml up
```

```bash
docker compose -f deploy/docker-compose.workstation.yml up
```

## Profile bootstrap scripts

- `./tools/bootstrap_minimal_profile.sh`: one-command DICOMweb + workflow + frontend bootstrap.
- `./tools/bootstrap_dimse_profile.sh`: one-command DIMSE-integrated bootstrap (web + workflow + DIMSE + frontend).
- `./tools/bootstrap_local_stack.sh`: compatibility entrypoint that delegates to either minimal profile or DIMSE profile (`--dimse`).

## Kubernetes reference manifests

- `deploy/k8s/dicom-core-runtime.yaml`
- `deploy/k8s/dicom-enterprise-runtime.yaml`

## Startup preflight

Run before launching profile stacks:

```bash
./tools/startup_preflight_check.sh
```

## Orthanc bridge recipe (fail-closed constraints)

`orthanc_adapter_example.sh` provides a reproducible adapter pattern:
- query studies from Orthanc via `/tools/find`,
- forward a local DICOM Part 10 file into DiCCY DICOMweb STOW.

Security/fail-closed controls:
- default to loopback endpoints; do not expose adapter endpoints publicly,
- require explicit credentials (`ORTHANC_USER`/`ORTHANC_PASS`) when Orthanc auth is enabled,
- treat all forwarded payloads as untrusted and rely on DiCCY fail-closed parsing limits,
- avoid forwarding PHI-bearing query payloads into logs; keep command-line invocations minimal.

Run:

```bash
./deploy/orthanc_adapter_example.sh find-study
./deploy/orthanc_adapter_example.sh stow-file /path/to/file.dcm
```
