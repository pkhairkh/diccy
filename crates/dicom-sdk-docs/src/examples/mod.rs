//! Complete, compilable example extensions for the DiCCY SDK.
//!
//! Each example demonstrates a different extension point:
//!
//! - [`custom_codec`] — Custom transfer syntax codec (NOP codec)
//! - [`custom_pack`] — Modality-specific pack (OCT pack)
//! - [`custom_overlay`] — Custom overlay renderer (heatmap + annotation)

pub mod custom_codec;
pub mod custom_pack;
pub mod custom_overlay;
