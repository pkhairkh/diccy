# PACS Integration Test Suite

Integration tests for DiCCY PACS workstation against reference PACS servers.

## Overview

This test suite validates DiCCY's interoperability with real PACS endpoints using
Docker Compose orchestration for the following reference implementations:

| Container     | Purpose                        | Ports                    |
|---------------|--------------------------------|--------------------------|
| Orthanc       | DIMSE interop (C-STORE/FIND/MOVE) | 4242 (DICOM), 8042 (HTTP) |
| dcm4chee      | DICOMweb round-trip (STOW/WADO)  | 8080 (HTTP), 11112 (DICOM) |
| HAPI FHIR     | FHIR mapping integration      | 9090 (HTTP)              |

## Usage

### Start containers

```bash
docker-compose up -d
```

### Set environment variables

```bash
export ORTHANC_HOST=localhost
export DCM4CHEE_HOST=localhost
export HAPI_FHIR_HOST=localhost
```

### Run integration tests

```bash
cargo test -p pacs-integration
```

### Stop containers

```bash
docker-compose down
```

## Test Scenarios

### Orthanc Interop (`orthanc_interop.rs`)

- **C-ECHO verification** — Association and connectivity check
- **C-STORE CT instance** — Store a single CT dataset
- **C-FIND studies** — Query studies by Patient ID
- **C-MOVE study** — Retrieve study via C-MOVE sub-operations
- **1000-instance CT study** — Stress test: store 1000 instances

### dcm4chee Interop (`dcm4chee_interop.rs`)

- **STOW → WADO round-trip** — Store and retrieve a dataset
- **QIDO search after STOW** — Verify search returns stored data
- **WADO metadata** — Retrieve DICOM JSON metadata
- **WADO rendered** — Retrieve rendered image (PNG/JPEG)
- **1000-instance study query** — Paginated QIDO queries

### HAPI FHIR Interop (`fhir_hapi_interop.rs`)

- **Patient mapping** — DICOM Patient → FHIR Patient → POST
- **ImagingStudy mapping** — DICOM Study → FHIR ImagingStudy → POST
- **Observation mapping** — DICOM SR → FHIR Observation → POST
- **Full round-trip** — Patient + Study + search verification

## CI Integration

These tests are designed to run as a nightly CI pipeline step:

```yaml
# Example GitHub Actions step
- name: Run PACS Integration Tests
  run: |
    docker-compose up -d
    sleep 30  # Wait for services to be ready
    export ORTHANC_HOST=localhost
    export DCM4CHEE_HOST=localhost
    export HAPI_FHIR_HOST=localhost
    cargo test -p pacs-integration
    docker-compose down
```

When environment variables are not set, tests are **skipped** (not failed)
to allow the test suite to compile and run in environments without Docker.
