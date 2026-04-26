#![deny(missing_docs)]

//! GSPS pack: deterministic shutter and presentation state application.

use dicom_core::{
    parse_f64_strict, parse_i32_strict, Dataset, Element, Error, ErrorKind, Result, Tag, Value,
    Vr,
};
#[cfg(feature = "rendering")]
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

/// Flip direction for a presentation state.
///
/// Replaces the boolean trap of `flip_x: Option<bool>` + `flip_y: Option<bool>`
/// with a single enum that makes all valid states explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Flip {
    /// No flip applied (default).
    #[default]
    None,
    /// Horizontal flip only.
    Horizontal,
    /// Vertical flip only.
    Vertical,
    /// Both horizontal and vertical flip.
    Both,
}

impl Flip {
    /// Return whether horizontal flip is active.
    pub fn is_horizontal(&self) -> bool {
        matches!(self, Flip::Horizontal | Flip::Both)
    }

    /// Return whether vertical flip is active.
    pub fn is_vertical(&self) -> bool {
        matches!(self, Flip::Vertical | Flip::Both)
    }

    /// Construct from individual horizontal and vertical flip flags.
    pub fn from_flags(flip_x: bool, flip_y: bool) -> Self {
        match (flip_x, flip_y) {
            (false, false) => Flip::None,
            (true, false) => Flip::Horizontal,
            (false, true) => Flip::Vertical,
            (true, true) => Flip::Both,
        }
    }
}

/// Rotation in 90-degree quadrants.
///
/// Replaces the integer trap of `rotation_quadrants: Option<i32>` where
/// only values 0–3 are valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Rotation {
    /// No rotation (0°).
    #[default]
    Q0,
    /// 90° rotation.
    Q90,
    /// 180° rotation.
    Q180,
    /// 270° rotation.
    Q270,
}

impl Rotation {
    /// Convert to the number of 90° quadrants (0–3).
    pub fn to_quadrants(&self) -> i32 {
        match self {
            Rotation::Q0 => 0,
            Rotation::Q90 => 1,
            Rotation::Q180 => 2,
            Rotation::Q270 => 3,
        }
    }

    /// Convert from a number of 90° quadrants.
    ///
    /// Returns `None` if the value is not in 0..=3.
    pub fn from_quadrants(q: i32) -> Option<Self> {
        match q.rem_euclid(4) {
            0 => Some(Rotation::Q0),
            1 => Some(Rotation::Q90),
            2 => Some(Rotation::Q180),
            3 => Some(Rotation::Q270),
            _ => None,
        }
    }

    /// Convert to degrees.
    pub fn to_degrees(&self) -> i32 {
        self.to_quadrants() * 90
    }
}

/// Parsed GSPS presentation state.
#[derive(Debug, Clone, PartialEq)]
pub struct PresentationState {
    /// Optional shutter.
    pub shutter: Option<Shutter>,
    /// Graphic annotation objects.
    pub graphics: Vec<GraphicObject>,
    /// Window center value for VOI LUT.
    pub window_center: Option<f64>,
    /// Window width value for VOI LUT.
    pub window_width: Option<f64>,
    /// Zoom factor (1.0 = no zoom).
    pub zoom: Option<f64>,
    /// Horizontal pan offset.
    pub pan_x: Option<f64>,
    /// Vertical pan offset.
    pub pan_y: Option<f64>,
    /// Rotation in 90-degree quadrants.
    pub rotation: Rotation,
    /// Flip direction.
    pub flip: Flip,
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
        Ok(Self {
            shutter,
            graphics,
            window_center: None,
            window_width: None,
            zoom: None,
            pan_x: None,
            pan_y: None,
            rotation: Rotation::default(),
            flip: Flip::default(),
        })
    }

    /// Apply this presentation state to a display frame.
    #[cfg(feature = "rendering")]
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

/// Domain-level descriptor for a GSPS overlay, containing shutter geometry,
/// graphic annotations, and spatial transforms without any rendering
/// dependency.
///
/// This is a type alias for [`PresentationState`] since the presentation
/// state fields are all pure domain data. Use this type when you need a
/// rendering-independent reference to the parsed GSPS data.
pub type GspsOverlayDescriptor = PresentationState;

#[cfg(feature = "rendering")]
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

// Tags for GSPS writeback
const TAG_SOP_CLASS_UID: Tag = Tag(0x0008, 0x0016);
const TAG_REFERENCED_SERIES_SEQUENCE: Tag = Tag(0x0008, 0x1115);
const TAG_REFERENCED_IMAGE_SEQUENCE: Tag = Tag(0x0008, 0x1140);
const TAG_REFERENCED_SOP_CLASS_UID: Tag = Tag(0x0008, 0x1150);
const TAG_REFERENCED_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x1155);
const TAG_WINDOW_CENTER: Tag = Tag(0x0028, 0x1050);
const TAG_WINDOW_WIDTH: Tag = Tag(0x0028, 0x1051);
const TAG_IMAGE_HORIZONTAL_FLIP: Tag = Tag(0x0070, 0x0202);
const TAG_IMAGE_ROTATION: Tag = Tag(0x0070, 0x0204);
const TAG_DISPLAYED_AREA_SELECTION_SEQUENCE: Tag = Tag(0x0070, 0x0050);
const TAG_DISPLAYED_AREA_TOP_LEFT: Tag = Tag(0x0070, 0x0052);
const TAG_PRESENTATION_PIXEL_MAGNIFICATION_RATIO: Tag = Tag(0x0070, 0x0100);

// ---------------------------------------------------------------------------
// Rendering helpers (feature-gated behind "rendering")
// ---------------------------------------------------------------------------

