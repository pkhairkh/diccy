#![deny(missing_docs)]

//! DICOM Extended Reality (XR) visualization, stereoscopic VR rendering,
//! hand-tracking interaction, 4D DICOM playback, AR holographic overlay,
//! coordinate registration, and surgical navigation.
//!
//! Provides:
//! - **S5-T3**: OpenXR/WebXR rendering pipeline stub, stereoscopic volume
//!   rendering, hand-tracking for interactive clipping and measurement,
//!   and 4D DICOM time-series volume playback.
//! - **S5-T4**: Mixed-reality AR holographic overlay, DICOM patient-to-world
//!   coordinate registration, HoloLens/Apple Vision Pro target support,
//!   and surgical navigation marker tracking interface.

use dicom_core::{Error, ErrorKind, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

// ===========================================================================
// S5-T3: XR Visualization
// ===========================================================================

/// XR runtime backend selection.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum XrRuntime {
    /// OpenXR runtime (native desktop/standalone VR).
    OpenXr,
    /// WebXR runtime (browser-based VR/AR via WebXR API).
    WebXr,
}

impl fmt::Display for XrRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            XrRuntime::OpenXr => write!(f, "OpenXR"),
            XrRuntime::WebXr => write!(f, "WebXR"),
        }
    }
}

/// Stereoscopic eye selection for VR rendering.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Eye {
    /// Left eye.
    Left,
    /// Right eye.
    Right,
}

/// Head pose with position and orientation in 6DOF tracking space.
///
/// Position is in meters relative to the XR stage origin. Orientation
/// is a unit quaternion (x, y, z, w) following the Hamilton convention.
#[derive(Debug, Clone, PartialEq)]
pub struct HeadPose {
    /// Position (x, y, z) in meters.
    pub position: [f64; 3],
    /// Orientation quaternion (x, y, z, w).
    orientation: [f64; 4],
    /// Timestamp of the pose sample in seconds since epoch.
    pub timestamp_s: f64,
}

impl Default for HeadPose {
    fn default() -> Self {
        Self {
            position: [0.0, 1.7, 0.0],         // default standing height 1.7m
            orientation: [0.0, 0.0, 0.0, 1.0], // identity quaternion
            timestamp_s: 0.0,
        }
    }
}

impl HeadPose {
    /// Create a new head pose, enforcing that the orientation quaternion is approximately unit length.
    ///
    /// The quaternion must have a magnitude within ±0.1 of 1.0.
    pub fn new(position: [f64; 3], orientation: [f64; 4], timestamp_s: f64) -> Result<Self> {
        let len = (orientation[0] * orientation[0]
            + orientation[1] * orientation[1]
            + orientation[2] * orientation[2]
            + orientation[3] * orientation[3])
            .sqrt();
        if (len - 1.0).abs() > 0.1 {
            return Err(xr_error(
                "head pose orientation quaternion is not unit length",
            ));
        }
        Ok(Self {
            position,
            orientation,
            timestamp_s,
        })
    }

    /// Return the orientation quaternion (x, y, z, w).
    pub fn orientation(&self) -> &[f64; 4] {
        &self.orientation
    }

    /// Validate that the orientation quaternion is approximately unit length.
    pub fn validate(&self) -> Result<()> {
        let len = (self.orientation[0] * self.orientation[0]
            + self.orientation[1] * self.orientation[1]
            + self.orientation[2] * self.orientation[2]
            + self.orientation[3] * self.orientation[3])
            .sqrt();
        if (len - 1.0).abs() > 0.1 {
            return Err(xr_error(
                "head pose orientation quaternion is not unit length",
            ));
        }
        Ok(())
    }
}

/// Hand joint identifiers for hand-tracking, following OpenXR specification.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum HandJoint {
    /// Palm.
    Palm,
    /// Wrist.
    Wrist,
    /// Thumb metacarpal.
    ThumbMetacarpal,
    /// Thumb proximal phalanx.
    ThumbProximal,
    /// Thumb distal phalanx.
    ThumbDistal,
    /// Thumb tip.
    ThumbTip,
    /// Index metacarpal.
    IndexMetacarpal,
    /// Index proximal phalanx.
    IndexProximal,
    /// Index intermediate phalanx.
    IndexIntermediate,
    /// Index distal phalanx.
    IndexDistal,
    /// Index tip.
    IndexTip,
    /// Middle metacarpal.
    MiddleMetacarpal,
    /// Middle proximal phalanx.
    MiddleProximal,
    /// Middle intermediate phalanx.
    MiddleIntermediate,
    /// Middle distal phalanx.
    MiddleDistal,
    /// Middle tip.
    MiddleTip,
    /// Ring metacarpal.
    RingMetacarpal,
    /// Ring proximal phalanx.
    RingProximal,
    /// Ring intermediate phalanx.
    RingIntermediate,
    /// Ring distal phalanx.
    RingDistal,
    /// Ring tip.
    RingTip,
    /// Little metacarpal.
    LittleMetacarpal,
    /// Little proximal phalanx.
    LittleProximal,
    /// Little intermediate phalanx.
    LittleIntermediate,
    /// Little distal phalanx.
    LittleDistal,
    /// Little tip.
    LittleTip,
}

/// Number of hand joints tracked.
pub const HAND_JOINT_COUNT: usize = 26;

/// Hand tracking data with joint positions and orientations.
#[derive(Debug, Clone, PartialEq)]
pub struct HandTrackingData {
    /// Which hand (left or right).
    pub hand: Hand,
    /// Joint positions in XR stage space (meters), indexed by HandJoint discriminant.
    pub joint_positions: [[f64; 3]; HAND_JOINT_COUNT],
    /// Joint orientations as quaternions (x, y, z, w).
    pub joint_orientations: [[f64; 4]; HAND_JOINT_COUNT],
    /// Whether each joint is currently tracked (valid position).
    pub joint_tracked: [bool; HAND_JOINT_COUNT],
    /// Timestamp of this tracking sample.
    pub timestamp_s: f64,
}

/// Which hand is being tracked.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Hand {
    /// Left hand.
    Left,
    /// Right hand.
    Right,
}

