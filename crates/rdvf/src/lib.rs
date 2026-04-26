#![deny(missing_docs)]

//! RDVF public API facade.

mod extensions;

pub use dicom_core::{
    capabilities, Capabilities, Dataset, Element, Error, ErrorKind, Limits, Result, Tag, Value, Vr,
};
#[cfg(feature = "raster-io")]
pub use dicom_pixel::{decode_raster_bytes, RasterFormat};
pub use dicom_pixel::{DisplayFrame, PixelFormat, PixelPipeline, PixelPipelineConfig, WindowLevel};
pub use dicom_series::{FrameKey, FrameRef, InstanceHeader, Series, Study};
pub use extensions::{
    ExtensionEvent, ExtensionEventKind, ExtensionProfile, ExtensionRegistry, ExtensionResult,
    ExtensionStatus, RegisteredExtensionResult, SampleEchoExtension, WorkflowExtension,
};
#[cfg(feature = "pack-enhanced")]
pub use pack_enhanced::{
    EnhancedFrameGeometry, EnhancedPack, EnhancedRescale, SOP_CLASS_ENHANCED_CT,
    SOP_CLASS_ENHANCED_MR,
};
#[cfg(feature = "gsps")]
pub use pack_gsps::{GspsPack, PresentationState};
#[cfg(feature = "pack-nm")]
pub use pack_nm::{
    extract_measurement_context as extract_nm_measurement_context,
    CalibrationSource as NmCalibrationSource, MeasurementWarning as NmMeasurementWarning,
    NmMeasurementContext, NmPack, NM_SOP_CLASS_UIDS, SOP_CLASS_NM,
};
#[cfg(feature = "pack-rt")]
pub use pack_rt::{RtDoseGrid, RtPack, RtReferenceGeometry, SOP_CLASS_RT_DOSE};
#[cfg(feature = "pack-seg")]
pub use pack_seg::{SegPack, SegReference, Segmentation, SegmentationType, SOP_CLASS_SEG};
#[cfg(feature = "pack-sr")]
pub use pack_sr::{
    apply_sr_update, extract_measurements, parse_authored_document, serialize_authored_document,
    Code, SrAuthoredDocument, SrAuthoringBuilder, SrAuthoringContentItem, SrAuthoringError,
    SrBuilderDefaults, SrMeasurement, SrPack, SrUpdateRequest,
};
#[cfg(feature = "pack-us")]
pub use pack_us::{
    extract_measurement_context as extract_us_measurement_context,
    CalibrationSource as UsCalibrationSource, MeasurementWarning as UsMeasurementWarning,
    UsMeasurementContext, UsPack, SOP_CLASS_US, SOP_CLASS_US_MF, US_SOP_CLASS_UIDS,
};
#[cfg(feature = "pack-xa")]
pub use pack_xa::{
    extract_measurement_context as extract_xa_measurement_context,
    CalibrationSource as XaCalibrationSource, MeasurementWarning as XaMeasurementWarning,
    XaMeasurementContext, XaPack, SOP_CLASS_XA, SOP_CLASS_XRF, XA_SOP_CLASS_UIDS,
};
pub use viewer_core::{
    reslice_volume, CacheMetrics, DeterministicCache, InteractionState, Measurement, MprError,
    MprFrame, MprLimits, MprRequest, ResampleKernel, SlicePlane, ToolState, TriPlanarPlane,
    TriPlanarState, ViewerModel, Viewport2D, VolumeAssemblyOptions, VolumeError, VolumeGrid,
};

/// Top-level RDVF configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Resource limits.
    pub limits: Limits,
    /// Build-time feature capabilities.
    pub capabilities: Capabilities,
}