#[cfg(feature = "rendering")]
const GRAPHIC_LUMA: u8 = 0xFF;
#[cfg(feature = "rendering")]
const GRAPHIC_COLOR: [u8; 3] = [0xFF, 0x00, 0x00];
const GRAPHIC_EPS: f64 = 1e-6;
const GRAPHIC_POINT_EPS: f64 = 1e-3;
const GRAPHIC_ORTHO_EPS: f64 = 1e-3;
#[cfg(feature = "rendering")]
const ELLIPSE_SEGMENTS: usize = 180;

#[cfg(feature = "rendering")]
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
        Some(element) => match element.value() {
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

#[cfg(feature = "rendering")]
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

#[cfg(feature = "rendering")]
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

#[cfg(feature = "rendering")]
fn graphic_to_pixel_f64(value: f64) -> f64 {
    value - 1.0
}

#[cfg(feature = "rendering")]
fn draw_polyline(frame: &mut DisplayFrame, width: i32, height: i32, points: &[(i32, i32)]) {
    if points.len() < 2 {
        return;
    }
    for pair in points.windows(2) {
        draw_line(frame, width, height, pair[0], pair[1]);
    }
}

#[cfg(feature = "rendering")]
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

#[cfg(feature = "rendering")]
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

#[cfg(feature = "rendering")]
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

#[cfg(feature = "rendering")]
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

#[cfg(feature = "rendering")]
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

#[cfg(feature = "rendering")]
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

#[cfg(feature = "rendering")]
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

#[cfg(feature = "rendering")]
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
        Some(element) => match element.value() {
            Value::Str(value) => Ok(Some(value.as_str())),
            Value::Uid(value) => Ok(Some(value.as_str())),
            _ => Err(invalid_tag_value(tag, "expected string")),
        },
        None => Ok(None),
    }
}

