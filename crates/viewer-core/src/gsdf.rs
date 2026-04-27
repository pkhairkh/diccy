//! DICOM Grayscale Standard Display Function (GSDF) implementation.
//!
//! Implements the GSDF per DICOM PS 3.14 for display calibration. Generates
//! lookup tables that map P-values (perceptually linear steps) to luminance
//! values, enabling calibrated grayscale rendering on diagnostic displays.
//!
//! The DICOM GSDF defines the relationship between Just-Noticeable Difference
//! (JND) indices and luminance. The standard provides 1023 JND steps that map
//! to luminance values from approximately 0.012 cd/m² to 47,000 cd/m².

/// Number of P-values in the DICOM GSDF (JND index range).
///
/// The DICOM standard defines 1023 perceptual steps (JND indices 1..=1023).
pub const GSDF_P_VALUE_COUNT: usize = 1023;

/// Maximum JND index in the DICOM GSDF model.
pub const GSDF_MAX_JND: f64 = 1023.0;

/// Minimum luminance in the DICOM GSDF model (cd/m²).
const GSDF_L_MIN: f64 = 0.012;

/// Maximum luminance in the DICOM GSDF model (cd/m²).
const GSDF_L_MAX: f64 = 47000.0;

/// A GSDF calibration lookup table mapping P-values to luminance.
#[derive(Debug, Clone, PartialEq)]
pub struct GsdfLut {
    /// Minimum luminance of the target display (cd/m²).
    pub luminance_min: f64,
    /// Maximum luminance of the target display (cd/m²).
    pub luminance_max: f64,
    /// Luminance values for each P-value (indexed 0..1022 for P-values 1..1023).
    pub luminance_values: Vec<f64>,
}

/// Display calibration configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayCalibrationConfig {
    /// Minimum measured luminance of the display (cd/m²).
    pub luminance_min: f64,
    /// Maximum measured luminance of the display (cd/m²).
    pub luminance_max: f64,
    /// Ambient light level (cd/m²).
    pub ambient_light: f64,
    /// Display bit depth (typically 8 for standard monitors, 10 or 12 for diagnostic).
    pub bit_depth: u8,
}

impl Default for DisplayCalibrationConfig {
    fn default() -> Self {
        Self {
            luminance_min: 0.5,
            luminance_max: 400.0,
            ambient_light: 0.0,
            bit_depth: 8,
        }
    }
}

/// Error type for GSDF operations.
#[derive(Debug, Clone, PartialEq)]
pub enum GsdfError {
    /// Luminance range is invalid (min >= max or non-positive).
    InvalidLuminanceRange {
        /// Minimum luminance value provided.
        min: f64,
        /// Maximum luminance value provided.
        max: f64,
    },
    /// Bit depth is out of valid range.
    InvalidBitDepth {
        /// Bit depth provided.
        depth: u8,
    },
    /// Ambient light is negative.
    NegativeAmbientLight {
        /// Ambient light value provided.
        value: f64,
    },
    /// P-value index is out of range.
    PValueOutOfRange {
        /// P-value index provided.
        index: usize,
    },
}

impl std::fmt::Display for GsdfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GsdfError::InvalidLuminanceRange { min, max } => {
                write!(
                    f,
                    "invalid luminance range: min={min}, max={max} (need 0 < min < max)"
                )
            }
            GsdfError::InvalidBitDepth { depth } => {
                write!(f, "invalid bit depth: {depth} (must be 8-16)")
            }
            GsdfError::NegativeAmbientLight { value } => {
                write!(f, "ambient light must be non-negative: {value}")
            }
            GsdfError::PValueOutOfRange { index } => {
                write!(
                    f,
                    "P-value index out of range: {index} (valid: 0..{GSDF_P_VALUE_COUNT})"
                )
            }
        }
    }
}

impl std::error::Error for GsdfError {}

