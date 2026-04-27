#![deny(missing_docs)]

//! PET modality pack: SUV scaling helpers and deterministic PET-over-CT fusion.
//!
//! The pack does not derive SUV scaling from DICOM tags; callers must supply
//! an explicit scale factor and raw modality values.

use dicom_core::{Error, ErrorKind, Result, Tag};

/// Trait providing volume data access for fusion operations.
///
/// This trait abstracts the concrete `viewer_core::VolumeGrid` type so that
/// modality-pet does not depend on the viewer-core crate for domain logic.
/// Implementors provide dimensional, voxel, and patient-space mapping access.
pub trait VolumeGridProvider {
    /// Return dimensions as `[x, y, z]`.
    fn dimensions(&self) -> [usize; 3];
    /// Return one voxel value if coordinates are in range.
    fn voxel(&self, x: usize, y: usize, z: usize) -> Option<i32>;
    /// Return voxel buffer in deterministic row-major layout.
    fn voxels(&self) -> &[i32];
    /// Return true when patient-space geometry is available.
    fn has_patient_geometry(&self) -> bool;
    /// Convert voxel coordinates to patient-space micrometers when geometry exists.
    fn voxel_to_patient_um(&self, x: usize, y: usize, z: usize) -> Option<[i64; 3]>;
    /// Convert patient-space micrometers to voxel-space floating coordinates.
    fn patient_to_voxel_f64(&self, patient_um: [f64; 3]) -> Option<[f64; 3]>;
}

impl std::fmt::Debug for dyn VolumeGridProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VolumeGridProvider")
            .field("dimensions", &self.dimensions())
            .finish()
    }
}

#[cfg(feature = "viewer-core")]
impl VolumeGridProvider for viewer_core::VolumeGrid {
    fn dimensions(&self) -> [usize; 3] {
        viewer_core::VolumeGrid::dimensions(self)
    }
    fn voxel(&self, x: usize, y: usize, z: usize) -> Option<i32> {
        viewer_core::VolumeGrid::voxel(self, x, y, z)
    }
    fn voxels(&self) -> &[i32] {
        viewer_core::VolumeGrid::voxels(self)
    }
    fn has_patient_geometry(&self) -> bool {
        viewer_core::VolumeGrid::patient_geometry(self).is_some()
    }
    fn voxel_to_patient_um(&self, x: usize, y: usize, z: usize) -> Option<[i64; 3]> {
        viewer_core::VolumeGrid::voxel_to_patient_um(self, x, y, z)
    }
    fn patient_to_voxel_f64(&self, patient_um: [f64; 3]) -> Option<[f64; 3]> {
        viewer_core::VolumeGrid::patient_to_voxel_f64(self, patient_um)
    }
}

/// PET Image Storage SOP Class UID (Tier 1, requires PET pack).
pub const SOP_CLASS_PET: &str = "1.2.840.10008.5.1.4.1.1.128";

/// Frame of Reference UID tag.
pub const TAG_FRAME_OF_REFERENCE_UID: Tag = Tag(0x0020, 0x0052);

/// Marker type for PET modality features.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PetPack;

impl PetPack {
    /// Return true when PET pack functionality is enabled for this crate.
    pub const fn enabled() -> bool {
        cfg!(feature = "modality-pet")
    }

    /// Require the PET pack to be enabled for PET SOP classes.
    pub fn ensure_pet_supported() -> Result<()> {
        if Self::enabled() {
            Ok(())
        } else {
            Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: SOP_CLASS_PET.to_string(),
                },
                "PET requires modality-pet feature",
            )))
        }
    }
}

/// Fusion inputs derived from PET and CT series metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FusionInput<'a> {
    /// Frame of Reference UID for the CT series.
    pub ct_frame_of_reference_uid: Option<&'a str>,
    /// Frame of Reference UID for the PET series.
    pub pet_frame_of_reference_uid: Option<&'a str>,
}

/// Validate PET/CT fusion eligibility.
pub fn validate_fusion(input: FusionInput<'_>) -> Result<()> {
    let ct_uid = input
        .ct_frame_of_reference_uid
        .ok_or_else(|| missing_required_tag(TAG_FRAME_OF_REFERENCE_UID))?;
    let pet_uid = input
        .pet_frame_of_reference_uid
        .ok_or_else(|| missing_required_tag(TAG_FRAME_OF_REFERENCE_UID))?;
    if ct_uid != pet_uid {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidTagValue {
                tag: TAG_FRAME_OF_REFERENCE_UID,
                detail: "Frame of Reference UID mismatch".to_string(),
            },
            "Frame of Reference UID mismatch",
        )));
    }
    Ok(())
}

/// SUV scaling input (caller-provided scale factor).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SuvScaleInput {
    /// Raw modality value.
    pub raw_value: f64,
    /// Scale factor to convert to SUV.
    pub scale_factor: f64,
}

