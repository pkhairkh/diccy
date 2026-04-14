# Kubernetes reference manifests (profile-oriented)

These manifests are reference examples for core and enterprise deployment modes.

- `dicom-core-runtime.yaml` — core-only runtime (minimal background worker / API-only path).
- `dicom-enterprise-runtime.yaml` — web + workflow backend + optional DIMSE integration stack.

Usage:

```bash
kubectl apply -f deploy/k8s/dicom-core-runtime.yaml
kubectl apply -f deploy/k8s/dicom-enterprise-runtime.yaml
```

Deployment assumptions:
- Replace `<release-id>` with a release artifact version (`PROFILE-YYYYMMDD`).
- Use namespace and image names from your release process.
- Mount state as `PersistentVolumeClaim` backed storage for crash-safe WAL and snapshots.
- For DIMSE integration, set TLS/material paths and auth mode for policy-compliant operation.
