# S6-T1: Multi-stage Dockerfile for dicom-web-server
# Builds a minimal production image for Kubernetes deployment.

# ---- Build stage ----
FROM rust:1.77-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/diccy

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
COPY crates/dicom-core/Cargo.toml crates/dicom-core/Cargo.toml
COPY crates/dicom-io/Cargo.toml crates/dicom-io/Cargo.toml
COPY crates/dicom-net/Cargo.toml crates/dicom-net/Cargo.toml
COPY crates/dicom-dimse/Cargo.toml crates/dicom-dimse/Cargo.toml
COPY crates/dicom-dimse-service/Cargo.toml crates/dicom-dimse-service/Cargo.toml
COPY crates/dicom-env-contract/Cargo.toml crates/dicom-env-contract/Cargo.toml
COPY crates/dicom-web/Cargo.toml crates/dicom-web/Cargo.toml
COPY crates/dicom-web-server/Cargo.toml crates/dicom-web-server/Cargo.toml
COPY crates/dicom-visualizer/Cargo.toml crates/dicom-visualizer/Cargo.toml
COPY crates/dicom-workflow-server/Cargo.toml crates/dicom-workflow-server/Cargo.toml
COPY crates/dicom-index/Cargo.toml crates/dicom-index/Cargo.toml
COPY crates/dicom-storage/Cargo.toml crates/dicom-storage/Cargo.toml
COPY crates/dicom-ups/Cargo.toml crates/dicom-ups/Cargo.toml
COPY crates/dicom-query/Cargo.toml crates/dicom-query/Cargo.toml
COPY crates/dicom-worklist/Cargo.toml crates/dicom-worklist/Cargo.toml
COPY crates/dicom-mpps/Cargo.toml crates/dicom-mpps/Cargo.toml
COPY crates/dicom-auth/Cargo.toml crates/dicom-auth/Cargo.toml
COPY crates/dicom-audit/Cargo.toml crates/dicom-audit/Cargo.toml
COPY crates/dicom-series/Cargo.toml crates/dicom-series/Cargo.toml
COPY crates/dicom-pixel/Cargo.toml crates/dicom-pixel/Cargo.toml
COPY crates/viewer-core/Cargo.toml crates/viewer-core/Cargo.toml
COPY crates/viewer-wgpu/Cargo.toml crates/viewer-wgpu/Cargo.toml
COPY crates/viewer-wasm/Cargo.toml crates/viewer-wasm/Cargo.toml
COPY crates/modality-ct/Cargo.toml crates/modality-ct/Cargo.toml
COPY crates/modality-pet/Cargo.toml crates/modality-pet/Cargo.toml
COPY crates/modality-mg/Cargo.toml crates/modality-mg/Cargo.toml
COPY crates/modality-xr/Cargo.toml crates/modality-xr/Cargo.toml
COPY crates/pack-shared/Cargo.toml crates/pack-shared/Cargo.toml
COPY crates/pack-enhanced/Cargo.toml crates/pack-enhanced/Cargo.toml
COPY crates/pack-gsps/Cargo.toml crates/pack-gsps/Cargo.toml
COPY crates/pack-seg/Cargo.toml crates/pack-seg/Cargo.toml
COPY crates/pack-rt/Cargo.toml crates/pack-rt/Cargo.toml
COPY crates/pack-sr/Cargo.toml crates/pack-sr/Cargo.toml
COPY crates/pack-us/Cargo.toml crates/pack-us/Cargo.toml
COPY crates/pack-nm/Cargo.toml crates/pack-nm/Cargo.toml
COPY crates/pack-xa/Cargo.toml crates/pack-xa/Cargo.toml
COPY crates/dicom-fhir/Cargo.toml crates/dicom-fhir/Cargo.toml
COPY crates/dicom-hl7/Cargo.toml crates/dicom-hl7/Cargo.toml
COPY crates/dicom-registration/Cargo.toml crates/dicom-registration/Cargo.toml
COPY crates/dicom-inference/Cargo.toml crates/dicom-inference/Cargo.toml
COPY crates/dicom-collab/Cargo.toml crates/dicom-collab/Cargo.toml
COPY crates/dicom-telerad/Cargo.toml crates/dicom-telerad/Cargo.toml
COPY crates/dicom-cardio/Cargo.toml crates/dicom-cardio/Cargo.toml
COPY crates/dicom-wsi/Cargo.toml crates/dicom-wsi/Cargo.toml
COPY crates/dicom-encapsulate/Cargo.toml crates/dicom-encapsulate/Cargo.toml
COPY crates/dicom-ihe/Cargo.toml crates/dicom-ihe/Cargo.toml
COPY crates/rdvf/Cargo.toml crates/rdvf/Cargo.toml

# Create dummy main.rs files to cache dependencies
RUN for crate in $(find crates -name "Cargo.toml" -exec dirname {} \;); do \
      mkdir -p "$crate/src" && \
      echo "#![allow(dead_code)]" > "$crate/src/lib.rs" && \
      touch "$crate/src/main.rs" 2>/dev/null || true; \
    done

RUN cargo build --release -p dicom-web-server 2>/dev/null || true

# Copy actual source code
COPY . .

# Build the actual server binary
RUN cargo build --release -p dicom-web-server

# ---- Runtime stage ----
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

RUN groupadd -r diccy && useradd -r -g diccy -s /sbin/nologin diccy

COPY --from=builder /usr/src/diccy/target/release/dicom-web-server /usr/local/bin/dicom-web-server

USER diccy

EXPOSE 5252/tcp

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["dicom-web-server", "--health-check"]

ENTRYPOINT ["dicom-web-server"]
