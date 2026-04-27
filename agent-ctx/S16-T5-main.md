# S16-T5 — OpenAPI/Swagger Specification for DICOMweb API

## Task ID: S16-T5
## Agent: main

## Summary

Created the `dicom-openapi` crate that auto-generates an OpenAPI 3.1 specification from DICOMweb route definitions.

## Files Created

- `crates/dicom-openapi/Cargo.toml` — Crate manifest
- `crates/dicom-openapi/src/lib.rs` — Main entry point with `generate_dicomweb_openapi()`, validation, and tests
- `crates/dicom-openapi/src/spec.rs` — Core OpenAPI 3.1 types (Info, Server, PathItem, Operation, Schema, Components, SecurityScheme, etc.)
- `crates/dicom-openapi/src/dicomweb_paths.rs` — DICOMweb endpoint definitions for QIDO-RS, WADO-RS, STOW-RS, WADO-URI, and DELETE
- `crates/dicom-openapi/src/dicomweb_schemas.rs` — DICOM data model schemas (DicomDataset, StowResponse, Error, reusable parameters)
- `crates/dicom-openapi/src/security_schemes.rs` — Bearer and OAuth2 security definitions

## Test Results

All 10 tests pass:
- `generate_spec_validates` — Generated spec passes structural validation
- `spec_has_all_dicomweb_paths` — All expected paths present
- `spec_includes_bearer_and_oauth2` — Security schemes defined
- `spec_serializes_to_valid_json` — Spec serializes to valid JSON
- `spec_has_schemas_for_core_types` — DicomDataset, StowResponse, Error schemas
- `spec_has_reusable_parameters` — PatientID, Modality, limit, offset
- `spec_qido_studies_has_get_and_head` — QIDO studies supports both methods
- `spec_stow_studies_has_post` — STOW studies has POST
- `spec_wado_uri_is_deprecated` — WADO-URI marked deprecated
- `empty_spec_fails_validation` — Validation catches empty specs

## Status: COMPLETE
