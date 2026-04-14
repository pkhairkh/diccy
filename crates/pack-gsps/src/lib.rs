#![deny(missing_docs)]

//! GSPS pack: deterministic shutter and presentation state application.

use dicom_core::{
    parse_f64_strict, parse_i32_strict, Dataset, Error, ErrorKind, Result, Tag, Value,
};
use dicom_pixel::{DisplayFrame, DisplayTransform, PixelFormat};

/// GSPS SOP Class UID.
pub const SOP_CLASS_GSPS: &str = "1.2.840.10008.5.1.4.1.1.11.1";

/// GSPS SOP Class manifest list.
pub const GSPS_SOP_CLASS_UIDS: &[&str] = &[SOP_CLASS_GSPS];

/// Marker type for GSPS pack features.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GspsPack;

impl GspsPack {
    /// Return true when GSPS pack functionality is enabled for this crate.
    pub const fn enabled() -> bool {
        cfg!(feature = "gsps")
    }

    /// Require the GSPS pack to be enabled for GSPS SOP classes.
    pub fn ensure_supported(sop_class_uid: &str) -> Result<()> {
        if !Self::enabled() {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                "gsps requires gsps feature",
            )));
        }
        if !GSPS_SOP_CLASS_UIDS.contains(&sop_class_uid) {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                "unsupported GSPS SOP class",
            )));
        }
        Ok(())
    }
}

/// Shutter description parsed from GSPS.
#[derive(Debug, Clone, PartialEq)]
pub enum Shutter {
    /// Rectangular shutter.
    Rect {
        /// Left column (1-based).
        left: i32,
        /// Right column (1-based).
        right: i32,
        /// Upper row (1-based).
        upper: i32,
        /// Lower row (1-based).
        lower: i32,
        /// Presentation value (0..=255).
        value: u8,
    },
    /// Circular shutter.
    Circular {
        /// Center column (1-based).
        center_x: i32,
        /// Center row (1-based).
        center_y: i32,
        /// Radius in pixels.
        radius: i32,
        /// Presentation value (0..=255).
        value: u8,
    },
    /// Polygonal shutter.
    Polygon {
        /// Polygon vertices (x, y) in 1-based display coordinates.
        points: Vec<(i32, i32)>,
        /// Presentation value (0..=255).
        value: u8,
    },
}

/// Supported GSPS graphic objects.
#[derive(Debug, Clone, PartialEq)]
pub enum GraphicObject {
    /// Point graphic defined by a single (x,y) pair.
    Point {
        /// Point in image coordinates.
        point: (f64, f64),
    },
    /// Polyline graphic defined by (x,y) pairs.
    Polyline {
        /// Polyline points in image coordinates.
        points: Vec<(f64, f64)>,
    },
    /// Interpolated graphic defined by control points.
    ///
    /// Current deterministic scope renders interpolated graphics as piecewise
    /// linear segments through control points.
    Interpolated {
        /// Interpolated control points in image coordinates.
        points: Vec<(f64, f64)>,
    },
    /// Circle graphic defined by center and perimeter points.
    Circle {
        /// Circle center in image coordinates.
        center: (f64, f64),
        /// Circle perimeter point in image coordinates.
        edge: (f64, f64),
    },
    /// Ellipse graphic defined by major/minor axis endpoints.
    ///
    /// The first two points are major-axis endpoints and the second two points
    /// are minor-axis endpoints.
    Ellipse {
        /// Major-axis start point.
        major_start: (f64, f64),
        /// Major-axis end point.
        major_end: (f64, f64),
        /// Minor-axis start point.
        minor_start: (f64, f64),
        /// Minor-axis end point.
        minor_end: (f64, f64),
    },
}

/// Parsed GSPS presentation state.
#[derive(Debug, Clone, PartialEq)]
pub struct PresentationState {
    /// Optional shutter.
    pub shutter: Option<Shutter>,
    /// Graphic annotation objects.
    pub graphics: Vec<GraphicObject>,
}

impl PresentationState {
    /// Parse a presentation state from a GSPS dataset.
    pub fn from_dataset(dataset: &Dataset) -> Result<Self> {
        let shutter = match read_str(dataset, TAG_SHUTTER_SHAPE)? {
            None => None,
            Some(shape) => {
                let value = read_shutter_value(dataset)?;
                match shape {
                    "RECTANGULAR" => {
                        let left = read_i32(dataset, TAG_SHUTTER_LEFT_VERT_EDGE)?;
                        let right = read_i32(dataset, TAG_SHUTTER_RIGHT_VERT_EDGE)?;
                        let upper = read_i32(dataset, TAG_SHUTTER_UPPER_HORIZ_EDGE)?;
                        let lower = read_i32(dataset, TAG_SHUTTER_LOWER_HORIZ_EDGE)?;
                        Some(Shutter::Rect {
                            left,
                            right,
                            upper,
                            lower,
                            value,
                        })
                    }
                    "CIRCULAR" => {
                        let center = read_i32_list(dataset, TAG_SHUTTER_CENTER, 2)?;
                        let radius = read_i32(dataset, TAG_SHUTTER_RADIUS)?;
                        Some(Shutter::Circular {
                            center_x: center[0],
                            center_y: center[1],
                            radius,
                            value,
                        })
                    }
                    "POLYGONAL" => {
                        let vertices = read_i32_vec(dataset, TAG_SHUTTER_VERTICES)?;
                        if vertices.len() < 6 || vertices.len() % 2 != 0 {
                            return Err(invalid_tag_value(
                                TAG_SHUTTER_VERTICES,
                                "polygonal shutter requires at least three x/y vertex pairs",
                            ));
                        }
                        let mut points = Vec::with_capacity(vertices.len() / 2);
                        for pair in vertices.chunks_exact(2) {
                            let x = pair[0];
                            let y = pair[1];
                            if x < 1 || y < 1 {
                                return Err(invalid_tag_value(
                                    TAG_SHUTTER_VERTICES,
                                    "polygonal shutter vertices must be >= 1",
                                ));
                            }
                            points.push((x, y));
                        }
                        Some(Shutter::Polygon { points, value })
                    }
                    other => {
                        return Err(invalid_tag_value(
                            TAG_SHUTTER_SHAPE,
                            format!("unsupported shutter shape {other}"),
                        ))
                    }
                }
            }
        };
        let graphics = parse_graphics(dataset)?;
        Ok(Self { shutter, graphics })
    }

