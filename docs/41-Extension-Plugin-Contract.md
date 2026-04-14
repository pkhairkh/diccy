# Extension and Plugin Contract

Status: **Baseline extension contract** (As of 2026-02-22)

## 1. Goal

Define a deterministic extension boundary for custom tools and data adapters without bypassing fail-closed limits or provenance controls.

## 2. Contract surface

Extensions must declare:
- `id` (stable unique identifier),
- supported product profile (`framework-core`, `workstation`, `backend-services`),
- handled event types,
- deterministic output schema version.

Extensions must not:
- bypass `Limits` checks,
- mutate source DICOM payloads in-place,
- emit PHI/PII into telemetry/audit channels without explicit policy coverage.

## 3. Event model

Baseline extension event envelope:
- event kind (`measurement_captured`, `dataset_indexed`, `workflow_state_changed`),
- deterministic payload map,
- context tuple (study/series/instance identifiers when available),
- optional provenance tags.

Output contract:
- deterministic key/value map,
- explicit status (`accepted`, `rejected`, `no_op`),
- explicit failure code for fail-closed outcomes.

## 4. Registration and ordering

- Registration is explicit and deterministic.
- Duplicate extension IDs are rejected.
- Execution order is stable by registration order unless profile policy overrides it.

## 5. Verification expectations

- Unit tests for duplicate-ID rejection and deterministic ordering.
- Integration test for one sample extension roundtrip.
- Documentation of profile boundaries and expected outputs.