impl Config {
    /// Create a new config with explicit limits.
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            capabilities: capabilities(),
        }
    }

    /// Create a config builder for incremental configuration.
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::new()
    }

    /// Serialize the configuration to a stable, line-based format.
    pub fn serialize(&self) -> String {
        const HEADER: &str = "rdvf_config_v1";
        let mut out = String::new();
        out.push_str(HEADER);
        out.push('\n');

        fn push_line(out: &mut String, key: &str, value: impl std::fmt::Display) {
            out.push_str(key);
            out.push('=');
            out.push_str(&value.to_string());
            out.push('\n');
        }

        push_line(
            &mut out,
            "limits.max_input_bytes",
            self.limits.max_input_bytes(),
        );
        push_line(
            &mut out,
            "limits.max_dataset_elements",
            self.limits.max_dataset_elements(),
        );
        push_line(
            &mut out,
            "limits.max_sequence_depth",
            self.limits.max_sequence_depth(),
        );
        push_line(
            &mut out,
            "limits.max_string_bytes",
            self.limits.max_string_bytes(),
        );
        push_line(
            &mut out,
            "limits.max_element_vl_bytes",
            self.limits.max_element_vl_bytes(),
        );
        push_line(
            &mut out,
            "limits.max_frames_per_instance",
            self.limits.max_frames_per_instance(),
        );
        push_line(
            &mut out,
            "limits.max_pixels_per_frame",
            self.limits.max_pixels_per_frame(),
        );
        push_line(
            &mut out,
            "limits.max_decompressed_bytes",
            self.limits.max_decompressed_bytes(),
        );
        push_line(
            &mut out,
            "limits.max_gpu_texture_bytes",
            self.limits.max_gpu_texture_bytes(),
        );
        push_line(
            &mut out,
            "limits.max_cache_bytes",
            self.limits.max_cache_bytes(),
        );

        for (key, enabled) in self.capabilities.report_rows() {
            push_line(&mut out, key, enabled);
        }

        out
    }

    /// Serialize the configuration to a JSON wrapper around the line-based format.
    pub fn to_json_string(&self) -> String {
        const HEADER: &str = "rdvf_config_v1";
        let payload = self.serialize();
        let mut out = String::from("{\"format\":\"");
        push_json_escaped(&mut out, HEADER);
        out.push_str("\",\"payload\":\"");
        push_json_escaped(&mut out, &payload);
        out.push_str("\"}");
        out
    }

    /// Deserialize a configuration from the JSON wrapper format.
    pub fn from_json_string(input: &str) -> std::result::Result<Self, ConfigParseError> {
        let trimmed = input.trim();
        if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
            return Err(ConfigParseError::InvalidJson);
        }

        let body = &trimmed[1..trimmed.len() - 1];
        let mut format: Option<String> = None;
        let mut payload: Option<String> = None;

        for (key, value) in split_json_kv_pairs(body)? {
            match key.as_str() {
                "format" => {
                    if format.is_some() {
                        return Err(ConfigParseError::DuplicateJsonKey { key });
                    }
                    format = Some(value);
                }
                "payload" => {
                    if payload.is_some() {
                        return Err(ConfigParseError::DuplicateJsonKey { key });
                    }
                    payload = Some(value);
                }
                _ => {
                    return Err(ConfigParseError::UnknownJsonKey { key });
                }
            }
        }

        let format = format.ok_or(ConfigParseError::MissingJsonKey { key: "format" })?;
        if format != "rdvf_config_v1" {
            return Err(ConfigParseError::MissingHeader);
        }

        let payload = payload.ok_or(ConfigParseError::MissingJsonKey { key: "payload" })?;
        Config::deserialize(&payload)
    }

    /// Deserialize a configuration from the stable, line-based format.
    pub fn deserialize(input: &str) -> std::result::Result<Self, ConfigParseError> {
        const HEADER: &str = "rdvf_config_v1";
        let mut lines = input.lines();
        let header = lines.next().ok_or(ConfigParseError::MissingHeader)?;
        if header != HEADER {
            return Err(ConfigParseError::MissingHeader);
        }

        let mut limits = Limits::default();
        let mut caps = Capabilities::new(
            false, false, false, false, false,
            false, false, false, false, false,
            false, false, false, false, false, false,
        );

        let mut seen = SeenFields::default();

        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let (key, value) =
                line.split_once('=')
                    .ok_or_else(|| ConfigParseError::InvalidLine {
                        line: line.to_string(),
                    })?;

            match key {
                "limits.max_input_bytes" => {
                    seen.mark("limits.max_input_bytes")?;
                    limits.set_max_input_bytes(parse_u64(key, value)?);
                }
                "limits.max_dataset_elements" => {
                    seen.mark("limits.max_dataset_elements")?;
                    limits.set_max_dataset_elements(parse_u64(key, value)?);
                }
                "limits.max_sequence_depth" => {
                    seen.mark("limits.max_sequence_depth")?;
                    limits.set_max_sequence_depth(parse_u64(key, value)?);
                }
                "limits.max_string_bytes" => {
                    seen.mark("limits.max_string_bytes")?;
                    limits.set_max_string_bytes(parse_u64(key, value)?);
                }
                "limits.max_element_vl_bytes" => {
                    seen.mark("limits.max_element_vl_bytes")?;
                    limits.set_max_element_vl_bytes(parse_u64(key, value)?);
                }
                "limits.max_frames_per_instance" => {
                    seen.mark("limits.max_frames_per_instance")?;
                    limits.set_max_frames_per_instance(parse_u64(key, value)?);
                }
                "limits.max_pixels_per_frame" => {
                    seen.mark("limits.max_pixels_per_frame")?;
                    limits.set_max_pixels_per_frame(parse_u64(key, value)?);
                }
                "limits.max_decompressed_bytes" => {
                    seen.mark("limits.max_decompressed_bytes")?;
                    limits.set_max_decompressed_bytes(parse_u64(key, value)?);
                }
                "limits.max_gpu_texture_bytes" => {
                    seen.mark("limits.max_gpu_texture_bytes")?;
                    limits.set_max_gpu_texture_bytes(parse_u64(key, value)?);
                }
                "limits.max_cache_bytes" => {
                    seen.mark("limits.max_cache_bytes")?;
                    limits.set_max_cache_bytes(parse_u64(key, value)?);
                }
                "capabilities.tier1_deflate" => {
                    seen.mark("capabilities.tier1_deflate")?;
                    caps.set_tier1_deflate(parse_bool(key, value)?);
                }
                "capabilities.codec_jpegls" => {
                    seen.mark("capabilities.codec_jpegls")?;
                    caps.set_codec_jpegls(parse_bool(key, value)?);
                }
                "capabilities.codec_j2k" => {
                    seen.mark("capabilities.codec_j2k")?;
                    caps.set_codec_j2k(parse_bool(key, value)?);
                }
                "capabilities.raster_io" => {
                    seen.mark("capabilities.raster_io")?;
                    caps.set_raster_io(parse_bool(key, value)?);
                }
                "capabilities.gsps" => {
                    seen.mark("capabilities.gsps")?;
                    caps.set_gsps(parse_bool(key, value)?);
                }
                "capabilities.modality_ct" => {
                    seen.mark("capabilities.modality_ct")?;
                    caps.set_modality_ct(parse_bool(key, value)?);
                }
                "capabilities.modality_pet" => {
                    seen.mark("capabilities.modality_pet")?;
                    caps.set_modality_pet(parse_bool(key, value)?);
                }
                "capabilities.modality_mg" => {
                    seen.mark("capabilities.modality_mg")?;
                    caps.set_modality_mg(parse_bool(key, value)?);
                }
                "capabilities.modality_xr" => {
                    seen.mark("capabilities.modality_xr")?;
                    caps.set_modality_xr(parse_bool(key, value)?);
                }
                "capabilities.pack_enhanced" => {
                    seen.mark("capabilities.pack_enhanced")?;
                    caps.set_pack_enhanced(parse_bool(key, value)?);
                }
                "capabilities.pack_us" => {
                    seen.mark("capabilities.pack_us")?;
                    caps.set_pack_us(parse_bool(key, value)?);
                }
                "capabilities.pack_nm" => {
                    seen.mark("capabilities.pack_nm")?;
                    caps.set_pack_nm(parse_bool(key, value)?);
                }
                "capabilities.pack_xa" => {
                    seen.mark("capabilities.pack_xa")?;
                    caps.set_pack_xa(parse_bool(key, value)?);
                }
                "capabilities.pack_seg" => {
                    seen.mark("capabilities.pack_seg")?;
                    caps.set_pack_seg(parse_bool(key, value)?);
                }
                "capabilities.pack_rt" => {
                    seen.mark("capabilities.pack_rt")?;
                    caps.set_pack_rt(parse_bool(key, value)?);
                }
                "capabilities.pack_sr" => {
                    seen.mark("capabilities.pack_sr")?;
                    caps.set_pack_sr(parse_bool(key, value)?);
                }
                _ => {
                    return Err(ConfigParseError::UnknownKey {
                        key: key.to_string(),
                    });
                }
            }
        }

        for key in SeenFields::required() {
            if !seen.is_set(key) {
                return Err(ConfigParseError::MissingField { key });
            }
        }

        Ok(Self {
            limits,
            capabilities: caps,
        })
    }
}