/// Apply SUV scaling with strict validation.
pub fn apply_suv(input: SuvScaleInput) -> Result<f64> {
    if !input.raw_value.is_finite() || !input.scale_factor.is_finite() {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidPixelTransform {
                stage: "suv".to_string(),
                detail: "SUV inputs must be finite".to_string(),
            },
            "SUV inputs must be finite",
        )));
    }
    if input.scale_factor <= 0.0 {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidPixelTransform {
                stage: "suv".to_string(),
                detail: "SUV scale factor must be > 0".to_string(),
            },
            "SUV scale factor must be > 0",
        )));
    }
    Ok(input.raw_value * input.scale_factor)
}

/// Fusion execution limits for PET-over-CT pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FusionExecutionLimits {
    /// Maximum output voxels in the resampled PET-on-CT volume.
    pub max_output_voxels: u64,
    /// Maximum output bytes for intermediate and overlay buffers.
    pub max_output_bytes: u64,
}

impl Default for FusionExecutionLimits {
    fn default() -> Self {
        Self {
            max_output_voxels: 512 * 512 * 2048,
            max_output_bytes: 512 * 1024 * 1024,
        }
    }
}

/// Deterministic PET overlay colormap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetColormap {
    /// Fixed hot-iron style map.
    HotIron,
}

/// Deterministic blending policy for PET overlay rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FusionBlendPolicy {
    /// Global alpha for PET overlay contribution (`0..=255`).
    pub alpha_u8: u8,
    /// Colormap for PET SUV values.
    pub colormap: PetColormap,
}

impl Default for FusionBlendPolicy {
    fn default() -> Self {
        Self {
            alpha_u8: 160,
            colormap: PetColormap::HotIron,
        }
    }
}

/// Input bundle for PET-over-CT fusion execution.
#[derive(Clone)]
pub struct PetCtFusionInput<'a> {
    /// CT reference volume.
    pub ct_volume: &'a dyn VolumeGridProvider,
    /// PET source volume.
    pub pet_volume: &'a dyn VolumeGridProvider,
    /// Frame-of-reference UID for CT.
    pub ct_frame_of_reference_uid: Option<&'a str>,
    /// Frame-of-reference UID for PET.
    pub pet_frame_of_reference_uid: Option<&'a str>,
    /// Scale factor used to convert PET modality values to SUV.
    pub pet_suv_scale_factor: f64,
}

/// Result of deterministic PET-over-CT fusion.
#[derive(Debug, Clone, PartialEq)]
pub struct PetCtFusionResult {
    /// Output dimensions aligned to CT reference grid.
    pub dimensions: [usize; 3],
    /// PET values resampled onto CT grid in SUV units.
    pub pet_on_ct_suv: Vec<f64>,
    /// Deterministic blended overlay bytes (`RGBA` per voxel).
    pub overlay_rgba: Vec<u8>,
    /// Stable cache key fragment.
    pub cache_key: String,
}

/// Validate fail-closed preconditions for PET-over-CT fusion execution.
pub fn validate_fusion_preconditions(
    input: &PetCtFusionInput<'_>,
    limits: FusionExecutionLimits,
) -> Result<()> {
    validate_fusion(FusionInput {
        ct_frame_of_reference_uid: input.ct_frame_of_reference_uid,
        pet_frame_of_reference_uid: input.pet_frame_of_reference_uid,
    })?;
    if !input.ct_volume.has_patient_geometry() || !input.pet_volume.has_patient_geometry() {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidTagValue {
                tag: TAG_FRAME_OF_REFERENCE_UID,
                detail: "fusion requires patient-space geometry on both volumes".to_string(),
            },
            "fusion requires patient-space geometry on both volumes",
        )));
    }
    let dims = input.ct_volume.dimensions();
    let voxel_count = dims[0] as u64 * dims[1] as u64 * dims[2] as u64;
    if voxel_count > limits.max_output_voxels {
        return Err(Box::new(Error::from_kind(
            ErrorKind::LimitExceeded {
                limit_name: "fusion.max_output_voxels",
                observed: voxel_count,
                allowed: limits.max_output_voxels,
            },
            "fusion output exceeds voxel budget",
        )));
    }
    let required_bytes = voxel_count
        .saturating_mul(8)
        .saturating_add(voxel_count.saturating_mul(4));
    if required_bytes > limits.max_output_bytes {
        return Err(Box::new(Error::from_kind(
            ErrorKind::LimitExceeded {
                limit_name: "fusion.max_output_bytes",
                observed: required_bytes,
                allowed: limits.max_output_bytes,
            },
            "fusion output exceeds byte budget",
        )));
    }
    Ok(())
}

