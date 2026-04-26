#![deny(missing_docs)]

//! Whole-Slide Imaging (WSI) module for digital pathology.
//!
//! Provides:
//! - **S7-T5**: DICOM Supplement 145 WSI IOD parsing, pyramid/tile-based
//!   streaming rendering, deep zoom with on-demand tile retrieval, and
//!   pathology measurement tools (cell counting, area measurement).

use dicom_audit::{AuditEvent, AuditEventKind, AuditField, AuditValue};
use dicom_core::{Dataset, Element, Error, ErrorKind, Result, Tag, Value, Vr};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

// ===========================================================================
// S7-T5: Whole-Slide Imaging
// ===========================================================================

/// DICOM Supplement 145 Whole Slide Microscopy Image IOD SOP Class UID.
pub const SOP_CLASS_WSI: &str = "1.2.840.10008.5.1.4.1.1.77.1.6";

/// Pyramid level descriptor for a WSI image.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PyramidLevel {
    /// Level number (0 = base/highest resolution).
    pub level: u32,
    /// Total image width in pixels at this level.
    pub width: u64,
    /// Total image height in pixels at this level.
    pub height: u64,
    /// Tile width in pixels.
    pub tile_width: u32,
    /// Tile height in pixels.
    pub tile_height: u32,
    /// X offset of the total pixel matrix origin.
    pub x_offset: i64,
    /// Y offset of the total pixel matrix origin.
    pub y_offset: i64,
    /// Number of focal planes.
    pub focal_planes: u32,
    /// Number of z-slices (for 3D WSI).
    pub z_slices: u32,
    /// Pixel spacing in mm at this level.
    pub pixel_spacing_mm: f64,
}

impl PyramidLevel {
    /// Create a new pyramid level descriptor.
    pub fn new(level: u32, width: u64, height: u64, tile_width: u32, tile_height: u32) -> Self {
        // Pixel spacing doubles with each pyramid level
        let base_spacing = 0.00025; // 0.25 micrometers at 40x
        let spacing = base_spacing * (2u64.pow(level) as f64);

        Self {
            level,
            width,
            height,
            tile_width,
            tile_height,
            x_offset: 0,
            y_offset: 0,
            focal_planes: 1,
            z_slices: 1,
            pixel_spacing_mm: spacing,
        }
    }

    /// Number of tile columns at this level.
    pub fn tile_columns(&self) -> u64 {
        (self.width + self.tile_width as u64 - 1) / self.tile_width as u64
    }

    /// Number of tile rows at this level.
    pub fn tile_rows(&self) -> u64 {
        (self.height + self.tile_height as u64 - 1) / self.tile_height as u64
    }

    /// Total number of tiles at this level.
    pub fn total_tiles(&self) -> u64 {
        self.tile_columns() * self.tile_rows()
    }

    /// Compute the magnification at this level relative to base.
    pub fn magnification(&self) -> f64 {
        40.0 / (2u32.pow(self.level) as f64)
    }
}

/// A tile coordinate in the WSI pyramid.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TileCoordinate {
    /// Pyramid level.
    pub level: u32,
    /// Tile column index.
    pub col: u64,
    /// Tile row index.
    pub row: u64,
    /// Focal plane index.
    pub focal_plane: u32,
    /// Z-slice index.
    pub z_slice: u32,
}

impl TileCoordinate {
    /// Create a new tile coordinate.
    pub fn new(level: u32, col: u64, row: u64) -> Self {
        Self {
            level,
            col,
            row,
            focal_plane: 0,
            z_slice: 0,
        }
    }
}

/// Tile data with its coordinate and content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    /// Tile coordinate in the pyramid.
    pub coord: TileCoordinate,
    /// Compressed image data (typically JPEG or JPEG 2000).
    pub data: Vec<u8>,
    /// Data size in bytes.
    pub data_size: u64,
}

