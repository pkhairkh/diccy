#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

echo "[1/5] Ingest/query/retrieve via DICOMweb service tests"
cargo test -p dicom-web service_qido_allows_and_emits_audit -- --exact
cargo test -p dicom-web service_stow_then_wado_round_trip -- --exact

echo "[2/5] Volume/MPR baseline path"
cargo test -p viewer-core axis_aligned_patient_requests_match_voxel_requests -- --exact

echo "[3/5] Tri-planar native demo"
cargo run -p rdvf --example tri_planar_demo

echo "[4/5] SR commit workflow path"
cargo test -p dicom-workflow-server sr_http_flow_create_update_retrieve_is_deterministic -- --exact

echo "[5/5] Fusion baseline path"
cargo test -p modality-pet resample_and_blend_pipeline_is_deterministic -- --exact

echo "Local demo complete: ingest/query/retrieve, MPR, SR, and fusion paths passed."