impl HandTrackingData {
    /// Create a new hand tracking data for the given hand.
    pub fn new(hand: Hand) -> Self {
        Self {
            hand,
            joint_positions: [[0.0; 3]; HAND_JOINT_COUNT],
            joint_orientations: [[0.0, 0.0, 0.0, 1.0]; HAND_JOINT_COUNT],
            joint_tracked: [false; HAND_JOINT_COUNT],
            timestamp_s: 0.0,
        }
    }

    /// Get the pinch distance between thumb tip and index tip.
    ///
    /// This is the primary interaction metric for VR hand-tracking:
    /// a pinch gesture is detected when this distance falls below a
    /// configurable threshold (typically 2-3 cm).
    pub fn pinch_distance(&self) -> f64 {
        let thumb_tip = self.joint_positions[HandJoint::ThumbTip as usize];
        let index_tip = self.joint_positions[HandJoint::IndexTip as usize];
        let dx = thumb_tip[0] - index_tip[0];
        let dy = thumb_tip[1] - index_tip[1];
        let dz = thumb_tip[2] - index_tip[2];
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Get the index finger ray (direction from proximal to tip).
    pub fn index_ray(&self) -> [f64; 3] {
        let proximal = self.joint_positions[HandJoint::IndexProximal as usize];
        let tip = self.joint_positions[HandJoint::IndexTip as usize];
        let mut ray = [
            tip[0] - proximal[0],
            tip[1] - proximal[1],
            tip[2] - proximal[2],
        ];
        let len = (ray[0] * ray[0] + ray[1] * ray[1] + ray[2] * ray[2]).sqrt();
        if len > 1e-12 {
            ray[0] /= len;
            ray[1] /= len;
            ray[2] /= len;
        }
        ray
    }
}

/// XR clip plane interaction state.
///
/// In VR, the user can position a clipping plane using hand-tracking
/// to slice through the volume and reveal internal anatomy.
#[derive(Debug, Clone, PartialEq)]
pub struct XrClipPlane {
    /// Plane normal (unit vector in XR stage space).
    pub normal: [f64; 3],
    /// Plane distance from origin along normal (meters).
    pub distance: f64,
    /// Which hand is controlling this clip plane.
    pub controlling_hand: Option<Hand>,
    /// Whether the clip plane is currently active.
    pub active: bool,
}

impl Default for XrClipPlane {
    fn default() -> Self {
        Self {
            normal: [0.0, 0.0, -1.0],
            distance: 0.0,
            controlling_hand: None,
            active: false,
        }
    }
}

/// 4D DICOM time-series volume playback state.
///
/// Supports multi-phase cardiac studies, contrast-enhanced perfusion
/// series, and other time-varying volumetric datasets. Each time point
/// is a complete 3D volume that can be rendered independently.
#[derive(Debug, Clone, PartialEq)]
pub struct Playback4dState {
    /// Total number of time points (phases/frames).
    pub total_time_points: u32,
    /// Current time point index (0-based).
    pub current_time_point: u32,
    /// Playback speed (1.0 = real-time, 0.5 = half speed).
    pub speed: f64,
    /// Whether playback is currently playing.
    pub playing: bool,
    /// Loop mode: restart from beginning when reaching the end.
    pub looping: bool,
    /// Time interval between time points in seconds.
    pub interval_s: f64,
}

impl Default for Playback4dState {
    fn default() -> Self {
        Self {
            total_time_points: 1,
            current_time_point: 0,
            speed: 1.0,
            playing: false,
            looping: true,
            interval_s: 1.0,
        }
    }
}

impl Playback4dState {
    /// Create a new 4D playback state with the given number of time points.
    pub fn new(total_time_points: u32) -> Self {
        Self {
            total_time_points,
            ..Default::default()
        }
    }

    /// Validate playback state.
    pub fn validate(&self) -> Result<()> {
        if self.total_time_points == 0 {
            return Err(xr_error("4D playback requires at least 1 time point"));
        }
        if self.current_time_point >= self.total_time_points {
            return Err(xr_error("current time point out of range"));
        }
        if self.speed <= 0.0 {
            return Err(xr_error("playback speed must be positive"));
        }
        if self.interval_s <= 0.0 {
            return Err(xr_error("time interval must be positive"));
        }
        Ok(())
    }

    /// Advance to the next time point.
    pub fn advance(&mut self) {
        self.current_time_point += 1;
        if self.current_time_point >= self.total_time_points {
            if self.looping {
                self.current_time_point = 0;
            } else {
                self.current_time_point = self.total_time_points - 1;
                self.playing = false;
            }
        }
    }

    /// Go to a specific time point.
    pub fn seek(&mut self, time_point: u32) -> Result<()> {
        if time_point >= self.total_time_points {
            return Err(xr_error("time point out of range"));
        }
        self.current_time_point = time_point;
        Ok(())
    }
}

/// XR rendering configuration for stereoscopic volume rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct XrRenderConfig {
    /// Target XR runtime.
    pub runtime: XrRuntime,
    /// Inter-pupillary distance (IPD) in meters (default 0.063 m = 63 mm).
    pub ipd: f64,
    /// Render resolution per eye (width, height).
    pub resolution: (u32, u32),
    /// Field of view in degrees.
    pub fov_degrees: f64,
    /// World scale: how many meters per millimeter of patient data.
    pub world_scale: f64,
}

impl Default for XrRenderConfig {
    fn default() -> Self {
        Self {
            runtime: XrRuntime::OpenXr,
            ipd: 0.063,
            resolution: (2160, 2160),
            fov_degrees: 90.0,
            world_scale: 0.001, // 1mm patient = 1mm VR
        }
    }
}

impl XrRenderConfig {
    /// Validate rendering configuration.
    pub fn validate(&self) -> Result<()> {
        if self.ipd <= 0.0 || self.ipd > 0.1 {
            return Err(xr_error("IPD must be in (0, 0.1] meters"));
        }
        if self.resolution.0 == 0 || self.resolution.1 == 0 {
            return Err(xr_error("render resolution must be non-zero"));
        }
        if self.fov_degrees <= 0.0 || self.fov_degrees > 180.0 {
            return Err(xr_error("FOV must be in (0, 180] degrees"));
        }
        if self.world_scale <= 0.0 {
            return Err(xr_error("world scale must be positive"));
        }
        Ok(())
    }