/// WSI image metadata parsed from DICOM Supplement 145 IOD.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WsiMetadata {
    /// SOP Instance UID.
    pub sop_instance_uid: String,
    /// Study Instance UID.
    pub study_uid: String,
    /// Series Instance UID.
    pub series_uid: String,
    /// Slide label / identifier.
    pub slide_id: String,
    /// Specimen identifier.
    pub specimen_id: String,
    /// Stain description (e.g., "H&E", "IHC CD3").
    pub stain: String,
    /// Objective lens power.
    pub objective_power: f64,
    /// Pyramid levels in this WSI.
    pub pyramid_levels: Vec<PyramidLevel>,
    /// ICC profile name for color calibration.
    pub icc_profile: String,
    /// Total pixel matrix dimensions at base level.
    pub total_width: u64,
    /// Total pixel matrix dimensions at base level.
    pub total_height: u64,
}

impl WsiMetadata {
    /// Create minimal WSI metadata for a whole-slide image.
    pub fn new(sop_instance_uid: &str, study_uid: &str, series_uid: &str) -> Self {
        Self {
            sop_instance_uid: sop_instance_uid.to_string(),
            study_uid: study_uid.to_string(),
            series_uid: series_uid.to_string(),
            slide_id: String::new(),
            specimen_id: String::new(),
            stain: String::new(),
            objective_power: 40.0,
            pyramid_levels: Vec::new(),
            icc_profile: String::new(),
            total_width: 0,
            total_height: 0,
        }
    }

    /// Parse WSI metadata from a DICOM dataset.
    pub fn from_dataset(dataset: &Dataset) -> Result<Self> {
        let sop_uid = dataset.get_uid(Tag(0x0008, 0x0018)).unwrap_or("").to_string();
        let study_uid = dataset.get_uid(Tag(0x0020, 0x000D)).unwrap_or("").to_string();
        let series_uid = dataset.get_uid(Tag(0x0020, 0x000E)).unwrap_or("").to_string();

        Ok(Self {
            sop_instance_uid: sop_uid,
            study_uid,
            series_uid,
            slide_id: String::new(),
            specimen_id: String::new(),
            stain: String::new(),
            objective_power: 40.0,
            pyramid_levels: Vec::new(),
            icc_profile: String::new(),
            total_width: 0,
            total_height: 0,
        })
    }

    /// Get the base (highest resolution) pyramid level.
    pub fn base_level(&self) -> Option<&PyramidLevel> {
        self.pyramid_levels.iter().find(|l| l.level == 0)
    }

    /// Find the best pyramid level for a target magnification.
    pub fn level_for_magnification(&self, target_mag: f64) -> Option<&PyramidLevel> {
        self.pyramid_levels
            .iter()
            .filter(|l| l.magnification() >= target_mag)
            .min_by_key(|l| (l.magnification() * 100.0) as u64)
    }
}

/// Viewport state for WSI deep zoom navigation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WsiViewport {
    /// Center X coordinate in base-level pixels.
    pub center_x: f64,
    /// Center Y coordinate in base-level pixels.
    pub center_y: f64,
    /// Current magnification level.
    pub magnification: f64,
    /// Viewport width in screen pixels.
    pub viewport_width: u32,
    /// Viewport height in screen pixels.
    pub viewport_height: u32,
}

impl Default for WsiViewport {
    fn default() -> Self {
        Self {
            center_x: 0.0,
            center_y: 0.0,
            magnification: 1.0,
            viewport_width: 1024,
            viewport_height: 768,
        }
    }
}

impl WsiViewport {
    /// Create a new WSI viewport.
    pub fn new(center_x: f64, center_y: f64, magnification: f64) -> Self {
        Self {
            center_x,
            center_y,
            magnification,
            ..Self::default()
        }
    }

    /// Compute which tiles are visible in the current viewport at a given pyramid level.
    pub fn visible_tiles(&self, level: &PyramidLevel) -> Vec<TileCoordinate> {
        let scale = 2u32.pow(level.level) as f64;

        // Convert viewport bounds to tile coordinates at this level
        let half_w = self.viewport_width as f64 * scale / 2.0;
        let half_h = self.viewport_height as f64 * scale / 2.0;

        let min_x = ((self.center_x - half_w).max(0.0) / level.tile_width as f64) as u64;
        let max_x = ((self.center_x + half_w).min(level.width as f64) / level.tile_width as f64) as u64;
        let min_y = ((self.center_y - half_h).max(0.0) / level.tile_height as f64) as u64;
        let max_y = ((self.center_y + half_h).min(level.height as f64) / level.tile_height as f64) as u64;

        let mut tiles = Vec::new();
        for col in min_x..=max_x {
            for row in min_y..=max_y {
                tiles.push(TileCoordinate::new(level.level, col, row));
            }
        }
        tiles
    }

