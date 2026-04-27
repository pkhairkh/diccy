#![deny(missing_docs)]

//! Transfer syntax decoding and pixel pipeline.

#[cfg(feature = "codec-jpegls")]
use charls::CharLS;
use dicom_core::{Dataset, Error, ErrorKind, Limits, Result, Tag, Value};
#[cfg(feature = "codec-j2k")]
use hayro_jpeg2000::{decode as decode_jpeg2000_bitmap, ColorSpace, DecodeSettings};
#[cfg(feature = "raster-io")]
use image::codecs::bmp::BmpDecoder;
#[cfg(feature = "raster-io")]
use image::codecs::jpeg::JpegDecoder;
#[cfg(feature = "raster-io")]
use image::codecs::png::PngDecoder;
#[cfg(feature = "raster-io")]
use image::codecs::tiff::TiffDecoder;
#[cfg(feature = "raster-io")]
use image::{ColorType, GenericImageView, ImageDecoder, ImageFormat};
#[cfg(feature = "pack-enhanced")]
use pack_enhanced::{
    select_frame_groups, EnhancedPack, SOP_CLASS_ENHANCED_CT, SOP_CLASS_ENHANCED_MR,
};
#[cfg(feature = "raster-io")]
use std::io::Cursor;

/// Output pixel formats supported by the CPU boundary.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PixelFormat {
    /// 8-bit grayscale.
    Luma8,
    /// 16-bit grayscale.
    Luma16,
    /// 8-bit RGBA.
    Rgba8,
}

/// A deterministic CPU-boundary frame output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayFrame {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Pixel format.
    pub format: PixelFormat,
    /// Pixel bytes.
    pub bytes: Vec<u8>,
}

/// Display-space transform applied after pixel decoding and overlays.
pub trait DisplayTransform {
    /// Apply the transform in-place.
    fn apply(&self, frame: &mut DisplayFrame) -> Result<()>;
}

impl<F> DisplayTransform for F
where
    F: Fn(&mut DisplayFrame) -> Result<()>,
{
    fn apply(&self, frame: &mut DisplayFrame) -> Result<()> {
        (self)(frame)
    }
}

/// Non-DICOM raster input formats.
#[cfg(feature = "raster-io")]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RasterFormat {
    /// BMP image bytes.
    Bmp,
    /// PNG image bytes.
    Png,
    /// JPEG image bytes.
    Jpeg,
    /// TIFF image bytes.
    Tiff,
}

#[cfg(feature = "raster-io")]
impl RasterFormat {
    fn image_format(self) -> ImageFormat {
        match self {
            RasterFormat::Bmp => ImageFormat::Bmp,
            RasterFormat::Png => ImageFormat::Png,
            RasterFormat::Jpeg => ImageFormat::Jpeg,
            RasterFormat::Tiff => ImageFormat::Tiff,
        }
    }
}

/// Decode a non-DICOM raster image into a deterministic CPU-boundary output.
#[cfg(feature = "raster-io")]
pub fn decode_raster_bytes(
    format: RasterFormat,
    bytes: &[u8],
    limits: &Limits,
) -> Result<DisplayFrame> {
    enforce_limit(
        "max_input_bytes",
        bytes.len() as u64,
        limits.max_input_bytes(),
    )?;

    let (width, height) = raster_dimensions(format, bytes, limits)?;
    let image = image::load_from_memory_with_format(bytes, format.image_format())
        .map_err(|_| decode_error("raster", "raster decode failed"))?;
    debug_assert_eq!(image.dimensions(), (width, height));

    match image.color() {
        ColorType::L8 => {
            let buf = image.to_luma8().into_raw();
            enforce_limit(
                "max_decompressed_bytes",
                buf.len() as u64,
                limits.max_decompressed_bytes(),
            )?;
            Ok(DisplayFrame {
                width,
                height,
                format: PixelFormat::Luma8,
                bytes: buf,
            })
        }
        ColorType::L16 => {
            let raw = image.to_luma16().into_raw();
            let mut out = Vec::with_capacity(raw.len().saturating_mul(2));
            for value in raw {
                out.extend_from_slice(&value.to_le_bytes());
            }
            enforce_limit(
                "max_decompressed_bytes",
                out.len() as u64,
                limits.max_decompressed_bytes(),
            )?;
            Ok(DisplayFrame {
                width,
                height,
                format: PixelFormat::Luma16,
                bytes: out,
            })
        }
        ColorType::La8 | ColorType::Rgb8 | ColorType::Rgba8 => {
            let buf = image.to_rgba8().into_raw();
            enforce_limit(
                "max_decompressed_bytes",
                buf.len() as u64,
                limits.max_decompressed_bytes(),
            )?;
            Ok(DisplayFrame {
                width,
                height,
                format: PixelFormat::Rgba8,
                bytes: buf,
            })
        }
        ColorType::La16 | ColorType::Rgb16 | ColorType::Rgba16 => {
            Err(decode_error("raster", "16-bit color raster unsupported"))
        }
        other => Err(decode_error(
            "raster",
            &format!("unsupported raster color type {other:?}"),
        )),
    }
}

#[cfg(feature = "raster-io")]
fn raster_dimensions(format: RasterFormat, bytes: &[u8], limits: &Limits) -> Result<(u32, u32)> {
    let (width, height) = match format {
        RasterFormat::Bmp => {
            let decoder = BmpDecoder::new(Cursor::new(bytes))
                .map_err(|_| decode_error("raster", "bmp header decode failed"))?;
            decoder.dimensions()
        }
        RasterFormat::Png => {
            let decoder = PngDecoder::new(Cursor::new(bytes))
                .map_err(|_| decode_error("raster", "png header decode failed"))?;
            decoder.dimensions()
        }
        RasterFormat::Jpeg => {
            let decoder = JpegDecoder::new(Cursor::new(bytes))
                .map_err(|_| decode_error("raster", "jpeg header decode failed"))?;
            decoder.dimensions()
        }
        RasterFormat::Tiff => {
            let decoder = TiffDecoder::new(Cursor::new(bytes))
                .map_err(|_| decode_error("raster", "tiff header decode failed"))?;
            decoder.dimensions()
        }
    };

    let pixels = (width as u64)
        .checked_mul(height as u64)
        .ok_or_else(|| limit_overflow("max_pixels_per_frame", limits.max_pixels_per_frame()))?;
    enforce_limit(
        "max_pixels_per_frame",
        pixels,
        limits.max_pixels_per_frame(),
    )?;
    Ok((width, height))
}

/// Input bundle for pixel decode calls.
#[derive(Debug, Clone, Copy)]
pub struct PixelDecodeInput<'a> {
    /// Source dataset.
    pub dataset: &'a Dataset,
    /// Transfer Syntax UID.
    pub transfer_syntax_uid: &'a str,
    /// Frame index (0-based).
    pub frame_index: u32,
}

/// Window/level selection.
#[derive(Debug, Clone, PartialEq)]
pub enum WindowLevel {
    /// Explicit center/width.
    Explicit {
        /// Window center.
        center: f64,
        /// Window width.
        width: f64,
    },
    /// VOI LUT selection by index.
    VoiLut {
        /// LUT index.
        index: usize,
    },
    /// Auto-window selection.
    Auto,
}

/// Pixel pipeline configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct PixelPipelineConfig {
    /// Resource limits.
    pub limits: Limits,
    /// Window/level configuration.
    pub window_level: WindowLevel,
    /// Auto-window target sample count.
    pub auto_window_samples: usize,
    /// Auto-window low percentile (0.0..1.0).
    pub auto_window_low_percentile: f64,
    /// Auto-window high percentile (0.0..1.0).
    pub auto_window_high_percentile: f64,
}

impl Default for PixelPipelineConfig {
    fn default() -> Self {
        Self {
            limits: Limits::default(),
            window_level: WindowLevel::Auto,
            auto_window_samples: 65_536,
            auto_window_low_percentile: 0.005,
            auto_window_high_percentile: 0.995,
        }
    }
}

/// Pixel pipeline executor.
#[derive(Debug, Clone)]
pub struct PixelPipeline {
    config: PixelPipelineConfig,
}

impl PixelPipeline {
    /// Create a pipeline with the provided configuration.
    pub fn new(config: PixelPipelineConfig) -> Self {
        Self { config }
    }

    /// Return the active configuration.
    pub fn config(&self) -> &PixelPipelineConfig {
        &self.config
    }

    /// Decode a frame and produce a deterministic CPU-boundary output.
    pub fn decode_frame(
        &self,
        dataset: &Dataset,
        transfer_syntax_uid: &str,
        frame_index: u32,
    ) -> Result<DisplayFrame> {
        let meta = PixelMeta::from_dataset(dataset, &self.config.limits)?;
        meta.validate_dimensions(&self.config.limits, frame_index)?;

        let raw = decode_pixels(
            dataset,
            &meta,
            transfer_syntax_uid,
            frame_index,
            &self.config,
        )?;

        let mut display = match meta.photometric {
            Photometric::Monochrome1 | Photometric::Monochrome2 => {
                let modality =
                    apply_modality_transform(dataset, &raw, &meta, &self.config, frame_index)?;
                let mut luma = apply_voi_transform(dataset, &modality, &self.config)?;
                if meta.photometric == Photometric::Monochrome1 {
                    for value in &mut luma {
                        *value = 1.0 - *value;
                        ensure_finite("voi", *value)?;
                    }
                }
                let bytes = pack_luma8(&luma)?;
                DisplayFrame {
                    width: meta.cols,
                    height: meta.rows,
                    format: PixelFormat::Luma8,
                    bytes,
                }
            }
            Photometric::Rgb => {
                let bytes = pack_rgba8(&raw.samples, meta.samples_per_pixel)?;
                DisplayFrame {
                    width: meta.cols,
                    height: meta.rows,
                    format: PixelFormat::Rgba8,
                    bytes,
                }
            }
            Photometric::YbrFull | Photometric::YbrFull422 => {
                let rgb_samples = ybr_to_rgb(&raw.samples)?;
                let bytes = pack_rgba8(&rgb_samples, meta.samples_per_pixel)?;
                DisplayFrame {
                    width: meta.cols,
                    height: meta.rows,
                    format: PixelFormat::Rgba8,
                    bytes,
                }
            }
        };

        apply_overlays(dataset, &meta, &mut display, &self.config.limits)?;
        Ok(display)
    }

    /// Decode a frame and apply a display-space transform after overlays.
    pub fn decode_frame_with_transform<T: DisplayTransform>(
        &self,
        dataset: &Dataset,
        transfer_syntax_uid: &str,
        frame_index: u32,
        transform: &T,
    ) -> Result<DisplayFrame> {
        let mut frame = self.decode_frame(dataset, transfer_syntax_uid, frame_index)?;
        transform.apply(&mut frame)?;
        Ok(frame)
    }

    /// Decode a frame using a bundled input struct.
    pub fn decode_input(&self, input: PixelDecodeInput<'_>) -> Result<DisplayFrame> {
        self.decode_frame(input.dataset, input.transfer_syntax_uid, input.frame_index)
    }

    /// Decode a frame using a bundled input struct and apply a display transform.
    pub fn decode_input_with_transform<T: DisplayTransform>(
        &self,
        input: PixelDecodeInput<'_>,
        transform: &T,
    ) -> Result<DisplayFrame> {
        self.decode_frame_with_transform(
            input.dataset,
            input.transfer_syntax_uid,
            input.frame_index,
            transform,
        )
    }
}

const TS_IMPLICIT_VR_LE: &str = "1.2.840.10008.1.2";
const TS_EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1";
const TS_DEFLATED_EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1.99";
const TS_RLE_LOSSLESS: &str = "1.2.840.10008.1.2.5";
const TS_JPEG_BASELINE: &str = "1.2.840.10008.1.2.4.50";
const TS_JPEGLS_LOSSLESS: &str = "1.2.840.10008.1.2.4.80";
const TS_JPEGLS_NEAR_LOSSLESS: &str = "1.2.840.10008.1.2.4.81";
const TS_JPEG2000_LOSSLESS: &str = "1.2.840.10008.1.2.4.90";
const TS_JPEG2000_LOSSY: &str = "1.2.840.10008.1.2.4.91";

mod tags {
    use dicom_core::Tag;