/// Assemble a volume from deterministic slice metadata.
pub fn assemble_volume_from_slices(
    slices: &[SlicePlane],
    options: VolumeAssemblyOptions,
) -> std::result::Result<VolumeGrid, VolumeError> {
    VolumeGrid::from_slices(slices, options)
}

/// Generate an MPR frame from an already assembled volume.
pub fn request_mpr_frame(
    volume: &VolumeGrid,
    request: &MprRequest,
    limits: MprLimits,
) -> std::result::Result<MprFrame, MprError> {
    reslice_volume(volume, request, limits)
}

impl Default for Config {
    fn default() -> Self {
        Self::new(Limits::default())
    }
}

/// Builder for `Config`.
#[derive(Debug, Clone, Default)]
pub struct ConfigBuilder {
    limits: Option<Limits>,
    capabilities: Option<Capabilities>,
}

impl ConfigBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Override limits.
    pub fn limits(mut self, limits: Limits) -> Self {
        self.limits = Some(limits);
        self
    }

    /// Override capabilities.
    pub fn capabilities(mut self, capabilities: Capabilities) -> Self {
        self.capabilities = Some(capabilities);
        self
    }

    /// Build a config using defaults where not explicitly provided.
    pub fn build(self) -> Config {
        Config {
            limits: self.limits.unwrap_or_default(),
            capabilities: self.capabilities.unwrap_or_else(capabilities),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct SeenFields {
    fields: std::collections::BTreeMap<&'static str, bool>,
}

impl SeenFields {
    fn required() -> Vec<&'static str> {
        let mut out = vec![
            "limits.max_input_bytes()",
            "limits.max_dataset_elements()",
            "limits.max_sequence_depth()",
            "limits.max_string_bytes()",
            "limits.max_element_vl_bytes()",
            "limits.max_frames_per_instance()",
            "limits.max_pixels_per_frame()",
            "limits.max_decompressed_bytes()",
            "limits.max_gpu_texture_bytes()",
            "limits.max_cache_bytes()",
        ];
        let keys = capabilities().report_rows();
        for (key, _) in keys {
            out.push(key);
        }
        out
    }

    fn mark(&mut self, key: &'static str) -> std::result::Result<(), ConfigParseError> {
        if self.fields.insert(key, true).unwrap_or(false) {
            return Err(ConfigParseError::DuplicateKey { key });
        }
        Ok(())
    }

    fn is_set(&self, key: &'static str) -> bool {
        self.fields.get(key).copied().unwrap_or(false)
    }
}

fn parse_u64(key: &str, value: &str) -> std::result::Result<u64, ConfigParseError> {
    value
        .parse::<u64>()
        .map_err(|_| ConfigParseError::InvalidValue {
            key: key.to_string(),
            value: value.to_string(),
        })
}

fn parse_bool(key: &str, value: &str) -> std::result::Result<bool, ConfigParseError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(ConfigParseError::InvalidValue {
            key: key.to_string(),
            value: value.to_string(),
        }),
    }
}

