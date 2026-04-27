#![deny(missing_docs)]

//! Shared value objects for the diccy workspace.
//!
//! This crate centralises value types that are consumed by multiple crates
//! (e.g. `dicom-core`, `dicom-pixel`, `viewer-core`) so that each crate can
//! depend on a single canonical definition instead of duplicating or
//! re-declaring its own copy.

/// Window/level selection for display rendering.
///
/// This is the canonical definition shared by the pixel pipeline and the
/// viewer core. Downstream crates should re-export from here rather than
/// defining their own equivalent type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowLevel {
    /// Explicit window center/width values.
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

impl WindowLevel {
    /// Create an explicit window/level from center and width.
    pub fn explicit(center: f64, width: f64) -> Self {
        Self::Explicit { center, width }
    }

    /// Create a VOI LUT selection.
    pub fn voi_lut(index: usize) -> Self {
        Self::VoiLut { index }
    }

    /// Return the auto variant.
    pub fn auto_window() -> Self {
        Self::Auto
    }

    /// Return the center value if this is an explicit window/level.
    pub fn center(&self) -> Option<f64> {
        match self {
            Self::Explicit { center, .. } => Some(*center),
            _ => None,
        }
    }

    /// Return the width value if this is an explicit window/level.
    pub fn width(&self) -> Option<f64> {
        match self {
            Self::Explicit { width, .. } => Some(*width),
            _ => None,
        }
    }

    /// Return true if this is the auto variant.
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

impl Default for WindowLevel {
    fn default() -> Self {
        Self::Auto
    }
}

/// Patient position as defined by DICOM tag (0018,5100).
///
/// Describes the patient orientation relative to the imaging equipment.
/// This is a shared value type used across modality packs, the viewer core,
/// and the pixel pipeline to avoid each crate defining its own enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PatientPosition {
    /// Head First - Prone.
    HFP,
    /// Head First - Supine.
    HFS,
    /// Head First - Decubitus Left.
    HFDL,
    /// Head First - Decubitus Right.
    HFDR,
    /// Feet First - Prone.
    FFP,
    /// Feet First - Supine.
    FFS,
    /// Feet First - Decubitus Left.
    FFDL,
    /// Feet First - Decubitus Right.
    FFDR,
}

impl PatientPosition {
    /// Parse a patient position from the DICOM CS string value.
    ///
    /// Returns `None` for unrecognised values.
    pub fn from_dicom_str(value: &str) -> Option<Self> {
        match value.trim() {
            "HFP" => Some(Self::HFP),
            "HFS" => Some(Self::HFS),
            "HFDL" => Some(Self::HFDL),
            "HFDR" => Some(Self::HFDR),
            "FFP" => Some(Self::FFP),
            "FFS" => Some(Self::FFS),
            "FFDL" => Some(Self::FFDL),
            "FFDR" => Some(Self::FFDR),
            _ => None,
        }
    }

    /// Return the standard DICOM CS string for this position.
    pub fn to_dicom_str(self) -> &'static str {
        match self {
            Self::HFP => "HFP",
            Self::HFS => "HFS",
            Self::HFDL => "HFDL",
            Self::HFDR => "HFDR",
            Self::FFP => "FFP",
            Self::FFS => "FFS",
            Self::FFDL => "FFDL",
            Self::FFDR => "FFDR",
        }
    }

    /// Return true if the patient is head-first.
    pub fn is_head_first(self) -> bool {
        matches!(self, Self::HFP | Self::HFS | Self::HFDL | Self::HFDR)
    }

    /// Return true if the patient is feet-first.
    pub fn is_feet_first(self) -> bool {
        matches!(self, Self::FFP | Self::FFS | Self::FFDL | Self::FFDR)
    }

    /// Return true if the patient is supine.
    pub fn is_supine(self) -> bool {
        matches!(self, Self::HFS | Self::FFS)
    }

    /// Return true if the patient is prone.
    pub fn is_prone(self) -> bool {
        matches!(self, Self::HFP | Self::FFP)
    }

    /// Return true if the patient is in a decubitus position.
    pub fn is_decubitus(self) -> bool {
        matches!(self, Self::HFDL | Self::HFDR | Self::FFDL | Self::FFDR)
    }
}
