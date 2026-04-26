#![cfg(feature = "raster-io")]

use dicom_core::{ErrorKind, Limits};
use dicom_pixel::{decode_raster_bytes, PixelFormat, RasterFormat};
use image::{DynamicImage, ImageBuffer, ImageFormat, Luma, Rgb};
use std::io::Cursor;

#[test]
fn raster_png_decodes() {
    // REQ-SEC-409
    let img = ImageBuffer::<Luma<u8>, _>::from_raw(1, 1, vec![0x7F]).expect("png buffer");
    let image = DynamicImage::ImageLuma8(img);
    let mut cursor = Cursor::new(Vec::new());
    image
        .write_to(&mut cursor, ImageFormat::Png)
        .expect("png encode");
    let bytes = cursor.into_inner();

    let limits = Limits::default();
    let frame = decode_raster_bytes(RasterFormat::Png, &bytes, &limits).expect("decode");
    assert_eq!(frame.format, PixelFormat::Luma8);
    assert_eq!(frame.bytes, vec![0x7F]);
}

#[test]
fn raster_tiff_decodes_luma16() {
    // REQ-SEC-409
    let img = ImageBuffer::<Luma<u16>, _>::from_raw(1, 1, vec![0x1234]).expect("tiff buffer");
    let image = DynamicImage::ImageLuma16(img);
    let mut cursor = Cursor::new(Vec::new());
    image
        .write_to(&mut cursor, ImageFormat::Tiff)
        .expect("tiff encode");
    let bytes = cursor.into_inner();

    let limits = Limits::default();
    let frame = decode_raster_bytes(RasterFormat::Tiff, &bytes, &limits).expect("decode");
    assert_eq!(frame.format, PixelFormat::Luma16);
    assert_eq!(frame.bytes, vec![0x34, 0x12]);
}

#[test]
fn raster_bmp_decodes() {
    // REQ-SEC-409
    let img =
        ImageBuffer::<Rgb<u8>, _>::from_raw(1, 1, vec![0xAB, 0xCD, 0xEF]).expect("bmp buffer");
    let image = DynamicImage::ImageRgb8(img);
    let mut cursor = Cursor::new(Vec::new());
    image
        .write_to(&mut cursor, ImageFormat::Bmp)
        .expect("bmp encode");
    let bytes = cursor.into_inner();

    let limits = Limits::default();
    let frame = decode_raster_bytes(RasterFormat::Bmp, &bytes, &limits).expect("decode");
    assert_eq!(frame.format, PixelFormat::Rgba8);
    assert_eq!(frame.bytes.len(), 4);
}

#[test]
fn raster_jpeg_decodes() {
    // REQ-SEC-409
    let img =
        ImageBuffer::<Rgb<u8>, _>::from_raw(1, 1, vec![0x12, 0x34, 0x56]).expect("jpeg buffer");
    let image = DynamicImage::ImageRgb8(img);
    let mut cursor = Cursor::new(Vec::new());
    image
        .write_to(&mut cursor, ImageFormat::Jpeg)
        .expect("jpeg encode");
    let bytes = cursor.into_inner();

    let limits = Limits::default();
    let frame = decode_raster_bytes(RasterFormat::Jpeg, &bytes, &limits).expect("decode");
    assert_eq!(frame.format, PixelFormat::Rgba8);
    assert_eq!(frame.bytes.len(), 4);
}

#[test]
fn raster_enforces_input_limit() {
    // REQ-SEC-409
    let limits = Limits {
        max_input_bytes: 1,
        ..Limits::default()
    };
    let err = decode_raster_bytes(RasterFormat::Png, &[0u8, 1u8], &limits)
        .expect_err("expected limit error");
    assert!(matches!(err.kind(), ErrorKind::LimitExceeded { .. }));
}