    pub const TAG_ROWS: Tag = Tag(0x0028, 0x0010);
    pub const TAG_COLUMNS: Tag = Tag(0x0028, 0x0011);
    #[cfg(feature = "pack-enhanced")]
    pub const TAG_SOP_CLASS_UID: Tag = Tag(0x0008, 0x0016);
    pub const TAG_SAMPLES_PER_PIXEL: Tag = Tag(0x0028, 0x0002);
    pub const TAG_PHOTOMETRIC_INTERPRETATION: Tag = Tag(0x0028, 0x0004);
    pub const TAG_BITS_ALLOCATED: Tag = Tag(0x0028, 0x0100);
    pub const TAG_BITS_STORED: Tag = Tag(0x0028, 0x0101);
    pub const TAG_HIGH_BIT: Tag = Tag(0x0028, 0x0102);
    pub const TAG_PIXEL_REPRESENTATION: Tag = Tag(0x0028, 0x0103);
    pub const TAG_PLANAR_CONFIGURATION: Tag = Tag(0x0028, 0x0006);
    pub const TAG_NUMBER_OF_FRAMES: Tag = Tag(0x0028, 0x0008);
    pub const TAG_PIXEL_DATA: Tag = Tag(0x7FE0, 0x0010);
    pub const TAG_PIXEL_PADDING_VALUE: Tag = Tag(0x0028, 0x0120);
    pub const TAG_RESCALE_INTERCEPT: Tag = Tag(0x0028, 0x1052);
    pub const TAG_RESCALE_SLOPE: Tag = Tag(0x0028, 0x1053);
    pub const TAG_WINDOW_CENTER: Tag = Tag(0x0028, 0x1050);
    pub const TAG_WINDOW_WIDTH: Tag = Tag(0x0028, 0x1051);
    pub const TAG_VOI_LUT_FUNCTION: Tag = Tag(0x0028, 0x1056);
    pub const TAG_VOI_LUT_SEQUENCE: Tag = Tag(0x0028, 0x3010);
    pub const TAG_MODALITY_LUT_SEQUENCE: Tag = Tag(0x0028, 0x3000);
    pub const TAG_LUT_DESCRIPTOR: Tag = Tag(0x0028, 0x3002);
    pub const TAG_LUT_DATA: Tag = Tag(0x0028, 0x3006);