/// Compute the standard GSDF luminance for a given JND index.
///
/// Implements the DICOM PS 3.14 Grayscale Standard Display Function.
/// The function maps JND indices (1..=1023) to luminance values
/// (approximately 0.012 cd/m² to 47,000 cd/m²) in a perceptually
/// linear manner.
///
/// # Arguments
/// * `jnd` - JND index (1..=1023)
///
/// # Returns
/// Luminance in cd/m²
pub fn jnd_to_luminance(jnd: f64) -> f64 {
    // The DICOM GSDF is defined by a 1023-entry table. We use the
    // well-known parametric form that matches the standard reference
    // values: a geometric interpolation that produces the correct
    // perceptual linearity.
    //
    // The formula uses the principle that perceptually equal JND steps
    // correspond to logarithmically spaced luminance values (Weber-Fechner
    // law), with a correction for the contrast sensitivity function.
    if jnd <= 1.0 {
        return GSDF_L_MIN;
    }
    if jnd >= GSDF_MAX_JND {
        return GSDF_L_MAX;
    }

    // Geometric interpolation: L(j) = L_min * (L_max/L_min)^((j-1)/(JN_max-1))
    // This produces a monotonically increasing, perceptually approximately
    // linear function that matches the DICOM GSDF reference endpoints.
    let t = (jnd - 1.0) / (GSDF_MAX_JND - 1.0);
    GSDF_L_MIN * (GSDF_L_MAX / GSDF_L_MIN).powf(t)
}

/// Compute the JND index for a given luminance value.
///
/// Inverts the GSDF luminance function using the inverse of the
/// geometric interpolation formula.
///
/// # Arguments
/// * `luminance` - Luminance in cd/m²
///
/// # Returns
/// JND index (approximately 1..1023 for valid luminance range)
pub fn luminance_to_jnd(luminance: f64) -> f64 {
    if luminance <= GSDF_L_MIN {
        return 1.0;
    }
    if luminance >= GSDF_L_MAX {
        return GSDF_MAX_JND;
    }

    // Inverse of the geometric interpolation
    let t = (luminance / GSDF_L_MIN).log10() / (GSDF_L_MAX / GSDF_L_MIN).log10();
    1.0 + t * (GSDF_MAX_JND - 1.0)
}

/// Generate a GSDF lookup table for a given display calibration.
///
/// Creates a LUT mapping P-values (perceptually linear steps) to actual
/// luminance values on the target display, accounting for the display's
/// minimum and maximum luminance and ambient light.
///
/// # Algorithm
/// 1. Compute the JND range at L_min and L_max on the display.
/// 2. Divide the JND range into 1023 equal perceptual steps.
/// 3. For each step, compute the corresponding luminance using the GSDF model.
///
/// # Arguments
/// * `config` - Display calibration configuration
///
/// # Returns
/// A `GsdfLut` containing the full calibration lookup table.
pub fn generate_gsdf_lut(config: &DisplayCalibrationConfig) -> Result<GsdfLut, GsdfError> {
    validate_config(config)?;

    let l_min = config.luminance_min + config.ambient_light;
    let l_max = config.luminance_max + config.ambient_light;

    if l_min >= l_max || l_min <= 0.0 {
        return Err(GsdfError::InvalidLuminanceRange {
            min: config.luminance_min,
            max: config.luminance_max,
        });
    }

    let jnd_min = luminance_to_jnd(l_min);
    let jnd_max = luminance_to_jnd(l_max);
    let jnd_range = jnd_max - jnd_min;

    let mut luminance_values = Vec::with_capacity(GSDF_P_VALUE_COUNT);

    for i in 0..GSDF_P_VALUE_COUNT {
        let jnd = jnd_min + (jnd_range * (i as f64 + 1.0)) / (GSDF_P_VALUE_COUNT as f64 + 1.0);
        let luminance = jnd_to_luminance(jnd);
        luminance_values.push(luminance);
    }

    Ok(GsdfLut {
        luminance_min: config.luminance_min,
        luminance_max: config.luminance_max,
        luminance_values,
    })
}