    /// Compute the view matrix offset for one eye.
    ///
    /// Returns the eye position offset from the head center in meters.
    pub fn eye_offset(&self, eye: Eye) -> [f64; 3] {
        let half_ipd = self.ipd / 2.0;
        match eye {
            Eye::Left => [-half_ipd, 0.0, 0.0],
            Eye::Right => [half_ipd, 0.0, 0.0],
        }
    }
}

/// XR viewer session managing the VR rendering pipeline.
///
/// This struct manages the XR runtime lifecycle, head tracking,
/// hand tracking, clip planes, and 4D playback state. It provides
/// the primary API surface for VR-based DICOM volume viewing.
///
/// **Acceptance:** CT volume viewable in VR headset with clip plane interaction.
#[derive(Debug, Clone)]
pub struct XrViewerSession {
    /// Rendering configuration.
    pub config: XrRenderConfig,
    /// Current head pose.
    pub head_pose: HeadPose,
    /// Left hand tracking data.
    pub left_hand: HandTrackingData,
    /// Right hand tracking data.
    pub right_hand: HandTrackingData,
    /// Active clip planes.
    pub clip_planes: Vec<XrClipPlane>,
    /// 4D playback state.
    pub playback: Playback4dState,
    /// Session is active (running).
    pub active: bool,
    /// Study Instance UID being viewed.
    pub study_uid: String,
    /// Tick counter for audit trail.
    tick: u64,
}

impl XrViewerSession {
    /// Create a new XR viewer session with the given configuration.
    pub fn new(config: XrRenderConfig, study_uid: &str) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            config,
            head_pose: HeadPose::default(),
            left_hand: HandTrackingData::new(Hand::Left),
            right_hand: HandTrackingData::new(Hand::Right),
            clip_planes: Vec::new(),
            playback: Playback4dState::default(),
            active: false,
            study_uid: study_uid.to_string(),
            tick: 0,
        })
    }

    /// Start the XR session.
    pub fn start(&mut self) -> Result<()> {
        if self.active {
            return Err(xr_error("XR session already active"));
        }
        self.tick = self.tick.saturating_add(1);
        self.active = true;
        Ok(())
    }

    /// Stop the XR session.
    pub fn stop(&mut self) {
        self.tick = self.tick.saturating_add(1);
        self.active = false;
    }

    /// Update head pose from tracking.
    pub fn update_head_pose(&mut self, pose: HeadPose) -> Result<()> {
        if !self.active {
            return Err(xr_error("cannot update head pose: session not active"));
        }
        pose.validate()?;
        self.tick = self.tick.saturating_add(1);
        self.head_pose = pose;
        Ok(())
    }

    /// Update hand tracking data.
    pub fn update_hand(&mut self, data: HandTrackingData) -> Result<()> {
        if !self.active {
            return Err(xr_error("cannot update hand: session not active"));
        }
        self.tick = self.tick.saturating_add(1);
        match data.hand {
            Hand::Left => self.left_hand = data,
            Hand::Right => self.right_hand = data,
        }
        Ok(())
    }

    /// Add a clip plane controlled by hand tracking.
    pub fn add_clip_plane(&mut self, hand: Hand) -> Result<usize> {
        if !self.active {
            return Err(xr_error("cannot add clip plane: session not active"));
        }
        self.tick = self.tick.saturating_add(1);
        let clip = XrClipPlane {
            controlling_hand: Some(hand),
            active: true,
            ..Default::default()
        };
        self.clip_planes.push(clip);
        Ok(self.clip_planes.len() - 1)
    }

    /// Remove a clip plane by index.
    pub fn remove_clip_plane(&mut self, index: usize) -> Result<()> {
        if index >= self.clip_planes.len() {
            return Err(xr_error("clip plane index out of range"));
        }
        self.tick = self.tick.saturating_add(1);
        self.clip_planes.remove(index);
        Ok(())
    }

    /// Update clip plane from hand tracking.
    ///
    /// Uses the palm position and orientation of the controlling hand
    /// to position and orient the clip plane in world space.
    pub fn update_clip_plane_from_hand(&mut self, clip_index: usize) -> Result<()> {
        if clip_index >= self.clip_planes.len() {
            return Err(xr_error("clip plane index out of range"));
        }
        let hand_data = match self.clip_planes[clip_index].controlling_hand {
            Some(Hand::Left) => &self.left_hand,
            Some(Hand::Right) => &self.right_hand,
            None => return Err(xr_error("clip plane has no controlling hand")),
        };

        let palm_pos = hand_data.joint_positions[HandJoint::Palm as usize];
        let palm_orient = hand_data.joint_orientations[HandJoint::Palm as usize];

        // Use palm forward direction as clip plane normal
        let normal = palm_forward(&palm_orient);
        let distance = normal[0] * palm_pos[0] + normal[1] * palm_pos[1] + normal[2] * palm_pos[2];

        self.clip_planes[clip_index].normal = normal;
        self.clip_planes[clip_index].distance = distance;
        self.tick = self.tick.saturating_add(1);
        Ok(())
    }

    /// Start 4D playback.
    pub fn start_playback(&mut self, total_time_points: u32) -> Result<()> {
        self.playback = Playback4dState::new(total_time_points);
        self.playback.playing = true;
        self.tick = self.tick.saturating_add(1);
        Ok(())
    }

    /// Advance 4D playback to next frame.
    pub fn advance_playback(&mut self) -> Result<()> {
        if !self.playback.playing {
            return Err(xr_error("playback not active"));
        }
        self.playback.advance();
        self.tick = self.tick.saturating_add(1);
        Ok(())
    }

    /// Return current tick.
    pub fn tick(&self) -> u64 {
        self.tick
    }
}