fn push_json_escaped(out: &mut String, input: &str) {
    use std::fmt::Write;

    for ch in input.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => {
                let _ = write!(out, "\\u{:04x}", ch as u32);
            }
            ch => out.push(ch),
        }
    }
}

fn split_json_kv_pairs(
    input: &str,
) -> std::result::Result<Vec<(String, String)>, ConfigParseError> {
    let mut pairs = Vec::new();
    let mut i = 0;
    let bytes = input.as_bytes();

    while i < bytes.len() {
        skip_ws(input, &mut i);
        if i >= bytes.len() {
            break;
        }

        let (key, next) = parse_json_string(input, i)?;
        i = next;
        skip_ws(input, &mut i);
        if i >= bytes.len() || bytes[i] != b':' {
            return Err(ConfigParseError::InvalidJson);
        }
        i += 1;
        skip_ws(input, &mut i);
        let (value, next) = parse_json_string(input, i)?;
        i = next;
        pairs.push((key, value));
        skip_ws(input, &mut i);
        if i >= bytes.len() {
            break;
        }
        if bytes[i] == b',' {
            i += 1;
            continue;
        }
        return Err(ConfigParseError::InvalidJson);
    }

    Ok(pairs)
}

fn skip_ws(input: &str, index: &mut usize) {
    let bytes = input.as_bytes();
    while *index < bytes.len() && bytes[*index].is_ascii_whitespace() {
        *index += 1;
    }
}

