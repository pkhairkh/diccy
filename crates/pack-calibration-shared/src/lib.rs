//! Shared calibration types for US/NM/XA modality packs.
//!
//! Eliminates character-for-character identical type definitions across
//! pack-us, pack-nm, and pack-xa.

/// Calibration source type (shared by US, NM, and XA modalities).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CalibrationSource {
    /// Internal calibration reference
    Internal,
    /// External calibration phantom
    External,
    /// No calibration performed
    None,
}

/// Measurement warning flags (9-variant superset from NM/XA).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MeasurementWarning {
    /// Values below measurable threshold
    BelowMeasurementThreshold,
    /// Values above measurable threshold
    AboveMeasurementThreshold,
    /// Multiple values present, using first
    MultipleValuesPresent,
    /// Measurement uncertainty exceeds threshold
    HighUncertainty,
    /// Calibration deviation detected
    CalibrationDeviation,
    /// Temperature out of range
    TemperatureOutOfRange,
    /// Dead time correction applied
    DeadTimeCorrectionApplied,
    /// Decay correction applied
    DecayCorrectionApplied,
    /// Partial view acquisition
    PartialViewAcquisition,
}

/// Epsilon for frame time comparisons.
pub const FRAME_TIME_EPS: f64 = 1e-6;

/// Extracted measurement context from a DICOM dataset.
#[derive(Debug, Clone)]
pub struct MeasurementContext {
    /// Frame time in seconds (for NM/XA modalities with cine).
    pub frame_time: Option<f64>,
    /// Calibration source for this measurement.
    pub calibration_source: CalibrationSource,
    /// Active warnings for this measurement.
    pub warnings: Vec<MeasurementWarning>,
}

impl MeasurementContext {
    /// Create a default measurement context.
    pub fn new() -> Self {
        Self {
            frame_time: None,
            calibration_source: CalibrationSource::None,
            warnings: Vec::new(),
        }
    }
}

impl Default for MeasurementContext {
    fn default() -> Self {
        Self::new()
    }
}
