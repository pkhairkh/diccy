# Error Code Mapping Tables (Renderer / Volume / Fusion / SR)

Status: **Implemented baseline mapping** (As of 2026-02-22)

## Renderer (WASM backend)

| Source type | Condition | Code |
|---|---|---|
| `BackendErrorCode::InitFailed` | WebGPU init/capability failure | `DVF.WASM.GPU.INIT_FAILED` |
| `BackendErrorCode::DeviceLost` | Device lost / submit on lost device | `DVF.WASM.GPU.DEVICE_LOST` |
| `BackendErrorCode::SubmitFailed` | Upload/submit failure or budget rejection | `DVF.WASM.GPU.SUBMIT_FAILED` |
| `BackendErrorCode::FlagDisabled` | Production WebGPU flag disabled | `DVF.WASM.GPU.FLAG_DISABLED` |

## Volume + MPR (`viewer-core`)

### Volume assembly

| Source type | Condition | Code |
|---|---|---|
| `VolumeError::EmptyInput` | No slices | `DVF.VOLUME.EMPTY_INPUT` |
| `VolumeError::InconsistentDimensions` | Slice geometry mismatch | `DVF.VOLUME.INCONSISTENT_DIMENSIONS` |
| `VolumeError::InvalidSlicePixels` | Invalid pixel buffer length | `DVF.VOLUME.INVALID_SLICE_PIXELS` |
| `VolumeError::NonUniformSpacing` | Non-uniform spacing rejected | `DVF.VOLUME.NON_UNIFORM_SPACING` |
| `VolumeError::SizeOverflow` | Allocation overflow | `DVF.VOLUME.SIZE_OVERFLOW` |

### MPR

| Source type | Condition | Code |
|---|---|---|
| `MprError::InvalidRequest` | Invalid dimensions/index/plane | `DVF.MPR.INVALID_REQUEST` |
| `MprError::SourceLimitExceeded` | Source volume exceeds limit | `DVF.MPR.SOURCE_LIMIT_EXCEEDED` |
| `MprError::OutputLimitExceeded` | Output exceeds limit | `DVF.MPR.OUTPUT_LIMIT_EXCEEDED` |

## Fusion (`modality-pet`)

| Condition | Code |
|---|---|
| Missing/mismatched Frame of Reference UID | `DVF.DICOM.MISSING_TAG` / `DVF.DICOM.INVALID_TAG_VALUE` |
| Invalid fusion geometry constraints | `DVF.DICOM.INVALID_TAG_VALUE` / `DVF.GEOM.INVALID` |
| Invalid SUV transform inputs | `DVF.PIXEL.INVALID_TRANSFORM` |
| Fusion limits exceeded | `DVF.SECURITY.LIMIT_EXCEEDED` |

## SR workflow service (`dicom-workflow-server`)

| Condition | Code |
|---|---|
| SR write principal unauthorized | `DVF.WORKFLOW.SR.AUTH_DENIED` |
| Missing idempotency key | `DVF.WORKFLOW.SR.IDEMPOTENCY_REQUIRED` |
| Idempotency replay payload mismatch | `DVF.WORKFLOW.SR.IDEMPOTENCY_CONFLICT` |
| Target SR document already exists | `DVF.WORKFLOW.SR.ALREADY_EXISTS` |
| Target SR document not found | `DVF.WORKFLOW.SR.NOT_FOUND` |
| SR version conflict | `DVF.WORKFLOW.SR.VERSION_CONFLICT` |
| Invalid SR lifecycle transition | `DVF.WORKFLOW.SR.INVALID_TRANSITION` |
| Unknown referenced SOP UID | `DVF.WORKFLOW.SR.UNKNOWN_REFERENCE` |
| SR snapshot parse failure | `DVF.WORKFLOW.SR.SNAPSHOT_INVALID` |
| SR persistence I/O failure | `DVF.WORKFLOW.SR.IO_ERROR` |

## Verification

- `cargo test -p viewer-core`
- `cargo test -p viewer-wasm`
- `cargo test -p modality-pet`
- `cargo test -p dicom-workflow-server`