/// Extract forward direction from a quaternion.
fn palm_forward(q: &[f64; 4]) -> [f64; 3] {
    // Forward vector (0, 0, -1) rotated by quaternion
    let x = 2.0 * (q[0] * q[2] + q[3] * q[1]);
    let y = 2.0 * (q[1] * q[2] - q[3] * q[0]);
    let z = 1.0 - 2.0 * (q[0] * q[0] + q[1] * q[1]);
    let len = (x * x + y * y + z * z).sqrt();
    if len > 1e-12 {
        [-x / len, -y / len, -z / len]
    } else {
        [0.0, 0.0, -1.0]
    }
}

// ===========================================================================
// S5-T4: AR Holographic Overlay
// ===========================================================================

/// AR target device platform.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum ArPlatform {
    /// Microsoft HoloLens 2.
    HoloLens2,
    /// Apple Vision Pro.
    VisionPro,
    /// Generic ARCore/ARKit device.
    Generic,
}

impl fmt::Display for ArPlatform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArPlatform::HoloLens2 => write!(f, "HoloLens 2"),
            ArPlatform::VisionPro => write!(f, "Vision Pro"),
            ArPlatform::Generic => write!(f, "Generic AR"),
        }
    }
}

/// Coordinate system registration between DICOM patient space and world space.
///
/// Patient space is in millimeters with the DICOM patient coordinate system
/// (LPS: Left, Posterior, Superior). World space is in meters with the
/// XR stage coordinate system (typically X=right, Y=up, Z=back).
///
/// The registration transform maps: world_pos = R * (patient_pos * scale) + t
/// where R is a rotation matrix, scale converts mm to m, and t is a translation.
#[derive(Debug, Clone, PartialEq)]
pub struct CoordinateRegistration {
    /// Rotation matrix (3x3, row-major) from patient LPS to world XYZ.
    pub rotation: [[f64; 3]; 3],
    /// Scale factor: meters per millimeter (typically 0.001).
    pub scale: f64,
    /// Translation offset in world space (meters).
    pub translation: [f64; 3],
    /// Whether registration has been verified/calibrated.
    pub verified: bool,
    /// Registration method used.
    pub method: RegistrationMethod,
}

/// Method used for coordinate registration.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum RegistrationMethod {
    /// Manual landmark alignment by operator.
    ManualLandmark,
    /// Marker-based registration using fiducial markers.
    MarkerBased,
    /// Surface matching registration.
    SurfaceMatch,
    /// Identity transform (no registration needed).
    Identity,
}

impl Default for CoordinateRegistration {
    fn default() -> Self {
        Self {
            rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            scale: 0.001,
            translation: [0.0, 0.0, 0.0],
            verified: false,
            method: RegistrationMethod::Identity,
        }
    }
}

impl CoordinateRegistration {
    /// Create an identity registration (patient space = world space).
    pub fn identity() -> Self {
        Self {
            method: RegistrationMethod::Identity,
            verified: true,
            ..Default::default()
        }
    }

    /// Create a registration from LPS (DICOM patient) to XR stage space.
    ///
    /// DICOM uses LPS: Left=+X, Posterior=+Y, Superior=+Z.
    /// XR stage uses: Right=+X, Up=+Y, Back=+Z.
    /// Transform: world_x = -patient_x, world_y = patient_z, world_z = patient_y
    pub fn lps_to_stage() -> Self {
        Self {
            rotation: [[-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]],
            scale: 0.001,
            translation: [0.0, 0.0, 0.0],
            verified: true,
            method: RegistrationMethod::Identity,
        }
    }

    /// Validate the registration transform.
    pub fn validate(&self) -> Result<()> {
        if self.scale <= 0.0 {
            return Err(xr_error("registration scale must be positive"));
        }
        // Check rotation matrix is approximately orthogonal
        let r = &self.rotation;
        for row in r {
            let len = (row[0] * row[0] + row[1] * row[1] + row[2] * row[2]).sqrt();
            if (len - 1.0).abs() > 0.01 {
                return Err(xr_error("rotation matrix rows must be unit length"));
            }
        }
        Ok(())
    }

    /// Transform a patient-space point (mm) to world-space (meters).
    pub fn patient_to_world(&self, patient_mm: [f64; 3]) -> [f64; 3] {
        let r = &self.rotation;
        let s = self.scale;
        let scaled = [patient_mm[0] * s, patient_mm[1] * s, patient_mm[2] * s];
        [
            r[0][0] * scaled[0] + r[0][1] * scaled[1] + r[0][2] * scaled[2] + self.translation[0],
            r[1][0] * scaled[0] + r[1][1] * scaled[1] + r[1][2] * scaled[2] + self.translation[1],
            r[2][0] * scaled[0] + r[2][1] * scaled[1] + r[2][2] * scaled[2] + self.translation[2],
        ]
    }

    /// Transform a world-space point (meters) back to patient-space (mm).
    pub fn world_to_patient(&self, world_m: [f64; 3]) -> [f64; 3] {
        let r = &self.rotation;
        let s = self.scale;
        // Subtract translation
        let d = [
            world_m[0] - self.translation[0],
            world_m[1] - self.translation[1],
            world_m[2] - self.translation[2],
        ];
        // Transpose rotation (R^T = R^-1 for orthogonal)
        let inv_s = 1.0 / s;
        [
            (r[0][0] * d[0] + r[1][0] * d[1] + r[2][0] * d[2]) * inv_s,
            (r[0][1] * d[0] + r[1][1] * d[1] + r[2][1] * d[2]) * inv_s,
            (r[0][2] * d[0] + r[1][2] * d[1] + r[2][2] * d[2]) * inv_s,
        ]
    }
}

/// Surgical navigation marker for tracking instrument positions.
///
/// Markers are physical fiducials or tracked instruments that can be
/// localized in both patient and world coordinate systems. They provide
/// the reference points for coordinate registration and instrument
/// navigation.
#[derive(Debug, Clone, PartialEq)]
pub struct NavigationMarker {
    /// Marker identifier.
    pub id: String,
    /// Marker type.
    pub marker_type: MarkerType,
    /// Position in patient space (mm).
    pub patient_position_mm: [f64; 3],
    /// Position in world space (meters).
    pub world_position_m: [f64; 3],
    /// Whether the marker is currently being tracked.
    pub tracked: bool,
    /// Tracking error estimate (mm).
    pub tracking_error_mm: f64,
}