/// Resample PET onto CT reference grid and convert to SUV units.
pub fn resample_pet_to_ct_grid(
    input: &PetCtFusionInput<'_>,
    limits: FusionExecutionLimits,
) -> Result<Vec<f64>> {
    validate_fusion_preconditions(input, limits)?;
    let dims = input.ct_volume.dimensions();
    let mut out = Vec::with_capacity(dims[0] * dims[1] * dims[2]);

    for z in 0..dims[2] {
        for y in 0..dims[1] {
            for x in 0..dims[0] {
                let Some(patient) = input.ct_volume.voxel_to_patient_um(x, y, z) else {
                    return Err(Box::new(Error::from_kind(
                        ErrorKind::InvalidTagValue {
                            tag: TAG_FRAME_OF_REFERENCE_UID,
                            detail: "ct voxel-to-patient mapping failed".to_string(),
                        },
                        "ct voxel-to-patient mapping failed",
                    )));
                };
                let Some(pet_voxel) = input.pet_volume.patient_to_voxel_f64([
                    patient[0] as f64,
                    patient[1] as f64,
                    patient[2] as f64,
                ]) else {
                    return Err(Box::new(Error::from_kind(
                        ErrorKind::InvalidTagValue {
                            tag: TAG_FRAME_OF_REFERENCE_UID,
                            detail: "pet patient-to-voxel mapping failed".to_string(),
                        },
                        "pet patient-to-voxel mapping failed",
                    )));
                };
                let pet_value = input
                    .pet_volume
                    .voxel(
                        pet_voxel[0].round() as usize,
                        pet_voxel[1].round() as usize,
                        pet_voxel[2].round() as usize,
                    )
                    .unwrap_or(0) as f64;
                out.push(apply_suv(SuvScaleInput {
                    raw_value: pet_value,
                    scale_factor: input.pet_suv_scale_factor,
                })?);
            }
        }
    }
    Ok(out)
}

/// Blend PET overlay over CT grayscale volume using deterministic policy.
pub fn blend_pet_overlay(
    ct_volume: &dyn VolumeGridProvider,
    pet_on_ct_suv: &[f64],
    policy: FusionBlendPolicy,
    limits: FusionExecutionLimits,
) -> Result<Vec<u8>> {
    let dims = ct_volume.dimensions();
    let voxel_count = dims[0] * dims[1] * dims[2];
    if pet_on_ct_suv.len() != voxel_count {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidGeometry {
                detail: "pet_on_ct_suv length does not match CT volume".to_string(),
            },
            "pet_on_ct_suv length does not match CT volume",
        )));
    }
    let out_bytes = (voxel_count as u64).saturating_mul(4);
    if out_bytes > limits.max_output_bytes {
        return Err(Box::new(Error::from_kind(
            ErrorKind::LimitExceeded {
                limit_name: "fusion.max_output_bytes",
                observed: out_bytes,
                allowed: limits.max_output_bytes,
            },
            "fusion overlay exceeds byte budget",
        )));
    }

    let alpha = policy.alpha_u8 as f64 / 255.0;
    let mut out = Vec::with_capacity(voxel_count * 4);
    for (index, suv) in pet_on_ct_suv.iter().enumerate() {
        let ct_value = ct_volume.voxels()[index].clamp(0, 255) as u8;
        let [pet_r, pet_g, pet_b] = pet_colormap_rgb(*suv, policy.colormap);
        let blended_r = ((1.0 - alpha) * ct_value as f64 + alpha * pet_r as f64).round() as u8;
        let blended_g = ((1.0 - alpha) * ct_value as f64 + alpha * pet_g as f64).round() as u8;
        let blended_b = ((1.0 - alpha) * ct_value as f64 + alpha * pet_b as f64).round() as u8;
        out.extend_from_slice(&[blended_r, blended_g, blended_b, 255]);
    }
    Ok(out)
}

/// Execute deterministic PET-over-CT fusion.
pub fn execute_pet_ct_fusion(
    input: &PetCtFusionInput<'_>,
    limits: FusionExecutionLimits,
    policy: FusionBlendPolicy,
) -> Result<PetCtFusionResult> {
    let pet_on_ct_suv = resample_pet_to_ct_grid(input, limits)?;
    let overlay_rgba = blend_pet_overlay(input.ct_volume, &pet_on_ct_suv, policy, limits)?;
    let dims = input.ct_volume.dimensions();
    let cache_key = format!(
        "pet-ct:{}x{}x{}:alpha{}:scale{:.6}",
        dims[0], dims[1], dims[2], policy.alpha_u8, input.pet_suv_scale_factor
    );
    Ok(PetCtFusionResult {
        dimensions: dims,
        pet_on_ct_suv,
        overlay_rgba,
        cache_key,
    })
}

fn pet_colormap_rgb(value: f64, colormap: PetColormap) -> [u8; 3] {
    match colormap {
        PetColormap::HotIron => {
            if !value.is_finite() || value <= 0.0 {
                return [0, 0, 0];
            }
            let normalized = (value / 8.0).clamp(0.0, 1.0);
            let red = (normalized * 255.0).round() as u8;
            let green = ((normalized * normalized) * 255.0).round() as u8;
            let blue = ((normalized * normalized * normalized) * 180.0).round() as u8;
            [red, green, blue]
        }
    }
}

/// Helper function for missing_required_tag
pub fn missing_required_tag(tag: Tag) -> Box<Error> {
    Error::from_kind(
        ErrorKind::MissingRequiredTag { tag },
        "missing required tag",
    )
    .with_context("tag", format!("{tag:?}"))
    .into()
}
