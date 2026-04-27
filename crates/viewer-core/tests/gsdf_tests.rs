// Auto-extracted from /home/z/diccy/crates/viewer-core/src/gsdf.rs
// S13-T8: Move inline tests to tests/ directories

use viewer_core::*;

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