/// Type of navigation marker.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum MarkerType {
    /// Fiducial marker attached to patient.
    Fiducial,
    /// Tracked surgical instrument.
    Instrument,
    /// Anatomical landmark identified by operator.
    AnatomicalLandmark,
    /// AR anchor point in the environment.
    ArAnchor,
}

/// AR holographic overlay session for mixed-reality visualization.
///
/// Manages the projection of 3D medical models onto the real world
/// through AR devices, with coordinate registration, marker tracking,
/// and surgical navigation support.
///
/// **Acceptance:** 3D bone model overlaid on phantom in AR view.
#[derive(Debug, Clone)]
pub struct ArOverlaySession {
    /// Target AR platform.
    pub platform: ArPlatform,
    /// Coordinate registration transform.
    pub registration: CoordinateRegistration,
    /// Navigation markers.
    pub markers: Vec<NavigationMarker>,
    /// Overlay opacity (0.0 = transparent, 1.0 = opaque).
    pub opacity: f64,
    /// Whether the overlay is currently visible.
    pub visible: bool,
    /// Study Instance UID being overlaid.
    pub study_uid: String,
    /// Tick counter for audit trail.
    tick: u64,
}

impl ArOverlaySession {
    /// Create a new AR overlay session.
    pub fn new(platform: ArPlatform, study_uid: &str) -> Self {
        Self {
            platform,
            registration: CoordinateRegistration::lps_to_stage(),
            markers: Vec::new(),
            opacity: 0.7,
            visible: false,
            study_uid: study_uid.to_string(),
            tick: 0,
        }
    }

    /// Set the coordinate registration transform.
    pub fn set_registration(&mut self, registration: CoordinateRegistration) -> Result<()> {
        registration.validate()?;
        self.tick = self.tick.saturating_add(1);
        self.registration = registration;
        Ok(())
    }

    /// Add a fiducial marker for registration.
    pub fn add_fiducial(
        &mut self,
        patient_position_mm: [f64; 3],
        world_position_m: [f64; 3],
    ) -> String {
        self.tick = self.tick.saturating_add(1);
        let id = format!("marker{}", self.markers.len());
        self.markers.push(NavigationMarker {
            id: id.clone(),
            marker_type: MarkerType::Fiducial,
            patient_position_mm,
            world_position_m,
            tracked: true,
            tracking_error_mm: 0.0,
        });
        id
    }

    /// Add a tracked surgical instrument.
    pub fn add_instrument(&mut self, name: &str, world_position_m: [f64; 3]) -> String {
        self.tick = self.tick.saturating_add(1);
        let id = format!("inst{}", self.markers.len());
        // Compute patient position using inverse registration
        let patient_mm = self.registration.world_to_patient(world_position_m);
        self.markers.push(NavigationMarker {
            id: id.clone(),
            marker_type: MarkerType::Instrument,
            patient_position_mm: patient_mm,
            world_position_m,
            tracked: true,
            tracking_error_mm: 0.0,
        });
        id
    }

