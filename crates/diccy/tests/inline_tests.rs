// Auto-extracted from /home/z/diccy/crates/diccy/src/lib.rs
// S13-T8: Move inline tests to tests/ directories

use diccy::*;

#[test]
#[cfg(all(
    feature = "tier1-deflate",
    feature = "codec-jpegls",
    feature = "codec-j2k",
    feature = "raster-io",
    feature = "gsps",
    feature = "pack-enhanced",
    feature = "pack-us",
    feature = "pack-nm",
    feature = "pack-xa",
    feature = "pack-seg",
    feature = "pack-rt",
    feature = "pack-sr",
    feature = "modality-ct",
    feature = "modality-pet",
    feature = "modality-mg",
    feature = "modality-xr"
))]
fn capabilities_all_features_enabled() {
    // REQ-FEAT-303, REQ-API-206: capability queries must reflect enabled features.
    let caps = capabilities();
    assert!(caps.tier1_deflate);
    assert!(caps.codec_jpegls);
    assert!(caps.codec_j2k);
    assert!(caps.raster_io);
    assert!(caps.gsps);
    assert!(caps.pack_enhanced);
    assert!(caps.pack_us);
    assert!(caps.pack_nm);
    assert!(caps.pack_xa);
    assert!(caps.pack_seg);
    assert!(caps.pack_rt);
    assert!(caps.pack_sr);
    assert!(caps.modality_ct);
    assert!(caps.modality_pet);
    assert!(caps.modality_mg);
    assert!(caps.modality_xr);
}

#[test]
#[cfg(not(any(
    feature = "tier1-deflate",
    feature = "codec-jpegls",
    feature = "codec-j2k",
    feature = "raster-io",
    feature = "gsps",
    feature = "pack-enhanced",
    feature = "pack-us",
    feature = "pack-nm",
    feature = "pack-xa",
    feature = "pack-seg",
    feature = "pack-rt",
    feature = "pack-sr",
    feature = "modality-ct",
    feature = "modality-pet",
    feature = "modality-mg",
    feature = "modality-xr"
)))]
fn capabilities_default_features_disabled() {
    // REQ-FEAT-303, REQ-API-206: default profile keeps optional DICCY features disabled.
    assert!(!cfg!(feature = "tier1-deflate"));
    assert!(!cfg!(feature = "codec-jpegls"));
    assert!(!cfg!(feature = "codec-j2k"));
    assert!(!cfg!(feature = "raster-io"));
    assert!(!cfg!(feature = "gsps"));
    assert!(!cfg!(feature = "pack-enhanced"));
    assert!(!cfg!(feature = "pack-us"));
    assert!(!cfg!(feature = "pack-nm"));
    assert!(!cfg!(feature = "pack-xa"));
    assert!(!cfg!(feature = "pack-seg"));
    assert!(!cfg!(feature = "pack-rt"));
    assert!(!cfg!(feature = "pack-sr"));
    assert!(!cfg!(feature = "modality-ct"));
    assert!(!cfg!(feature = "modality-pet"));
    assert!(!cfg!(feature = "modality-mg"));
    assert!(!cfg!(feature = "modality-xr"));

    // Capability detection and default config must stay consistent.
    let caps = capabilities();
    let config = Config::default();
    assert_eq!(config.limits, Limits::default());
    assert_eq!(config.capabilities, caps);
}

#[test]
fn config_roundtrip_default() {
    // REQ-ARCH-121, REQ-ARCH-122, REQ-API-207: config must be serializable and stable.
    let config = Config::default();
    let serialized = config.serialize();
    let deserialized = Config::deserialize(&serialized).expect("deserialize config");
    assert_eq!(deserialized, config);
}

#[test]
fn config_json_roundtrip_default() {
    // REQ-ARCH-121: JSON wrapper must round-trip through parser.
    let config = Config::default();
    let json = config.to_json_string();
    let parsed = Config::from_json_string(&json).expect("parse json config");
    assert_eq!(parsed, config);
}

#[test]
fn config_rejects_unknown_header() {
    // REQ-ARCH-121: format version must be validated.
    let config = Config::default();
    let serialized = config.serialize();
    let payload = serialized
        .split_once('\n')
        .map(|(_, rest)| rest)
        .unwrap_or_default();
    let bad = format!("diccy_config_v2\n{payload}");
    assert_eq!(
        Config::deserialize(&bad),
        Err(ConfigParseError::MissingHeader)
    );
}

#[test]
fn config_builder_overrides() {
    // REQ-ARCH-122, REQ-API-207: builder must wire limits and capabilities.
    let mut limits = Limits::default();
    limits.set_max_input_bytes(42);
    let mut caps = capabilities();
    caps.set_tier1_deflate(!caps.tier1_deflate());

    let config = Config::builder()
        .limits(limits.clone())
        .capabilities(caps.clone())
        .build();

    assert_eq!(config.limits, limits);
    assert_eq!(config.capabilities, caps);
}

#[test]
fn config_rejects_unknown_key() {
    // REQ-ARCH-121: unknown keys must be rejected.
    let mut serialized = Config::default().serialize();
    serialized.push_str("limits.unknown=1\n");
    assert_eq!(
        Config::deserialize(&serialized),
        Err(ConfigParseError::UnknownKey {
            key: "limits.unknown".to_string()
        })
    );
}

#[test]
fn config_rejects_invalid_value() {
    // REQ-ARCH-121: invalid values must be rejected.
    let serialized = Config::default().serialize();
    let bad = serialized.replace(
        "limits.max_input_bytes=536870912",
        "limits.max_input_bytes=bad",
    );
    assert_eq!(
        Config::deserialize(&bad),
        Err(ConfigParseError::InvalidValue {
            key: "limits.max_input_bytes".to_string(),
            value: "bad".to_string()
        })
    );
}

#[test]
fn config_rejects_duplicate_key() {
    // REQ-ARCH-121: duplicate keys must be rejected.
    let mut serialized = Config::default().serialize();
    serialized.push_str("limits.max_input_bytes=1\n");
    assert_eq!(
        Config::deserialize(&serialized),
        Err(ConfigParseError::DuplicateKey {
            key: "limits.max_input_bytes"
        })
    );
}

#[test]
fn config_rejects_missing_field() {
    // REQ-ARCH-121: missing fields must be rejected.
    let serialized = Config::default().serialize();
    let mut lines: Vec<&str> = serialized.lines().collect();
    lines.retain(|line| !line.starts_with("limits.max_cache_bytes="));
    let stripped = lines.join("\n");
    assert_eq!(
        Config::deserialize(&stripped),
        Err(ConfigParseError::MissingField {
            key: "limits.max_cache_bytes"
        })
    );
}

#[test]
fn config_json_rejects_unknown_key() {
    // REQ-ARCH-121: JSON wrapper must validate keys.
    let json = "{\"format\":\"diccy_config_v1\",\"payload\":\"diccy_config_v1\\nlimits.max_input_bytes=1\\n\",\"extra\":\"oops\"}";
    assert_eq!(
        Config::from_json_string(json),
        Err(ConfigParseError::UnknownJsonKey {
            key: "extra".to_string()
        })
    );
}