    /// Apply this presentation state to a display frame.
    pub fn apply_to(&self, frame: &mut DisplayFrame) -> Result<()> {
        if let Some(shutter) = &self.shutter {
            apply_shutter(frame, shutter)?;
        }
        if !self.graphics.is_empty() {
            apply_graphics(frame, &self.graphics)?;
        }
        Ok(())
    }
}

impl DisplayTransform for PresentationState {
    fn apply(&self, frame: &mut DisplayFrame) -> Result<()> {
        self.apply_to(frame)
    }
}

const TAG_SHUTTER_SHAPE: Tag = Tag(0x0018, 0x1600);
const TAG_SHUTTER_LEFT_VERT_EDGE: Tag = Tag(0x0018, 0x1602);
const TAG_SHUTTER_RIGHT_VERT_EDGE: Tag = Tag(0x0018, 0x1604);
const TAG_SHUTTER_UPPER_HORIZ_EDGE: Tag = Tag(0x0018, 0x1606);
const TAG_SHUTTER_LOWER_HORIZ_EDGE: Tag = Tag(0x0018, 0x1608);
const TAG_SHUTTER_CENTER: Tag = Tag(0x0018, 0x160A);
const TAG_SHUTTER_RADIUS: Tag = Tag(0x0018, 0x160C);
const TAG_SHUTTER_VERTICES: Tag = Tag(0x0018, 0x1620);
const TAG_SHUTTER_PRESENTATION_VALUE: Tag = Tag(0x0018, 0x1622);
const TAG_GRAPHIC_ANNOTATION_SEQUENCE: Tag = Tag(0x0070, 0x0001);
const TAG_GRAPHIC_OBJECT_SEQUENCE: Tag = Tag(0x0070, 0x0009);
const TAG_GRAPHIC_DATA: Tag = Tag(0x0070, 0x0022);
const TAG_GRAPHIC_TYPE: Tag = Tag(0x0070, 0x0023);

const GRAPHIC_LUMA: u8 = 0xFF;
const GRAPHIC_COLOR: [u8; 3] = [0xFF, 0x00, 0x00];
const GRAPHIC_EPS: f64 = 1e-6;
const GRAPHIC_POINT_EPS: f64 = 1e-3;
const GRAPHIC_ORTHO_EPS: f64 = 1e-3;
const ELLIPSE_SEGMENTS: usize = 180;

fn apply_shutter(frame: &mut DisplayFrame, shutter: &Shutter) -> Result<()> {
    match shutter {
        Shutter::Rect {
            left,
            right,
            upper,
            lower,
            value,
        } => apply_rect(frame, *left, *right, *upper, *lower, *value),
        Shutter::Circular {
            center_x,
            center_y,
            radius,
            value,
        } => apply_circle(frame, *center_x, *center_y, *radius, *value),
        Shutter::Polygon { points, value } => apply_polygon(frame, points, *value),
    }
}