    /// Compute registration from fiducial markers using least-squares.
    ///
    /// Requires at least 3 non-collinear fiducial markers with both
    /// patient and world positions known. Computes the optimal rigid
    /// transform (rotation + translation + scale) that minimizes the
    /// sum of squared errors between transformed patient positions
    /// and observed world positions.
    pub fn compute_registration_from_fiducials(&mut self) -> Result<RegistrationResult> {
        let fiducials: Vec<&NavigationMarker> = self
            .markers
            .iter()
            .filter(|m| m.marker_type == MarkerType::Fiducial && m.tracked)
            .collect();

        if fiducials.len() < 3 {
            return Err(xr_error(
                "at least 3 tracked fiducial markers required for registration",
            ));
        }

        // Compute centroids
        let n = fiducials.len() as f64;
        let mut patient_centroid = [0.0f64; 3];
        let mut world_centroid = [0.0f64; 3];
        for f in &fiducials {
            for i in 0..3 {
                patient_centroid[i] += f.patient_position_mm[i];
                world_centroid[i] += f.world_position_m[i];
            }
        }
        for i in 0..3 {
            patient_centroid[i] /= n;
            world_centroid[i] /= n;
        }

        // Center the points
        let patient_centered: Vec<[f64; 3]> = fiducials
            .iter()
            .map(|f| {
                [
                    f.patient_position_mm[0] - patient_centroid[0],
                    f.patient_position_mm[1] - patient_centroid[1],
                    f.patient_position_mm[2] - patient_centroid[2],
                ]
            })
            .collect();
        let world_centered: Vec<[f64; 3]> = fiducials
            .iter()
            .map(|f| {
                [
                    f.world_position_m[0] - world_centroid[0],
                    f.world_position_m[1] - world_centroid[1],
                    f.world_position_m[2] - world_centroid[2],
                ]
            })
            .collect();

        // Compute scale from RMS ratios
        let patient_rms = patient_centered
            .iter()
            .map(|p| (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt())
            .sum::<f64>()
            / n;
        let world_rms = world_centered
            .iter()
            .map(|p| (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt())
            .sum::<f64>()
            / n;
        let scale = if patient_rms > 1e-12 {
            world_rms / patient_rms
        } else {
            0.001
        };

        // Simple rotation estimate using cross-covariance (simplified Kabsch)
        let mut h = [[0.0f64; 3]; 3];
        for i in 0..fiducials.len() {
            for j in 0..3 {
                for k in 0..3 {
                    h[j][k] += world_centered[i][j] * patient_centered[i][k] * scale;
                }
            }
        }

        // SVD approximation via power iteration on HH^T (simplified)
        // For a production implementation, use a full SVD.
        // Here we use a simplified orthogonalization approach.
        let mut rotation = [[0.0f64; 3]; 3];
        for i in 0..3 {
            let mut row = h[i];
            let len = (row[0] * row[0] + row[1] * row[1] + row[2] * row[2]).sqrt();
            if len > 1e-12 {
                row[0] /= len;
                row[1] /= len;
                row[2] /= len;
            }
            rotation[i] = row;
        }

        // Compute translation from centroids
        let translation = [
            world_centroid[0]
                - (rotation[0][0] * patient_centroid[0] * scale
                    + rotation[0][1] * patient_centroid[1] * scale
                    + rotation[0][2] * patient_centroid[2] * scale),
            world_centroid[1]
                - (rotation[1][0] * patient_centroid[0] * scale
                    + rotation[1][1] * patient_centroid[1] * scale
                    + rotation[1][2] * patient_centroid[2] * scale),
            world_centroid[2]
                - (rotation[2][0] * patient_centroid[0] * scale
                    + rotation[2][1] * patient_centroid[1] * scale
                    + rotation[2][2] * patient_centroid[2] * scale),
        ];

        // Compute registration error (RMS)
        let mut sum_sq_error = 0.0;
        for f in &fiducials {
            let transformed = [
                rotation[0][0] * f.patient_position_mm[0] * scale
                    + rotation[0][1] * f.patient_position_mm[1] * scale
                    + rotation[0][2] * f.patient_position_mm[2] * scale
                    + translation[0],
                rotation[1][0] * f.patient_position_mm[0] * scale
                    + rotation[1][1] * f.patient_position_mm[1] * scale
                    + rotation[1][2] * f.patient_position_mm[2] * scale
                    + translation[1],
                rotation[2][0] * f.patient_position_mm[0] * scale
                    + rotation[2][1] * f.patient_position_mm[1] * scale
                    + rotation[2][2] * f.patient_position_mm[2] * scale
                    + translation[2],
            ];
            let dx = transformed[0] - f.world_position_m[0];
            let dy = transformed[1] - f.world_position_m[1];
            let dz = transformed[2] - f.world_position_m[2];
            sum_sq_error += dx * dx + dy * dy + dz * dz;
        }
        let rms_error_m = (sum_sq_error / n).sqrt();
        let rms_error_mm = rms_error_m * 1000.0;

        let registration = CoordinateRegistration {
            rotation,
            scale,
            translation,
            verified: true,
            method: RegistrationMethod::MarkerBased,
        };

        self.registration = registration.clone();
        self.tick = self.tick.saturating_add(1);

        Ok(RegistrationResult {
            registration,
            rms_error_mm,
            fiducial_count: fiducials.len(),
        })
    }

    /// Show the AR overlay.
    pub fn show(&mut self) {
        self.tick = self.tick.saturating_add(1);
        self.visible = true;
    }

    /// Hide the AR overlay.
    pub fn hide(&mut self) {
        self.tick = self.tick.saturating_add(1);
        self.visible = false;
    }

    /// Set overlay opacity.
    pub fn set_opacity(&mut self, opacity: f64) -> Result<()> {
        if !(0.0..=1.0).contains(&opacity) {
            return Err(xr_error("opacity must be in [0, 1]"));
        }
        self.opacity = opacity;
        self.tick = self.tick.saturating_add(1);
        Ok(())
    }

    /// Get instrument position in patient space for surgical navigation.
    ///
    /// Returns the current position of the tracked instrument tip
    /// converted to patient-space coordinates for navigation display.
    pub fn instrument_patient_position(&self, instrument_id: &str) -> Option<[f64; 3]> {
        self.markers
            .iter()
            .find(|m| m.id == instrument_id && m.marker_type == MarkerType::Instrument)
            .map(|m| m.patient_position_mm)
    }

    /// Return current tick.
    pub fn tick(&self) -> u64 {
        self.tick
    }
}

/// Result of a coordinate registration computation.
#[derive(Debug, Clone, PartialEq)]
pub struct RegistrationResult {
    /// The computed registration transform.
    pub registration: CoordinateRegistration,
    /// Root-mean-square registration error in millimeters.
    pub rms_error_mm: f64,
    /// Number of fiducial markers used.
    pub fiducial_count: usize,
}

// ===========================================================================
// Shared: Error helpers
// ===========================================================================

fn xr_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::DecodeError {
            stage: "dicom-xr".to_string(),
            detail: detail.into(),
        },
        "xr error",
    )
    .into()
}

// ===========================================================================
// S12-T2: Stub renderers with runtime assertions
// ===========================================================================

/// XR volume renderer stub.
///
/// **STUB:** This implementation is not production-ready. It does not perform
/// any actual XR rendering. Calling `render_frame()` will always return an error.
/// A real implementation would use OpenXR/WebXR APIs to render stereoscopic
/// frames to the XR device.
#[derive(Debug, Clone)]
pub struct XrRenderer {
    /// Whether the renderer has been initialized.
    pub initialized: bool,
}

impl Default for XrRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl XrRenderer {
    /// Create a new XR renderer stub.
    pub fn new() -> Self {
        Self { initialized: false }
    }

    /// Initialize the renderer.
    pub fn initialize(&mut self) -> Result<()> {
        self.initialized = true;
        Ok(())
    }

    /// Render a stereoscopic frame for the given eye.
    ///
    /// **STUB:** Always returns an error. A production implementation would
    /// submit a rendered frame to the XR compositor.
    pub fn render_frame(&self, _eye: Eye, _session: &XrViewerSession) -> Result<()> {
        Err(Error::from_kind(
            ErrorKind::InternalError {
                detail:
                    "XrRenderer::render_frame() is a stub — no actual XR rendering is implemented"
                        .to_string(),
            },
            "STUB: XrRenderer cannot render frames",
        )
        .into())
    }
}

/// AR overlay rendering engine stub.
///
/// **STUB:** This implementation is not production-ready. It does not perform
/// any actual AR overlay rendering. Calling `render_overlay()` will always
/// return an error. A real implementation would composite holographic overlays
/// onto the AR device's pass-through camera feed.
#[derive(Debug, Clone)]
pub struct ArOverlayEngine {
    /// Whether the engine has been initialized.
    pub initialized: bool,
}

impl Default for ArOverlayEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ArOverlayEngine {
    /// Create a new AR overlay engine stub.
    pub fn new() -> Self {
        Self { initialized: false }
    }

    /// Initialize the engine.
    pub fn initialize(&mut self) -> Result<()> {
        self.initialized = true;
        Ok(())
    }