fn parse_json_string(
    input: &str,
    start: usize,
) -> std::result::Result<(String, usize), ConfigParseError> {
    let bytes = input.as_bytes();
    if start >= bytes.len() || bytes[start] != b'"' {
        return Err(ConfigParseError::InvalidJson);
    }

    let mut i = start + 1;
    let mut out = String::new();
    while i < bytes.len() {
        match bytes[i] {
            b'"' => return Ok((out, i + 1)),
            b'\\' => {
                i += 1;
                if i >= bytes.len() {
                    return Err(ConfigParseError::InvalidJson);
                }
                match bytes[i] {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => {
                        if i + 4 >= bytes.len() {
                            return Err(ConfigParseError::InvalidJson);
                        }
                        let hex = &input[i + 1..i + 5];
                        let code = u32::from_str_radix(hex, 16)
                            .map_err(|_| ConfigParseError::InvalidJson)?;
                        if let Some(ch) = char::from_u32(code) {
                            out.push(ch);
                        } else {
                            return Err(ConfigParseError::InvalidJson);
                        }
                        i += 4;
                    }
                    _ => return Err(ConfigParseError::InvalidJson),
                }
            }
            byte => out.push(byte as char),
        }
        i += 1;
    }

    Err(ConfigParseError::InvalidJson)
}

/// Errors produced when parsing a serialized `Config`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigParseError {
    /// The header line is missing or invalid.
    MissingHeader,
    /// A line was not in `key=value` form.
    InvalidLine {
        /// The raw line that could not be parsed.
        line: String,
    },
    /// A key appeared more than once.
    DuplicateKey {
        /// The duplicated key.
        key: &'static str,
    },
    /// A key is not recognized by this format.
    UnknownKey {
        /// The unknown key.
        key: String,
    },
    /// A value could not be parsed for its key.
    InvalidValue {
        /// The key that failed to parse.
        key: String,
        /// The raw value that failed to parse.
        value: String,
    },
    /// A required key was missing from the serialized form.
    MissingField {
        /// The required key that was missing.
        key: &'static str,
    },
    /// JSON wrapper was invalid.
    InvalidJson,
    /// Required JSON key missing.
    MissingJsonKey {
        /// The required JSON key that was missing.
        key: &'static str,
    },
    /// JSON key appears more than once.
    DuplicateJsonKey {
        /// The duplicated JSON key.
        key: String,
    },
    /// JSON key is not recognized.
    UnknownJsonKey {
        /// The unknown JSON key.
        key: String,
    },
}