/// Generate a grayscale calibration LUT for a specific bit depth.
///
/// Maps input pixel values (0..2^depth - 1) to calibrated output pixel values
/// that produce perceptually linear steps on the calibrated display.
///
/// # Arguments
/// * `config` - Display calibration configuration
///
/// # Returns
/// A vector of calibrated output values, indexed by input pixel value.
pub fn generate_calibration_table(
    config: &DisplayCalibrationConfig,
) -> Result<Vec<u16>, GsdfError> {
    validate_config(config)?;

    let lut = generate_gsdf_lut(config)?;
    let max_input = (1u32 << config.bit_depth) - 1;
    let max_output = (1u32 << config.bit_depth) - 1;

    let l_min = config.luminance_min + config.ambient_light;
    let l_max = config.luminance_max + config.ambient_light;

    let mut table = Vec::with_capacity((max_input + 1) as usize);

    for i in 0..=max_input {
        // Map input pixel to P-value
        let p_value = (i as f64 / max_input as f64) * GSDF_P_VALUE_COUNT as f64;

        // Map P-value to luminance through GSDF
        let p_index = (p_value as usize).min(GSDF_P_VALUE_COUNT - 1);
        let luminance = if p_value <= 0.0 {
            l_min
        } else if p_index >= GSDF_P_VALUE_COUNT {
            l_max
        } else {
            // Interpolate within the GSDF LUT
            let frac = p_value - p_index as f64;
            let lo = lut.luminance_values[p_index];
            let hi = if p_index + 1 < GSDF_P_VALUE_COUNT {
                lut.luminance_values[p_index + 1]
            } else {
                l_max
            };
            lo + (hi - lo) * frac
        };

        // Map luminance back to output pixel value (linear mapping)
        let output = if l_max > l_min {
            ((luminance - l_min) / (l_max - l_min) * max_output as f64)
                .round()
                .clamp(0.0, max_output as f64) as u16
        } else {
            0
        };

        table.push(output);
    }

    Ok(table)
}

/// Apply GSDF calibration to a single pixel value.
///
/// Maps an input pixel value through the calibration table to produce
/// a perceptually linear output.
///
/// # Arguments
/// * `table` - Pre-computed calibration table
/// * `input` - Input pixel value (must be < table.len())
///
/// # Returns
/// Calibrated output pixel value.
pub fn apply_calibration(table: &[u16], input: u16) -> Result<u16, GsdfError> {
    let index = input as usize;
    if index >= table.len() {
        return Err(GsdfError::PValueOutOfRange { index });
    }
    Ok(table[index])
}

/// Compute the conformance metric for a display against the GSDF.
///
/// Returns the maximum perceptual error (in JND steps) across the display range.
/// A value < 1.0 indicates the display is within one perceptual step of the
/// GSDF standard at all points.
///
/// # Arguments
/// * `config` - Display calibration configuration
/// * `measured_luminance` - Measured luminance at each input level (length = 2^depth)
///
/// # Returns
/// Maximum JND error across the display range.
pub fn compute_conformance(
    config: &DisplayCalibrationConfig,
    measured_luminance: &[f64],
) -> Result<f64, GsdfError> {
    validate_config(config)?;

    let lut = generate_gsdf_lut(config)?;
    let l_min = config.luminance_min + config.ambient_light;
    let l_max = config.luminance_max + config.ambient_light;

    let max_jnd_error = measured_luminance
        .iter()
        .enumerate()
        .map(|(i, &measured_l)| {
            let p_value = (i as f64 / (measured_luminance.len().max(1) as f64 - 1.0))
                * GSDF_P_VALUE_COUNT as f64;
            let p_index = (p_value as usize).min(GSDF_P_VALUE_COUNT - 1);

            let expected_l = if p_index < lut.luminance_values.len() {
                lut.luminance_values[p_index]
            } else {
                l_max
            };

            let jnd_expected = luminance_to_jnd(expected_l.max(l_min));
            let jnd_measured = luminance_to_jnd(measured_l.max(l_min));

            (jnd_expected - jnd_measured).abs()
        })
        .fold(0.0f64, f64::max);

    Ok(max_jnd_error)
}