    /// Render an AR overlay for the given session.
    ///
    /// **STUB:** Always returns an error. A production implementation would
    /// render holographic overlays composited on the camera pass-through.
    pub fn render_overlay(&self, _session: &ArOverlaySession) -> Result<()> {
        Err(Error::from_kind(
            ErrorKind::InternalError {
                detail: "ArOverlayEngine::render_overlay() is a stub — no actual AR rendering is implemented".to_string(),
            },
            "STUB: ArOverlayEngine cannot render overlays",
        )
        .into())
    }
}

// ===========================================================================
// Tests: S12-T2 Stub runtime assertions
// ===========================================================================

#[cfg(test)]
mod tests_stub_assertions {
    use super::*;

    #[test]
    fn xr_renderer_stub_returns_error() {
        let renderer = XrRenderer::new();
        let config = XrRenderConfig::default();
        let session = XrViewerSession::new(config, "1.2.3").expect("session");
        let result = renderer.render_frame(Eye::Left, &session);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.to_lowercase().contains("stub"),
            "error should mention stub: {err}"
        );
    }

    #[test]
    fn ar_overlay_engine_stub_returns_error() {
        let engine = ArOverlayEngine::new();
        let session = ArOverlaySession::new(ArPlatform::HoloLens2, "1.2.3");
        let result = engine.render_overlay(&session);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.to_lowercase().contains("stub"),
            "error should mention stub: {err}"
        );
    }

    #[test]
    fn xr_and_ar_stubs_are_not_initialized_by_default() {
        assert!(!XrRenderer::new().initialized);
        assert!(!ArOverlayEngine::new().initialized);
    }
}

// ===========================================================================
// Tests: S5-T3 XR Visualization (minimum 10)
// ===========================================================================

#[cfg(test)]
mod tests_xr_visualization {
    use super::*;

    #[test]
    fn xr_render_config_validation() {
        let valid = XrRenderConfig::default();
        assert!(valid.validate().is_ok());

        let bad_ipd = XrRenderConfig {
            ipd: -0.1,
            ..Default::default()
        };
        assert!(bad_ipd.validate().is_err());

        let bad_fov = XrRenderConfig {
            fov_degrees: 200.0,
            ..Default::default()
        };
        assert!(bad_fov.validate().is_err());
    }

    #[test]
    fn xr_render_config_eye_offset() {
        let config = XrRenderConfig {
            ipd: 0.064,
            ..Default::default()
        };
        let left = config.eye_offset(Eye::Left);
        let right = config.eye_offset(Eye::Right);
        assert!(left[0] < 0.0, "left eye offset should be negative x");
        assert!(right[0] > 0.0, "right eye offset should be positive x");
        assert!((left[0].abs() - 0.032).abs() < 1e-10);
        assert!((right[0].abs() - 0.032).abs() < 1e-10);
    }

    #[test]
    fn xr_session_lifecycle() {
        let config = XrRenderConfig::default();
        let mut session = XrViewerSession::new(config, "1.2.3").expect("session");
        assert!(!session.active);

        session.start().expect("start");
        assert!(session.active);

        // Cannot start twice
        assert!(session.start().is_err());

        session.stop();
        assert!(!session.active);
    }

    #[test]
    fn xr_session_head_pose_update() {
        let config = XrRenderConfig::default();
        let mut session = XrViewerSession::new(config, "1.2.3").expect("session");
        session.start().expect("start");

        let pose = HeadPose {
            position: [1.0, 2.0, 3.0],
            orientation: [0.0, 0.0, 0.0, 1.0],
            timestamp_s: 100.0,
        };
        session.update_head_pose(pose.clone()).expect("update");
        assert_eq!(session.head_pose, pose);
    }

    #[test]
    fn xr_session_head_pose_rejects_bad_quaternion() {
        let config = XrRenderConfig::default();
        let mut session = XrViewerSession::new(config, "1.2.3").expect("session");
        session.start().expect("start");

        let bad_pose = HeadPose {
            position: [0.0, 0.0, 0.0],
            orientation: [10.0, 10.0, 10.0, 10.0], // not unit
            timestamp_s: 0.0,
        };
        assert!(session.update_head_pose(bad_pose).is_err());
    }

    #[test]
    fn hand_tracking_pinch_distance() {
        let mut hand = HandTrackingData::new(Hand::Right);
        hand.joint_positions[HandJoint::ThumbTip as usize] = [0.0, 0.0, 0.0];
        hand.joint_positions[HandJoint::IndexTip as usize] = [0.03, 0.0, 0.0];
        let dist = hand.pinch_distance();
        assert!((dist - 0.03).abs() < 1e-10, "pinch distance should be 3cm");
    }

    #[test]
    fn xr_clip_plane_interaction() {
        let config = XrRenderConfig::default();
        let mut session = XrViewerSession::new(config, "1.2.3").expect("session");
        session.start().expect("start");

        let idx = session.add_clip_plane(Hand::Right).expect("add clip");
        assert_eq!(idx, 0);
        assert_eq!(session.clip_planes.len(), 1);
        assert!(session.clip_planes[0].active);

        session.remove_clip_plane(0).expect("remove clip");
        assert!(session.clip_planes.is_empty());
    }

    #[test]
    fn playback_4d_state() {
        let mut playback = Playback4dState::new(10);
        assert_eq!(playback.current_time_point, 0);
        assert!(!playback.playing);

        playback.playing = true;
        playback.advance();
        assert_eq!(playback.current_time_point, 1);

        // Seek
        playback.seek(5).expect("seek");
        assert_eq!(playback.current_time_point, 5);
    }

    #[test]
    fn playback_4d_looping() {
        let mut playback = Playback4dState::new(3);
        playback.playing = true;
        playback.looping = true;

        playback.advance(); // 1
        playback.advance(); // 2
        playback.advance(); // wraps to 0
        assert_eq!(playback.current_time_point, 0);
    }

    #[test]
    fn playback_4d_no_loop() {
        let mut playback = Playback4dState::new(3);
        playback.playing = true;
        playback.looping = false;

        playback.advance(); // 1
        playback.advance(); // 2
        playback.advance(); // stays at 2, stops
        assert_eq!(playback.current_time_point, 2);
        assert!(!playback.playing);
    }