    pub const TAG_OVERLAY_ROWS: Tag = Tag(0x6000, 0x0010);
    pub const TAG_OVERLAY_COLUMNS: Tag = Tag(0x6000, 0x0011);
    pub const TAG_OVERLAY_ORIGIN: Tag = Tag(0x6000, 0x0050);
    pub const TAG_OVERLAY_BITS_ALLOCATED: Tag = Tag(0x6000, 0x0100);
    pub const TAG_OVERLAY_BIT_POSITION: Tag = Tag(0x6000, 0x0102);
    pub const TAG_OVERLAY_DATA: Tag = Tag(0x6000, 0x3000);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Photometric {
    Monochrome1,
    Monochrome2,
    Rgb,
    YbrFull,
    YbrFull422,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PixelRepresentation {
    Unsigned,
    Signed,
}

#[derive(Debug, Clone)]
struct PixelMeta {
    rows: u32,
    cols: u32,
    samples_per_pixel: u8,
    photometric: Photometric,
    bits_allocated: u16,
    bits_stored: u16,
    pixel_representation: PixelRepresentation,
    planar_configuration: u16,
    number_of_frames: u32,
    pixel_padding_value: Option<i32>,
}

impl PixelMeta {
    fn from_dataset(dataset: &Dataset, limits: &Limits) -> Result<Self> {
        let rows = read_u16_required(dataset, tags::TAG_ROWS)? as u32;
        let cols = read_u16_required(dataset, tags::TAG_COLUMNS)? as u32;
        let samples_per_pixel = read_u16_required(dataset, tags::TAG_SAMPLES_PER_PIXEL)? as u8;
        let photometric_raw =
            read_string_required(dataset, tags::TAG_PHOTOMETRIC_INTERPRETATION, limits)?;
        let photometric_value = trim_dicom_cs_padding(&photometric_raw);
        let photometric = match photometric_value {
            "MONOCHROME1" => Photometric::Monochrome1,
            "MONOCHROME2" => Photometric::Monochrome2,
            "RGB" => Photometric::Rgb,
            "YBR_FULL" => Photometric::YbrFull,
            "YBR_FULL_422" => Photometric::YbrFull422,
            other => {
                return Err(Error::from_kind(
                    ErrorKind::InvalidTagValue {
                        tag: tags::TAG_PHOTOMETRIC_INTERPRETATION,
                        detail: format!("unsupported photometric interpretation: {other}"),
                    },
                    "unsupported photometric interpretation",
                )
                .into())
            }
        };
        let bits_allocated = read_u16_required(dataset, tags::TAG_BITS_ALLOCATED)?;
        if bits_allocated != 8 && bits_allocated != 16 {
            return Err(invalid_tag_value(
                tags::TAG_BITS_ALLOCATED,
                "BitsAllocated must be 8 or 16",
            ));
        }
        let bits_stored = read_u16_required(dataset, tags::TAG_BITS_STORED)?;
        let high_bit = read_u16_required(dataset, tags::TAG_HIGH_BIT)?;
        if bits_stored == 0 || bits_stored > bits_allocated {
            return Err(invalid_tag_value(
                tags::TAG_BITS_STORED,
                "BitsStored out of range",
            ));
        }
        if high_bit + 1 != bits_stored {
            return Err(invalid_tag_value(
                tags::TAG_HIGH_BIT,
                "HighBit must equal BitsStored-1",
            ));
        }

        let pixel_representation = if matches!(
            photometric,
            Photometric::Monochrome1 | Photometric::Monochrome2
        ) {
            let repr = read_u16_required(dataset, tags::TAG_PIXEL_REPRESENTATION)?;
            match repr {
                0 => PixelRepresentation::Unsigned,
                1 => PixelRepresentation::Signed,
                _ => {
                    return Err(invalid_tag_value(
                        tags::TAG_PIXEL_REPRESENTATION,
                        "PixelRepresentation must be 0 or 1",
                    ))
                }
            }
        } else {
            PixelRepresentation::Unsigned
        };

        let planar_configuration = if samples_per_pixel > 1 {
            read_u16_required(dataset, tags::TAG_PLANAR_CONFIGURATION)?
        } else {
            0
        };
        if samples_per_pixel > 1 && planar_configuration > 1 {
            return Err(invalid_tag_value(
                tags::TAG_PLANAR_CONFIGURATION,
                "PlanarConfiguration must be 0 or 1",
            ));
        }

        let number_of_frames =
            read_optional_i32(dataset, tags::TAG_NUMBER_OF_FRAMES, limits)?.unwrap_or(1);
        if number_of_frames <= 0 {
            return Err(invalid_tag_value(
                tags::TAG_NUMBER_OF_FRAMES,
                "NumberOfFrames must be >= 1",
            ));
        }

        let pixel_padding_value =
            read_optional_i16(dataset, tags::TAG_PIXEL_PADDING_VALUE)?.map(|v| v as i32);

        if samples_per_pixel == 1
            && matches!(
                photometric,
                Photometric::Rgb | Photometric::YbrFull | Photometric::YbrFull422
            )
        {
            return Err(invalid_tag_value(
                tags::TAG_SAMPLES_PER_PIXEL,
                "color photometric interpretations require 3 samples per pixel",
            ));
        }
        if samples_per_pixel > 1
            && matches!(
                photometric,
                Photometric::Monochrome1 | Photometric::Monochrome2
            )
        {
            return Err(invalid_tag_value(
                tags::TAG_SAMPLES_PER_PIXEL,
                "MONOCHROME requires 1 sample per pixel",
            ));
        }
        if matches!(
            photometric,
            Photometric::Rgb | Photometric::YbrFull | Photometric::YbrFull422
        ) && bits_allocated != 8
        {
            return Err(invalid_tag_value(
                tags::TAG_BITS_ALLOCATED,
                "color photometric interpretations require 8-bit samples",
            ));
        }
        if matches!(photometric, Photometric::YbrFull422) && planar_configuration != 0 {
            return Err(invalid_tag_value(
                tags::TAG_PLANAR_CONFIGURATION,
                "YBR_FULL_422 requires PlanarConfiguration = 0",
            ));
        }
        if matches!(photometric, Photometric::YbrFull422) && !cols.is_multiple_of(2) {
            return Err(invalid_tag_value(
                tags::TAG_COLUMNS,
                "YBR_FULL_422 requires an even Columns value",
            ));
        }

        Ok(Self {
            rows,
            cols,
            samples_per_pixel,
            photometric,
            bits_allocated,
            bits_stored,
            pixel_representation,
            planar_configuration,
            number_of_frames: number_of_frames as u32,
            pixel_padding_value,
        })
    }

    fn validate_dimensions(&self, limits: &Limits, frame_index: u32) -> Result<()> {
        if frame_index >= self.number_of_frames {
            return Err(Error::from_kind(
                ErrorKind::InvalidTagValue {
                    tag: tags::TAG_NUMBER_OF_FRAMES,
                    detail: "frame index out of range".to_string(),
                },
                "frame index out of range",
            )
            .into());
        }
        enforce_limit(
            "max_frames_per_instance",
            self.number_of_frames as u64,
            limits.max_frames_per_instance(),
        )?;
        let pixels_per_frame = (self.rows as u64).saturating_mul(self.cols as u64);
        enforce_limit(
            "max_pixels_per_frame",
            pixels_per_frame,
            limits.max_pixels_per_frame(),
        )?;

        let bytes_per_sample = (self.bits_allocated / 8) as u64;
        let bytes_per_frame = pixels_per_frame
            .checked_mul(self.samples_per_pixel as u64)
            .and_then(|v| v.checked_mul(bytes_per_sample))
            .ok_or_else(|| {
                limit_overflow("max_decompressed_bytes", limits.max_decompressed_bytes())
            })?;

        let total_bytes = bytes_per_frame
            .checked_mul(self.number_of_frames as u64)
            .ok_or_else(|| {
                limit_overflow("max_decompressed_bytes", limits.max_decompressed_bytes())
            })?;
        enforce_limit(
            "max_decompressed_bytes",
            total_bytes,
            limits.max_decompressed_bytes(),
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct RawFrame {
    width: u32,
    height: u32,
    samples_per_pixel: u8,
    samples: Vec<i32>,
}

#[derive(Debug, Clone)]
struct ModalityFrame {
    width: u32,
    height: u32,
    values: Vec<f64>,
    padding_value: Option<i32>,
}

#[derive(Debug, Clone)]
struct Lut {
    first_mapped: i32,
    entry_bits: u16,
    values: Vec<u16>,
}

fn decode_pixels(
    dataset: &Dataset,
    meta: &PixelMeta,
    transfer_syntax_uid: &str,
    frame_index: u32,
    config: &PixelPipelineConfig,
) -> Result<RawFrame> {
    let unsupported = |uid: &str| {
        Err(Error::from_kind(
            ErrorKind::UnsupportedTransferSyntax {
                transfer_syntax_uid: uid.to_string(),
            },
            "unsupported transfer syntax",
        )
        .into())
    };
    let pixel_data = match dataset.get(tags::TAG_PIXEL_DATA) {
        Some(element) => match element.value() {
            Value::Bytes(bytes) => bytes.as_slice(),
            _ => {
                return Err(invalid_tag_value(
                    tags::TAG_PIXEL_DATA,
                    "Pixel Data must be raw bytes",
                ))
            }
        },
        None => return Err(missing_required_tag(tags::TAG_PIXEL_DATA)),
    };

    if matches!(meta.photometric, Photometric::YbrFull)
        && !matches!(
            transfer_syntax_uid,
            TS_IMPLICIT_VR_LE | TS_EXPLICIT_VR_LE | TS_DEFLATED_EXPLICIT_VR_LE | TS_RLE_LOSSLESS
        )
    {
        return Err(decode_error(
            "photometric",
            "YBR_FULL currently requires native uncompressed/deflated Little Endian or RLE Lossless transfer syntaxes",
        ));
    }
    if matches!(meta.photometric, Photometric::YbrFull422)
        && !matches!(
            transfer_syntax_uid,
            TS_IMPLICIT_VR_LE | TS_EXPLICIT_VR_LE | TS_DEFLATED_EXPLICIT_VR_LE
        )
    {
        return Err(decode_error(
            "photometric",
            "YBR_FULL_422 currently requires native uncompressed or deflated Little Endian transfer syntaxes",
        ));
    }

    match transfer_syntax_uid {
        TS_IMPLICIT_VR_LE | TS_EXPLICIT_VR_LE | TS_DEFLATED_EXPLICIT_VR_LE => {
            decode_uncompressed(pixel_data, meta, frame_index)
        }
        TS_RLE_LOSSLESS => decode_rle(pixel_data, meta, config),
        TS_JPEG_BASELINE => decode_jpeg_baseline(pixel_data, meta),
        #[cfg(feature = "codec-jpegls")]
        TS_JPEGLS_LOSSLESS | TS_JPEGLS_NEAR_LOSSLESS => decode_jpegls(pixel_data, meta, config),
        #[cfg(not(feature = "codec-jpegls"))]
        TS_JPEGLS_LOSSLESS | TS_JPEGLS_NEAR_LOSSLESS => unsupported(transfer_syntax_uid),
        #[cfg(feature = "codec-j2k")]
        TS_JPEG2000_LOSSLESS | TS_JPEG2000_LOSSY => decode_jpeg2000(pixel_data, meta, config),
        #[cfg(not(feature = "codec-j2k"))]
        TS_JPEG2000_LOSSLESS | TS_JPEG2000_LOSSY => unsupported(transfer_syntax_uid),
        other => unsupported(other),
    }
}

fn decode_uncompressed(data: &[u8], meta: &PixelMeta, frame_index: u32) -> Result<RawFrame> {
    let pixels_per_frame = meta.rows as usize * meta.cols as usize;
    let bytes_per_sample = (meta.bits_allocated / 8) as usize;
    let bytes_per_frame = if matches!(meta.photometric, Photometric::YbrFull422) {
        pixels_per_frame
            .checked_mul(2)
            .ok_or_else(|| decode_error("decode", "pixel frame size overflow"))?
    } else {
        pixels_per_frame
            .checked_mul(meta.samples_per_pixel as usize)
            .and_then(|v| v.checked_mul(bytes_per_sample))
            .ok_or_else(|| decode_error("decode", "pixel frame size overflow"))?
    };

    let offset = frame_index as usize * bytes_per_frame;
    let end = offset + bytes_per_frame;
    if end > data.len() {
        return Err(decode_error(
            "decode",
            "pixel data shorter than declared dimensions",
        ));
    }

    let mut samples = Vec::with_capacity(pixels_per_frame * meta.samples_per_pixel as usize);
    let slice = &data[offset..end];
    if matches!(meta.photometric, Photometric::YbrFull422) {
        if !meta.cols.is_multiple_of(2) {
            return Err(invalid_tag_value(
                tags::TAG_COLUMNS,
                "YBR_FULL_422 requires an even Columns value",
            ));
        }
        if !slice.len().is_multiple_of(4) {
            return Err(decode_error(
                "decode",
                "YBR_FULL_422 pixel data length is not aligned to 4-byte pairs",
            ));
        }
        let mut i = 0;
        while i + 3 < slice.len() {
            let y0 = slice[i] as i32;
            let y1 = slice[i + 1] as i32;
            let cb = slice[i + 2] as i32;
            let cr = slice[i + 3] as i32;
            samples.push(y0);
            samples.push(cb);
            samples.push(cr);
            samples.push(y1);
            samples.push(cb);
            samples.push(cr);
            i += 4;
        }
        if i != slice.len() {
            return Err(decode_error(
                "decode",
                "YBR_FULL_422 pixel data length is not aligned to 4-byte pairs",
            ));
        }
    } else {
        match meta.bits_allocated {
            8 => {
                for &b in slice {
                    samples.push(mask_and_sign_extend(b as i32, meta));
                }
            }
            16 => {
                let mut i = 0;
                while i + 1 < slice.len() {
                    let raw = u16::from_le_bytes([slice[i], slice[i + 1]]);
                    let value = match meta.pixel_representation {
                        PixelRepresentation::Unsigned => raw as i32,
                        PixelRepresentation::Signed => (raw as i16) as i32,
                    };
                    samples.push(mask_and_sign_extend(value, meta));
                    i += 2;
                }
                if i != slice.len() {
                    return Err(decode_error(
                        "decode",
                        "pixel data length is not aligned to 16-bit samples",
                    ));
                }
            }
            _ => {
                return Err(invalid_tag_value(
                    tags::TAG_BITS_ALLOCATED,
                    "BitsAllocated must be 8 or 16",
                ))
            }
        }
    }

    let mut raw = RawFrame {
        width: meta.cols,
        height: meta.rows,
        samples_per_pixel: meta.samples_per_pixel,
        samples,
    };
    if meta.samples_per_pixel > 1
        && meta.planar_configuration == 1
        && !matches!(meta.photometric, Photometric::YbrFull422)
    {
        reorder_planar(&mut raw)?;
    }
    Ok(raw)
}

fn decode_rle(data: &[u8], meta: &PixelMeta, config: &PixelPipelineConfig) -> Result<RawFrame> {
    if meta.number_of_frames > 1 {
        return Err(decode_error(
            "rle",
            "encapsulated multi-frame pixel data requires offset table support",
        ));
    }

    let start = find_rle_header_offset(data)?;
    let data = &data[start..];

    if data.len() < 64 {
        return Err(decode_error("rle", "RLE header too short"));
    }

    let segment_count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    if !(1..=15).contains(&segment_count) {
        return Err(decode_error("rle", "invalid RLE segment count"));
    }

    let bytes_per_sample = (meta.bits_allocated / 8) as usize;
    let expected_segments = meta.samples_per_pixel as usize * bytes_per_sample;
    if segment_count != expected_segments {
        return Err(decode_error("rle", "RLE segment count mismatch"));
    }

    let mut offsets = Vec::with_capacity(segment_count + 1);
    for i in 0..segment_count {
        let base = 4 + i * 4;
        let offset =
            u32::from_le_bytes([data[base], data[base + 1], data[base + 2], data[base + 3]])
                as usize;
        if offset >= data.len() {
            return Err(decode_error("rle", "RLE segment offset out of range"));
        }
        offsets.push(offset);
    }
    offsets.push(data.len());

    let pixels_per_frame = meta.rows as usize * meta.cols as usize;
    let mut decoded_segments: Vec<Vec<u8>> = Vec::with_capacity(segment_count);
    for i in 0..segment_count {
        let start = offsets[i];
        let end = offsets[i + 1];
        if end < start {
            return Err(decode_error("rle", "RLE segment offsets not monotonic"));
        }
        let segment_data = &data[start..end];
        let decoded = decode_packbits(segment_data, pixels_per_frame, &config.limits)?;
        if decoded.len() != pixels_per_frame {
            return Err(decode_error("rle", "RLE segment decoded size mismatch"));
        }
        decoded_segments.push(decoded);
    }

    let spp = meta.samples_per_pixel as usize;
    let mut samples = vec![0i32; pixels_per_frame * spp];
    for sample_index in 0..spp {
        if meta.bits_allocated == 8 {
            for (pixel, &byte) in decoded_segments[sample_index].iter().enumerate() {
                let value = mask_and_sign_extend(byte as i32, meta);
                samples[pixel * spp + sample_index] = value;
            }
        } else {
            let low_segment = &decoded_segments[sample_index * 2];
            let high_segment = &decoded_segments[sample_index * 2 + 1];
            for (pixel, (&low, &high)) in low_segment.iter().zip(high_segment).enumerate() {
                let raw = ((high as u16) << 8) | (low as u16);
                let value = match meta.pixel_representation {
                    PixelRepresentation::Unsigned => raw as i32,
                    PixelRepresentation::Signed => (raw as i16) as i32,
                };
                samples[pixel * spp + sample_index] = mask_and_sign_extend(value, meta);
            }
        }
    }

    Ok(RawFrame {
        width: meta.cols,
        height: meta.rows,
        samples_per_pixel: meta.samples_per_pixel,
        samples,
    })
}

fn reorder_planar(raw: &mut RawFrame) -> Result<()> {
    if raw.samples_per_pixel != 3 {
        return Err(decode_error(
            "photometric",
            "planar configuration only supported for 3-sample color data",
        ));
    }
    let pixels = raw.width as usize * raw.height as usize;
    if raw.samples.len() != pixels * 3 {
        return Err(decode_error("photometric", "unexpected planar RGB length"));
    }
    let plane_len = pixels;
    let (r_plane, rest) = raw.samples.split_at(plane_len);
    let (g_plane, b_plane) = rest.split_at(plane_len);
    let mut interleaved = Vec::with_capacity(raw.samples.len());
    for i in 0..pixels {
        interleaved.push(r_plane[i]);
        interleaved.push(g_plane[i]);
        interleaved.push(b_plane[i]);
    }
    raw.samples = interleaved;
    Ok(())
}

fn find_rle_header_offset(data: &[u8]) -> Result<usize> {
    let search_limit = data.len().min(256);
    let mut offset = 0usize;
    while offset + 64 <= search_limit {
        let segment_count = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        if (1..=15).contains(&segment_count) {
            let mut ok = true;
            for i in 0..segment_count {
                let base = offset + 4 + i * 4;
                let off = u32::from_le_bytes([
                    data[base],
                    data[base + 1],
                    data[base + 2],
                    data[base + 3],
                ]) as usize;
                if off < 64 || off >= data.len() {
                    ok = false;
                    break;
                }
            }
            if ok {
                return Ok(offset);
            }
        }
        offset += 4;
    }
    Ok(0)
}

fn decode_packbits(data: &[u8], expected: usize, limits: &Limits) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(expected);
    let mut i = 0;
    while i < data.len() {
        let header = data[i] as i8;
        i += 1;
        if header >= 0 {
            let count = header as usize + 1;
            if i + count > data.len() {
                return Err(decode_error("rle", "PackBits literal run exceeds input"));
            }
            out.extend_from_slice(&data[i..i + count]);
            i += count;
        } else if header >= -127 {
            let count = (1 - header as i16) as usize;
            if i >= data.len() {
                return Err(decode_error("rle", "PackBits repeat run missing byte"));
            }
            let value = data[i];
            i += 1;
            out.extend(std::iter::repeat_n(value, count));
        }
        if out.len() as u64 > limits.max_decompressed_bytes() {
            return Err(limit_overflow(
                "max_decompressed_bytes",
                limits.max_decompressed_bytes(),
            ));
        }
    }
    Ok(out)
}

fn decode_jpeg_baseline(data: &[u8], meta: &PixelMeta) -> Result<RawFrame> {
    if meta.number_of_frames > 1 {
        return Err(decode_error(
            "jpeg",
            "encapsulated multi-frame pixel data requires offset table support",
        ));
    }

    let start = find_jpeg_start(data).ok_or_else(|| decode_error("jpeg", "missing SOI marker"))?;
    let data = &data[start..];

    let info = parse_jpeg_header(data)?;
    if info.precision > 8 {
        return Err(decode_error("jpeg", "JPEG baseline must be 8-bit"));
    }
    if info.components as u8 != meta.samples_per_pixel {
        return Err(decode_error("jpeg", "JPEG component count mismatch"));
    }

    let mut decoder = jpeg_decoder::Decoder::new(data);
    let pixels = decoder
        .decode()
        .map_err(|_| decode_error("jpeg", "JPEG decode failed"))?;
    let metadata = decoder
        .info()
        .ok_or_else(|| decode_error("jpeg", "JPEG metadata missing"))?;
    if metadata.width != meta.cols as u16 || metadata.height != meta.rows as u16 {
        return Err(decode_error("jpeg", "JPEG dimensions do not match dataset"));
    }

    let mut samples = Vec::with_capacity(pixels.len());
    for &b in &pixels {
        samples.push(b as i32);
    }

    Ok(RawFrame {
        width: meta.cols,
        height: meta.rows,
        samples_per_pixel: meta.samples_per_pixel,
        samples,
    })
}

#[cfg(feature = "codec-jpegls")]
fn decode_jpegls(data: &[u8], meta: &PixelMeta, config: &PixelPipelineConfig) -> Result<RawFrame> {
    if meta.number_of_frames > 1 {
        return Err(decode_error(
            "jpegls",
            "encapsulated multi-frame pixel data requires offset table support",
        ));
    }

    fn decode_inner(slice: &[u8]) -> Result<(charls::FrameInfo, Vec<u8>)> {
        let mut info_decoder = CharLS::default();
        let info = info_decoder
            .get_frame_info(slice)
            .map_err(|_| decode_error("jpegls", "JPEG-LS header decode failed"))?;
        let mut data_decoder = CharLS::default();
        let decoded = data_decoder
            .decode(slice)
            .map_err(|_| decode_error("jpegls", "JPEG-LS decode failed"))?;
        Ok((info, decoded))
    }

    let (info, decoded) = match decode_inner(data) {
        Ok(result) => result,
        Err(_) => {
            let start = find_jpeg_start(data)
                .ok_or_else(|| decode_error("jpegls", "missing SOI marker"))?;
            decode_inner(&data[start..])?
        }
    };
    if info.width != meta.cols || info.height != meta.rows {
        return Err(decode_error(
            "jpegls",
            "JPEG-LS dimensions do not match dataset",
        ));
    }
    if info.component_count as u8 != meta.samples_per_pixel {
        return Err(decode_error("jpegls", "JPEG-LS component count mismatch"));
    }
    if info.bits_per_sample as u16 != meta.bits_stored || meta.bits_allocated < meta.bits_stored {
        return Err(decode_error("jpegls", "JPEG-LS bit depth mismatch"));
    }
    if !matches!(meta.bits_allocated, 8 | 16) {
        return Err(invalid_tag_value(
            tags::TAG_BITS_ALLOCATED,
            "BitsAllocated must be 8 or 16",
        ));
    }

    let pixels_per_frame = meta.rows as usize * meta.cols as usize;
    let bytes_per_sample = (meta.bits_allocated / 8) as usize;
    let expected = pixels_per_frame * meta.samples_per_pixel as usize * bytes_per_sample;
    if decoded.len() != expected {
        return Err(decode_error("jpegls", "JPEG-LS decoded size mismatch"));
    }
    enforce_limit(
        "max_decompressed_bytes",
        decoded.len() as u64,
        config.limits.max_decompressed_bytes(),
    )?;

    let mut samples = Vec::with_capacity(pixels_per_frame * meta.samples_per_pixel as usize);
    match meta.bits_allocated {
        8 => {
            for &b in &decoded {
                samples.push(mask_and_sign_extend(b as i32, meta));
            }
        }
        16 => {
            let mut i = 0;
            while i + 1 < decoded.len() {
                let raw = u16::from_le_bytes([decoded[i], decoded[i + 1]]);
                let value = match meta.pixel_representation {
                    PixelRepresentation::Unsigned => raw as i32,
                    PixelRepresentation::Signed => (raw as i16) as i32,
                };
                samples.push(mask_and_sign_extend(value, meta));
                i += 2;
            }
        }
        _ => {}
    }

    Ok(RawFrame {
        width: meta.cols,
        height: meta.rows,
        samples_per_pixel: meta.samples_per_pixel,
        samples,
    })
}

#[cfg(feature = "codec-j2k")]
fn decode_jpeg2000(
    data: &[u8],
    meta: &PixelMeta,
    config: &PixelPipelineConfig,
) -> Result<RawFrame> {
    if meta.number_of_frames > 1 {
        return Err(decode_error(
            "j2k",
            "encapsulated multi-frame pixel data requires offset table support",
        ));
    }
    if meta.pixel_representation == PixelRepresentation::Signed {
        return Err(decode_error("j2k", "JPEG2000 signed pixels unsupported"));
    }
    if meta.bits_allocated != 8 || meta.bits_stored > 8 {
        return Err(invalid_tag_value(
            tags::TAG_BITS_ALLOCATED,
            "JPEG2000 requires 8-bit samples",
        ));
    }

    let start = find_j2k_start(data).ok_or_else(|| decode_error("j2k", "missing codestream"))?;
    let slice = &data[start..];
    let bitmap = decode_jpeg2000_bitmap(slice, &DecodeSettings::default())
        .map_err(|_| decode_error("j2k", "JPEG2000 decode failed"))?;
    if bitmap.width != meta.cols || bitmap.height != meta.rows {
        return Err(decode_error(
            "j2k",
            "JPEG2000 dimensions do not match dataset",
        ));
    }
    if bitmap.has_alpha {
        return Err(decode_error("j2k", "JPEG2000 alpha unsupported"));
    }

    let expected_spp = match bitmap.color_space {
        ColorSpace::Gray => 1,
        ColorSpace::RGB => 3,
        _ => return Err(decode_error("j2k", "JPEG2000 color space unsupported")),
    };
    if expected_spp as u8 != meta.samples_per_pixel {
        return Err(decode_error("j2k", "JPEG2000 component count mismatch"));
    }
    match (bitmap.color_space, meta.photometric) {
        (ColorSpace::Gray, Photometric::Monochrome1 | Photometric::Monochrome2) => {}
        (ColorSpace::RGB, Photometric::Rgb) => {}
        _ => return Err(decode_error("j2k", "JPEG2000 photometric mismatch")),
    }

    enforce_limit(
        "max_decompressed_bytes",
        bitmap.data.len() as u64,
        config.limits.max_decompressed_bytes(),
    )?;

    let mut samples = Vec::with_capacity(bitmap.data.len());
    for &b in &bitmap.data {
        samples.push(b as i32);
    }

    Ok(RawFrame {
        width: meta.cols,
        height: meta.rows,
        samples_per_pixel: meta.samples_per_pixel,
        samples,
    })
}

#[derive(Debug, Clone, Copy)]
struct JpegHeaderInfo {
    precision: u8,
    components: u8,
}

fn parse_jpeg_header(data: &[u8]) -> Result<JpegHeaderInfo> {
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
        return Err(decode_error("jpeg", "invalid SOI"));
    }
    let mut i = 2;
    while i + 4 <= data.len() {
        if data[i] != 0xFF {
            return Err(decode_error("jpeg", "invalid marker"));
        }
        let marker = data[i + 1];
        i += 2;
        if marker == 0xD9 || marker == 0xDA {
            break;
        }
        if i + 2 > data.len() {
            break;
        }
        let length = u16::from_be_bytes([data[i], data[i + 1]]) as usize;
        if length < 2 || i + length > data.len() {
            return Err(decode_error("jpeg", "invalid segment length"));
        }
        if marker == 0xC0 {
            if length < 8 {
                return Err(decode_error("jpeg", "invalid SOF0 length"));
            }
            let precision = data[i + 2];
            let components = data[i + 7];
            return Ok(JpegHeaderInfo {
                precision,
                components,
            });
        }
        i += length;
    }
    Err(decode_error("jpeg", "missing SOF0 marker"))
}

fn find_jpeg_start(data: &[u8]) -> Option<usize> {
    data.windows(2).position(|w| w == [0xFF, 0xD8])
}

#[cfg(feature = "codec-j2k")]
fn find_j2k_start(data: &[u8]) -> Option<usize> {
    const JP2_MAGIC: &[u8] = b"\x00\x00\x00\x0C\x6A\x50\x20\x20";
    const CODESTREAM_MAGIC: &[u8] = b"\xFF\x4F\xFF\x51";
    let search_limit = data.len().min(512);
    let mut i = 0;
    while i < search_limit {
        let tail = &data[i..search_limit];
        if tail.starts_with(JP2_MAGIC) || tail.starts_with(CODESTREAM_MAGIC) {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn apply_modality_transform(
    dataset: &Dataset,
    raw: &RawFrame,
    meta: &PixelMeta,
    config: &PixelPipelineConfig,
    frame_index: u32,
) -> Result<ModalityFrame> {
    let lut = read_lut_sequence(dataset, tags::TAG_MODALITY_LUT_SEQUENCE, &config.limits, 0)?;
    let rescale = read_rescale(dataset, &config.limits, frame_index)?;

    let mut values = Vec::with_capacity(raw.samples.len());
    for &sample in &raw.samples {
        let value = if let Some(lut) = &lut {
            apply_lut(lut, sample)?
        } else if let Some((slope, intercept)) = rescale {
            let v = (sample as f64) * slope + intercept;
            ensure_finite("modality", v)?;
            v
        } else {
            sample as f64
        };
        values.push(value);
    }

    Ok(ModalityFrame {
        width: raw.width,
        height: raw.height,
        values,
        padding_value: meta.pixel_padding_value,
    })
}

fn read_rescale(
    dataset: &Dataset,
    limits: &Limits,
    frame_index: u32,
) -> Result<Option<(f64, f64)>> {
    let _ = frame_index;
    #[cfg(feature = "pack-enhanced")]
    {
        if is_enhanced_sop_class(dataset, limits)? {
            let sop = read_string_required(dataset, tags::TAG_SOP_CLASS_UID, limits)?;
            EnhancedPack::ensure_supported(sop.as_str())?;
            let groups = select_frame_groups(dataset, frame_index, limits)?;
            if let Some(rescale) = pack_enhanced::extract_frame_rescale(&groups, limits)? {
                return Ok(Some((rescale.slope, rescale.intercept)));
            }
        }
    }
    let slope = read_optional_f64(dataset, tags::TAG_RESCALE_SLOPE, limits)?;
    let intercept = read_optional_f64(dataset, tags::TAG_RESCALE_INTERCEPT, limits)?;
    match (slope, intercept) {
        (Some(slope), Some(intercept)) => Ok(Some((slope, intercept))),
        (None, None) => Ok(None),
        _ => Err(invalid_tag_value(
            tags::TAG_RESCALE_SLOPE,
            "Rescale Slope/Intercept must both be present",
        )),
    }
}

#[cfg(feature = "pack-enhanced")]
fn is_enhanced_sop_class(dataset: &Dataset, limits: &Limits) -> Result<bool> {
    let Some(sop) = read_optional_string(dataset, tags::TAG_SOP_CLASS_UID, limits)? else {
        return Ok(false);
    };
    Ok(matches!(
        sop.as_str(),
        SOP_CLASS_ENHANCED_CT | SOP_CLASS_ENHANCED_MR
    ))
}

fn apply_voi_transform(
    dataset: &Dataset,
    modality: &ModalityFrame,
    config: &PixelPipelineConfig,
) -> Result<Vec<f64>> {
    match config.window_level {
        WindowLevel::Explicit { center, width } => {
            apply_window_values(modality, center, width, VoiFunction::Linear)
        }
        WindowLevel::VoiLut { index } => {
            let lut =
                read_lut_sequence(dataset, tags::TAG_VOI_LUT_SEQUENCE, &config.limits, index)?
                    .ok_or_else(|| decode_error("voi", "VOI LUT sequence missing"))?;
            apply_voi_lut(modality, &lut)
        }
        WindowLevel::Auto => {
            if let Some(lut) =
                read_lut_sequence(dataset, tags::TAG_VOI_LUT_SEQUENCE, &config.limits, 0)?
            {
                apply_voi_lut(modality, &lut)
            } else if let Some((center, width, func)) = read_window_level(dataset, &config.limits)?
            {
                apply_window_values(modality, center, width, func)
            } else {
                let (center, width, func) = auto_window(modality, config)?;
                apply_window_values(modality, center, width, func)
            }
        }
    }
}

fn apply_voi_lut(modality: &ModalityFrame, lut: &Lut) -> Result<Vec<f64>> {
    let max = (1u64 << lut.entry_bits.min(16) as u64) as f64 - 1.0;
    let mut luma = Vec::with_capacity(modality.values.len());
    for &value in &modality.values {
        ensure_finite("voi", value)?;
        let idx = (value.round() as i32) - lut.first_mapped;
        let idx = idx.max(0) as usize;
        let idx = idx.min(lut.values.len().saturating_sub(1));
        let v = lut.values[idx] as f64;
        let l = (v / max).clamp(0.0, 1.0);
        luma.push(l);
    }
    Ok(luma)
}

#[derive(Debug, Clone, Copy)]
enum VoiFunction {
    Linear,
    LinearExact,
}

fn read_window_level(
    dataset: &Dataset,
    limits: &Limits,
) -> Result<Option<(f64, f64, VoiFunction)>> {
    let center = read_optional_f64(dataset, tags::TAG_WINDOW_CENTER, limits)?;
    let width = read_optional_f64(dataset, tags::TAG_WINDOW_WIDTH, limits)?;
    let func = read_voi_function(dataset, limits)?;
    match (center, width) {
        (Some(center), Some(width)) => Ok(Some((center, width, func))),
        (None, None) => Ok(None),
        _ => Err(invalid_tag_value(
            tags::TAG_WINDOW_CENTER,
            "Window Center/Width must both be present",
        )),
    }
}

fn read_voi_function(dataset: &Dataset, limits: &Limits) -> Result<VoiFunction> {
    let func = read_optional_string(dataset, tags::TAG_VOI_LUT_FUNCTION, limits)?;
    match func.as_deref() {
        None | Some("LINEAR") => Ok(VoiFunction::Linear),
        Some("LINEAR_EXACT") => Ok(VoiFunction::LinearExact),
        Some(other) => Err(invalid_tag_value(
            tags::TAG_VOI_LUT_FUNCTION,
            format!("Unsupported VOI LUT function: {other}"),
        )),
    }
}

fn apply_window_values(
    modality: &ModalityFrame,
    center: f64,
    width: f64,
    func: VoiFunction,
) -> Result<Vec<f64>> {
    let mut luma = Vec::with_capacity(modality.values.len());
    for &value in &modality.values {
        ensure_finite("voi", value)?;
        luma.push(apply_window(value, center, width, func)?);
    }
    Ok(luma)
}

fn apply_window(x: f64, center: f64, mut width: f64, func: VoiFunction) -> Result<f64> {
    if !width.is_finite() || !center.is_finite() {
        return Err(decode_error("voi", "Window parameters must be finite"));
    }
    if width < 1.0 {
        width = 1.0;
    }

    let l = match func {
        VoiFunction::Linear => {
            let lower = center - 0.5 - (width - 1.0) / 2.0;
            let upper = center - 0.5 + (width - 1.0) / 2.0;
            if x <= lower {
                0.0
            } else if x > upper {
                1.0
            } else {
                ((x - (center - 0.5)) / (width - 1.0) + 0.5).clamp(0.0, 1.0)
            }
        }
        VoiFunction::LinearExact => {
            let lower = center - width / 2.0;
            let upper = center + width / 2.0;
            if x <= lower {
                0.0
            } else if x > upper {
                1.0
            } else {
                ((x - center) / width + 0.5).clamp(0.0, 1.0)
            }
        }
    };
    ensure_finite("voi", l)?;
    Ok(l)
}

fn auto_window(
    modality: &ModalityFrame,
    config: &PixelPipelineConfig,
) -> Result<(f64, f64, VoiFunction)> {
    validate_auto_window_config(config)?;
    let total_pixels = (modality.width as usize) * (modality.height as usize);
    let target = config.auto_window_samples.max(1);
    let stride = ((total_pixels as f64 / target as f64).sqrt().floor() as usize).max(1);

    let mut samples = Vec::new();
    for (i, &value) in modality.values.iter().enumerate() {
        if i % stride != 0 {
            continue;
        }
        if let Some(padding) = modality.padding_value {
            if (value as i32) == padding {
                continue;
            }
        }
        ensure_finite("voi", value)?;
        samples.push(value);
    }

    if samples.is_empty() {
        return Err(decode_error("voi", "missing renderable pixels"));
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let m = samples.len();
    let low_idx = (config.auto_window_low_percentile * (m - 1) as f64).floor() as usize;
    let high_idx = (config.auto_window_high_percentile * (m - 1) as f64).ceil() as usize;
    let low = samples[low_idx];
    let high = samples[high_idx.min(m - 1)];

    let width = (high - low).max(1.0);
    let center = (high + low) / 2.0;
    Ok((center, width, VoiFunction::Linear))
}

fn validate_auto_window_config(config: &PixelPipelineConfig) -> Result<()> {
    let low = config.auto_window_low_percentile;
    let high = config.auto_window_high_percentile;
    if !low.is_finite() {
        return Err(invalid_pixel_transform(
            "voi",
            "auto_window_low_percentile must be finite",
        ));
    }
    if !high.is_finite() {
        return Err(invalid_pixel_transform(
            "voi",
            "auto_window_high_percentile must be finite",
        ));
    }
    if !(0.0..=1.0).contains(&low) {
        return Err(invalid_pixel_transform(
            "voi",
            "auto_window_low_percentile must be within [0.0, 1.0]",
        ));
    }
    if !(0.0..=1.0).contains(&high) {
        return Err(invalid_pixel_transform(
            "voi",
            "auto_window_high_percentile must be within [0.0, 1.0]",
        ));
    }
    if low > high {
        return Err(invalid_pixel_transform(
            "voi",
            "auto_window percentiles must satisfy low <= high",
        ));
    }
    Ok(())
}

fn pack_luma8(values: &[f64]) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(values.len());
    for &value in values {
        ensure_finite("pack", value)?;
        let l = value.clamp(0.0, 1.0);
        let q = if l >= 0.0 {
            (l * 255.0 + 0.5).floor()
        } else {
            (l * 255.0 - 0.5).ceil()
        };
        out.push(q.clamp(0.0, 255.0) as u8);
    }
    Ok(out)
}

fn pack_rgba8(samples: &[i32], samples_per_pixel: u8) -> Result<Vec<u8>> {
    if samples_per_pixel != 3 {
        return Err(decode_error(
            "pack",
            "RGB output requires 3 samples per pixel",
        ));
    }
    let mut out = Vec::with_capacity(samples.len() / 3 * 4);
    let mut i = 0;
    while i + 2 < samples.len() {
        out.push(samples[i].clamp(0, 255) as u8);
        out.push(samples[i + 1].clamp(0, 255) as u8);
        out.push(samples[i + 2].clamp(0, 255) as u8);
        out.push(255u8);
        i += 3;
    }
    Ok(out)
}

fn ybr_to_rgb(samples: &[i32]) -> Result<Vec<i32>> {
    if !samples.len().is_multiple_of(3) {
        return Err(decode_error(
            "photometric",
            "YBR sample buffer length is not divisible by 3",
        ));
    }

    let mut out = Vec::with_capacity(samples.len());
    let mut i = 0;
    while i + 2 < samples.len() {
        let y = samples[i];
        let cb = samples[i + 1] - 128;
        let cr = samples[i + 2] - 128;

        // Deterministic fixed-point conversion to RGB at integer precision.
        let r = y + ((91_881 * cr + 32_768) >> 16);
        let g = y - ((22_554 * cb + 46_802 * cr + 32_768) >> 16);
        let b = y + ((116_130 * cb + 32_768) >> 16);

        out.push(r);
        out.push(g);
        out.push(b);
        i += 3;
    }

    Ok(out)
}

fn apply_overlays(
    dataset: &Dataset,
    meta: &PixelMeta,
    frame: &mut DisplayFrame,
    limits: &Limits,
) -> Result<()> {
    let overlay_data = match dataset.get(tags::TAG_OVERLAY_DATA) {
        None => return Ok(()),
        Some(element) => match element.value() {
            Value::Bytes(bytes) => bytes,
            _ => {
                return Err(invalid_tag_value(
                    tags::TAG_OVERLAY_DATA,
                    "overlay data must be bytes",
                ))
            }
        },
    };
    let rows = read_u16_required(dataset, tags::TAG_OVERLAY_ROWS)? as usize;
    let cols = read_u16_required(dataset, tags::TAG_OVERLAY_COLUMNS)? as usize;
    let (row_origin, col_origin) = read_overlay_origin(dataset, limits)?;
    let bits_allocated = read_u16_required(dataset, tags::TAG_OVERLAY_BITS_ALLOCATED)?;
    if bits_allocated != 1 {
        return Err(invalid_tag_value(
            tags::TAG_OVERLAY_BITS_ALLOCATED,
            "overlay bits allocated must be 1",
        ));
    }
    let bit_position = read_u16_required(dataset, tags::TAG_OVERLAY_BIT_POSITION)?;
    if bit_position > 7 {
        return Err(invalid_tag_value(
            tags::TAG_OVERLAY_BIT_POSITION,
            "overlay bit position must be 0..7",
        ));
    }

    if row_origin < 1 || col_origin < 1 {
        return Err(invalid_tag_value(
            tags::TAG_OVERLAY_ORIGIN,
            "overlay origin must be >= 1",
        ));
    }
    let row_origin = (row_origin - 1) as usize;
    let col_origin = (col_origin - 1) as usize;
    if row_origin + rows > meta.rows as usize || col_origin + cols > meta.cols as usize {
        return Err(decode_error(
            "overlay",
            "overlay dimensions exceed image bounds",
        ));
    }

    let total_bits = rows * cols;
    let required_bytes = total_bits.div_ceil(8);
    if overlay_data.len() < required_bytes {
        return Err(decode_error(
            "overlay",
            "overlay data shorter than declared dimensions",
        ));
    }

    for idx in 0..total_bits {
        let byte = overlay_data[idx / 8];
        let bit = (byte >> (idx % 8)) & 1;
        if bit == 0 {
            continue;
        }
        let row = row_origin + (idx / cols);
        let col = col_origin + (idx % cols);
        match frame.format {
            PixelFormat::Luma8 => {
                let idx = row * meta.cols as usize + col;
                if let Some(slot) = frame.bytes.get_mut(idx) {
                    *slot = 255;
                }
            }
            PixelFormat::Rgba8 => {
                let idx = (row * meta.cols as usize + col) * 4;
                if idx + 3 < frame.bytes.len() {
                    frame.bytes[idx] = 255;
                    frame.bytes[idx + 1] = 0;
                    frame.bytes[idx + 2] = 0;
                    frame.bytes[idx + 3] = 255;
                }
            }
            PixelFormat::Luma16 => {}
        }
    }
    Ok(())
}

fn mask_and_sign_extend(value: i32, meta: &PixelMeta) -> i32 {
    if meta.bits_stored >= 16 {
        return value;
    }
    let mask = (1u32 << meta.bits_stored) - 1;
    let masked = (value as u32) & mask;
    if meta.pixel_representation == PixelRepresentation::Signed {
        let sign_bit = 1u32 << (meta.bits_stored - 1);
        if masked & sign_bit != 0 {
            let extended = (masked | (!mask)) as i32;
            return extended;
        }
    }
    masked as i32
}

fn ensure_finite(stage: &'static str, value: f64) -> Result<()> {
    if !value.is_finite() {
        return Err(invalid_pixel_transform(stage, "non-finite value"));
    }
    Ok(())
}

fn read_u16_required(dataset: &Dataset, tag: Tag) -> Result<u16> {
    read_u16(dataset, tag)?.ok_or_else(|| missing_required_tag(tag))
}

fn read_u16(dataset: &Dataset, tag: Tag) -> Result<Option<u16>> {
    match dataset.get(tag) {
        None => Ok(None),
        Some(element) => match element.value() {
            Value::Bytes(bytes) => parse_u16_from_bytes(tag, bytes).map(Some),
            Value::Str(value) => dicom_core::parse_i32_strict(tag, trim_dicom_padding(value))
                .map(|v| v as u16)
                .map(Some),
            _ => Err(invalid_tag_value(tag, "expected numeric value")),
        },
    }
}

fn read_optional_i32(dataset: &Dataset, tag: Tag, limits: &Limits) -> Result<Option<i32>> {
    match dataset.get(tag) {
        None => Ok(None),
        Some(element) => match element.value() {
            Value::Str(value) => {
                enforce_limit(
                    "max_string_bytes",
                    value.len() as u64,
                    limits.max_string_bytes(),
                )?;
                dicom_core::parse_i32_strict(tag, trim_dicom_padding(value)).map(Some)
            }
            Value::Bytes(bytes) => parse_i32_from_bytes(tag, bytes).map(Some),
            _ => Err(invalid_tag_value(tag, "expected integer value")),
        },
    }
}

fn read_optional_i16(dataset: &Dataset, tag: Tag) -> Result<Option<i16>> {
    match dataset.get(tag) {
        None => Ok(None),
        Some(element) => match element.value() {
            Value::Bytes(bytes) => parse_i16_from_bytes(tag, bytes).map(Some),
            Value::Str(value) => dicom_core::parse_i32_strict(tag, trim_dicom_padding(value))
                .map(|v| v as i16)
                .map(Some),
            _ => Err(invalid_tag_value(tag, "expected integer value")),
        },
    }
}

fn read_optional_f64(dataset: &Dataset, tag: Tag, limits: &Limits) -> Result<Option<f64>> {
    match dataset.get(tag) {
        None => Ok(None),
        Some(element) => match element.value() {
            Value::Str(value) => {
                enforce_limit(
                    "max_string_bytes",
                    value.len() as u64,
                    limits.max_string_bytes(),
                )?;
                dicom_core::parse_f64_strict(tag, trim_dicom_padding(value)).map(Some)
            }
            Value::Bytes(bytes) => {
                enforce_limit(
                    "max_string_bytes",
                    bytes.len() as u64,
                    limits.max_string_bytes(),
                )?;
                let s = String::from_utf8_lossy(bytes);
                dicom_core::parse_f64_strict(tag, trim_dicom_padding(&s)).map(Some)
            }
            _ => Err(invalid_tag_value(tag, "expected float value")),
        },
    }
}

fn read_optional_string(dataset: &Dataset, tag: Tag, limits: &Limits) -> Result<Option<String>> {
    match dataset.get(tag) {
        None => Ok(None),
        Some(element) => match element.value() {
            Value::Str(value) => {
                enforce_limit(
                    "max_string_bytes",
                    value.len() as u64,
                    limits.max_string_bytes(),
                )?;
                Ok(Some(value.clone()))
            }
            Value::Uid(value) => {
                enforce_limit(
                    "max_string_bytes",
                    value.len() as u64,
                    limits.max_string_bytes(),
                )?;
                Ok(Some(value.clone()))
            }
            Value::Bytes(bytes) => {
                enforce_limit(
                    "max_string_bytes",
                    bytes.len() as u64,
                    limits.max_string_bytes(),
                )?;
                Ok(Some(String::from_utf8_lossy(bytes).into_owned()))
            }
            _ => Err(invalid_tag_value(tag, "expected string value")),
        },
    }
}

fn read_overlay_origin(dataset: &Dataset, limits: &Limits) -> Result<(i32, i32)> {
    match dataset.get(tags::TAG_OVERLAY_ORIGIN) {
        None => Ok((1, 1)),
        Some(element) => match element.value() {
            Value::Bytes(bytes) => {
                enforce_limit(
                    "max_element_vl_bytes",
                    bytes.len() as u64,
                    limits.max_element_vl_bytes(),
                )?;
                if bytes.len() == 4 {
                    let row = i16::from_le_bytes([bytes[0], bytes[1]]) as i32;
                    let col = i16::from_le_bytes([bytes[2], bytes[3]]) as i32;
                    Ok((row, col))
                } else if bytes.len() == 8 {
                    let row = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                    let col = i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
                    Ok((row, col))
                } else {
                    Err(invalid_tag_value(
                        tags::TAG_OVERLAY_ORIGIN,
                        "overlay origin byte length invalid",
                    ))
                }
            }
            Value::Str(value) => {
                enforce_limit(
                    "max_string_bytes",
                    value.len() as u64,
                    limits.max_string_bytes(),
                )?;
                let parts: Vec<&str> = value.split('\\').collect();
                if parts.len() != 2 {
                    return Err(invalid_tag_value(
                        tags::TAG_OVERLAY_ORIGIN,
                        "overlay origin must be row\\col",
                    ));
                }
                let row = dicom_core::parse_i32_strict(tags::TAG_OVERLAY_ORIGIN, parts[0])?;
                let col = dicom_core::parse_i32_strict(tags::TAG_OVERLAY_ORIGIN, parts[1])?;
                Ok((row, col))
            }
            _ => Err(invalid_tag_value(
                tags::TAG_OVERLAY_ORIGIN,
                "overlay origin must be numeric or string",
            )),
        },
    }
}

fn read_string_required(dataset: &Dataset, tag: Tag, limits: &Limits) -> Result<String> {
    match dataset.get(tag) {
        None => Err(missing_required_tag(tag)),
        Some(element) => match element.value() {
            Value::Str(value) => {
                enforce_limit(
                    "max_string_bytes",
                    value.len() as u64,
                    limits.max_string_bytes(),
                )?;
                Ok(value.clone())
            }
            Value::Uid(value) => {
                enforce_limit(
                    "max_string_bytes",
                    value.len() as u64,
                    limits.max_string_bytes(),
                )?;
                Ok(value.clone())
            }
            Value::Bytes(bytes) => {
                enforce_limit(
                    "max_string_bytes",
                    bytes.len() as u64,
                    limits.max_string_bytes(),
                )?;
                Ok(String::from_utf8_lossy(bytes).into_owned())
            }
            _ => Err(invalid_tag_value(tag, "expected string value")),
        },
    }
}

fn trim_dicom_padding(value: &str) -> &str {
    value.trim_matches(|ch| ch == ' ' || ch == '\0')
}

fn trim_dicom_cs_padding(value: &str) -> &str {
    trim_dicom_padding(value)
}

fn read_bytes_required(dataset: &Dataset, tag: Tag) -> Result<Vec<u8>> {
    match dataset.get(tag) {
        None => Err(missing_required_tag(tag)),
        Some(element) => match element.value() {
            Value::Bytes(bytes) => Ok(bytes.clone()),
            _ => Err(invalid_tag_value(tag, "expected bytes")),
        },
    }
}

fn parse_u16_from_bytes(tag: Tag, bytes: &[u8]) -> Result<u16> {
    match bytes.len() {
        2 => Ok(u16::from_le_bytes([bytes[0], bytes[1]])),
        4 => Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as u16),
        _ => Err(invalid_tag_value(tag, "invalid u16 byte length")),
    }
}

fn parse_i16_from_bytes(tag: Tag, bytes: &[u8]) -> Result<i16> {
    match bytes.len() {
        2 => Ok(i16::from_le_bytes([bytes[0], bytes[1]])),
        4 => Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as i16),
        _ => Err(invalid_tag_value(tag, "invalid i16 byte length")),
    }
}

fn parse_i32_from_bytes(tag: Tag, bytes: &[u8]) -> Result<i32> {
    match bytes.len() {
        4 => Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])),
        2 => Ok(i16::from_le_bytes([bytes[0], bytes[1]]) as i32),
        _ => Err(invalid_tag_value(tag, "invalid i32 byte length")),
    }
}

fn missing_required_tag(tag: Tag) -> Box<Error> {
    Error::from_kind(
        ErrorKind::MissingRequiredTag { tag },
        "missing required tag",
    )
    .with_context("tag", format!("{tag:?}"))
    .into()
}

fn invalid_tag_value(tag: Tag, detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::InvalidTagValue {
            tag,
            detail: detail.into(),
        },
        "invalid tag value",
    )
    .into()
}