#[cfg(test)]
mod tests {
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
        let caps = super::capabilities();
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
        // REQ-FEAT-303, REQ-API-206: default profile keeps optional RDVF features disabled.
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
        let caps = super::capabilities();
        let config = super::Config::default();
        assert_eq!(config.limits, super::Limits::default());
        assert_eq!(config.capabilities, caps);
    }

    #[test]
    fn config_roundtrip_default() {
        // REQ-ARCH-121, REQ-ARCH-122, REQ-API-207: config must be serializable and stable.
        let config = super::Config::default();
        let serialized = config.serialize();
        let deserialized = super::Config::deserialize(&serialized).expect("deserialize config");
        assert_eq!(deserialized, config);
    }

    #[test]
    fn config_json_roundtrip_default() {
        // REQ-ARCH-121: JSON wrapper must round-trip through parser.
        let config = super::Config::default();
        let json = config.to_json_string();
        let parsed = super::Config::from_json_string(&json).expect("parse json config");
        assert_eq!(parsed, config);
    }

    #[test]
    fn config_rejects_unknown_header() {
        // REQ-ARCH-121: format version must be validated.
        let config = super::Config::default();
        let serialized = config.serialize();
        let payload = serialized
            .split_once('\n')
            .map(|(_, rest)| rest)
            .unwrap_or_default();
        let bad = format!("rdvf_config_v2\n{payload}");
        assert_eq!(
            super::Config::deserialize(&bad),
            Err(super::ConfigParseError::MissingHeader)
        );
    }

    #[test]
    fn config_builder_overrides() {
        // REQ-ARCH-122, REQ-API-207: builder must wire limits and capabilities.
        let mut limits = super::Limits::default();
        limits.set_max_input_bytes(42);
        let mut caps = super::capabilities();
        caps.set_tier1_deflate(!caps.tier1_deflate());

        let config = super::Config::builder()
            .limits(limits.clone())
            .capabilities(caps.clone())
            .build();

        assert_eq!(config.limits, limits);
        assert_eq!(config.capabilities, caps);
    }

    #[test]
    fn config_rejects_unknown_key() {
        // REQ-ARCH-121: unknown keys must be rejected.
        let mut serialized = super::Config::default().serialize();
        serialized.push_str("limits.unknown=1\n");
        assert_eq!(
            super::Config::deserialize(&serialized),
            Err(super::ConfigParseError::UnknownKey {
                key: "limits.unknown".to_string()
            })
        );
    }

    #[test]
    fn config_rejects_invalid_value() {
        // REQ-ARCH-121: invalid values must be rejected.
        let serialized = super::Config::default().serialize();
        let bad = serialized.replace(
            "limits.max_input_bytes=536870912",
            "limits.max_input_bytes=bad",
        );
        assert_eq!(
            super::Config::deserialize(&bad),
            Err(super::ConfigParseError::InvalidValue {
                key: "limits.max_input_bytes".to_string(),
                value: "bad".to_string()
            })
        );
    }

    #[test]
    fn config_rejects_duplicate_key() {
        // REQ-ARCH-121: duplicate keys must be rejected.
        let mut serialized = super::Config::default().serialize();
        serialized.push_str("limits.max_input_bytes=1\n");
        assert_eq!(
            super::Config::deserialize(&serialized),
            Err(super::ConfigParseError::DuplicateKey {
                key: "limits.max_input_bytes"
            })
        );
    }

    #[test]
    fn config_rejects_missing_field() {
        // REQ-ARCH-121: missing fields must be rejected.
        let serialized = super::Config::default().serialize();
        let mut lines: Vec<&str> = serialized.lines().collect();
        lines.retain(|line| !line.starts_with("limits.max_cache_bytes="));
        let stripped = lines.join("\n");
        assert_eq!(
            super::Config::deserialize(&stripped),
            Err(super::ConfigParseError::MissingField {
                key: "limits.max_cache_bytes"
            })
        );
    }

    #[test]
    fn config_json_rejects_unknown_key() {
        // REQ-ARCH-121: JSON wrapper must validate keys.
        let json = "{\"format\":\"rdvf_config_v1\",\"payload\":\"rdvf_config_v1\\nlimits.max_input_bytes=1\\n\",\"extra\":\"oops\"}";
        assert_eq!(
            super::Config::from_json_string(json),
            Err(super::ConfigParseError::UnknownJsonKey {
                key: "extra".to_string()
            })
        );
    }
}