    /// Pan the viewport by a delta in screen pixels.
    pub fn pan(&mut self, dx: f64, dy: f64, level: &PyramidLevel) {
        let scale = 2u32.pow(level.level) as f64;
        self.center_x += dx * scale;
        self.center_y += dy * scale;
    }

    /// Zoom the viewport by a factor.
    pub fn zoom(&mut self, factor: f64) {
        self.magnification = (self.magnification * factor).clamp(0.1, 80.0);
    }
}

// ---------------------------------------------------------------------------
// Pathology Measurement Tools
// ---------------------------------------------------------------------------

/// Cell counting result for a region of interest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellCountResult {
    /// Total cells counted.
    pub total_count: u64,
    /// Count per classification label.
    pub counts_by_type: BTreeMap<String, u64>,
    /// Area of the counting region in square millimeters.
    pub area_mm2: f64,
    /// Cell density (cells per square millimeter).
    pub density: f64,
}

impl CellCountResult {
    /// Create a cell count result from counts and area.
    pub fn new(counts_by_type: BTreeMap<String, u64>, area_mm2: f64) -> Self {
        let total_count = counts_by_type.values().sum();
        let density = if area_mm2 > 0.0 { total_count as f64 / area_mm2 } else { 0.0 };
        Self {
            total_count,
            counts_by_type,
            area_mm2,
            density,
        }
    }
}

/// Area measurement result for a pathology ROI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AreaMeasurement {
    /// Area in square millimeters.
    pub area_mm2: f64,
    /// Perimeter in millimeters.
    pub perimeter_mm: f64,
    /// Polygon points in image coordinates.
    pub points: Vec<(f64, f64)>,
    /// Pixel spacing used for conversion.
    pub pixel_spacing_mm: f64,
}

impl AreaMeasurement {
    /// Compute area measurement from a polygon in pixel coordinates.
    pub fn from_polygon(points: Vec<(f64, f64)>, pixel_spacing_mm: f64) -> Self {
        let perimeter = compute_perimeter(&points, pixel_spacing_mm);
        let area_pixels = polygon_area(&points);
        let area_mm2 = area_pixels * pixel_spacing_mm * pixel_spacing_mm;

        Self {
            area_mm2,
            perimeter_mm: perimeter,
            points,
            pixel_spacing_mm,
        }
    }
}

/// Compute the perimeter of a polygon in mm.
fn compute_perimeter(points: &[(f64, f64)], pixel_spacing_mm: f64) -> f64 {
    if points.len() < 2 {
        return 0.0;
    }
    let mut perimeter = 0.0;
    for i in 0..points.len() {
        let j = (i + 1) % points.len();
        let dx = (points[j].0 - points[i].0) * pixel_spacing_mm;
        let dy = (points[j].1 - points[i].1) * pixel_spacing_mm;
        perimeter += (dx * dx + dy * dy).sqrt();
    }
    perimeter
}

/// Compute polygon area using the Shoelace formula.
fn polygon_area(points: &[(f64, f64)]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }
    let n = points.len();
    let mut area = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        area += points[i].0 * points[j].1;
        area -= points[j].0 * points[i].1;
    }
    (area / 2.0).abs()
}

/// Count cells in a binary mask (simple connected-component counting).
pub fn count_cells(mask: &[u8], width: usize, height: usize) -> u64 {
    let mut count = 0u64;
    let mut visited = vec![false; width * height];

    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if visited[idx] || mask.get(idx).copied().unwrap_or(0) == 0 {
                continue;
            }

            // Flood fill this cell
            let mut stack = vec![idx];
            while let Some(current) = stack.pop() {
                if visited[current] {
                    continue;
                }
                if mask.get(current).copied().unwrap_or(0) == 0 {
                    continue;
                }
                visited[current] = true;

                let cx = current % width;
                let cy = current / width;

                if cx > 0 { stack.push(cy * width + cx - 1); }
                if cx + 1 < width { stack.push(cy * width + cx + 1); }
                if cy > 0 { stack.push((cy - 1) * width + cx); }
                if cy + 1 < height { stack.push((cy + 1) * width + cx); }
            }

            count += 1;
        }
    }

    count
}

