#![deny(missing_docs)]

//! IEC 62304 regulatory documentation bundle for the DiCCY PACS workstation.
//!
//! This crate compiles the complete software lifecycle documentation required
//! for regulatory certification under IEC 62304 and ISO 14971:
//!
//! - **SRS** (Software Requirements Specification) — IEC 62304 Section 5.2
//! - **SDD** (Software Design Description) — IEC 62304 Section 5.3
//! - **STP** (Software Test Plan) — IEC 62304 Section 5.7
//! - **RMF** (Risk Management File) — ISO 14971 / IEC 62304 Section 7
//! - **Determinism guarantees** — deterministic rendering documentation

pub mod determinism;
pub mod rmf;
pub mod sdd;
pub mod srs;
pub mod stp;