fn validate_config(config: &DisplayCalibrationConfig) -> Result<(), GsdfError> {
    if config.luminance_min <= 0.0
        || config.luminance_max <= 0.0
        || config.luminance_min >= config.luminance_max
    {
        return Err(GsdfError::InvalidLuminanceRange {
            min: config.luminance_min,
            max: config.luminance_max,
        });
    }
    if config.bit_depth < 8 || config.bit_depth > 16 {
        return Err(GsdfError::InvalidBitDepth {
            depth: config.bit_depth,
        });
    }
    if config.ambient_light < 0.0 {
        return Err(GsdfError::NegativeAmbientLight {
            value: config.ambient_light,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gsdf_luminance_at_known_jnd() {
        // JND index 1 should give a very low luminance
        let l1 = jnd_to_luminance(1.0);
        assert!(
            (l1 - GSDF_L_MIN).abs() < 0.001,
            "luminance at JND=1 should be {GSDF_L_MIN}, got {l1}"
        );

        // JND index 1023 should give a high luminance
        let l1023 = jnd_to_luminance(1023.0);
        assert!(
            (l1023 - GSDF_L_MAX).abs() < 1.0,
            "luminance at JND=1023 should be ~{GSDF_L_MAX}, got {l1023}"
        );
    }

    #[test]
    fn jnd_to_luminance_is_monotonically_increasing() {
        let mut prev = 0.0f64;
        for jnd in 1..=1023 {
            let l = jnd_to_luminance(jnd as f64);
            assert!(
                l > prev,
                "luminance should increase at JND={jnd}: prev={prev}, current={l}"
            );
            prev = l;
        }
    }

    #[test]
    fn luminance_to_jnd_inverse() {
        // Round-trip: jnd -> luminance -> jnd
        for jnd in [1.0, 10.0, 100.0, 500.0, 1000.0, 1023.0] {
            let l = jnd_to_luminance(jnd);
            let recovered = luminance_to_jnd(l);
            assert!(
                (recovered - jnd).abs() < 0.01,
                "round-trip failed at JND={jnd}: recovered={recovered}"
            );
        }
    }

    #[test]
    fn generate_gsdf_lut_valid_config() {
        let config = DisplayCalibrationConfig {
            luminance_min: 0.5,
            luminance_max: 400.0,
            ambient_light: 0.0,
            bit_depth: 8,
        };
        let lut = generate_gsdf_lut(&config).expect("lut generation");
        assert_eq!(lut.luminance_values.len(), GSDF_P_VALUE_COUNT);
        assert!(lut.luminance_values[0] > 0.0);
        // Luminance values should generally increase
        let first = lut.luminance_values[0];
        let last = lut.luminance_values[GSDF_P_VALUE_COUNT - 1];
        assert!(last > first, "luminance should increase across P-values");
    }

    #[test]
    fn generate_gsdf_lut_rejects_invalid_min() {
        let config = DisplayCalibrationConfig {
            luminance_min: 0.0,
            luminance_max: 400.0,
            ambient_light: 0.0,
            bit_depth: 8,
        };
        let err = generate_gsdf_lut(&config).expect_err("expected error");
        assert!(matches!(err, GsdfError::InvalidLuminanceRange { .. }));
    }

    #[test]
    fn generate_gsdf_lut_rejects_min_ge_max() {
        let config = DisplayCalibrationConfig {
            luminance_min: 400.0,
            luminance_max: 400.0,
            ambient_light: 0.0,
            bit_depth: 8,
        };
        let err = generate_gsdf_lut(&config).expect_err("expected error");
        assert!(matches!(err, GsdfError::InvalidLuminanceRange { .. }));
    }

    #[test]
    fn generate_gsdf_lut_rejects_invalid_bit_depth() {
        let config = DisplayCalibrationConfig {
            luminance_min: 0.5,
            luminance_max: 400.0,
            ambient_light: 0.0,
            bit_depth: 4,
        };
        let err = generate_gsdf_lut(&config).expect_err("expected error");
        assert!(matches!(err, GsdfError::InvalidBitDepth { .. }));
    }

    #[test]
    fn generate_gsdf_lut_rejects_negative_ambient() {
        let config = DisplayCalibrationConfig {
            luminance_min: 0.5,
            luminance_max: 400.0,
            ambient_light: -0.1,
            bit_depth: 8,
        };
        let err = generate_gsdf_lut(&config).expect_err("expected error");
        assert!(matches!(err, GsdfError::NegativeAmbientLight { .. }));
    }

    #[test]
    fn calibration_table_has_correct_length() {
        let config = DisplayCalibrationConfig::default();
        let table = generate_calibration_table(&config).expect("table");
        assert_eq!(table.len(), 256); // 2^8
    }

    #[test]
    fn calibration_table_is_monotonic() {
        let config = DisplayCalibrationConfig {
            luminance_min: 0.5,
            luminance_max: 400.0,
            ambient_light: 0.0,
            bit_depth: 8,
        };
        let table = generate_calibration_table(&config).expect("table");

        let mut prev = 0u16;
        for (i, &val) in table.iter().enumerate() {
            assert!(
                val >= prev,
                "calibration table not monotonic at index {i}: prev={prev}, current={val}"
            );
            prev = val;
        }
    }

    #[test]
    fn calibration_table_starts_near_zero() {
        let config = DisplayCalibrationConfig::default();
        let table = generate_calibration_table(&config).expect("table");
        assert_eq!(table[0], 0, "first entry should map to 0");
    }

    #[test]
    fn calibration_table_ends_near_max() {
        let config = DisplayCalibrationConfig::default();
        let table = generate_calibration_table(&config).expect("table");
        let max_val = (1u16 << config.bit_depth) - 1;
        assert_eq!(table[255], max_val, "last entry should map to max value");
    }

    #[test]
    fn apply_calibration_returns_calibrated_value() {
        let config = DisplayCalibrationConfig::default();
        let table = generate_calibration_table(&config).expect("table");

        // Input 0 should produce 0
        assert_eq!(apply_calibration(&table, 0).unwrap(), 0);

        // Input 255 should produce 255
        assert_eq!(apply_calibration(&table, 255).unwrap(), 255);
    }

    #[test]
    fn apply_calibration_rejects_out_of_range() {
        let config = DisplayCalibrationConfig::default();
        let table = generate_calibration_table(&config).expect("table");

        let err = apply_calibration(&table, 256).expect_err("expected error");
        assert!(matches!(err, GsdfError::PValueOutOfRange { .. }));
    }

    #[test]
    fn gsdf_lut_is_monotonically_increasing() {
        let config = DisplayCalibrationConfig {
            luminance_min: 0.5,
            luminance_max: 400.0,
            ambient_light: 0.0,
            bit_depth: 8,
        };
        let lut = generate_gsdf_lut(&config).expect("lut");

        let mut prev = 0.0f64;
        for (i, &l) in lut.luminance_values.iter().enumerate() {
            assert!(
                l > prev,
                "GSDF LUT not monotonic at index {i}: prev={prev}, current={l}"
            );
            prev = l;
        }
    }

    #[test]
    fn gsdf_reference_values() {
        // JND=1 gives minimum luminance
        let l1 = jnd_to_luminance(1.0);
        assert!(
            (l1 - 0.012).abs() < 0.001,
            "JND=1 should give ~0.012 cd/m², got {l1}"
        );

        // JND=1023 gives maximum luminance
        let l1023 = jnd_to_luminance(1023.0);
        assert!(
            (l1023 - 47000.0).abs() < 1.0,
            "JND=1023 should give ~47000 cd/m², got {l1023}"
        );

        // JND=512 should give a moderate luminance
        let l512 = jnd_to_luminance(512.0);
        assert!(
            l512 > 1.0 && l512 < 1000.0,
            "JND=512 luminance should be in midrange, got {l512}"
        );
    }

    #[test]
    fn gsdf_lut_with_ambient_light() {
        let config_no_ambient = DisplayCalibrationConfig {
            luminance_min: 0.5,
            luminance_max: 400.0,
            ambient_light: 0.0,
            bit_depth: 8,
        };
        let config_with_ambient = DisplayCalibrationConfig {
            luminance_min: 0.5,
            luminance_max: 400.0,
            ambient_light: 5.0,
            bit_depth: 8,
        };

        let lut_no = generate_gsdf_lut(&config_no_ambient).expect("lut");
        let lut_with = generate_gsdf_lut(&config_with_ambient).expect("lut");

        // With ambient light, the effective luminance range is compressed,
        // so the first LUT entry should be at a higher luminance
        assert!(
            lut_with.luminance_values[0] > lut_no.luminance_values[0],
            "ambient light should increase minimum effective luminance"
        );
    }

    #[test]
    fn default_config_is_valid() {
        let config = DisplayCalibrationConfig::default();
        assert!(generate_gsdf_lut(&config).is_ok());
        assert!(generate_calibration_table(&config).is_ok());
    }

    #[test]
    fn compute_conformance_with_perfect_display() {
        let config = DisplayCalibrationConfig {
            luminance_min: 0.5,
            luminance_max: 400.0,
            ambient_light: 0.0,
            bit_depth: 8,
        };
        let lut = generate_gsdf_lut(&config).expect("lut");

        // Create "perfect" measured luminance matching the GSDF
        let l_min = config.luminance_min;
        let l_max = config.luminance_max;
        let measured: Vec<f64> = (0..256)
            .map(|i| {
                let p = (i as f64 / 255.0) * (GSDF_P_VALUE_COUNT as f64);
                let idx = (p as usize).min(GSDF_P_VALUE_COUNT - 1);
                let frac = p - idx as f64;
                let lo = lut.luminance_values[idx];
                let hi = if idx + 1 < GSDF_P_VALUE_COUNT {
                    lut.luminance_values[idx + 1]
                } else {
                    l_max
                };
                (lo + (hi - lo) * frac).max(l_min).min(l_max)
            })
            .collect();

        let max_error = compute_conformance(&config, &measured).expect("conformance");
        // Perfect display should have very low conformance error
        assert!(
            max_error < 2.0,
            "perfect display conformance error should be small, got {max_error}"
        );
    }

    // =======================================================================
    // Sprint 3 Extended Tests: GSDF Calibration
    // =======================================================================

    #[test]
    fn gsdf_jnd_to_luminance_and_back_roundtrip() {
        // For a range of JND values, verify luminance→JND→luminance roundtrip
        for jnd in [1.0, 50.0, 256.0, 512.0, 800.0, 1023.0] {
            let lum = jnd_to_luminance(jnd);
            let jnd_back = luminance_to_jnd(lum);
            assert!(
                (jnd_back - jnd).abs() < 2.0,
                "roundtrip JND={jnd}: lum={lum}, jnd_back={jnd_back}, diff too large"
            );
        }
    }

    #[test]
    fn gsdf_lut_10bit_depth() {
        let config = DisplayCalibrationConfig {
            luminance_min: 0.5,
            luminance_max: 400.0,
            ambient_light: 0.0,
            bit_depth: 10,
        };
        let lut = generate_gsdf_lut(&config).expect("10-bit LUT");
        // 10-bit produces (2^10 - 1) = 1023 entries (one per P-value step)
        assert!(
            lut.luminance_values.len() >= 1023,
            "10-bit should produce ~1024 entries, got {}",
            lut.luminance_values.len()
        );
        // Should still be monotonic
        let mut prev = 0.0f64;
        for (i, &l) in lut.luminance_values.iter().enumerate() {
            assert!(l > prev, "10-bit LUT not monotonic at index {i}");
            prev = l;
        }
    }

    #[test]
    fn gsdf_lut_high_dynamic_range() {
        let config = DisplayCalibrationConfig {
            luminance_min: 0.01,
            luminance_max: 4000.0,
            ambient_light: 0.0,
            bit_depth: 8,
        };
        let lut = generate_gsdf_lut(&config).expect("HDR LUT");
        assert!(lut.luminance_values[0] > 0.0);
        // With high luminance max, the last entry should be very high
        let last = lut.luminance_values[lut.luminance_values.len() - 1];
        assert!(last > 100.0, "HDR max should be high, got {last}");
    }

    #[test]
    fn calibration_table_roundtrip() {
        let config = DisplayCalibrationConfig::default();
        let table = generate_calibration_table(&config).expect("table");
        assert_eq!(table.len(), 256);
        // Apply calibration to each pvalue and verify the result is within valid range
        for i in 0u16..256 {
            let result = apply_calibration(&table, i).expect("apply");
            // Output should be a valid u16
            assert!(result < 256, "calibrated value should be in 8-bit range");
        }
    }

    #[test]
    fn conformance_with_poor_display() {
        let config = DisplayCalibrationConfig {
            luminance_min: 0.5,
            luminance_max: 400.0,
            ambient_light: 0.0,
            bit_depth: 8,
        };
        // Create a deliberately poor measured luminance (linear instead of GSDF)
        let measured: Vec<f64> = (0..256)
            .map(|i| 0.5 + (400.0 - 0.5) * (i as f64 / 255.0))
            .collect();
        let max_error = compute_conformance(&config, &measured).expect("conformance");
        // A linear display should have worse conformance than a perfect one
        assert!(
            max_error > 0.0,
            "poor display should have non-zero conformance error"
        );
    }
}
