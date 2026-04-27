use diccy::{capabilities, Capabilities, Config, ConfigParseError, Limits};

fn serialized_keys(serialized: &str) -> Vec<String> {
    serialized
        .lines()
        .skip(1)
        .filter_map(|line| line.split_once('=').map(|(key, _)| key.to_string()))
        .collect()
}

#[test]
fn runtime_config_serialization_uses_version_header_and_stable_key_order() {
    // REQ-HI-370
    let config = Config::default();
    let serialized = config.serialize();
    let lines: Vec<&str> = serialized.lines().collect();
    assert_eq!(lines.first().copied(), Some("diccy_config_v1"));

    let keys = serialized_keys(&serialized);
    let expected = vec![
        "limits.max_input_bytes",
        "limits.max_dataset_elements",
        "limits.max_sequence_depth",
        "limits.max_string_bytes",
        "limits.max_element_vl_bytes",
        "limits.max_frames_per_instance",
        "limits.max_pixels_per_frame",
        "limits.max_decompressed_bytes",
        "limits.max_gpu_texture_bytes",
        "limits.max_cache_bytes",
        "capabilities.tier1_deflate",
        "capabilities.codec_jpegls",
        "capabilities.codec_j2k",
        "capabilities.raster_io",
        "capabilities.gsps",
        "capabilities.modality_ct",
        "capabilities.modality_pet",
        "capabilities.modality_mg",
        "capabilities.modality_xr",
        "capabilities.pack_enhanced",
        "capabilities.pack_us",
        "capabilities.pack_nm",
        "capabilities.pack_xa",
        "capabilities.pack_seg",
        "capabilities.pack_rt",
        "capabilities.pack_sr",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
    assert_eq!(keys, expected);

    // Stable ordering must be deterministic across repeated serialization.
    assert_eq!(serialized, config.serialize());
}

#[test]
fn runtime_config_parser_rejects_unknown_duplicate_missing_and_invalid_entries() {
    // REQ-HI-371
    let serialized = Config::default().serialize();

    let mut unknown = serialized.clone();
    unknown.push_str("limits.unknown=1\n");
    assert_eq!(
        Config::deserialize(&unknown),
        Err(ConfigParseError::UnknownKey {
            key: "limits.unknown".to_string(),
        })
    );

    let mut duplicate = serialized.clone();
    duplicate.push_str("limits.max_input_bytes=1\n");
    assert_eq!(
        Config::deserialize(&duplicate),
        Err(ConfigParseError::DuplicateKey {
            key: "limits.max_input_bytes",
        })
    );

    let missing = serialized
        .lines()
        .filter(|line| !line.starts_with("limits.max_cache_bytes="))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        Config::deserialize(&missing),
        Err(ConfigParseError::MissingField {
            key: "limits.max_cache_bytes",
        })
    );

    let invalid = serialized.replace(
        "limits.max_input_bytes=536870912",
        "limits.max_input_bytes=bad",
    );
    assert_eq!(
        Config::deserialize(&invalid),
        Err(ConfigParseError::InvalidValue {
            key: "limits.max_input_bytes".to_string(),
            value: "bad".to_string(),
        })
    );
}

#[test]
fn runtime_config_json_wrapper_enforces_format_payload_and_key_uniqueness() {
    // REQ-HI-372
    let config = Config::default();
    let json = config.to_json_string();
    assert_eq!(Config::from_json_string(&json).expect("roundtrip"), config);

    let missing_format = format!("{{\"payload\":\"{}\"}}", config.serialize());
    assert_eq!(
        Config::from_json_string(&missing_format),
        Err(ConfigParseError::MissingJsonKey { key: "format" })
    );

    let duplicate_format = format!(
        "{{\"format\":\"diccy_config_v1\",\"format\":\"diccy_config_v1\",\"payload\":\"{}\"}}",
        config.serialize()
    );
    assert_eq!(
        Config::from_json_string(&duplicate_format),
        Err(ConfigParseError::DuplicateJsonKey {
            key: "format".to_string(),
        })
    );

    let unknown = format!(
        "{{\"format\":\"diccy_config_v1\",\"payload\":\"{}\",\"extra\":\"oops\"}}",
        config.serialize()
    );
    assert_eq!(
        Config::from_json_string(&unknown),
        Err(ConfigParseError::UnknownJsonKey {
            key: "extra".to_string(),
        })
    );
}

#[test]
fn runtime_capability_and_builder_contracts_are_deterministic() {
    // REQ-HI-373, REQ-HI-374
    let caps = capabilities();
    let default_config = Config::default();
    assert_eq!(default_config.capabilities, caps);

    let mut custom_limits = Limits::default();
    custom_limits.set_max_input_bytes(123);
    custom_limits.set_max_cache_bytes(456);
    let mut custom_caps = Capabilities::new(
        !caps.tier1_deflate(),
        !caps.codec_jpegls(),
        !caps.codec_j2k(),
        !caps.raster_io(),
        !caps.gsps(),
        !caps.modality_ct(),
        !caps.modality_pet(),
        !caps.modality_mg(),
        !caps.modality_xr(),
        !caps.pack_enhanced(),
        !caps.pack_us(),
        !caps.pack_nm(),
        !caps.pack_xa(),
        !caps.pack_seg(),
        !caps.pack_rt(),
        !caps.pack_sr(),
    );

    let built = Config::builder()
        .limits(custom_limits.clone())
        .capabilities(custom_caps.clone())
        .build();
    assert_eq!(built.limits, custom_limits);
    assert_eq!(built.capabilities, custom_caps);

    // Builder overrides must produce deterministic serialized output.
    assert_eq!(built.serialize(), built.serialize());
}