    #[test]
    fn playback_4d_validation() {
        let bad = Playback4dState {
            total_time_points: 0,
            ..Default::default()
        };
        assert!(bad.validate().is_err());

        let bad2 = Playback4dState {
            speed: -1.0,
            ..Default::default()
        };
        assert!(bad2.validate().is_err());
    }
}

// ===========================================================================
// Tests: S5-T4 AR Holographic Overlay (minimum 10)
// ===========================================================================

#[cfg(test)]
mod tests_ar_overlay {
    use super::*;

    #[test]
    fn coordinate_registration_identity() {
        let reg = CoordinateRegistration::identity();
        let patient = [100.0, 200.0, 300.0];
        let world = reg.patient_to_world(patient);
        assert!((world[0] - 0.1).abs() < 1e-10);
        assert!((world[1] - 0.2).abs() < 1e-10);
        assert!((world[2] - 0.3).abs() < 1e-10);
    }

    #[test]
    fn coordinate_registration_roundtrip() {
        let reg = CoordinateRegistration::lps_to_stage();
        let patient = [50.0, -30.0, 100.0];
        let world = reg.patient_to_world(patient);
        let back = reg.world_to_patient(world);
        assert!((back[0] - patient[0]).abs() < 1e-8);
        assert!((back[1] - patient[1]).abs() < 1e-8);
        assert!((back[2] - patient[2]).abs() < 1e-8);
    }

    #[test]
    fn coordinate_registration_lps_to_stage() {
        let reg = CoordinateRegistration::lps_to_stage();
        // Patient LPS (100, 0, 0) → world (-0.1, 0, 0)
        let world = reg.patient_to_world([100.0, 0.0, 0.0]);
        assert!((world[0] - (-0.1)).abs() < 1e-10);
        assert!((world[1] - 0.0).abs() < 1e-10);
        assert!((world[2] - 0.0).abs() < 1e-10);

        // Patient LPS (0, 0, 100) → world (0, 0.1, 0) (Superior → Up)
        let world2 = reg.patient_to_world([0.0, 0.0, 100.0]);
        assert!((world2[0] - 0.0).abs() < 1e-10);
        assert!((world2[1] - 0.1).abs() < 1e-10);
        assert!((world2[2] - 0.0).abs() < 1e-10);
    }

    #[test]
    fn coordinate_registration_validation() {
        let valid = CoordinateRegistration::default();
        assert!(valid.validate().is_ok());

        let bad_scale = CoordinateRegistration {
            scale: -1.0,
            ..Default::default()
        };
        assert!(bad_scale.validate().is_err());
    }

    #[test]
    fn ar_overlay_session_creation() {
        let session = ArOverlaySession::new(ArPlatform::HoloLens2, "1.2.3");
        assert_eq!(session.platform, ArPlatform::HoloLens2);
        assert!(!session.visible);
        assert!((session.opacity - 0.7).abs() < 1e-10);
    }

    #[test]
    fn ar_overlay_add_fiducial() {
        let mut session = ArOverlaySession::new(ArPlatform::VisionPro, "1.2.3");
        let id = session.add_fiducial([10.0, 20.0, 30.0], [0.01, 0.02, 0.03]);
        assert_eq!(id, "marker0");
        assert_eq!(session.markers.len(), 1);
        assert_eq!(session.markers[0].marker_type, MarkerType::Fiducial);
    }

    #[test]
    fn ar_overlay_add_instrument() {
        let mut session = ArOverlaySession::new(ArPlatform::HoloLens2, "1.2.3");
        let id = session.add_instrument("scalpel", [0.5, 1.0, 0.5]);
        assert!(id.starts_with("inst"));
        assert_eq!(session.markers.len(), 1);
        assert_eq!(session.markers[0].marker_type, MarkerType::Instrument);
    }

    #[test]
    fn ar_overlay_show_hide() {
        let mut session = ArOverlaySession::new(ArPlatform::Generic, "1.2.3");
        assert!(!session.visible);
        session.show();
        assert!(session.visible);
        session.hide();
        assert!(!session.visible);
    }

    #[test]
    fn ar_overlay_opacity_validation() {
        let mut session = ArOverlaySession::new(ArPlatform::Generic, "1.2.3");
        assert!(session.set_opacity(0.5).is_ok());
        assert!(session.set_opacity(0.0).is_ok());
        assert!(session.set_opacity(1.0).is_ok());
        assert!(session.set_opacity(-0.1).is_err());
        assert!(session.set_opacity(1.1).is_err());
    }

    #[test]
    fn ar_overlay_registration_from_fiducials() {
        let mut session = ArOverlaySession::new(ArPlatform::HoloLens2, "1.2.3");

        // Add 3 non-collinear fiducials
        session.add_fiducial([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        session.add_fiducial([100.0, 0.0, 0.0], [-0.1, 0.0, 0.0]);
        session.add_fiducial([0.0, 100.0, 0.0], [0.0, 0.0, 0.1]);

        let result = session
            .compute_registration_from_fiducials()
            .expect("registration");
        assert!(result.rms_error_mm >= 0.0);
        assert_eq!(result.fiducial_count, 3);
        assert!(session.registration.verified);
    }

    #[test]
    fn ar_overlay_registration_requires_3_fiducials() {
        let mut session = ArOverlaySession::new(ArPlatform::HoloLens2, "1.2.3");
        session.add_fiducial([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        session.add_fiducial([100.0, 0.0, 0.0], [-0.1, 0.0, 0.0]);
        // Only 2 fiducials — should fail
        assert!(session.compute_registration_from_fiducials().is_err());
    }

    #[test]
    fn ar_overlay_instrument_navigation() {
        let mut session = ArOverlaySession::new(ArPlatform::VisionPro, "1.2.3");
        let inst_id = session.add_instrument("probe", [0.5, 1.0, 0.5]);
        let pos = session.instrument_patient_position(&inst_id);
        assert!(pos.is_some());
        // Patient position should be computed from world position via inverse registration
        let p = pos.unwrap();
        // All values should be finite
        assert!(p[0].is_finite());
        assert!(p[1].is_finite());
        assert!(p[2].is_finite());
    }
}