fn parse_graphics(dataset: &Dataset) -> Result<Vec<GraphicObject>> {
    let Some(annotations) = read_sequence(dataset, TAG_GRAPHIC_ANNOTATION_SEQUENCE)? else {
        return Ok(Vec::new());
    };
    let mut objects = Vec::new();
    for annotation in annotations {
        let Some(graphics) = read_sequence(annotation, TAG_GRAPHIC_OBJECT_SEQUENCE)? else {
            continue;
        };
        for graphic in graphics {
            let graphic_type = read_str(graphic, TAG_GRAPHIC_TYPE)?
                .ok_or_else(|| missing_required_tag(TAG_GRAPHIC_TYPE))?;
            match graphic_type {
                "POINT" => {
                    let points = read_graphic_points(graphic)?;
                    if points.len() != 1 {
                        return Err(invalid_tag_value(
                            TAG_GRAPHIC_DATA,
                            "POINT requires exactly one x/y pair",
                        ));
                    }
                    objects.push(GraphicObject::Point { point: points[0] });
                }
                "POLYLINE" => {
                    let points = read_graphic_points(graphic)?;
                    if points.len() < 2 {
                        return Err(invalid_tag_value(
                            TAG_GRAPHIC_DATA,
                            "POLYLINE requires at least two x/y pairs",
                        ));
                    }
                    objects.push(GraphicObject::Polyline { points });
                }
                "INTERPOLATED" => {
                    let points = read_graphic_points(graphic)?;
                    if points.len() < 2 {
                        return Err(invalid_tag_value(
                            TAG_GRAPHIC_DATA,
                            "INTERPOLATED requires at least two x/y pairs",
                        ));
                    }
                    objects.push(GraphicObject::Interpolated { points });
                }
                "CIRCLE" => {
                    let points = read_graphic_points(graphic)?;
                    if points.len() != 2 {
                        return Err(invalid_tag_value(
                            TAG_GRAPHIC_DATA,
                            "CIRCLE requires exactly two x/y pairs",
                        ));
                    }
                    let center = points[0];
                    let edge = points[1];
                    let radius = ((edge.0 - center.0).powi(2) + (edge.1 - center.1).powi(2)).sqrt();
                    if !radius.is_finite() || radius <= GRAPHIC_EPS {
                        return Err(invalid_tag_value(
                            TAG_GRAPHIC_DATA,
                            "CIRCLE radius must be finite and > 0",
                        ));
                    }
                    objects.push(GraphicObject::Circle { center, edge });
                }
                "ELLIPSE" => {
                    let points = read_graphic_points(graphic)?;
                    if points.len() != 4 {
                        return Err(invalid_tag_value(
                            TAG_GRAPHIC_DATA,
                            "ELLIPSE requires exactly four x/y pairs",
                        ));
                    }
                    let major_start = points[0];
                    let major_end = points[1];
                    let minor_start = points[2];
                    let minor_end = points[3];
                    let major_center = midpoint(major_start, major_end);
                    let minor_center = midpoint(minor_start, minor_end);
                    if !approx_point(major_center, minor_center) {
                        return Err(invalid_tag_value(
                            TAG_GRAPHIC_DATA,
                            "ELLIPSE major/minor axes must share a center",
                        ));
                    }
                    let major = (
                        (major_end.0 - major_start.0) * 0.5,
                        (major_end.1 - major_start.1) * 0.5,
                    );
                    let minor = (
                        (minor_end.0 - minor_start.0) * 0.5,
                        (minor_end.1 - minor_start.1) * 0.5,
                    );
                    let major_norm = (major.0 * major.0 + major.1 * major.1).sqrt();
                    let minor_norm = (minor.0 * minor.0 + minor.1 * minor.1).sqrt();
                    if !major_norm.is_finite()
                        || !minor_norm.is_finite()
                        || major_norm <= GRAPHIC_EPS
                        || minor_norm <= GRAPHIC_EPS
                    {
                        return Err(invalid_tag_value(
                            TAG_GRAPHIC_DATA,
                            "ELLIPSE axes must be finite and non-zero",
                        ));
                    }
                    let dot = major.0 * minor.0 + major.1 * minor.1;
                    if dot.abs() > GRAPHIC_ORTHO_EPS * major_norm * minor_norm {
                        return Err(invalid_tag_value(
                            TAG_GRAPHIC_DATA,
                            "ELLIPSE axes must be orthogonal",
                        ));
                    }
                    objects.push(GraphicObject::Ellipse {
                        major_start,
                        major_end,
                        minor_start,
                        minor_end,
                    });
                }
                other => {
                    return Err(invalid_tag_value(
                        TAG_GRAPHIC_TYPE,
                        format!("unsupported graphic type {other}"),
                    ))
                }
            }
        }
    }
    Ok(objects)
}

fn read_graphic_points(dataset: &Dataset) -> Result<Vec<(f64, f64)>> {
    let values = read_f64_list(dataset, TAG_GRAPHIC_DATA)?;
    if values.len() < 2 || values.len() % 2 != 0 {
        return Err(invalid_tag_value(
            TAG_GRAPHIC_DATA,
            "graphic data must contain x/y pairs",
        ));
    }
    let mut points = Vec::with_capacity(values.len() / 2);
    for chunk in values.chunks_exact(2) {
        let x = chunk[0];
        let y = chunk[1];
        if !x.is_finite() || !y.is_finite() {
            return Err(invalid_tag_value(
                TAG_GRAPHIC_DATA,
                "graphic coordinates must be finite",
            ));
        }
        points.push((x, y));
    }
    Ok(points)
}

fn read_f64_list(dataset: &Dataset, tag: Tag) -> Result<Vec<f64>> {
    match dataset.get(tag) {
        Some(element) => match &element.value {
            Value::Str(value) => {
                let parts: Vec<&str> = value.split('\\').collect();
                let mut values = Vec::with_capacity(parts.len());
                for part in parts {
                    let parsed = parse_f64_strict(tag, part)?;
                    values.push(parsed);
                }
                Ok(values)
            }
            Value::Bytes(bytes) => {
                if bytes.len() % 4 != 0 {
                    return Err(invalid_tag_value(tag, "graphic data length invalid"));
                }
                let mut values = Vec::with_capacity(bytes.len() / 4);
                for chunk in bytes.chunks_exact(4) {
                    let raw = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                    let value = raw as f64;
                    values.push(value);
                }
                Ok(values)
            }
            Value::F64(value) => Ok(vec![*value]),
            _ => Err(invalid_tag_value(tag, "expected graphic data values")),
        },
        None => Err(missing_required_tag(tag)),
    }
}

fn apply_graphics(frame: &mut DisplayFrame, graphics: &[GraphicObject]) -> Result<()> {
    let width = frame.width as i32;
    let height = frame.height as i32;
    for graphic in graphics {
        match graphic {
            GraphicObject::Point { point } => {
                let x = graphic_to_pixel(point.0, width);
                let y = graphic_to_pixel(point.1, height);
                paint_graphic_pixel(frame, width, height, x, y);
            }
            GraphicObject::Polyline { points } => {
                let mut coords = Vec::with_capacity(points.len());
                for (x, y) in points {
                    let px = graphic_to_pixel(*x, width);
                    let py = graphic_to_pixel(*y, height);
                    coords.push((px, py));
                }
                draw_polyline(frame, width, height, &coords);
            }
            GraphicObject::Interpolated { points } => {
                let mut coords = Vec::with_capacity(points.len());
                for (x, y) in points {
                    let px = graphic_to_pixel(*x, width);
                    let py = graphic_to_pixel(*y, height);
                    coords.push((px, py));
                }
                draw_polyline(frame, width, height, &coords);
            }
            GraphicObject::Circle { center, edge } => {
                let cx = graphic_to_pixel_f64(center.0);
                let cy = graphic_to_pixel_f64(center.1);
                let ex = graphic_to_pixel_f64(edge.0);
                let ey = graphic_to_pixel_f64(edge.1);
                let radius = ((ex - cx).powi(2) + (ey - cy).powi(2)).sqrt();
                draw_circle_outline(frame, width, height, cx, cy, radius);
            }
            GraphicObject::Ellipse {
                major_start,
                major_end,
                minor_start,
                minor_end,
            } => {
                draw_ellipse_outline(
                    frame,
                    width,
                    height,
                    *major_start,
                    *major_end,
                    *minor_start,
                    *minor_end,
                );
            }
        }
    }
    Ok(())
}