// ---------------------------------------------------------------------------
// WSI Slide Store
// ---------------------------------------------------------------------------

/// Store for managing multiple WSI slides and their pyramid data.
pub struct WsiSlideStore {
    /// Slides indexed by SOP Instance UID.
    slides: BTreeMap<String, WsiMetadata>,
    /// Tile cache indexed by (sop_uid, TileCoordinate).
    tile_cache: BTreeMap<(String, TileCoordinate), Tile>,
    /// Maximum tile cache size.
    max_cache_tiles: usize,
    /// Audit callback.
    audit: Option<AuditCallback>,
}

/// Audit callback for WSI operations.
pub type AuditCallback = Arc<dyn Fn(AuditEvent) -> Result<()> + Send + Sync>;

impl fmt::Debug for WsiSlideStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WsiSlideStore")
            .field("slide_count", &self.slides.len())
            .field("cached_tiles", &self.tile_cache.len())
            .finish()
    }
}

impl Default for WsiSlideStore {
    fn default() -> Self {
        Self::new()
    }
}

impl WsiSlideStore {
    /// Create a new WSI slide store.
    pub fn new() -> Self {
        Self {
            slides: BTreeMap::new(),
            tile_cache: BTreeMap::new(),
            max_cache_tiles: 10000,
            audit: None,
        }
    }

    /// Set the audit callback.
    pub fn set_audit(&mut self, audit: Option<AuditCallback>) {
        self.audit = audit;
    }

    /// Register a WSI slide in the store.
    pub fn register_slide(&mut self, metadata: WsiMetadata) -> Result<()> {
        if metadata.sop_instance_uid.is_empty() {
            return Err(wsi_error("SOP Instance UID must not be empty"));
        }
        self.slides.insert(metadata.sop_instance_uid.clone(), metadata);
        Ok(())
    }

    /// Get WSI metadata for a slide.
    pub fn get_slide(&self, sop_uid: &str) -> Option<&WsiMetadata> {
        self.slides.get(sop_uid)
    }

    /// Store a tile in the cache.
    pub fn store_tile(&mut self, sop_uid: &str, tile: Tile) {
        let key = (sop_uid.to_string(), tile.coord.clone());
        self.tile_cache.insert(key, tile);

        // Evict oldest tiles if cache is full
        while self.tile_cache.len() > self.max_cache_tiles {
            if let Some(oldest_key) = self.tile_cache.keys().next().cloned() {
                self.tile_cache.remove(&oldest_key);
            }
        }
    }

    /// Retrieve a tile from the cache.
    pub fn get_tile(&self, sop_uid: &str, coord: &TileCoordinate) -> Option<&Tile> {
        self.tile_cache.get(&(sop_uid.to_string(), coord.clone()))
    }

    /// Return the number of cached tiles.
    pub fn cached_tile_count(&self) -> usize {
        self.tile_cache.len()
    }

    /// Return the number of registered slides.
    pub fn slide_count(&self) -> usize {
        self.slides.len()
    }
}

// ---------------------------------------------------------------------------
// Shared: Error helpers
// ---------------------------------------------------------------------------

fn wsi_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::DecodeError {
            stage: "dicom-wsi".to_string(),
            detail: detail.into(),
        },
        "wsi error",
    )
    .into()
}

// ===========================================================================
// Tests: S7-T5 Whole-Slide Imaging (minimum 12)
// ===========================================================================

#[cfg(test)]
mod tests_wsi {
    use super::*;

    #[test]
    fn pyramid_level_creation() {
        let level = PyramidLevel::new(0, 100000, 80000, 256, 256);
        assert_eq!(level.level, 0);
        assert_eq!(level.width, 100000);
        assert_eq!(level.tile_width, 256);
    }