fn read_sequence(dataset: &Dataset, tag: Tag) -> Result<Option<&[Dataset]>> {
    match dataset.get(tag) {
        Some(element) => match element.value() {
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

// ---------------------------------------------------------------------------
// GSPS writeback (encoding) support
// ---------------------------------------------------------------------------

/// Builder for constructing [`PresentationState`] objects for writeback.
///
/// Provides a fluent API for assembling presentation state attributes before
/// encoding them to a DICOM GSPS dataset via [`encode_presentation_state`].
///
/// # Example
///
/// ```
/// use pack_gsps::{PresentationStateBuilder, Shutter, encode_presentation_state};
///
/// let state = PresentationStateBuilder::new()
///     .with_window_level(40.0, 400.0)
///     .with_zoom(2.0)
///     .build();
/// let dataset = encode_presentation_state(&state);
/// ```
#[derive(Debug, Clone, Default)]
pub struct PresentationStateBuilder {
    /// Optional shutter.
    shutter: Option<Shutter>,
    /// Graphic annotation objects.
    graphics: Vec<GraphicObject>,
    /// Window center value.
    window_center: Option<f64>,
    /// Window width value.
    window_width: Option<f64>,
    /// Zoom factor (1.0 = no zoom).
    zoom: Option<f64>,
    /// Horizontal pan offset.
    pan_x: Option<f64>,
    /// Vertical pan offset.
    pan_y: Option<f64>,
    /// Rotation in 90-degree quadrants.
    rotation: Rotation,
    /// Flip direction.
    flip: Flip,
}

impl PresentationStateBuilder {
    /// Create an empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a shutter to the presentation state.
    pub fn with_shutter(mut self, shutter: Shutter) -> Self {
        self.shutter = Some(shutter);
        self
    }

    /// Add a graphic object to the presentation state.
    pub fn with_graphic(mut self, graphic: GraphicObject) -> Self {
        self.graphics.push(graphic);
        self
    }

    /// Add window center and width values.
    pub fn with_window_level(mut self, center: f64, width: f64) -> Self {
        self.window_center = Some(center);
        self.window_width = Some(width);
        self
    }

    /// Add a zoom factor (1.0 = no zoom).
    pub fn with_zoom(mut self, zoom: f64) -> Self {
        self.zoom = Some(zoom);
        self
    }

    /// Add a pan offset.
    pub fn with_pan(mut self, pan_x: f64, pan_y: f64) -> Self {
        self.pan_x = Some(pan_x);
        self.pan_y = Some(pan_y);
        self
    }

    /// Add rotation.
    pub fn with_rotation(mut self, rotation: Rotation) -> Self {
        self.rotation = rotation;
        self
    }

    /// Add flip direction.
    pub fn with_flip(mut self, flip: Flip) -> Self {
        self.flip = flip;
        self
    }

    /// Build the [`PresentationState`] from the configured attributes.
    pub fn build(self) -> PresentationState {
        PresentationState {
            shutter: self.shutter,
            graphics: self.graphics,
            window_center: self.window_center,
            window_width: self.window_width,
            zoom: self.zoom,
            pan_x: self.pan_x,
            pan_y: self.pan_y,
            rotation: self.rotation,
            flip: self.flip,
        }
    }
}

/// Encode a [`PresentationState`] into a DICOM [`Dataset`].
///
/// Produces a GSPS-conformant dataset containing:
/// - SOP Class UID set to the GSPS SOP Class
/// - Shutter tags if a shutter is present
/// - Graphic Annotation Sequence if graphics are present
/// - Window Center / Window Width if set
/// - Spatial transform attributes (zoom, pan, rotation, flip) if set
pub fn encode_presentation_state(state: &PresentationState) -> Dataset {
    let mut dataset = Dataset::new();

    // SOP Class UID
    dataset.insert(Element::new(TAG_SOP_CLASS_UID, Vr::Ui, Value::Uid(SOP_CLASS_GSPS.to_string()),
    ).unwrap());

    // Shutter
    if let Some(shutter) = &state.shutter {
        encode_shutter(&mut dataset, shutter);
    }

    // Graphics
    if !state.graphics.is_empty() {
        encode_graphics_sequence(&mut dataset, &state.graphics);
    }

    // Window center / width
    if let (Some(center), Some(width)) = (state.window_center, state.window_width) {
        if center.is_finite() && width.is_finite() && width > 0.0 {
            dataset.insert(Element::new(TAG_WINDOW_CENTER, Vr::Ds, Value::Str(format_ds(center)),
            ).unwrap());
            dataset.insert(Element::new(TAG_WINDOW_WIDTH, Vr::Ds, Value::Str(format_ds(width)),
            ).unwrap());
        }
    }

    // Spatial transform
    encode_spatial_transform(&mut dataset, state);

    dataset
}

/// Viewer viewport state that captures the current visual presentation.
///
/// This struct is a convenience type for capturing the full set of viewport
/// parameters from an image viewer and encoding them as a GSPS presentation
/// state via [`encode_viewport_as_gsps`].
#[derive(Debug, Clone, PartialEq)]
pub struct ViewportState {
    /// Window center value.
    pub window_center: f64,
    /// Window width value.
    pub window_width: f64,
    /// Zoom factor (1.0 = no zoom).
    pub zoom: f64,
    /// Horizontal pan offset.
    pub pan_x: f64,
    /// Vertical pan offset.
    pub pan_y: f64,
    /// Rotation in 90-degree quadrants.
    pub rotation: Rotation,
    /// Flip direction.
    pub flip: Flip,
    /// Referenced SOP Instance UID of the source image.
    pub referenced_sop_instance_uid: String,
}

/// Encode a [`ViewportState`] as a GSPS DICOM [`Dataset`].
///
/// Convenience function that converts a viewport state into a complete GSPS
/// dataset, including a reference to the source image SOP Instance UID.
pub fn encode_viewport_as_gsps(viewport: &ViewportState) -> Dataset {
    let state = PresentationState {
        shutter: None,
        graphics: Vec::new(),
        window_center: Some(viewport.window_center),
        window_width: Some(viewport.window_width),
        zoom: Some(viewport.zoom),
        pan_x: Some(viewport.pan_x),
        pan_y: Some(viewport.pan_y),
        rotation: viewport.rotation,
        flip: viewport.flip,
    };

    let mut dataset = encode_presentation_state(&state);

    // Add referenced image sequence pointing to the source instance.
    let mut referenced_image = Dataset::new();
    referenced_image.insert(Element::new(TAG_REFERENCED_SOP_INSTANCE_UID, Vr::Ui, Value::Uid(viewport.referenced_sop_instance_uid.clone()),
    ).unwrap());

    let mut referenced_series = Dataset::new();
    referenced_series.insert(Element::new(TAG_REFERENCED_IMAGE_SEQUENCE, Vr::Sq, Value::Sequence(vec![referenced_image]),
    ).unwrap());

    dataset.insert(Element::new(TAG_REFERENCED_SERIES_SEQUENCE, Vr::Sq, Value::Sequence(vec![referenced_series]),
    ).unwrap());

    dataset
}

// ---------------------------------------------------------------------------
// Writeback helper functions
// ---------------------------------------------------------------------------

/// Format an f64 as a DICOM DS (Decimal String) value.
///
/// Produces a clean decimal string without leading `+` or whitespace,
/// consistent with the strict parsing rules in [`parse_f64_strict`].
fn format_ds(value: f64) -> String {
    debug_assert!(value.is_finite(), "DS value must be finite");
    // For integer-valued floats within i64 range, emit without decimal point.
    if value == value.floor() && value.abs() < 1e15 {
        (value as i64).to_string()
    } else {
        value.to_string()
    }
}

/// Encode a [`Shutter`] into the dataset as DICOM shutter tags.
fn encode_shutter(dataset: &mut Dataset, shutter: &Shutter) {
    match shutter {
        Shutter::Rect {
            left,
            right,
            upper,
            lower,
            value,
        } => {
            dataset.insert(Element::new(TAG_SHUTTER_SHAPE, Vr::Cs, Value::Str("RECTANGULAR".to_string()),
            ).unwrap());
            dataset.insert(Element::new(TAG_SHUTTER_LEFT_VERT_EDGE, Vr::Is, Value::Str(left.to_string()),
            ).unwrap());
            dataset.insert(Element::new(TAG_SHUTTER_RIGHT_VERT_EDGE, Vr::Is, Value::Str(right.to_string()),
            ).unwrap());
            dataset.insert(Element::new(TAG_SHUTTER_UPPER_HORIZ_EDGE, Vr::Is, Value::Str(upper.to_string()),
            ).unwrap());
            dataset.insert(Element::new(TAG_SHUTTER_LOWER_HORIZ_EDGE, Vr::Is, Value::Str(lower.to_string()),
            ).unwrap());
            dataset.insert(Element::new(TAG_SHUTTER_PRESENTATION_VALUE, Vr::Us, Value::Str((*value).to_string()),
            ).unwrap());
        }
        Shutter::Circular {
            center_x,
            center_y,
            radius,
            value,
        } => {
            dataset.insert(Element::new(TAG_SHUTTER_SHAPE, Vr::Cs, Value::Str("CIRCULAR".to_string()),
            ).unwrap());
            dataset.insert(Element::new(TAG_SHUTTER_CENTER, Vr::Is, Value::Str(format!("{}\\{}", center_x, center_y)),
            ).unwrap());
            dataset.insert(Element::new(TAG_SHUTTER_RADIUS, Vr::Is, Value::Str(radius.to_string()),
            ).unwrap());
            dataset.insert(Element::new(TAG_SHUTTER_PRESENTATION_VALUE, Vr::Us, Value::Str((*value).to_string()),
            ).unwrap());
        }
        Shutter::Polygon { points, value } => {
            dataset.insert(Element::new(TAG_SHUTTER_SHAPE, Vr::Cs, Value::Str("POLYGONAL".to_string()),
            ).unwrap());
            let vertices: Vec<String> = points
                .iter()
                .flat_map(|(x, y)| [x.to_string(), y.to_string()])
                .collect();
            dataset.insert(Element::new(TAG_SHUTTER_VERTICES, Vr::Is, Value::Str(vertices.join("\\")),
            ).unwrap());
            dataset.insert(Element::new(TAG_SHUTTER_PRESENTATION_VALUE, Vr::Us, Value::Str((*value).to_string()),
            ).unwrap());
        }
    }
}

/// Encode graphic objects into a Graphic Annotation Sequence in the dataset.
fn encode_graphics_sequence(dataset: &mut Dataset, graphics: &[GraphicObject]) {
    let graphic_items: Vec<Dataset> = graphics
        .iter()
        .map(|g| {
            let mut item = Dataset::new();
            let (graphic_type, graphic_data) = match g {
                GraphicObject::Point { point } => (
                    "POINT",
                    format_ds(point.0) + "\\" + &format_ds(point.1),
                ),
                GraphicObject::Polyline { points } => {
                    let data: Vec<String> = points
                        .iter()
                        .flat_map(|(x, y)| [format_ds(*x), format_ds(*y)])
                        .collect();
                    ("POLYLINE", data.join("\\"))
                }
                GraphicObject::Interpolated { points } => {
                    let data: Vec<String> = points
                        .iter()
                        .flat_map(|(x, y)| [format_ds(*x), format_ds(*y)])
                        .collect();
                    ("INTERPOLATED", data.join("\\"))
                }
                GraphicObject::Circle { center, edge } => (
                    "CIRCLE",
                    format_ds(center.0)
                        + "\\"
                        + &format_ds(center.1)
                        + "\\"
                        + &format_ds(edge.0)
                        + "\\"
                        + &format_ds(edge.1),
                ),
                GraphicObject::Ellipse {
                    major_start,
                    major_end,
                    minor_start,
                    minor_end,
                } => (
                    "ELLIPSE",
                    format_ds(major_start.0)
                        + "\\"
                        + &format_ds(major_start.1)
                        + "\\"
                        + &format_ds(major_end.0)
                        + "\\"
                        + &format_ds(major_end.1)
                        + "\\"
                        + &format_ds(minor_start.0)
                        + "\\"
                        + &format_ds(minor_start.1)
                        + "\\"
                        + &format_ds(minor_end.0)
                        + "\\"
                        + &format_ds(minor_end.1),
                ),
            };
            item.insert(Element::new(TAG_GRAPHIC_TYPE, Vr::Cs, Value::Str(graphic_type.to_string()),
            ).unwrap());
            item.insert(Element::new(TAG_GRAPHIC_DATA, Vr::Ds, Value::Str(graphic_data),
            ).unwrap());
            item
        })
        .collect();

    let mut annotation = Dataset::new();
    annotation.insert(Element::new(TAG_GRAPHIC_OBJECT_SEQUENCE, Vr::Sq, Value::Sequence(graphic_items),
    ).unwrap());

    dataset.insert(Element::new(TAG_GRAPHIC_ANNOTATION_SEQUENCE, Vr::Sq, Value::Sequence(vec![annotation]),
    ).unwrap());
}

/// Encode spatial transform attributes (zoom, pan, rotation, flip).
///
/// Rotation and flip are encoded using the standard Spatial Transformation
/// Module tags. Zoom is stored as Presentation Pixel Magnification Ratio
/// inside a Displayed Area Selection Sequence. Pan offsets are included
/// in the same sequence item.
fn encode_spatial_transform(dataset: &mut Dataset, state: &PresentationState) {
    let has_rotation = state.rotation != Rotation::Q0;
    let has_flip = state.flip != Flip::None;
    let has_zoom = state.zoom.is_some();
    let has_pan = state.pan_x.is_some() || state.pan_y.is_some();

    if !has_rotation && !has_flip && !has_zoom && !has_pan {
        return;
    }

    // Compute effective DICOM flip and rotation.
    // DICOM GSPS supports Image Horizontal Flip (0070,0202) and Image
    // Rotation (0070,0204). Vertical flip is represented as horizontal
    // flip combined with 180° rotation.
    let flip_x = state.flip.is_horizontal();
    let flip_y = state.flip.is_vertical();
    let rotation_q = state.rotation.to_quadrants();

    let effective_flip_x = flip_x ^ flip_y;
    let effective_rotation = (rotation_q + if flip_y { 2 } else { 0 }) % 4;

    // Write Image Horizontal Flip
    dataset.insert(Element::new(TAG_IMAGE_HORIZONTAL_FLIP, Vr::Cs, Value::Str(
        if effective_flip_x { "Y".to_string() } else { "N".to_string() }
    )).unwrap());

    // Write Image Rotation (in degrees)
    dataset.insert(Element::new(TAG_IMAGE_ROTATION, Vr::Is, Value::Str((effective_rotation * 90).to_string()),
    ).unwrap());

    // Write zoom and pan in a Displayed Area Selection Sequence
    if has_zoom || has_pan {
        let mut area_item = Dataset::new();

        if let Some(zoom) = state.zoom {
            if zoom.is_finite() && zoom > 0.0 {
                area_item.insert(Element::new(TAG_PRESENTATION_PIXEL_MAGNIFICATION_RATIO, Vr::Ds, Value::Str(format_ds(zoom)),
                ).unwrap());
            }
        }

        if has_pan {
            let pan_x = state.pan_x.unwrap_or(0.0);
            let pan_y = state.pan_y.unwrap_or(0.0);
            if pan_x.is_finite() && pan_y.is_finite() {
                area_item.insert(Element::new(TAG_DISPLAYED_AREA_TOP_LEFT, Vr::Ds, Value::Str(format!("{}\\{}", format_ds(pan_y), format_ds(pan_x))),
                ).unwrap());
            }
        }

        dataset.insert(Element::new(TAG_DISPLAYED_AREA_SELECTION_SEQUENCE, Vr::Sq, Value::Sequence(vec![area_item]),
        ).unwrap());
    }
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
        dataset.insert(Element::new(TAG_SHUTTER_SHAPE, Vr::Cs, Value::Str("RECTANGULAR".to_string()),
        ).unwrap());
        dataset.insert(Element::new(TAG_SHUTTER_LEFT_VERT_EDGE, Vr::Is, Value::Str(left.to_string()),
        ).unwrap());
        dataset.insert(Element::new(TAG_SHUTTER_RIGHT_VERT_EDGE, Vr::Is, Value::Str(right.to_string()),
        ).unwrap());
        dataset.insert(Element::new(TAG_SHUTTER_UPPER_HORIZ_EDGE, Vr::Is, Value::Str(upper.to_string()),
        ).unwrap());
        dataset.insert(Element::new(TAG_SHUTTER_LOWER_HORIZ_EDGE, Vr::Is, Value::Str(lower.to_string()),
        ).unwrap());
        dataset.insert(Element::new(TAG_SHUTTER_PRESENTATION_VALUE, Vr::Us, Value::Str(value.to_string()),
        ).unwrap());
        dataset
    }

    fn build_rect_dataset() -> Dataset {
        build_rect_dataset_with_bounds(1, 1, 1, 1, 0)
    }

    fn build_polygon_dataset(vertices: &str, value: u8) -> Dataset {
        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_SHUTTER_SHAPE, Vr::Cs, Value::Str("POLYGONAL".to_string()),
        ).unwrap());
        dataset.insert(Element::new(TAG_SHUTTER_VERTICES, Vr::Is, Value::Str(vertices.to_string()),
        ).unwrap());
        dataset.insert(Element::new(TAG_SHUTTER_PRESENTATION_VALUE, Vr::Us, Value::Str(value.to_string()),
        ).unwrap());
        dataset
    }

    fn build_graphic_dataset_with_type(graphic_type: &str, graphic_data: &str) -> Dataset {
        let mut graphic = Dataset::new();
        graphic.insert(Element::new(TAG_GRAPHIC_TYPE, Vr::Cs, Value::Str(graphic_type.to_string()),
        ).unwrap());
        graphic.insert(Element::new(TAG_GRAPHIC_DATA, Vr::Ds, Value::Str(graphic_data.to_string()),
        ).unwrap());

        let mut annotation = Dataset::new();
        annotation.insert(Element::new(TAG_GRAPHIC_OBJECT_SEQUENCE, Vr::Sq, Value::Sequence(vec![graphic]),
        ).unwrap());

        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_GRAPHIC_ANNOTATION_SEQUENCE, Vr::Sq, Value::Sequence(vec![annotation]),
        ).unwrap());
        dataset
    }

    fn build_graphic_dataset() -> Dataset {
        build_graphic_dataset_with_type("POLYLINE", "1\\1\\2\\1\\2\\2")
    }

    fn build_image_dataset(rows: u16, cols: u16, pixel_data: Vec<u8>) -> Dataset {
        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_ROWS, Vr::Us, Value::Bytes(rows.to_le_bytes().to_vec()),
        ).unwrap());
        dataset.insert(Element::new(TAG_COLUMNS, Vr::Us, Value::Bytes(cols.to_le_bytes().to_vec()),
        ).unwrap());
        dataset.insert(Element::new(TAG_SAMPLES_PER_PIXEL, Vr::Us, Value::Bytes(1u16.to_le_bytes().to_vec()),
        ).unwrap());
        dataset.insert(Element::new(TAG_PHOTOMETRIC_INTERPRETATION, Vr::Cs, Value::Str("MONOCHROME2".to_string()),
        ).unwrap());
        dataset.insert(Element::new(TAG_BITS_ALLOCATED, Vr::Us, Value::Bytes(8u16.to_le_bytes().to_vec()),
        ).unwrap());
        dataset.insert(Element::new(TAG_BITS_STORED, Vr::Us, Value::Bytes(8u16.to_le_bytes().to_vec()),
        ).unwrap());
        dataset.insert(Element::new(TAG_HIGH_BIT, Vr::Us, Value::Bytes(7u16.to_le_bytes().to_vec()),
        ).unwrap());
        dataset.insert(Element::new(TAG_PIXEL_REPRESENTATION, Vr::Us, Value::Bytes(0u16.to_le_bytes().to_vec()),
        ).unwrap());
        dataset.insert(Element::new(TAG_PIXEL_DATA, Vr::Ob, Value::Bytes(pixel_data),
        ).unwrap());
        dataset
    }

    fn add_overlay(dataset: &mut Dataset, rows: u16, cols: u16, data: Vec<u8>) {
        dataset.insert(Element::new(TAG_OVERLAY_ROWS, Vr::Us, Value::Bytes(rows.to_le_bytes().to_vec()),
        ).unwrap());
        dataset.insert(Element::new(TAG_OVERLAY_COLUMNS, Vr::Us, Value::Bytes(cols.to_le_bytes().to_vec()),
        ).unwrap());
        dataset.insert(Element::new(TAG_OVERLAY_BITS_ALLOCATED, Vr::Us, Value::Bytes(1u16.to_le_bytes().to_vec()),
        ).unwrap());
        dataset.insert(Element::new(TAG_OVERLAY_BIT_POSITION, Vr::Us, Value::Bytes(0u16.to_le_bytes().to_vec()),
        ).unwrap());
        dataset.insert(Element::new(TAG_OVERLAY_DATA, Vr::Ob, Value::Bytes(data),
        ).unwrap());
    }

    #[test]
    #[cfg(not(feature = "gsps"))]
    fn gsps_pack_disabled_rejects() {
        // REQ-FEAT-302
        assert!(!GspsPack::enabled());
        let err = GspsPack::ensure_supported(SOP_CLASS_GSPS).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.UNSUPPORTED_SOP");
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
        assert!(matches!(err.kind(), ErrorKind::InvalidTagValue { .. }));
    }

    #[test]
    fn invalid_circle_topology_rejected() {
        // REQ-UI-065, REQ-GSPS-304, REQ-CONF-091
        let dataset = build_graphic_dataset_with_type("CIRCLE", "2\\2");
        let err = PresentationState::from_dataset(&dataset).expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::InvalidTagValue { .. }));
    }

    #[test]
    fn invalid_ellipse_topology_rejected() {
        // REQ-UI-065, REQ-GSPS-304, REQ-CONF-091
        let dataset = build_graphic_dataset_with_type("ELLIPSE", "1\\1\\3\\1\\1\\3\\4\\4");
        let err = PresentationState::from_dataset(&dataset).expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::InvalidTagValue { .. }));
    }

    #[test]
    fn invalid_polygonal_shutter_rejected() {
        // REQ-UI-065, REQ-GSPS-304
        let dataset = build_polygon_dataset("1\\1\\2\\2", 0);
        let err = PresentationState::from_dataset(&dataset).expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::InvalidTagValue { .. }));
    }

    // -----------------------------------------------------------------------
    // Writeback (encoding) tests
    // -----------------------------------------------------------------------

    #[test]
    fn builder_creates_empty_state() {
        // REQ-GSPS-302
        let state = PresentationStateBuilder::new().build();
        assert!(state.shutter.is_none());
        assert!(state.graphics.is_empty());
        assert!(state.window_center.is_none());
        assert!(state.window_width.is_none());
        assert!(state.zoom.is_none());
        assert!(state.pan_x.is_none());
        assert!(state.pan_y.is_none());
        assert!(state.rotation_quadrants.is_none());
        assert!(state.flip_x.is_none());
        assert!(state.flip_y.is_none());
    }

    #[test]
    fn builder_with_window_level() {
        // REQ-GSPS-302
        let state = PresentationStateBuilder::new()
            .with_window_level(40.0, 400.0)
            .build();
        assert_eq!(state.window_center, Some(40.0));
        assert_eq!(state.window_width, Some(400.0));
    }

    #[test]
    fn builder_with_spatial_transform() {
        // REQ-GSPS-302
        let state = PresentationStateBuilder::new()
            .with_zoom(2.5)
            .with_pan(10.0, -5.0)
            .with_rotation(1)
            .with_flip(true, false)
            .build();
        assert_eq!(state.zoom, Some(2.5));
        assert_eq!(state.pan_x, Some(10.0));
        assert_eq!(state.pan_y, Some(-5.0));
        assert_eq!(state.rotation_quadrants, Some(1));
        assert_eq!(state.flip_x, Some(true));
        assert_eq!(state.flip_y, Some(false));
    }

    #[test]
    fn builder_with_shutter_and_graphics() {
        // REQ-GSPS-302
        let state = PresentationStateBuilder::new()
            .with_shutter(Shutter::Rect {
                left: 1,
                right: 10,
                upper: 1,
                lower: 10,
                value: 0,
            })
            .with_graphic(GraphicObject::Point { point: (5.0, 5.0) })
            .with_graphic(GraphicObject::Polyline {
                points: vec![(1.0, 1.0), (2.0, 2.0)],
            })
            .build();
        assert!(state.shutter.is_some());
        assert_eq!(state.graphics.len(), 2);
    }

    #[test]
    fn encode_empty_state_has_sop_class_uid() {
        // REQ-GSPS-302
        let state = PresentationStateBuilder::new().build();
        let dataset = encode_presentation_state(&state);
        let sop_class = dataset.get_uid(TAG_SOP_CLASS_UID);
        assert_eq!(sop_class, Some(SOP_CLASS_GSPS));
    }

    #[test]
    fn encode_rect_shutter_roundtrip() {
        // REQ-GSPS-302
        let original = PresentationStateBuilder::new()
            .with_shutter(Shutter::Rect {
                left: 5,
                right: 20,
                upper: 3,
                lower: 15,
                value: 128,
            })
            .build();
        let dataset = encode_presentation_state(&original);
        let parsed = PresentationState::from_dataset(&dataset).expect("parse encoded");
        assert_eq!(parsed.shutter, original.shutter);
        assert!(parsed.graphics.is_empty());
    }

    #[test]
    fn encode_circular_shutter_roundtrip() {
        // REQ-GSPS-302
        let original = PresentationStateBuilder::new()
            .with_shutter(Shutter::Circular {
                center_x: 100,
                center_y: 200,
                radius: 50,
                value: 64,
            })
            .build();
        let dataset = encode_presentation_state(&original);
        let parsed = PresentationState::from_dataset(&dataset).expect("parse encoded");
        assert_eq!(parsed.shutter, original.shutter);
    }

    #[test]
    fn encode_polygon_shutter_roundtrip() {
        // REQ-GSPS-302
        let original = PresentationStateBuilder::new()
            .with_shutter(Shutter::Polygon {
                points: vec![(1, 1), (100, 1), (100, 100)],
                value: 0,
            })
            .build();
        let dataset = encode_presentation_state(&original);
        let parsed = PresentationState::from_dataset(&dataset).expect("parse encoded");
        assert_eq!(parsed.shutter, original.shutter);
    }

    #[test]
    fn encode_polyline_graphic_roundtrip() {
        // REQ-GSPS-302
        let original = PresentationStateBuilder::new()
            .with_graphic(GraphicObject::Polyline {
                points: vec![(1.0, 1.0), (2.0, 3.0), (4.0, 5.0)],
            })
            .build();
        let dataset = encode_presentation_state(&original);
        let parsed = PresentationState::from_dataset(&dataset).expect("parse encoded");
        assert_eq!(parsed.graphics, original.graphics);
    }

    #[test]
    fn encode_point_graphic_roundtrip() {
        // REQ-GSPS-302
        let original = PresentationStateBuilder::new()
            .with_graphic(GraphicObject::Point { point: (10.5, 20.3) })
            .build();
        let dataset = encode_presentation_state(&original);
        let parsed = PresentationState::from_dataset(&dataset).expect("parse encoded");
        assert_eq!(parsed.graphics, original.graphics);
    }

    #[test]
    fn encode_interpolated_graphic_roundtrip() {
        // REQ-GSPS-302
        let original = PresentationStateBuilder::new()
            .with_graphic(GraphicObject::Interpolated {
                points: vec![(1.0, 1.0), (5.0, 5.0), (10.0, 1.0)],
            })
            .build();
        let dataset = encode_presentation_state(&original);
        let parsed = PresentationState::from_dataset(&dataset).expect("parse encoded");
        assert_eq!(parsed.graphics, original.graphics);
    }

    #[test]
    fn encode_circle_graphic_roundtrip() {
        // REQ-GSPS-302
        let original = PresentationStateBuilder::new()
            .with_graphic(GraphicObject::Circle {
                center: (50.0, 50.0),
                edge: (60.0, 50.0),
            })
            .build();
        let dataset = encode_presentation_state(&original);
        let parsed = PresentationState::from_dataset(&dataset).expect("parse encoded");
        assert_eq!(parsed.graphics, original.graphics);
    }

    #[test]
    fn encode_window_level() {
        // REQ-GSPS-302
        let state = PresentationStateBuilder::new()
            .with_window_level(40.0, 400.0)
            .build();
        let dataset = encode_presentation_state(&state);
        let center = dataset
            .get(TAG_WINDOW_CENTER)
            .and_then(|e| match &e.value {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        let width = dataset
            .get(TAG_WINDOW_WIDTH)
            .and_then(|e| match &e.value {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        assert_eq!(center, Some("40"));
        assert_eq!(width, Some("400"));
    }

    #[test]
    fn encode_spatial_transform_no_flip() {
        // REQ-GSPS-302
        let state = PresentationStateBuilder::new()
            .with_rotation(1)
            .with_flip(false, false)
            .build();
        let dataset = encode_presentation_state(&state);
        let flip = dataset
            .get(TAG_IMAGE_HORIZONTAL_FLIP)
            .and_then(|e| match &e.value {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        let rotation = dataset
            .get(TAG_IMAGE_ROTATION)
            .and_then(|e| match &e.value {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        assert_eq!(flip, Some("N"));
        assert_eq!(rotation, Some("90")); // quadrant 1 = 90°
    }

    #[test]
    fn encode_spatial_transform_horizontal_flip() {
        // REQ-GSPS-302
        let state = PresentationStateBuilder::new()
            .with_rotation(0)
            .with_flip(true, false)
            .build();
        let dataset = encode_presentation_state(&state);
        let flip = dataset
            .get(TAG_IMAGE_HORIZONTAL_FLIP)
            .and_then(|e| match &e.value {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        let rotation = dataset
            .get(TAG_IMAGE_ROTATION)
            .and_then(|e| match &e.value {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        assert_eq!(flip, Some("Y")); // flip_x XOR flip_y = true
        assert_eq!(rotation, Some("0")); // no rotation + no flip_y
    }

    #[test]
    fn encode_spatial_transform_vertical_flip() {
        // REQ-GSPS-302: vertical flip = horizontal flip + 180° rotation
        let state = PresentationStateBuilder::new()
            .with_rotation(0)
            .with_flip(false, true)
            .build();
        let dataset = encode_presentation_state(&state);
        let flip = dataset
            .get(TAG_IMAGE_HORIZONTAL_FLIP)
            .and_then(|e| match &e.value {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        let rotation = dataset
            .get(TAG_IMAGE_ROTATION)
            .and_then(|e| match &e.value {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        assert_eq!(flip, Some("Y")); // flip_x XOR flip_y = true
        assert_eq!(rotation, Some("180")); // rotation_q + 2 = 2 quadrants
    }

    #[test]
    fn encode_spatial_transform_both_flips() {
        // REQ-GSPS-302: both flips = 180° rotation, no horizontal flip
        let state = PresentationStateBuilder::new()
            .with_rotation(0)
            .with_flip(true, true)
            .build();
        let dataset = encode_presentation_state(&state);
        let flip = dataset
            .get(TAG_IMAGE_HORIZONTAL_FLIP)
            .and_then(|e| match &e.value {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        let rotation = dataset
            .get(TAG_IMAGE_ROTATION)
            .and_then(|e| match &e.value {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        assert_eq!(flip, Some("N")); // flip_x XOR flip_y = false
        assert_eq!(rotation, Some("180")); // rotation_q + 2 (from flip_y)
    }

    #[test]
    fn encode_zoom_in_displayed_area() {
        // REQ-GSPS-302
        let state = PresentationStateBuilder::new()
            .with_zoom(2.5)
            .build();
        let dataset = encode_presentation_state(&state);
        let seq = dataset
            .get(TAG_DISPLAYED_AREA_SELECTION_SEQUENCE)
            .and_then(|e| match &e.value {
                Value::Sequence(items) => Some(items.as_slice()),
                _ => None,
            });
        assert!(seq.is_some());
        let items = seq.unwrap();
        assert_eq!(items.len(), 1);
        let zoom_val = items[0]
            .get(TAG_PRESENTATION_PIXEL_MAGNIFICATION_RATIO)
            .and_then(|e| match &e.value {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        assert_eq!(zoom_val, Some("2.5"));
    }

    #[test]
    fn encode_pan_in_displayed_area() {
        // REQ-GSPS-302
        let state = PresentationStateBuilder::new()
            .with_pan(10.0, -5.0)
            .build();
        let dataset = encode_presentation_state(&state);
        let seq = dataset
            .get(TAG_DISPLAYED_AREA_SELECTION_SEQUENCE)
            .and_then(|e| match &e.value {
                Value::Sequence(items) => Some(items.as_slice()),
                _ => None,
            });
        assert!(seq.is_some());
        let items = seq.unwrap();
        assert_eq!(items.len(), 1);
        let pan_val = items[0]
            .get(TAG_DISPLAYED_AREA_TOP_LEFT)
            .and_then(|e| match &e.value {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        // Format is "pan_y\\pan_x"
        assert_eq!(pan_val, Some("-5\\10"));
    }

    #[test]
    fn viewport_state_struct() {
        // REQ-GSPS-302
        let viewport = ViewportState {
            window_center: 40.0,
            window_width: 400.0,
            zoom: 1.5,
            pan_x: 5.0,
            pan_y: -3.0,
            rotation_quadrants: 1,
            flip_x: false,
            flip_y: true,
            referenced_sop_instance_uid: "1.2.3.4.5".to_string(),
        };
        assert_eq!(viewport.window_center, 40.0);
        assert_eq!(viewport.zoom, 1.5);
        assert_eq!(viewport.flip_y, true);
        assert_eq!(viewport.referenced_sop_instance_uid, "1.2.3.4.5");
    }

    #[test]
    fn encode_viewport_as_gsps_contains_reference() {
        // REQ-GSPS-302
        let viewport = ViewportState {
            window_center: 40.0,
            window_width: 400.0,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            rotation_quadrants: 0,
            flip_x: false,
            flip_y: false,
            referenced_sop_instance_uid: "1.2.840.10008.5.1.4.1.1.2".to_string(),
        };
        let dataset = encode_viewport_as_gsps(&viewport);

        // Should have SOP Class UID
        let sop_class = dataset.get_uid(TAG_SOP_CLASS_UID);
        assert_eq!(sop_class, Some(SOP_CLASS_GSPS));

        // Should have window center/width
        let center = dataset
            .get(TAG_WINDOW_CENTER)
            .and_then(|e| match &e.value {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        assert_eq!(center, Some("40"));

        // Should have referenced series sequence with the SOP Instance UID
        let ref_series = dataset
            .get(TAG_REFERENCED_SERIES_SEQUENCE)
            .and_then(|e| match &e.value {
                Value::Sequence(items) => Some(items.as_slice()),
                _ => None,
            });
        assert!(ref_series.is_some());
        let series_items = ref_series.unwrap();
        assert_eq!(series_items.len(), 1);
        let ref_images = series_items[0]
            .get(TAG_REFERENCED_IMAGE_SEQUENCE)
            .and_then(|e| match &e.value {
                Value::Sequence(items) => Some(items.as_slice()),
                _ => None,
            });
        assert!(ref_images.is_some());
        let image_items = ref_images.unwrap();
        assert_eq!(image_items.len(), 1);
        let uid = image_items[0].get_uid(TAG_REFERENCED_SOP_INSTANCE_UID);
        assert_eq!(uid, Some("1.2.840.10008.5.1.4.1.1.2"));
    }

    #[test]
    fn encode_viewport_with_rotation_and_flip() {
        // REQ-GSPS-302
        let viewport = ViewportState {
            window_center: 500.0,
            window_width: 2000.0,
            zoom: 3.0,
            pan_x: 100.0,
            pan_y: 50.0,
            rotation_quadrants: 2,
            flip_x: true,
            flip_y: false,
            referenced_sop_instance_uid: "9.9.9".to_string(),
        };
        let dataset = encode_viewport_as_gsps(&viewport);

        // Check flip and rotation are encoded
        let flip = dataset
            .get(TAG_IMAGE_HORIZONTAL_FLIP)
            .and_then(|e| match &e.value {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        let rotation = dataset
            .get(TAG_IMAGE_ROTATION)
            .and_then(|e| match &e.value {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        assert_eq!(flip, Some("Y")); // flip_x=true, flip_y=false -> effective_flip_x=true
        assert_eq!(rotation, Some("180")); // rotation_q=2, flip_y=false -> 2*90=180

        // Check zoom
        let seq = dataset
            .get(TAG_DISPLAYED_AREA_SELECTION_SEQUENCE)
            .and_then(|e| match &e.value {
                Value::Sequence(items) => Some(items.as_slice()),
                _ => None,
            })
            .expect("displayed area sequence");
        let zoom_val = seq[0]
            .get(TAG_PRESENTATION_PIXEL_MAGNIFICATION_RATIO)
            .and_then(|e| match &e.value {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            });
        assert_eq!(zoom_val, Some("3"));
    }

    #[test]
    fn encode_no_spatial_transform_when_none_set() {
        // REQ-GSPS-302
        let state = PresentationStateBuilder::new().build();
        let dataset = encode_presentation_state(&state);
        assert!(dataset.get(TAG_IMAGE_HORIZONTAL_FLIP).is_none());
        assert!(dataset.get(TAG_IMAGE_ROTATION).is_none());
        assert!(dataset.get(TAG_DISPLAYED_AREA_SELECTION_SEQUENCE).is_none());
    }

    #[test]
    fn format_ds_integer_values() {
        // REQ-GSPS-302
        assert_eq!(format_ds(40.0), "40");
        assert_eq!(format_ds(0.0), "0");
        assert_eq!(format_ds(-7.0), "-7");
    }

    #[test]
    fn format_ds_fractional_values() {
        // REQ-GSPS-302
        assert_eq!(format_ds(1.5), "1.5");
        assert_eq!(format_ds(0.001), "0.001");
    }
}