fn decode_error(stage: &'static str, detail: &str) -> Box<Error> {
    Error::from_kind(
        ErrorKind::DecodeError {
            stage: stage.to_string(),
            detail: detail.to_string(),
        },
        "decode error",
    )
    .into()
}

fn invalid_pixel_transform(stage: &'static str, detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::InvalidPixelTransform {
            stage: stage.to_string(),
            detail: detail.into(),
        },
        "invalid pixel transform",
    )
    .into()
}

fn limit_overflow(limit_name: &'static str, allowed: u64) -> Box<Error> {
    Error::from_kind(
        ErrorKind::LimitExceeded {
            limit_name,
            observed: allowed.saturating_add(1),
            allowed,
        },
        "limit exceeded",
    )
    .into()
}

fn enforce_limit(limit_name: &'static str, observed: u64, allowed: u64) -> Result<()> {
    dicom_core::enforce_limit(limit_name, observed, allowed)
}

fn read_lut_sequence(
    dataset: &Dataset,
    tag: Tag,
    limits: &Limits,
    index: usize,
) -> Result<Option<Lut>> {
    let sequence = match dataset.get(tag) {
        None => return Ok(None),
        Some(element) => match element.value() {
            Value::Sequence(items) => items,
            _ => return Err(invalid_tag_value(tag, "LUT Sequence must be a sequence")),
        },
    };
    if sequence.is_empty() {
        return Ok(None);
    }
    if index >= sequence.len() {
        return Err(invalid_tag_value(tag, "LUT sequence index out of range"));
    }
    let item = &sequence[index];
    let descriptor_bytes = read_bytes_required(item, tags::TAG_LUT_DESCRIPTOR)?;
    if descriptor_bytes.len() < 6 {
        return Err(invalid_tag_value(
            tags::TAG_LUT_DESCRIPTOR,
            "LUT Descriptor must have 3 US values",
        ));
    }
    let count = u16::from_le_bytes([descriptor_bytes[0], descriptor_bytes[1]]) as u32;
    let first_mapped = i16::from_le_bytes([descriptor_bytes[2], descriptor_bytes[3]]) as i32;
    let entry_bits = u16::from_le_bytes([descriptor_bytes[4], descriptor_bytes[5]]);
    let num_entries = if count == 0 { 65_536 } else { count as usize };

    let data_bytes = read_bytes_required(item, tags::TAG_LUT_DATA)?;
    let values = if entry_bits <= 8 {
        if data_bytes.len() < num_entries {
            return Err(invalid_tag_value(
                tags::TAG_LUT_DATA,
                "LUT Data length too small",
            ));
        }
        data_bytes[..num_entries]
            .iter()
            .map(|b| *b as u16)
            .collect()
    } else {
        let needed = num_entries * 2;
        if data_bytes.len() < needed {
            return Err(invalid_tag_value(
                tags::TAG_LUT_DATA,
                "LUT Data length too small",
            ));
        }
        let mut vals = Vec::with_capacity(num_entries);
        for i in 0..num_entries {
            let idx = i * 2;
            let v = u16::from_le_bytes([data_bytes[idx], data_bytes[idx + 1]]);
            vals.push(v);
        }
        vals
    };

    enforce_limit(
        "max_element_vl_bytes",
        data_bytes.len() as u64,
        limits.max_element_vl_bytes(),
    )?;

    Ok(Some(Lut {
        first_mapped,
        entry_bits,
        values,
    }))
}