fn graphic_to_pixel(value: f64, max: i32) -> i32 {
    let mut idx = value.round() as i32 - 1;
    if idx < 0 {
        idx = 0;
    }
    if idx >= max {
        idx = max - 1;
    }
    idx
}

fn graphic_to_pixel_f64(value: f64) -> f64 {
    value - 1.0
}

fn draw_polyline(frame: &mut DisplayFrame, width: i32, height: i32, points: &[(i32, i32)]) {
    if points.len() < 2 {
        return;
    }
    for pair in points.windows(2) {
        draw_line(frame, width, height, pair[0], pair[1]);
    }
}

fn draw_circle_outline(
    frame: &mut DisplayFrame,
    width: i32,
    height: i32,
    cx: f64,
    cy: f64,
    radius: f64,
) {
    if !cx.is_finite() || !cy.is_finite() || !radius.is_finite() || radius <= GRAPHIC_EPS {
        return;
    }
    let cx = cx.round() as i32;
    let cy = cy.round() as i32;
    let mut x = radius.round().max(1.0) as i32;
    let mut y = 0i32;
    let mut decision = 1 - x;

    while y <= x {
        paint_graphic_pixel(frame, width, height, cx + x, cy + y);
        paint_graphic_pixel(frame, width, height, cx + y, cy + x);
        paint_graphic_pixel(frame, width, height, cx - y, cy + x);
        paint_graphic_pixel(frame, width, height, cx - x, cy + y);
        paint_graphic_pixel(frame, width, height, cx - x, cy - y);
        paint_graphic_pixel(frame, width, height, cx - y, cy - x);
        paint_graphic_pixel(frame, width, height, cx + y, cy - x);
        paint_graphic_pixel(frame, width, height, cx + x, cy - y);
        y += 1;
        if decision <= 0 {
            decision += 2 * y + 1;
        } else {
            x -= 1;
            decision += 2 * (y - x) + 1;
        }
    }
}

fn draw_ellipse_outline(
    frame: &mut DisplayFrame,
    width: i32,
    height: i32,
    major_start: (f64, f64),
    major_end: (f64, f64),
    minor_start: (f64, f64),
    minor_end: (f64, f64),
) {
    let ms = (
        graphic_to_pixel_f64(major_start.0),
        graphic_to_pixel_f64(major_start.1),
    );
    let me = (
        graphic_to_pixel_f64(major_end.0),
        graphic_to_pixel_f64(major_end.1),
    );
    let ns = (
        graphic_to_pixel_f64(minor_start.0),
        graphic_to_pixel_f64(minor_start.1),
    );
    let ne = (
        graphic_to_pixel_f64(minor_end.0),
        graphic_to_pixel_f64(minor_end.1),
    );

    let center_major = midpoint(ms, me);
    let center_minor = midpoint(ns, ne);
    if !approx_point(center_major, center_minor) {
        return;
    }
    let center = center_major;
    let major = ((me.0 - ms.0) * 0.5, (me.1 - ms.1) * 0.5);
    let minor = ((ne.0 - ns.0) * 0.5, (ne.1 - ns.1) * 0.5);
    let major_norm = (major.0 * major.0 + major.1 * major.1).sqrt();
    let minor_norm = (minor.0 * minor.0 + minor.1 * minor.1).sqrt();
    if !major_norm.is_finite()
        || !minor_norm.is_finite()
        || major_norm <= GRAPHIC_EPS
        || minor_norm <= GRAPHIC_EPS
    {
        return;
    }
    let dot = major.0 * minor.0 + major.1 * minor.1;
    if dot.abs() > GRAPHIC_ORTHO_EPS * major_norm * minor_norm {
        return;
    }

    let mut prev: Option<(i32, i32)> = None;
    for step in 0..=ELLIPSE_SEGMENTS {
        let theta = (step as f64) * std::f64::consts::TAU / (ELLIPSE_SEGMENTS as f64);
        let x = center.0 + major.0 * theta.cos() + minor.0 * theta.sin();
        let y = center.1 + major.1 * theta.cos() + minor.1 * theta.sin();
        let point = (x.round() as i32, y.round() as i32);
        if let Some(last) = prev {
            draw_line(frame, width, height, last, point);
        }
        prev = Some(point);
    }
}

fn midpoint(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5)
}

fn approx_point(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() <= GRAPHIC_POINT_EPS && (a.1 - b.1).abs() <= GRAPHIC_POINT_EPS
}