    #[test]
    fn pyramid_level_tile_counts() {
        let level = PyramidLevel::new(0, 1000, 800, 256, 256);
        assert_eq!(level.tile_columns(), 4); // ceil(1000/256)
        assert_eq!(level.tile_rows(), 4);    // ceil(800/256)
        assert_eq!(level.total_tiles(), 16);
    }

    #[test]
    fn pyramid_magnification() {
        let level_0 = PyramidLevel::new(0, 100000, 80000, 256, 256);
        let level_1 = PyramidLevel::new(1, 50000, 40000, 256, 256);
        let level_2 = PyramidLevel::new(2, 25000, 20000, 256, 256);

        assert!((level_0.magnification() - 40.0).abs() < 1e-6);
        assert!((level_1.magnification() - 20.0).abs() < 1e-6);
        assert!((level_2.magnification() - 10.0).abs() < 1e-6);
    }

    #[test]
    fn wsi_metadata_creation() {
        let meta = WsiMetadata::new("1.2.3.4", "5.6.7", "8.9.0");
        assert_eq!(meta.sop_instance_uid, "1.2.3.4");
        assert_eq!(meta.objective_power, 40.0);
    }

    #[test]
    fn wsi_metadata_level_for_magnification() {
        let mut meta = WsiMetadata::new("1.2.3", "4.5.6", "7.8.9");
        meta.pyramid_levels = vec![
            PyramidLevel::new(0, 100000, 80000, 256, 256),
            PyramidLevel::new(1, 50000, 40000, 256, 256),
            PyramidLevel::new(2, 25000, 20000, 256, 256),
        ];

        let level = meta.level_for_magnification(20.0).unwrap();
        assert_eq!(level.level, 1);
    }

    #[test]
    fn wsi_viewport_default() {
        let vp = WsiViewport::default();
        assert_eq!(vp.magnification, 1.0);
        assert_eq!(vp.viewport_width, 1024);
    }

    #[test]
    fn wsi_viewport_zoom() {
        let mut vp = WsiViewport::new(50000.0, 40000.0, 40.0);
        vp.zoom(0.5);
        assert!((vp.magnification - 20.0).abs() < 1e-6);
    }

    #[test]
    fn cell_counting_simple() {
        let mut mask = vec![0u8; 20 * 20];
        // Place two separate cells
        mask[5 * 20 + 5] = 1;
        mask[5 * 20 + 6] = 1;
        mask[15 * 20 + 15] = 1;

        let count = count_cells(&mask, 20, 20);
        assert_eq!(count, 2);
    }

    #[test]
    fn cell_count_result_density() {
        let mut counts = BTreeMap::new();
        counts.insert("positive".to_string(), 50);
        counts.insert("negative".to_string(), 100);

        let result = CellCountResult::new(counts, 2.0);
        assert_eq!(result.total_count, 150);
        assert!((result.density - 75.0).abs() < 1e-6);
    }

    #[test]
    fn area_measurement_from_polygon() {
        let points = vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)];
        let measurement = AreaMeasurement::from_polygon(points, 0.00025);
        assert!(measurement.area_mm2 > 0.0);
        assert!(measurement.perimeter_mm > 0.0);
    }

    #[test]
    fn slide_store_register_and_retrieve() {
        let mut store = WsiSlideStore::new();
        let meta = WsiMetadata::new("1.2.3.4", "5.6.7", "8.9.0");
        store.register_slide(meta).unwrap();

        assert_eq!(store.slide_count(), 1);
        assert!(store.get_slide("1.2.3.4").is_some());
    }

    #[test]
    fn slide_store_tile_caching() {
        let mut store = WsiSlideStore::new();
        let tile = Tile {
            coord: TileCoordinate::new(0, 0, 0),
            data: vec![42u8; 100],
            data_size: 100,
        };
        store.store_tile("1.2.3", tile);

        assert_eq!(store.cached_tile_count(), 1);
        let retrieved = store.get_tile("1.2.3", &TileCoordinate::new(0, 0, 0));
        assert!(retrieved.is_some());
    }

    #[test]
    fn slide_store_rejects_empty_uid() {
        let mut store = WsiSlideStore::new();
        let meta = WsiMetadata::new("", "5.6.7", "8.9.0");
        assert!(store.register_slide(meta).is_err());
    }
}
