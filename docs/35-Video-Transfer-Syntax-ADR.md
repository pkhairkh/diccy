# ADR: Video Transfer Syntax Strategy

Status: Accepted  
Date: 2026-02-21

## Context

The conformance envelope recognizes MPEG/H.264/HEVC transfer syntax UIDs at the IO layer, while decode capability depends on explicit feature flags.

## Decision

- Keep video transfer syntax recognition in `dicom-io` for explicit fail-closed handling.
- Keep decode support feature-gated (`codec-mpeg2`, `codec-h264`, `codec-hevc`).
- Default builds remain fail-closed when these codec features are disabled.
- Distinguish recognized-but-undecodable outcomes using typed unsupported-transfer-syntax errors.

## Consequences

- Users can see explicit transfer-syntax handling without silent fallback.
- Default artifact remains conservative for safety and supply-chain predictability.
- Runtime capability reporting remains tied to compile-time feature configuration.

## Verification

- `cargo test -p dicom-io` with and without codec feature flags.
- Docs matrix checks in `docs/03-DICOM-Conformance-Envelope.md` and `docs/05-Pixel-Pipeline.md`.