fn draw_line(
    frame: &mut DisplayFrame,
    width: i32,
    height: i32,
    start: (i32, i32),
    end: (i32, i32),
) {
    let (mut x0, mut y0) = start;
    let (x1, y1) = end;
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        paint_graphic_pixel(frame, width, height, x0, y0);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn paint_graphic_pixel(frame: &mut DisplayFrame, width: i32, height: i32, x: i32, y: i32) {
    if x < 0 || y < 0 || x >= width || y >= height {
        return;
    }
    let idx = (y as u32 * frame.width + x as u32) as usize;
    match frame.format {
        PixelFormat::Luma8 => {
            if let Some(slot) = frame.bytes.get_mut(idx) {
                *slot = GRAPHIC_LUMA;
            }
        }
        PixelFormat::Luma16 => {
            let start = idx * 2;
            if start + 1 < frame.bytes.len() {
                frame.bytes[start] = GRAPHIC_LUMA;
                frame.bytes[start + 1] = 0;
            }
        }
        PixelFormat::Rgba8 => {
            let start = idx * 4;
            if start + 3 < frame.bytes.len() {
                frame.bytes[start] = GRAPHIC_COLOR[0];
                frame.bytes[start + 1] = GRAPHIC_COLOR[1];
                frame.bytes[start + 2] = GRAPHIC_COLOR[2];
                frame.bytes[start + 3] = 0xFF;
            }
        }
    }
}

fn apply_rect(
    frame: &mut DisplayFrame,
    left: i32,
    right: i32,
    upper: i32,
    lower: i32,
    value: u8,
) -> Result<()> {
    let width = frame.width as i32;
    let height = frame.height as i32;
    if left < 1 || right < 1 || upper < 1 || lower < 1 {
        return Err(invalid_tag_value(
            TAG_SHUTTER_SHAPE,
            "shutter edges must be >= 1",
        ));
    }
    if left > right || upper > lower {
        return Err(invalid_tag_value(
            TAG_SHUTTER_SHAPE,
            "invalid shutter bounds",
        ));
    }
    let left0 = left - 1;
    let right0 = right - 1;
    let upper0 = upper - 1;
    let lower0 = lower - 1;
    for y in 0..height {
        for x in 0..width {
            let inside = x >= left0 && x <= right0 && y >= upper0 && y <= lower0;
            if !inside {
                paint_pixel(frame, x, y, value);
            }
        }
    }
    Ok(())
}

fn apply_circle(
    frame: &mut DisplayFrame,
    center_x: i32,
    center_y: i32,
    radius: i32,
    value: u8,
) -> Result<()> {
    let width = frame.width as i32;
    let height = frame.height as i32;
    if radius <= 0 {
        return Err(invalid_tag_value(TAG_SHUTTER_RADIUS, "radius must be > 0"));
    }
    let r2 = (radius as i64) * (radius as i64);
    for y in 0..height {
        for x in 0..width {
            let dx = (x + 1 - center_x) as i64;
            let dy = (y + 1 - center_y) as i64;
            let inside = dx * dx + dy * dy <= r2;
            if !inside {
                paint_pixel(frame, x, y, value);
            }
        }
    }
    Ok(())
}

fn apply_polygon(frame: &mut DisplayFrame, points: &[(i32, i32)], value: u8) -> Result<()> {
    if points.len() < 3 {
        return Err(invalid_tag_value(
            TAG_SHUTTER_VERTICES,
            "polygonal shutter requires at least three vertices",
        ));
    }
    let width = frame.width as i32;
    let height = frame.height as i32;
    for y in 0..height {
        for x in 0..width {
            let inside = point_in_polygon(x + 1, y + 1, points);
            if !inside {
                paint_pixel(frame, x, y, value);
            }
        }
    }
    Ok(())
}

fn point_in_polygon(x: i32, y: i32, points: &[(i32, i32)]) -> bool {
    let xf = x as f64;
    let yf = y as f64;
    let mut inside = false;
    let mut previous = points[points.len() - 1];
    for &current in points {
        let (x1, y1) = previous;
        let (x2, y2) = current;
        let y1f = y1 as f64;
        let y2f = y2 as f64;
        let x1f = x1 as f64;
        let x2f = x2 as f64;
        if (y1f > yf) != (y2f > yf) {
            let t = (yf - y1f) / (y2f - y1f);
            let x_intersect = x1f + t * (x2f - x1f);
            if xf < x_intersect {
                inside = !inside;
            }
        }
        previous = current;
    }
    inside
}

fn paint_pixel(frame: &mut DisplayFrame, x: i32, y: i32, value: u8) {
    let idx = (y as u32 * frame.width + x as u32) as usize;
    match frame.format {
        PixelFormat::Luma8 => {
            if let Some(slot) = frame.bytes.get_mut(idx) {
                *slot = value;
            }
        }
        PixelFormat::Luma16 => {
            let start = idx * 2;
            if start + 1 < frame.bytes.len() {
                frame.bytes[start] = value;
                frame.bytes[start + 1] = 0;
            }
        }
        PixelFormat::Rgba8 => {
            let start = idx * 4;
            if start + 3 < frame.bytes.len() {
                frame.bytes[start] = value;
                frame.bytes[start + 1] = value;
                frame.bytes[start + 2] = value;
                frame.bytes[start + 3] = 0xFF;
            }
        }
    }
}

fn read_str(dataset: &Dataset, tag: Tag) -> Result<Option<&str>> {
    match dataset.get(tag) {
        Some(element) => match &element.value {
            Value::Str(value) => Ok(Some(value.as_str())),
            Value::Uid(value) => Ok(Some(value.as_str())),
            _ => Err(invalid_tag_value(tag, "expected string")),
        },
        None => Ok(None),
    }
}

fn read_sequence(dataset: &Dataset, tag: Tag) -> Result<Option<&[Dataset]>> {
    match dataset.get(tag) {
        Some(element) => match &element.value {
            Value::Sequence(items) => Ok(Some(items.as_slice())),
            _ => Err(invalid_tag_value(tag, "expected sequence")),
        },
        None => Ok(None),
    }
}

fn read_i32(dataset: &Dataset, tag: Tag) -> Result<i32> {
    let Some(raw) = read_str(dataset, tag)? else {
        return Err(missing_required_tag(tag));
    };
    parse_i32_strict(tag, raw)
}

fn read_i32_list(dataset: &Dataset, tag: Tag, expected: usize) -> Result<[i32; 2]> {
    let Some(raw) = read_str(dataset, tag)? else {
        return Err(missing_required_tag(tag));
    };
    let parts: Vec<&str> = raw.split('\\').collect();
    if parts.len() != expected {
        return Err(invalid_tag_value(tag, "unexpected number of values"));
    }
    let mut values = [0i32; 2];
    for (idx, part) in parts.iter().enumerate().take(2) {
        values[idx] = parse_i32_strict(tag, part)?;
    }
    Ok(values)
}

fn read_i32_vec(dataset: &Dataset, tag: Tag) -> Result<Vec<i32>> {
    let Some(raw) = read_str(dataset, tag)? else {
        return Err(missing_required_tag(tag));
    };
    let parts: Vec<&str> = raw.split('\\').collect();
    let mut values = Vec::with_capacity(parts.len());
    for part in parts {
        values.push(parse_i32_strict(tag, part)?);
    }
    Ok(values)
}

fn read_shutter_value(dataset: &Dataset) -> Result<u8> {
    let Some(raw) = read_str(dataset, TAG_SHUTTER_PRESENTATION_VALUE)? else {
        return Ok(0);
    };
    let value = parse_i32_strict(TAG_SHUTTER_PRESENTATION_VALUE, raw)?;
    if !(0..=255).contains(&value) {
        return Err(invalid_tag_value(
            TAG_SHUTTER_PRESENTATION_VALUE,
            "shutter presentation value must be 0..255",
        ));
    }
    Ok(value as u8)
}

fn missing_required_tag(tag: Tag) -> Box<Error> {
    Error::from_kind(
        ErrorKind::MissingRequiredTag { tag },
        "missing required tag",
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_core::{Element, Tag, Vr};
    use dicom_pixel::{PixelPipeline, PixelPipelineConfig, WindowLevel};

    const GSPS_MANIFEST: &str = include_str!("../manifest.toml");
    const TS_IMPLICIT_VR_LE: &str = "1.2.840.10008.1.2";

    const TAG_ROWS: Tag = Tag(0x0028, 0x0010);
    const TAG_COLUMNS: Tag = Tag(0x0028, 0x0011);
    const TAG_SAMPLES_PER_PIXEL: Tag = Tag(0x0028, 0x0002);
    const TAG_PHOTOMETRIC_INTERPRETATION: Tag = Tag(0x0028, 0x0004);
    const TAG_BITS_ALLOCATED: Tag = Tag(0x0028, 0x0100);
    const TAG_BITS_STORED: Tag = Tag(0x0028, 0x0101);
    const TAG_HIGH_BIT: Tag = Tag(0x0028, 0x0102);
    const TAG_PIXEL_REPRESENTATION: Tag = Tag(0x0028, 0x0103);
    const TAG_PIXEL_DATA: Tag = Tag(0x7FE0, 0x0010);
    const TAG_OVERLAY_ROWS: Tag = Tag(0x6000, 0x0010);
    const TAG_OVERLAY_COLUMNS: Tag = Tag(0x6000, 0x0011);
    const TAG_OVERLAY_BITS_ALLOCATED: Tag = Tag(0x6000, 0x0100);
    const TAG_OVERLAY_BIT_POSITION: Tag = Tag(0x6000, 0x0102);
    const TAG_OVERLAY_DATA: Tag = Tag(0x6000, 0x3000);

    fn parse_manifest_uids() -> Vec<String> {
        GSPS_MANIFEST
            .split('"')
            .enumerate()
            .filter_map(|(idx, part)| {
                if idx % 2 == 1 {
                    Some(part.to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    fn build_rect_dataset_with_bounds(
        left: i32,
        right: i32,
        upper: i32,
        lower: i32,
        value: u8,
    ) -> Dataset {
        let mut dataset = Dataset::new();
        dataset.insert(Element {
            tag: TAG_SHUTTER_SHAPE,
            vr: Vr::Cs,
            value: Value::Str("RECTANGULAR".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SHUTTER_LEFT_VERT_EDGE,
            vr: Vr::Is,
            value: Value::Str(left.to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SHUTTER_RIGHT_VERT_EDGE,
            vr: Vr::Is,
            value: Value::Str(right.to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SHUTTER_UPPER_HORIZ_EDGE,
            vr: Vr::Is,
            value: Value::Str(upper.to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SHUTTER_LOWER_HORIZ_EDGE,
            vr: Vr::Is,
            value: Value::Str(lower.to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SHUTTER_PRESENTATION_VALUE,
            vr: Vr::Us,
            value: Value::Str(value.to_string()),
        });
        dataset
    }

    fn build_rect_dataset() -> Dataset {
        build_rect_dataset_with_bounds(1, 1, 1, 1, 0)
    }

    fn build_polygon_dataset(vertices: &str, value: u8) -> Dataset {
        let mut dataset = Dataset::new();
        dataset.insert(Element {
            tag: TAG_SHUTTER_SHAPE,
            vr: Vr::Cs,
            value: Value::Str("POLYGONAL".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SHUTTER_VERTICES,
            vr: Vr::Is,
            value: Value::Str(vertices.to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SHUTTER_PRESENTATION_VALUE,
            vr: Vr::Us,
            value: Value::Str(value.to_string()),
        });
        dataset
    }

    fn build_graphic_dataset_with_type(graphic_type: &str, graphic_data: &str) -> Dataset {
        let mut graphic = Dataset::new();
        graphic.insert(Element {
            tag: TAG_GRAPHIC_TYPE,
            vr: Vr::Cs,
            value: Value::Str(graphic_type.to_string()),
        });
        graphic.insert(Element {
            tag: TAG_GRAPHIC_DATA,
            vr: Vr::Ds,
            value: Value::Str(graphic_data.to_string()),
        });

        let mut annotation = Dataset::new();
        annotation.insert(Element {
            tag: TAG_GRAPHIC_OBJECT_SEQUENCE,
            vr: Vr::Sq,
            value: Value::Sequence(vec![graphic]),
        });

        let mut dataset = Dataset::new();
        dataset.insert(Element {
            tag: TAG_GRAPHIC_ANNOTATION_SEQUENCE,
            vr: Vr::Sq,
            value: Value::Sequence(vec![annotation]),
        });
        dataset
    }

    fn build_graphic_dataset() -> Dataset {
        build_graphic_dataset_with_type("POLYLINE", "1\\1\\2\\1\\2\\2")
    }

    fn build_image_dataset(rows: u16, cols: u16, pixel_data: Vec<u8>) -> Dataset {
        let mut dataset = Dataset::new();
        dataset.insert(Element {
            tag: TAG_ROWS,
            vr: Vr::Us,
            value: Value::Bytes(rows.to_le_bytes().to_vec()),
        });
        dataset.insert(Element {
            tag: TAG_COLUMNS,
            vr: Vr::Us,
            value: Value::Bytes(cols.to_le_bytes().to_vec()),
        });
        dataset.insert(Element {
            tag: TAG_SAMPLES_PER_PIXEL,
            vr: Vr::Us,
            value: Value::Bytes(1u16.to_le_bytes().to_vec()),
        });
        dataset.insert(Element {
            tag: TAG_PHOTOMETRIC_INTERPRETATION,
            vr: Vr::Cs,
            value: Value::Str("MONOCHROME2".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_BITS_ALLOCATED,
            vr: Vr::Us,
            value: Value::Bytes(8u16.to_le_bytes().to_vec()),
        });
        dataset.insert(Element {
            tag: TAG_BITS_STORED,
            vr: Vr::Us,
            value: Value::Bytes(8u16.to_le_bytes().to_vec()),
        });
        dataset.insert(Element {
            tag: TAG_HIGH_BIT,
            vr: Vr::Us,
            value: Value::Bytes(7u16.to_le_bytes().to_vec()),
        });
        dataset.insert(Element {
            tag: TAG_PIXEL_REPRESENTATION,
            vr: Vr::Us,
            value: Value::Bytes(0u16.to_le_bytes().to_vec()),
        });
        dataset.insert(Element {
            tag: TAG_PIXEL_DATA,
            vr: Vr::Ob,
            value: Value::Bytes(pixel_data),
        });
        dataset
    }

    fn add_overlay(dataset: &mut Dataset, rows: u16, cols: u16, data: Vec<u8>) {
        dataset.insert(Element {
            tag: TAG_OVERLAY_ROWS,
            vr: Vr::Us,
            value: Value::Bytes(rows.to_le_bytes().to_vec()),
        });
        dataset.insert(Element {
            tag: TAG_OVERLAY_COLUMNS,
            vr: Vr::Us,
            value: Value::Bytes(cols.to_le_bytes().to_vec()),
        });
        dataset.insert(Element {
            tag: TAG_OVERLAY_BITS_ALLOCATED,
            vr: Vr::Us,
            value: Value::Bytes(1u16.to_le_bytes().to_vec()),
        });
        dataset.insert(Element {
            tag: TAG_OVERLAY_BIT_POSITION,
            vr: Vr::Us,
            value: Value::Bytes(0u16.to_le_bytes().to_vec()),
        });
        dataset.insert(Element {
            tag: TAG_OVERLAY_DATA,
            vr: Vr::Ob,
            value: Value::Bytes(data),
        });
    }

    #[test]
    #[cfg(not(feature = "gsps"))]
    fn gsps_pack_disabled_rejects() {
        // REQ-FEAT-302
        assert!(!GspsPack::enabled());
        let err = GspsPack::ensure_supported(SOP_CLASS_GSPS).unwrap_err();
        assert_eq!(err.code, "DVF.DICOM.UNSUPPORTED_SOP");
    }

    #[test]
    #[cfg(feature = "gsps")]
    fn gsps_pack_enabled_allows() {
        // REQ-FEAT-302
        assert!(GspsPack::enabled());
        GspsPack::ensure_supported(SOP_CLASS_GSPS).expect("gsps pack enabled");
    }

    #[test]
    fn manifest_matches_constants() {
        // REQ-CONF-083
        let parsed = parse_manifest_uids();
        assert!(!parsed.is_empty());
        for uid in GSPS_SOP_CLASS_UIDS {
            assert!(parsed.contains(&uid.to_string()));
        }
    }

    #[test]
    fn rect_shutter_applied() {
        // REQ-UI-060, REQ-UI-061, REQ-GSPS-300, REQ-GSPS-301
        let dataset = build_rect_dataset();
        let state = PresentationState::from_dataset(&dataset).expect("parse");
        let mut frame = DisplayFrame {
            width: 2,
            height: 2,
            format: PixelFormat::Luma8,
            bytes: vec![10, 10, 10, 10],
        };
        state.apply_to(&mut frame).expect("apply");
        assert_eq!(frame.bytes, vec![10, 0, 0, 0]);
    }

    #[test]
    fn polygonal_shutter_applied() {
        // REQ-UI-060, REQ-UI-061, REQ-GSPS-300, REQ-GSPS-301
        let dataset = build_polygon_dataset("2\\2\\4\\2\\4\\4\\2\\4", 0);
        let state = PresentationState::from_dataset(&dataset).expect("parse");
        let mut frame = DisplayFrame {
            width: 5,
            height: 5,
            format: PixelFormat::Luma8,
            bytes: vec![9; 25],
        };
        state.apply_to(&mut frame).expect("apply");
        // Center pixel remains unchanged; outside corners are shuttered.
        assert_eq!(frame.bytes[12], 9);
        assert_eq!(frame.bytes[0], 0);
    }

    #[test]
    fn polyline_graphic_applied() {
        // REQ-UI-064, REQ-GSPS-303, REQ-CONF-091
        let dataset = build_graphic_dataset();
        let state = PresentationState::from_dataset(&dataset).expect("parse");
        let mut frame = DisplayFrame {
            width: 3,
            height: 3,
            format: PixelFormat::Luma8,
            bytes: vec![0; 9],
        };
        state.apply_to(&mut frame).expect("apply");
        assert!(frame.bytes.iter().any(|value| *value != 0));
    }

    #[test]
    fn point_graphic_applied() {
        // REQ-UI-064, REQ-GSPS-303, REQ-CONF-091
        let dataset = build_graphic_dataset_with_type("POINT", "2\\2");
        let state = PresentationState::from_dataset(&dataset).expect("parse");
        let mut frame = DisplayFrame {
            width: 3,
            height: 3,
            format: PixelFormat::Luma8,
            bytes: vec![0; 9],
        };
        state.apply_to(&mut frame).expect("apply");
        assert_eq!(frame.bytes[4], GRAPHIC_LUMA);
    }

    #[test]
    fn interpolated_graphic_applied() {
        // REQ-UI-064, REQ-GSPS-303, REQ-CONF-091
        let dataset = build_graphic_dataset_with_type("INTERPOLATED", "1\\3\\2\\2\\3\\1");
        let state = PresentationState::from_dataset(&dataset).expect("parse");
        let mut frame = DisplayFrame {
            width: 3,
            height: 3,
            format: PixelFormat::Luma8,
            bytes: vec![0; 9],
        };
        state.apply_to(&mut frame).expect("apply");
        assert!(frame.bytes.iter().any(|value| *value != 0));
    }

    #[test]
    fn circle_graphic_applied() {
        // REQ-UI-064, REQ-GSPS-303, REQ-CONF-091
        let dataset = build_graphic_dataset_with_type("CIRCLE", "3\\3\\5\\3");
        let state = PresentationState::from_dataset(&dataset).expect("parse");
        let mut frame = DisplayFrame {
            width: 7,
            height: 7,
            format: PixelFormat::Luma8,
            bytes: vec![0; 49],
        };
        state.apply_to(&mut frame).expect("apply");
        assert!(frame.bytes.iter().any(|value| *value != 0));
    }

    #[test]
    fn ellipse_graphic_applied() {
        // REQ-UI-064, REQ-GSPS-303, REQ-CONF-091
        let dataset = build_graphic_dataset_with_type("ELLIPSE", "2\\4\\6\\4\\4\\2\\4\\6");
        let state = PresentationState::from_dataset(&dataset).expect("parse");
        let mut frame = DisplayFrame {
            width: 7,
            height: 7,
            format: PixelFormat::Luma8,
            bytes: vec![0; 49],
        };
        state.apply_to(&mut frame).expect("apply");
        assert!(frame.bytes.iter().any(|value| *value != 0));
    }

    #[test]
    fn gsps_overlay_precedence_after_overlays() {
        // REQ-UI-062, REQ-GSPS-300, REQ-GSPS-302
        let mut dataset = build_image_dataset(2, 2, vec![128u8; 4]);
        add_overlay(&mut dataset, 1, 1, vec![0x01]);
        let gsps_dataset = build_rect_dataset_with_bounds(2, 2, 2, 2, 0);
        let state = PresentationState::from_dataset(&gsps_dataset).expect("parse");

        let config = PixelPipelineConfig {
            window_level: WindowLevel::Explicit {
                center: 128.0,
                width: 256.0,
            },
            ..PixelPipelineConfig::default()
        };
        let pipeline = PixelPipeline::new(config);

        let base = pipeline
            .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
            .expect("frame");
        assert_eq!(base.format, PixelFormat::Luma8);
        assert_eq!(base.bytes[0], 255);

        let mut manual = base.clone();
        state.apply_to(&mut manual).expect("apply");

        let combined = pipeline
            .decode_frame_with_transform(&dataset, TS_IMPLICIT_VR_LE, 0, &state)
            .expect("frame");
        assert_eq!(combined.bytes, manual.bytes);
        assert_eq!(combined.bytes[0], 0);
        assert_eq!(combined.bytes[3], 128);
    }

    #[test]
    fn unsupported_graphic_type_rejected() {
        // REQ-UI-065, REQ-GSPS-304, REQ-CONF-091
        let dataset = build_graphic_dataset_with_type("BEZIER", "1\\1\\2\\2");
        let err = PresentationState::from_dataset(&dataset).expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::InvalidTagValue { .. }));
    }

    #[test]
    fn invalid_circle_topology_rejected() {
        // REQ-UI-065, REQ-GSPS-304, REQ-CONF-091
        let dataset = build_graphic_dataset_with_type("CIRCLE", "2\\2");
        let err = PresentationState::from_dataset(&dataset).expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::InvalidTagValue { .. }));
    }

    #[test]
    fn invalid_ellipse_topology_rejected() {
        // REQ-UI-065, REQ-GSPS-304, REQ-CONF-091
        let dataset = build_graphic_dataset_with_type("ELLIPSE", "1\\1\\3\\1\\1\\3\\4\\4");
        let err = PresentationState::from_dataset(&dataset).expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::InvalidTagValue { .. }));
    }

    #[test]
    fn invalid_polygonal_shutter_rejected() {
        // REQ-UI-065, REQ-GSPS-304
        let dataset = build_polygon_dataset("1\\1\\2\\2", 0);
        let err = PresentationState::from_dataset(&dataset).expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::InvalidTagValue { .. }));
    }
}