fn apply_lut(lut: &Lut, sample: i32) -> Result<f64> {
    let mut idx = sample - lut.first_mapped;
    if idx < 0 {
        idx = 0;
    }
    let idx = idx as usize;
    let idx = idx.min(lut.values.len().saturating_sub(1));
    Ok(lut.values[idx] as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_core::Element;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_env_vars<F, R>(pairs: &[(&str, Option<&str>)], f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let previous: Vec<(&str, Option<String>)> = pairs
            .iter()
            .map(|(name, _)| (*name, std::env::var(name).ok()))
            .collect();
        for (name, value) in pairs {
            match value {
                Some(raw) => std::env::set_var(name, raw),
                None => std::env::remove_var(name),
            }
        }
        let result = f();
        for (name, value) in previous {
            match value {
                Some(raw) => std::env::set_var(name, raw),
                None => std::env::remove_var(name),
            }
        }
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn make_dataset(
        rows: u16,
        cols: u16,
        samples_per_pixel: u16,
        photometric: &str,
        bits_allocated: u16,
        bits_stored: u16,
        high_bit: u16,
        pixel_representation: u16,
        planar_configuration: Option<u16>,
        pixel_data: Vec<u8>,
    ) -> Dataset {
        let mut dataset = Dataset::new();
        dataset.insert(
            Element::new(
                tags::TAG_ROWS,
                dicom_core::Vr::Us,
                Value::Bytes(rows.to_le_bytes().to_vec()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                tags::TAG_COLUMNS,
                dicom_core::Vr::Us,
                Value::Bytes(cols.to_le_bytes().to_vec()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                tags::TAG_SAMPLES_PER_PIXEL,
                dicom_core::Vr::Us,
                Value::Bytes(samples_per_pixel.to_le_bytes().to_vec()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                tags::TAG_PHOTOMETRIC_INTERPRETATION,
                dicom_core::Vr::Cs,
                Value::Str(photometric.to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                tags::TAG_BITS_ALLOCATED,
                dicom_core::Vr::Us,
                Value::Bytes(bits_allocated.to_le_bytes().to_vec()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                tags::TAG_BITS_STORED,
                dicom_core::Vr::Us,
                Value::Bytes(bits_stored.to_le_bytes().to_vec()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                tags::TAG_HIGH_BIT,
                dicom_core::Vr::Us,
                Value::Bytes(high_bit.to_le_bytes().to_vec()),
            )
            .unwrap(),
        );
        if photometric.starts_with("MONOCHROME") {
            dataset.insert(
                Element::new(
                    tags::TAG_PIXEL_REPRESENTATION,
                    dicom_core::Vr::Us,
                    Value::Bytes(pixel_representation.to_le_bytes().to_vec()),
                )
                .unwrap(),
            );
        }
        if let Some(planar) = planar_configuration {
            dataset.insert(
                Element::new(
                    tags::TAG_PLANAR_CONFIGURATION,
                    dicom_core::Vr::Us,
                    Value::Bytes(planar.to_le_bytes().to_vec()),
                )
                .unwrap(),
            );
        }
        dataset.insert(
            Element::new(
                tags::TAG_PIXEL_DATA,
                dicom_core::Vr::Ob,
                Value::Bytes(pixel_data),
            )
            .unwrap(),
        );
        dataset
    }

    #[test]
    fn default_pipeline_config_uses_limits_default() {
        // REQ-SEC-402: defaults apply when not overridden.
        let config = PixelPipelineConfig::default();
        assert_eq!(config.limits, Limits::default());
        assert_eq!(config.window_level, WindowLevel::Auto);
    }

    #[cfg(feature = "pack-enhanced")]
    #[test]
    fn enhanced_sop_uid_uid_value_is_accepted_in_optional_string_path() {
        // REQ-PIX-245: UID-valued SOP Class tags must be accepted on optional string reads.
        let mut dataset = Dataset::new();
        dataset.insert(
            Element::new(
                tags::TAG_SOP_CLASS_UID,
                dicom_core::Vr::Ui,
                Value::Uid(SOP_CLASS_ENHANCED_CT.to_string()),
            )
            .unwrap(),
        );
        assert!(is_enhanced_sop_class(&dataset, &Limits::default()).expect("enhanced sop"));
    }

    #[test]
    fn decode_rejects_missing_required_tag() {
        // REQ-PIX-230
        let dataset = Dataset::new();
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let err = pipeline
            .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
            .expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::MissingRequiredTag { .. }));
    }

    #[test]
    fn decode_accepts_padded_photometric_interpretation() {
        // REQ-PIX-245
        let dataset = make_dataset(1, 1, 1, "MONOCHROME2 ", 8, 8, 7, 0, None, vec![128u8]);
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let frame = pipeline
            .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
            .expect("frame");
        assert_eq!(frame.format, PixelFormat::Luma8);
    }

    #[test]
    fn decode_accepts_padded_rescale_values() {
        // REQ-PIX-245
        let mut dataset = make_dataset(1, 1, 1, "MONOCHROME2", 16, 16, 15, 0, None, vec![1, 0]);
        dataset.insert(
            Element::new(
                tags::TAG_RESCALE_INTERCEPT,
                dicom_core::Vr::Ds,
                Value::Str("0 ".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                tags::TAG_RESCALE_SLOPE,
                dicom_core::Vr::Ds,
                Value::Str("1 ".to_string()),
            )
            .unwrap(),
        );
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let frame = pipeline
            .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
            .expect("frame");
        assert_eq!(frame.format, PixelFormat::Luma8);
    }

    #[test]
    fn decode_enforces_limits() {
        // REQ-PIX-204
        let dataset = make_dataset(
            0x7FFF,
            0x7FFF,
            1,
            "MONOCHROME2",
            8,
            8,
            7,
            0,
            None,
            vec![128u8],
        );
        let mut config = PixelPipelineConfig::default();
        config.limits.set_max_pixels_per_frame(1_000);
        let pipeline = PixelPipeline::new(config);
        let err = pipeline
            .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
            .expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::LimitExceeded { .. }));
    }

    #[test]
    fn monochrome1_inversion_applied_after_voi() {
        // REQ-PIX-242
        let mut dataset = make_dataset(1, 1, 1, "MONOCHROME1", 8, 8, 7, 0, None, vec![128u8]);
        dataset.insert(
            Element::new(
                tags::TAG_WINDOW_CENTER,
                dicom_core::Vr::Ds,
                Value::Str("128".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                tags::TAG_WINDOW_WIDTH,
                dicom_core::Vr::Ds,
                Value::Str("256".to_string()),
            )
            .unwrap(),
        );
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let frame = pipeline
            .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
            .expect("frame");
        assert_eq!(frame.format, PixelFormat::Luma8);
        assert_eq!(frame.bytes[0], 127u8);
    }

    #[test]
    fn voi_linear_exact_boundary() {
        // REQ-PIX-265
        let mut dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, vec![0u8]);
        dataset.insert(
            Element::new(
                tags::TAG_WINDOW_CENTER,
                dicom_core::Vr::Ds,
                Value::Str("0".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                tags::TAG_WINDOW_WIDTH,
                dicom_core::Vr::Ds,
                Value::Str("2".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                tags::TAG_VOI_LUT_FUNCTION,
                dicom_core::Vr::Cs,
                Value::Str("LINEAR_EXACT".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                tags::TAG_PIXEL_DATA,
                dicom_core::Vr::Ob,
                Value::Bytes(vec![0u8]),
            )
            .unwrap(),
        );
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let frame = pipeline
            .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
            .expect("frame");
        assert_eq!(frame.bytes[0], 128u8);
    }

    #[test]
    fn auto_window_deterministic() {
        // REQ-PIX-267
        let dataset = make_dataset(
            2,
            2,
            1,
            "MONOCHROME2",
            8,
            8,
            7,
            0,
            None,
            vec![0, 64, 128, 255],
        );
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let frame_a = pipeline
            .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
            .expect("frame a");
        let frame_b = pipeline
            .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
            .expect("frame b");
        assert_eq!(frame_a.bytes, frame_b.bytes);
    }

    #[test]
    fn env_contract_changes_do_not_affect_cpu_voi_modality_oracle() {
        // REQ-OPS-006: CPU VOI/modality transforms remain authoritative and env parsing independent.
        let mut dataset = make_dataset(
            1,
            1,
            1,
            "MONOCHROME2",
            16,
            16,
            15,
            0,
            None,
            vec![0x80, 0x00],
        );
        dataset.insert(
            Element::new(
                tags::TAG_RESCALE_INTERCEPT,
                dicom_core::Vr::Ds,
                Value::Str("-1024".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                tags::TAG_RESCALE_SLOPE,
                dicom_core::Vr::Ds,
                Value::Str("1".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                tags::TAG_WINDOW_CENTER,
                dicom_core::Vr::Ds,
                Value::Str("40".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                tags::TAG_WINDOW_WIDTH,
                dicom_core::Vr::Ds,
                Value::Str("400".to_string()),
            )
            .unwrap(),
        );
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let baseline = pipeline
            .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
            .expect("baseline decode");

        with_env_vars(
            &[
                ("DICOM_WEB_WORKERS", Some("64")),
                ("DICOM_WORKFLOW_QUERY_RATE_LIMIT", Some("999")),
                ("DICOM_DIMSE_MAX_INPUT_BYTES", Some("1048576")),
            ],
            || {
                let with_env = pipeline
                    .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
                    .expect("decode with unrelated env vars set");
                assert_eq!(baseline.bytes, with_env.bytes);
                assert_eq!(baseline.format, with_env.format);
                assert_eq!(baseline.width, with_env.width);
                assert_eq!(baseline.height, with_env.height);
            },
        );
    }

    #[test]
    fn auto_window_rejects_nan_percentile_config() {
        // REQ-PIX-220
        let dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, vec![128u8]);
        let config = PixelPipelineConfig {
            auto_window_low_percentile: f64::NAN,
            ..PixelPipelineConfig::default()
        };
        let pipeline = PixelPipeline::new(config);
        let err = pipeline
            .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
            .expect_err("expected error");
        assert!(matches!(
            err.kind(),
            ErrorKind::InvalidPixelTransform { .. }
        ));
    }

    #[test]
    fn auto_window_rejects_out_of_range_percentiles() {
        // REQ-PIX-220
        let dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, vec![128u8]);
        let config = PixelPipelineConfig {
            auto_window_low_percentile: -0.1,
            auto_window_high_percentile: 1.1,
            ..PixelPipelineConfig::default()
        };
        let pipeline = PixelPipeline::new(config);
        let err = pipeline
            .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
            .expect_err("expected error");
        assert!(matches!(
            err.kind(),
            ErrorKind::InvalidPixelTransform { .. }
        ));
    }

    #[test]
    fn auto_window_rejects_inverted_percentiles() {
        // REQ-PIX-220
        let dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, vec![128u8]);
        let config = PixelPipelineConfig {
            auto_window_low_percentile: 0.9,
            auto_window_high_percentile: 0.1,
            ..PixelPipelineConfig::default()
        };
        let pipeline = PixelPipeline::new(config);
        let err = pipeline
            .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
            .expect_err("expected error");
        assert!(matches!(
            err.kind(),
            ErrorKind::InvalidPixelTransform { .. }
        ));
    }

    #[test]
    fn explicit_window_rejects_non_finite_center() {
        // REQ-PIX-220
        let dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, vec![128u8]);
        let config = PixelPipelineConfig {
            window_level: WindowLevel::Explicit {
                center: f64::INFINITY,
                width: 2.0,
            },
            ..PixelPipelineConfig::default()
        };
        let pipeline = PixelPipeline::new(config);
        let err = pipeline
            .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
            .expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn non_finite_rescale_rejected() {
        // REQ-PIX-220
        let mut dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, vec![128u8]);
        dataset.insert(
            Element::new(
                tags::TAG_RESCALE_SLOPE,
                dicom_core::Vr::Ds,
                Value::Str("1e309".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                tags::TAG_RESCALE_INTERCEPT,
                dicom_core::Vr::Ds,
                Value::Str("0".to_string()),
            )
            .unwrap(),
        );
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let err = pipeline
            .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
            .expect_err("expected error");
        assert!(matches!(
            err.kind(),
            ErrorKind::InvalidTagValue { .. } | ErrorKind::InvalidPixelTransform { .. }
        ));
    }

    #[test]
    fn modality_rescale_overflow_rejected() {
        // REQ-PIX-220, REQ-PIX-253
        let mut dataset = make_dataset(
            1,
            1,
            1,
            "MONOCHROME2",
            16,
            16,
            15,
            1,
            None,
            i16::MAX.to_le_bytes().to_vec(),
        );
        dataset.insert(
            Element::new(
                tags::TAG_RESCALE_SLOPE,
                dicom_core::Vr::Ds,
                Value::Str("1e308".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                tags::TAG_RESCALE_INTERCEPT,
                dicom_core::Vr::Ds,
                Value::Str("1e308".to_string()),
            )
            .unwrap(),
        );
        let config = PixelPipelineConfig {
            window_level: WindowLevel::Explicit {
                center: 0.0,
                width: 2.0,
            },
            ..PixelPipelineConfig::default()
        };
        let pipeline = PixelPipeline::new(config);
        let err = pipeline
            .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
            .expect_err("expected error");
        assert!(matches!(
            err.kind(),
            ErrorKind::InvalidPixelTransform { .. }
        ));
    }

    #[test]
    fn rgb_planar_reorder() {
        // REQ-PIX-243
        let dataset = make_dataset(1, 1, 3, "RGB", 8, 8, 7, 0, Some(1), vec![1, 2, 3]);
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let frame = pipeline
            .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
            .expect("frame");
        assert_eq!(frame.format, PixelFormat::Rgba8);
        assert_eq!(frame.bytes, vec![1, 2, 3, 255]);
    }

    #[test]
    fn ybr_full_uncompressed_decodes_to_rgba() {
        // REQ-PIX-240, REQ-PIX-243
        let dataset = make_dataset(1, 1, 3, "YBR_FULL", 8, 8, 7, 0, Some(0), vec![76, 85, 255]);
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let frame = pipeline
            .decode_frame(&dataset, TS_EXPLICIT_VR_LE, 0)
            .expect("frame");
        assert_eq!(frame.format, PixelFormat::Rgba8);
        assert_eq!(frame.bytes, vec![254, 0, 0, 255]);
    }

    #[test]
    fn ybr_full_deflated_decodes_to_rgba() {
        // REQ-TS-204, REQ-PIX-240, REQ-PIX-243
        let dataset = make_dataset(1, 1, 3, "YBR_FULL", 8, 8, 7, 0, Some(0), vec![76, 85, 255]);
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let frame = pipeline
            .decode_frame(&dataset, TS_DEFLATED_EXPLICIT_VR_LE, 0)
            .expect("frame");
        assert_eq!(frame.format, PixelFormat::Rgba8);
        assert_eq!(frame.bytes, vec![254, 0, 0, 255]);
    }

    #[test]
    fn ybr_full_rle_decodes_to_rgba() {
        // REQ-PIX-240, REQ-PIX-243
        let dataset = make_dataset(
            1,
            1,
            3,
            "YBR_FULL",
            8,
            8,
            7,
            0,
            Some(0),
            sample_rle_triplet(76, 85, 255),
        );
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let frame = pipeline
            .decode_frame(&dataset, TS_RLE_LOSSLESS, 0)
            .expect("frame");
        assert_eq!(frame.format, PixelFormat::Rgba8);
        assert_eq!(frame.bytes, vec![254, 0, 0, 255]);
    }

    #[test]
    fn ybr_full_422_uncompressed_decodes_to_rgba() {
        // REQ-PIX-240, REQ-PIX-243
        let dataset = make_dataset(
            1,
            2,
            3,
            "YBR_FULL_422",
            8,
            8,
            7,
            0,
            Some(0),
            vec![76, 76, 85, 255],
        );
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let frame = pipeline
            .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
            .expect("frame");
        assert_eq!(frame.format, PixelFormat::Rgba8);
        assert_eq!(frame.bytes, vec![254, 0, 0, 255, 254, 0, 0, 255]);
    }

    #[test]
    fn ybr_full_422_deflated_decodes_to_rgba() {
        // REQ-TS-204, REQ-PIX-240, REQ-PIX-243
        let dataset = make_dataset(
            1,
            2,
            3,
            "YBR_FULL_422",
            8,
            8,
            7,
            0,
            Some(0),
            vec![76, 76, 85, 255],
        );
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let frame = pipeline
            .decode_frame(&dataset, TS_DEFLATED_EXPLICIT_VR_LE, 0)
            .expect("frame");
        assert_eq!(frame.format, PixelFormat::Rgba8);
        assert_eq!(frame.bytes, vec![254, 0, 0, 255, 254, 0, 0, 255]);
    }

    #[test]
    fn ybr_full_422_rejects_odd_columns() {
        // REQ-PIX-240
        let dataset = make_dataset(1, 1, 3, "YBR_FULL_422", 8, 8, 7, 0, Some(0), vec![76, 76]);
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let err = pipeline
            .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
            .expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::InvalidTagValue { .. }));
    }

    #[test]
    fn ybr_full_422_rejects_planar_configuration_1() {
        // REQ-PIX-240
        let dataset = make_dataset(
            1,
            2,
            3,
            "YBR_FULL_422",
            8,
            8,
            7,
            0,
            Some(1),
            vec![76, 76, 85, 255],
        );
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let err = pipeline
            .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
            .expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::InvalidTagValue { .. }));
    }

    #[test]
    fn ybr_rejects_non_native_transfer_syntax() {
        // REQ-PIX-240
        let dataset = make_dataset(1, 1, 3, "YBR_FULL", 8, 8, 7, 0, Some(0), sample_jpeg_rgb());
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let err = pipeline
            .decode_frame(&dataset, TS_JPEG_BASELINE, 0)
            .expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn jpeg_baseline_multiframe_requires_offset_table_support() {
        // REQ-CONF-084, REQ-CONF-086: encapsulated multi-frame decode must fail closed
        // when offset-table indexing support is not enabled.
        let dataset = make_dataset(1, 1, 3, "RGB", 8, 8, 7, 0, Some(0), sample_jpeg_rgb());
        let mut dataset = dataset;
        dataset.insert(
            Element::new(
                tags::TAG_NUMBER_OF_FRAMES,
                dicom_core::Vr::Is,
                Value::Str("2".to_string()),
            )
            .unwrap(),
        );
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let err = pipeline
            .decode_frame(&dataset, TS_JPEG_BASELINE, 0)
            .expect_err("expected multi-frame rejection");
        match &err.kind() {
            ErrorKind::DecodeError { detail, .. } => {
                assert!(detail.contains("offset table support"));
            }
            _ => panic!("expected decode error"),
        }
    }

    #[test]
    fn jpeg_component_mismatch_rejected() {
        // REQ-PIX-238
        let dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, sample_jpeg_rgb());
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let err = pipeline
            .decode_frame(&dataset, TS_JPEG_BASELINE, 0)
            .expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    #[cfg(feature = "codec-jpegls")]
    fn jpegls_decodes_lossless_sample() {
        // REQ-TS-203, REQ-CODEC-352
        let mut charls = charls::CharLS::default();
        let frame = charls::FrameInfo {
            width: 1,
            height: 1,
            bits_per_sample: 8,
            component_count: 1,
        };
        let encoded = charls.encode(frame, 0, &[128u8]).expect("encode");

        let mut dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, encoded);
        dataset.insert(
            Element::new(
                tags::TAG_WINDOW_CENTER,
                dicom_core::Vr::Ds,
                Value::Str("128".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                tags::TAG_WINDOW_WIDTH,
                dicom_core::Vr::Ds,
                Value::Str("256".to_string()),
            )
            .unwrap(),
        );

        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let frame = pipeline
            .decode_frame(&dataset, TS_JPEGLS_LOSSLESS, 0)
            .expect("frame");
        assert_eq!(frame.format, PixelFormat::Luma8);
        assert_eq!(frame.bytes, vec![128u8]);
    }

    #[test]
    #[cfg(feature = "codec-jpegls")]
    fn jpegls_rejects_dimension_mismatch() {
        // REQ-CODEC-350
        let mut charls = charls::CharLS::default();
        let frame = charls::FrameInfo {
            width: 1,
            height: 1,
            bits_per_sample: 8,
            component_count: 1,
        };
        let encoded = charls.encode(frame, 0, &[42u8]).expect("encode");

        let dataset = make_dataset(2, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, encoded);
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let err = pipeline
            .decode_frame(&dataset, TS_JPEGLS_LOSSLESS, 0)
            .expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    #[cfg(feature = "codec-jpegls")]
    fn jpegls_multiframe_requires_offset_table_support() {
        // REQ-CONF-084, REQ-CONF-086
        let mut charls = charls::CharLS::default();
        let frame = charls::FrameInfo {
            width: 1,
            height: 1,
            bits_per_sample: 8,
            component_count: 1,
        };
        let encoded = charls.encode(frame, 0, &[42u8]).expect("encode");
        let mut dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, encoded);
        dataset.insert(
            Element::new(
                tags::TAG_NUMBER_OF_FRAMES,
                dicom_core::Vr::Is,
                Value::Str("2".to_string()),
            )
            .unwrap(),
        );
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let err = pipeline
            .decode_frame(&dataset, TS_JPEGLS_LOSSLESS, 0)
            .expect_err("expected multi-frame rejection");
        match &err.kind() {
            ErrorKind::DecodeError { detail, .. } => {
                assert!(detail.contains("offset table support"));
            }
            _ => panic!("expected decode error"),
        }
    }

    #[test]
    #[cfg(feature = "codec-jpegls")]
    fn jpegls_enforces_max_decompressed_bytes() {
        // REQ-CODEC-351
        let mut charls = charls::CharLS::default();
        let frame = charls::FrameInfo {
            width: 2,
            height: 1,
            bits_per_sample: 8,
            component_count: 1,
        };
        let encoded = charls.encode(frame, 0, &[10u8, 20u8]).expect("encode");

        let dataset = make_dataset(1, 2, 1, "MONOCHROME2", 8, 8, 7, 0, None, encoded);
        let mut config = PixelPipelineConfig::default();
        config.limits.set_max_decompressed_bytes(1);
        let pipeline = PixelPipeline::new(config);
        let err = pipeline
            .decode_frame(&dataset, TS_JPEGLS_LOSSLESS, 0)
            .expect_err("expected error");
        assert!(matches!(
            err.kind(),
            ErrorKind::LimitExceeded {
                limit_name: "max_decompressed_bytes",
                ..
            }
        ));
    }

    #[test]
    #[cfg(feature = "codec-jpegls")]
    fn jpegls_codec_roundtrip() {
        // REQ-TS-203
        let mut charls = charls::CharLS::default();
        let frame = charls::FrameInfo {
            width: 1,
            height: 1,
            bits_per_sample: 8,
            component_count: 1,
        };
        let encoded = charls.encode(frame, 0, &[200u8]).expect("encode");
        let decoded = charls.decode(&encoded).expect("decode");
        assert_eq!(decoded, vec![200u8]);
    }

    #[test]
    #[cfg(feature = "codec-j2k")]
    fn jpeg2000_decodes_sample() {
        // REQ-TS-203, REQ-CODEC-352
        let bytes = include_bytes!("../../../corpus/j2k_file1.jp2").to_vec();
        let bitmap = hayro_jpeg2000::decode(&bytes, &hayro_jpeg2000::DecodeSettings::default())
            .expect("decode bitmap");
        if bitmap.has_alpha {
            return;
        }
        let (samples_per_pixel, photometric) = match bitmap.color_space {
            hayro_jpeg2000::ColorSpace::Gray => (1u16, "MONOCHROME2"),
            hayro_jpeg2000::ColorSpace::RGB => (3u16, "RGB"),
            _ => return,
        };

        let rows = u16::try_from(bitmap.height).expect("rows");
        let cols = u16::try_from(bitmap.width).expect("cols");
        let dataset = make_dataset(
            rows,
            cols,
            samples_per_pixel,
            photometric,
            8,
            8,
            7,
            0,
            if samples_per_pixel > 1 { Some(0) } else { None },
            bytes,
        );
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let frame = pipeline
            .decode_frame(&dataset, TS_JPEG2000_LOSSLESS, 0)
            .expect("frame");
        let expected_format = if samples_per_pixel == 1 {
            PixelFormat::Luma8
        } else {
            PixelFormat::Rgba8
        };
        assert_eq!(frame.format, expected_format);
    }

    #[test]
    #[cfg(feature = "codec-j2k")]
    fn jpeg2000_multiframe_requires_offset_table_support() {
        // REQ-CONF-084, REQ-CONF-086
        let bytes = include_bytes!("../../../corpus/j2k_file1.jp2").to_vec();
        let bitmap = hayro_jpeg2000::decode(&bytes, &hayro_jpeg2000::DecodeSettings::default())
            .expect("decode bitmap");
        if bitmap.has_alpha {
            return;
        }
        let (samples_per_pixel, photometric) = match bitmap.color_space {
            hayro_jpeg2000::ColorSpace::Gray => (1u16, "MONOCHROME2"),
            hayro_jpeg2000::ColorSpace::RGB => (3u16, "RGB"),
            _ => return,
        };
        let rows = u16::try_from(bitmap.height).expect("rows");
        let cols = u16::try_from(bitmap.width).expect("cols");
        let mut dataset = make_dataset(
            rows,
            cols,
            samples_per_pixel,
            photometric,
            8,
            8,
            7,
            0,
            if samples_per_pixel > 1 { Some(0) } else { None },
            bytes,
        );
        dataset.insert(
            Element::new(
                tags::TAG_NUMBER_OF_FRAMES,
                dicom_core::Vr::Is,
                Value::Str("2".to_string()),
            )
            .unwrap(),
        );
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let err = pipeline
            .decode_frame(&dataset, TS_JPEG2000_LOSSLESS, 0)
            .expect_err("expected multi-frame rejection");
        match &err.kind() {
            ErrorKind::DecodeError { detail, .. } => {
                assert!(detail.contains("offset table support"));
            }
            _ => panic!("expected decode error"),
        }
    }

    #[test]
    fn decode_rejects_short_uncompressed_pixel_data() {
        // REQ-PIX-231
        let dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, Vec::new());
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let err = pipeline
            .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
            .expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn voi_function_rejects_sigmoid() {
        // REQ-PIX-262
        let mut dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, vec![128u8]);
        dataset.insert(
            Element::new(
                tags::TAG_WINDOW_CENTER,
                dicom_core::Vr::Ds,
                Value::Str("1".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                tags::TAG_WINDOW_WIDTH,
                dicom_core::Vr::Ds,
                Value::Str("2".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                tags::TAG_VOI_LUT_FUNCTION,
                dicom_core::Vr::Cs,
                Value::Str("SIGMOID".to_string()),
            )
            .unwrap(),
        );
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let err = pipeline
            .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
            .expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::InvalidTagValue { .. }));
    }

    #[test]
    fn overlay_bounds_rejected() {
        // REQ-PIX-280
        let mut dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, vec![128u8]);
        dataset.insert(
            Element::new(
                tags::TAG_OVERLAY_ROWS,
                dicom_core::Vr::Us,
                Value::Bytes(vec![0x02, 0x00]),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                tags::TAG_OVERLAY_COLUMNS,
                dicom_core::Vr::Us,
                Value::Bytes(vec![0x02, 0x00]),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                tags::TAG_OVERLAY_BITS_ALLOCATED,
                dicom_core::Vr::Us,
                Value::Bytes(vec![0x01, 0x00]),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                tags::TAG_OVERLAY_BIT_POSITION,
                dicom_core::Vr::Us,
                Value::Bytes(vec![0x00, 0x00]),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                tags::TAG_OVERLAY_DATA,
                dicom_core::Vr::Ob,
                Value::Bytes(vec![0xFF]),
            )
            .unwrap(),
        );
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let err = pipeline
            .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
            .expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn rle_header_too_short_rejected() {
        // REQ-PIX-235
        let dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, vec![0u8; 10]);
        let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
        let err = pipeline
            .decode_frame(&dataset, TS_RLE_LOSSLESS, 0)
            .expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    fn sample_jpeg_rgb() -> Vec<u8> {
        vec![
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x01,
            0x00, 0x60, 0x00, 0x60, 0x00, 0x00, 0xFF, 0xDB, 0x00, 0x43, 0x00, 0x08, 0x06, 0x06,
            0x07, 0x06, 0x05, 0x08, 0x07, 0x07, 0x07, 0x09, 0x09, 0x08, 0x0A, 0x0C, 0x14, 0x0D,
            0x0C, 0x0B, 0x0B, 0x0C, 0x19, 0x12, 0x13, 0x0F, 0x14, 0x1D, 0x1A, 0x1F, 0x1E, 0x1D,
            0x1A, 0x1C, 0x1C, 0x20, 0x24, 0x2E, 0x27, 0x20, 0x22, 0x2C, 0x23, 0x1C, 0x1C, 0x28,
            0x37, 0x29, 0x2C, 0x30, 0x31, 0x34, 0x34, 0x34, 0x1F, 0x27, 0x39, 0x3D, 0x38, 0x32,
            0x3C, 0x2E, 0x33, 0x34, 0x32, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x01, 0x00, 0x01,
            0x03, 0x01, 0x11, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11, 0x01, 0xFF, 0xC4, 0x00, 0x14,
            0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0xFF, 0xC4, 0x00, 0x14, 0x10, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xDA, 0x00, 0x0C, 0x03, 0x01,
            0x00, 0x02, 0x11, 0x03, 0x11, 0x00, 0x3F, 0x00, 0xD2, 0xCF, 0x20, 0xFF, 0xD9,
        ]
    }

    fn sample_rle_triplet(y: u8, cb: u8, cr: u8) -> Vec<u8> {
        let mut data = vec![0u8; 64];
        data[0..4].copy_from_slice(&(3u32).to_le_bytes());
        data[4..8].copy_from_slice(&(64u32).to_le_bytes());
        data[8..12].copy_from_slice(&(66u32).to_le_bytes());
        data[12..16].copy_from_slice(&(68u32).to_le_bytes());
        data.extend_from_slice(&[0x00, y]);
        data.extend_from_slice(&[0x00, cb]);
        data.extend_from_slice(&[0x00, cr]);
        data
    }
}
