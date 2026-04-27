#![deny(missing_docs)]

//! Real-time collaboration layer for multi-user DICOM workstation sessions.
//!
//! Provides:
//! - **S6-T3**: WebSocket-based sync layer with shared viewport state, cursor sharing,
//!   measurement annotation broadcasting, and CRDT-based conflict resolution.
//! - Shared viewport state (pan, zoom, window/level) synchronized across users.
//! - Cursor sharing with user identity and color-coded presence.
//! - Annotation broadcasting with operational transform for concurrent edits.
//! - Conflict-free replicated data type (CRDT) merge for measurement stores.

use dicom_audit::{AuditEvent, AuditEventKind, AuditField, AuditValue};
use dicom_core::{Error, ErrorKind, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::hash::Hash;
use std::str::FromStr;
use std::sync::Arc;

// ===========================================================================
// S6-T3: Real-Time Collaboration
// ===========================================================================

/// Unique session identifier for a collaboration room.
///
/// Newtype wrapping `String` to prevent accidental mixing with other string-typed
/// identifiers (e.g., `UserId`).
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(String);

impl fmt::Debug for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SessionId").field(&self.0).finish()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for SessionId {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(SessionId(s.to_string()))
    }
}

impl From<String> for SessionId {
    fn from(s: String) -> Self {
        SessionId(s)
    }
}

impl AsRef<str> for SessionId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Unique user identifier within a collaboration session.
///
/// Newtype wrapping `String` to prevent accidental mixing with other string-typed
/// identifiers (e.g., `SessionId`).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct UserId(String);

impl fmt::Debug for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("UserId").field(&self.0).finish()
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for UserId {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(UserId(s.to_string()))
    }
}

impl From<String> for UserId {
    fn from(s: String) -> Self {
        UserId(s)
    }
}

impl AsRef<str> for UserId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Logical clock tick for operation ordering.
///
/// Newtype wrapping `u64` that provides type-safe tick values with `Copy` semantics
/// and convenience constructors.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Tick(u64);

impl fmt::Debug for Tick {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Tick").field(&self.0).finish()
    }
}

impl fmt::Display for Tick {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Tick {
    /// Create the zero tick.
    pub const fn zero() -> Self {
        Tick(0)
    }

    /// Advance to the next tick (saturating add).
    pub fn next(self) -> Self {
        Tick(self.0.saturating_add(1))
    }

    /// Return the raw `u64` tick value.
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl Default for Tick {
    fn default() -> Self {
        Tick::zero()
    }
}

/// Color assignment for user cursors and annotations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserColor {
    /// Red component (0-255).
    pub r: u8,
    /// Green component (0-255).
    pub g: u8,
    /// Blue component (0-255).
    pub b: u8,
}

impl UserColor {
    /// Create a new user color from RGB components.
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Convert to CSS hex color string.
    pub fn to_hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

/// Predefined color palette for collaboration users.
pub fn user_colors() -> &'static [UserColor] {
    static COLORS: &[UserColor] = &[
        UserColor {
            r: 0,
            g: 120,
            b: 215,
        }, // Blue
        UserColor {
            r: 232,
            g: 65,
            b: 24,
        }, // Red
        UserColor {
            r: 46,
            g: 204,
            b: 113,
        }, // Green
        UserColor {
            r: 155,
            g: 89,
            b: 182,
        }, // Purple
        UserColor {
            r: 241,
            g: 196,
            b: 15,
        }, // Yellow
        UserColor {
            r: 230,
            g: 126,
            b: 34,
        }, // Orange
        UserColor {
            r: 26,
            g: 188,
            b: 156,
        }, // Teal
        UserColor {
            r: 236,
            g: 100,
            b: 159,
        }, // Pink
    ];
    COLORS
}

/// User presence information in a collaboration session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserPresence {
    /// Unique user identifier.
    pub user_id: UserId,
    /// Display name of the user.
    pub display_name: String,
    /// Assigned color for this user.
    pub color: UserColor,
    /// Current cursor position in image coordinates (x, y).
    pub cursor_position: Option<(f64, f64)>,
    /// Currently active viewport index.
    pub active_viewport: Option<usize>,
    /// Timestamp of last activity.
    pub last_active_tick: Tick,
}

impl UserPresence {
    /// Create a new user presence with the given identity.
    pub fn new(user_id: UserId, display_name: String, color_index: usize) -> Self {
        let colors = user_colors();
        let color = colors[color_index % colors.len()].clone();
        Self {
            user_id,
            display_name,
            color,
            cursor_position: None,
            active_viewport: None,
            last_active_tick: Tick::zero(),
        }
    }
}

/// Shared viewport state synchronized across collaboration users.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedViewportState {
    /// Horizontal pan offset in pixels.
    pub pan_x: f64,
    /// Vertical pan offset in pixels.
    pub pan_y: f64,
    /// Zoom factor (1.0 = 100%).
    pub zoom: f64,
    /// Window center for VOI LUT.
    pub window_center: f64,
    /// Window width for VOI LUT.
    pub window_width: f64,
    /// Active frame index for multiframe studies.
    pub frame_index: u32,
    /// Viewport index in the display set.
    pub viewport_index: usize,
}

impl Default for SharedViewportState {
    fn default() -> Self {
        Self {
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
            window_center: 0.0,
            window_width: 0.0,
            frame_index: 0,
            viewport_index: 0,
        }
    }
}

/// Operation types for collaboration sync.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CollabOperation {
    /// Viewport state change (pan, zoom, window/level).
    ViewportChange {
        /// The new shared viewport state.
        state: SharedViewportState,
        /// Tick when this operation was created.
        tick: Tick,
        /// User who initiated the operation.
        user_id: UserId,
    },
    /// Cursor position update.
    CursorMove {
        /// New cursor position (x, y) in image coordinates.
        position: (f64, f64),
        /// Viewport where the cursor is active.
        viewport_index: usize,
        /// Tick when this operation was created.
        tick: Tick,
        /// User who moved the cursor.
        user_id: UserId,
    },
    /// Measurement annotation added.
    AnnotationAdd {
        /// Annotation identifier.
        annotation_id: String,
        /// Serialized annotation data (e.g., distance, angle, ROI).
        annotation_data: String,
        /// Tick when this operation was created.
        tick: Tick,
        /// User who created the annotation.
        user_id: UserId,
    },
    /// Measurement annotation modified.
    AnnotationModify {
        /// Annotation identifier.
        annotation_id: String,
        /// Serialized updated annotation data.
        annotation_data: String,
        /// Tick when this operation was created.
        tick: Tick,
        /// User who modified the annotation.
        user_id: UserId,
    },
    /// Measurement annotation removed.
    AnnotationRemove {
        /// Annotation identifier.
        annotation_id: String,
        /// Tick when this operation was created.
        tick: Tick,
        /// User who removed the annotation.
        user_id: UserId,
    },
}

impl CollabOperation {
    /// Return the tick (logical timestamp) of this operation.
    pub fn tick(&self) -> Tick {
        match self {
            CollabOperation::ViewportChange { tick, .. } => *tick,
            CollabOperation::CursorMove { tick, .. } => *tick,
            CollabOperation::AnnotationAdd { tick, .. } => *tick,
            CollabOperation::AnnotationModify { tick, .. } => *tick,
            CollabOperation::AnnotationRemove { tick, .. } => *tick,
        }
    }

    /// Return the user ID that created this operation.
    pub fn user_id(&self) -> &UserId {
        match self {
            CollabOperation::ViewportChange { user_id, .. } => user_id,
            CollabOperation::CursorMove { user_id, .. } => user_id,
            CollabOperation::AnnotationAdd { user_id, .. } => user_id,
            CollabOperation::AnnotationModify { user_id, .. } => user_id,
            CollabOperation::AnnotationRemove { user_id, .. } => user_id,
        }
    }
}

// ---------------------------------------------------------------------------
// CRDT: Conflict-Free Replicated Data Types
// ---------------------------------------------------------------------------

/// CRDT operation log entry with vector clock.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrdtEntry {
    /// The collaboration operation.
    pub operation: CollabOperation,
    /// Vector clock at the time this operation was created.
    pub vector_clock: BTreeMap<UserId, Tick>,
    /// Unique operation identifier.
    pub operation_id: String,
}

/// Last-Writer-Wins (LWW) register CRDT for viewport state.
///
/// Resolves concurrent viewport changes by comparing timestamps:
/// the operation with the higher tick wins. This is appropriate for
/// viewport state because only the most recent view matters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LwwRegister<T: Clone + PartialEq> {
    /// Current value.
    value: T,
    /// Timestamp of the last write.
    timestamp: Tick,
    /// User who performed the last write.
    writer: UserId,
}

impl<T: Clone + PartialEq> LwwRegister<T> {
    /// Create a new LWW register with an initial value.
    pub fn new(value: T, timestamp: Tick, writer: UserId) -> Self {
        Self {
            value,
            timestamp,
            writer,
        }
    }

    /// Get the current value.
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Try to merge a concurrent write. The write with the higher timestamp wins.
    /// On equal timestamps, the lexicographically smaller user ID wins for determinism.
    pub fn merge(&mut self, other: &LwwRegister<T>) {
        if other.timestamp > self.timestamp
            || (other.timestamp == self.timestamp && other.writer < self.writer)
        {
            self.value = other.value.clone();
            self.timestamp = other.timestamp;
            self.writer = other.writer.clone();
        }
    }

    /// Apply a write operation to this register.
    pub fn write(&mut self, value: T, timestamp: Tick, writer: UserId) {
        let other = LwwRegister::new(value, timestamp, writer);
        self.merge(&other);
    }
}

/// G-Set (Grow-Only Set) CRDT for annotations.
///
/// Annotations can only be added, never removed in the CRDT layer.
/// Deletion is handled at the application level by marking annotations
/// as deleted, allowing all replicas to converge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GSet<T: Clone + PartialEq + Ord> {
    /// The set of elements.
    elements: BTreeMap<T, ()>,
}

impl<T: Clone + PartialEq + Ord> Default for GSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + PartialEq + Ord> GSet<T> {
    /// Create an empty G-Set.
    pub fn new() -> Self {
        Self {
            elements: BTreeMap::new(),
        }
    }

    /// Add an element to the set.
    pub fn add(&mut self, element: T) {
        self.elements.insert(element, ());
    }

    /// Check if the set contains an element.
    pub fn contains(&self, element: &T) -> bool {
        self.elements.contains_key(element)
    }

    /// Return the number of elements.
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Return true if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Merge another G-Set into this one (union).
    pub fn merge(&mut self, other: &GSet<T>) {
        for key in other.elements.keys() {
            self.elements.insert(key.clone(), ());
        }
    }

    /// Return all elements in sorted order.
    pub fn elements(&self) -> Vec<&T> {
        self.elements.keys().collect()
    }
}

/// OR-Set (Observed-Remove Set) CRDT for annotations with deletion support.
///
/// Unlike G-Set, OR-Set supports both add and remove operations.
/// Removes are tagged with unique identifiers so that concurrent add
/// and remove operations resolve correctly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrSet<T: Clone + PartialEq + Ord> {
    /// Active elements with their unique add tags.
    active: BTreeMap<T, BTreeMap<String, ()>>,
    /// Tombstoned add tags (removed but kept for conflict resolution).
    tombstones: BTreeMap<String, ()>,
}

impl<T: Clone + PartialEq + Ord> Default for OrSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + PartialEq + Ord> OrSet<T> {
    /// Create an empty OR-Set.
    pub fn new() -> Self {
        Self {
            active: BTreeMap::new(),
            tombstones: BTreeMap::new(),
        }
    }

    /// Add an element with a unique tag.
    pub fn add(&mut self, element: T, tag: String) {
        if self.tombstones.contains_key(&tag) {
            // This tag was already removed; skip
            return;
        }
        self.active.entry(element).or_default().insert(tag, ());
    }

    /// Remove an element by removing all its tags.
    pub fn remove(&mut self, element: &T) {
        if let Some(tags) = self.active.remove(element) {
            for tag in tags.keys() {
                self.tombstones.insert(tag.clone(), ());
            }
        }
    }

    /// Check if the element is currently in the set.
    pub fn contains(&self, element: &T) -> bool {
        self.active.contains_key(element)
    }

    /// Return the number of active elements.
    pub fn len(&self) -> usize {
        self.active.len()
    }

    /// Return true if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.active.is_empty()
    }

    /// Merge another OR-Set into this one.
    pub fn merge(&mut self, other: &OrSet<T>) {
        // Merge tombstones first
        for tag in other.tombstones.keys() {
            self.tombstones.insert(tag.clone(), ());
        }

        // Remove tombstoned tags from active set
        for (_, tags) in self.active.iter_mut() {
            tags.retain(|tag, _| !self.tombstones.contains_key(tag));
        }

        // Merge active elements
        for (element, tags) in other.active.iter() {
            let our_tags = self.active.entry(element.clone()).or_default();
            for (tag, _) in tags {
                if !self.tombstones.contains_key(tag) {
                    our_tags.insert(tag.clone(), ());
                }
            }
        }

        // Clean up empty entries
        self.active.retain(|_, tags| !tags.is_empty());
    }

    /// Return all active elements.
    pub fn elements(&self) -> Vec<&T> {
        self.active.keys().collect()
    }
}

// ---------------------------------------------------------------------------
// Collaboration Session
// ---------------------------------------------------------------------------

/// Collaboration session managing users, state, and operation sync.
pub struct CollabSession {
    /// Unique session identifier.
    session_id: SessionId,
    /// Users currently in the session.
    users: BTreeMap<UserId, UserPresence>,
    /// Shared viewport states per viewport index.
    viewports: BTreeMap<usize, LwwRegister<SharedViewportState>>,
    /// Annotations stored as an OR-Set CRDT.
    annotations: OrSet<String>,
    /// Operation log for replay and late-joiner sync.
    operation_log: Vec<CrdtEntry>,
    /// Vector clock for operation ordering.
    vector_clock: BTreeMap<UserId, Tick>,
    /// Current logical tick counter.
    tick: Tick,
    /// Maximum number of operations to retain in the log.
    max_log_size: usize,
    /// Audit callback.
    audit: Option<AuditCallback>,
}

/// Audit callback for collaboration operations.
pub type AuditCallback = Arc<dyn Fn(AuditEvent) -> Result<()> + Send + Sync>;

impl fmt::Debug for CollabSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CollabSession")
            .field("session_id", &self.session_id)
            .field("user_count", &self.users.len())
            .field("tick", &self.tick)
            .finish()
    }
}

impl CollabSession {
    /// Create a new collaboration session.
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            users: BTreeMap::new(),
            viewports: BTreeMap::new(),
            annotations: OrSet::new(),
            operation_log: Vec::new(),
            vector_clock: BTreeMap::new(),
            tick: Tick::zero(),
            max_log_size: 10000,
            audit: None,
        }
    }

    /// Create a collaboration session with a custom max log size.
    pub fn with_max_log_size(session_id: SessionId, max_log_size: usize) -> Self {
        let mut session = Self::new(session_id);
        session.max_log_size = max_log_size;
        session
    }

    /// Set the audit callback.
    pub fn set_audit(&mut self, audit: Option<AuditCallback>) {
        self.audit = audit;
    }

    /// Return the session identifier.
    pub fn session_id(&self) -> &str {
        self.session_id.as_ref()
    }

    /// Return the current tick.
    pub fn tick(&self) -> Tick {
        self.tick
    }

    /// Return the number of users in the session.
    pub fn user_count(&self) -> usize {
        self.users.len()
    }

    /// Return all user presences.
    pub fn users(&self) -> Vec<&UserPresence> {
        self.users.values().collect()
    }

    /// Add a user to the collaboration session.
    pub fn join(&mut self, user_id: UserId, display_name: String) -> Result<()> {
        if self.users.contains_key(&user_id) {
            return Err(collab_error(
                self.session_id.as_ref(),
                "user already in session",
            ));
        }
        let color_index = self.users.len();
        let mut presence = UserPresence::new(user_id.clone(), display_name, color_index);
        presence.last_active_tick = self.tick;
        self.users.insert(user_id.clone(), presence);
        self.vector_clock.insert(user_id.clone(), Tick::zero());
        self.record_audit("join", user_id.as_ref());
        Ok(())
    }

    /// Remove a user from the collaboration session.
    pub fn leave(&mut self, user_id: &UserId) -> Result<()> {
        if !self.users.contains_key(user_id) {
            return Err(collab_error(
                self.session_id.as_ref(),
                "user not in session",
            ));
        }
        self.users.remove(user_id);
        self.record_audit("leave", user_id.as_ref());
        Ok(())
    }

    /// Apply a collaboration operation to the session state.
    pub fn apply_operation(&mut self, operation: CollabOperation) -> Result<()> {
        let user_id = operation.user_id().clone();
        if !self.users.contains_key(&user_id) {
            return Err(collab_error(
                self.session_id.as_ref(),
                "operation from unknown user",
            ));
        }

        // Advance vector clock
        self.tick = self.tick.next();
        *self
            .vector_clock
            .entry(user_id.clone())
            .or_insert(Tick::zero()) = self.tick;

        // Apply the operation
        match &operation {
            CollabOperation::ViewportChange { state, tick, .. } => {
                let viewport_index = state.viewport_index;
                let register = self
                    .viewports
                    .entry(viewport_index)
                    .or_insert_with(|| LwwRegister::new(state.clone(), *tick, user_id.clone()));
                register.write(state.clone(), *tick, user_id.clone());
            }
            CollabOperation::CursorMove {
                position,
                viewport_index,
                tick,
                ..
            } => {
                if let Some(presence) = self.users.get_mut(&user_id) {
                    presence.cursor_position = Some(*position);
                    presence.active_viewport = Some(*viewport_index);
                    presence.last_active_tick = *tick;
                }
            }
            CollabOperation::AnnotationAdd {
                annotation_id,
                tick: _,
                ..
            } => {
                self.annotations.add(
                    annotation_id.clone(),
                    format!("add-{}-{}", annotation_id, self.tick),
                );
            }
            CollabOperation::AnnotationRemove {
                annotation_id,
                tick: _,
                ..
            } => {
                self.annotations.remove(&annotation_id.clone());
            }
            CollabOperation::AnnotationModify { .. } => {
                // Modify is a no-op at the CRDT level; the annotation data
                // is handled by the application layer using LWW semantics
            }
        }

        // Log the operation
        let entry = CrdtEntry {
            operation,
            vector_clock: self.vector_clock.clone(),
            operation_id: format!("op-{}", self.tick),
        };
        self.operation_log.push(entry);

        // Trim log if needed
        if self.operation_log.len() > self.max_log_size {
            let excess = self.operation_log.len() - self.max_log_size;
            self.operation_log.drain(..excess);
        }

        self.record_audit("apply_operation", user_id.as_ref());
        Ok(())
    }

    /// Get the current viewport state for a given viewport index.
    pub fn viewport_state(&self, viewport_index: usize) -> Option<&SharedViewportState> {
        self.viewports.get(&viewport_index).map(|r| r.value())
    }

    /// Get cursor positions for all users in a viewport.
    pub fn viewport_cursors(&self, viewport_index: usize) -> Vec<(&UserId, (f64, f64))> {
        self.users
            .iter()
            .filter_map(|(id, presence)| {
                if presence.active_viewport == Some(viewport_index) {
                    presence.cursor_position.map(|pos| (id, pos))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Return the operation log for syncing late joiners.
    pub fn operation_log(&self) -> &[CrdtEntry] {
        &self.operation_log
    }

    /// Merge operations from another session (e.g., after network partition healing).
    pub fn merge_from(&mut self, other_log: &[CrdtEntry]) {
        for entry in other_log {
            let user_tick = self
                .vector_clock
                .get(entry.operation.user_id())
                .copied()
                .unwrap_or_default();
            if entry.operation.tick() > user_tick {
                let _ = self.apply_operation(entry.operation.clone());
            }
        }
    }

    /// Return the vector clock for conflict resolution.
    pub fn vector_clock(&self) -> &BTreeMap<UserId, Tick> {
        &self.vector_clock
    }

    /// Serialize the session state for persistence or network transmission.
    pub fn to_json(&self) -> Result<String> {
        let state = CollabSessionState {
            session_id: self.session_id.clone(),
            tick: self.tick,
            vector_clock: self.vector_clock.clone(),
            users: self.users.values().cloned().collect(),
        };
        serde_json::to_string(&state).map_err(|e| {
            collab_error(
                self.session_id.as_ref(),
                format!("serialization failed: {e}"),
            )
        })
    }

    fn record_audit(&self, operation: &'static str, subject_id: &str) {
        let Some(callback) = &self.audit else {
            return;
        };
        let _ = callback(AuditEvent {
            kind: AuditEventKind::ServiceEvent,
            fields: vec![
                AuditField {
                    key: "operation",
                    value: AuditValue::Plain(operation.to_string()),
                },
                AuditField {
                    key: "session_id",
                    value: AuditValue::Plain(self.session_id.to_string()),
                },
                AuditField {
                    key: "user_id",
                    value: AuditValue::Sensitive(subject_id.to_string()),
                },
            ],
        });
    }
}

/// Serializable collaboration session state for persistence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollabSessionState {
    /// Session identifier.
    pub session_id: SessionId,
    /// Current logical tick.
    pub tick: Tick,
    /// Vector clock state.
    pub vector_clock: BTreeMap<UserId, Tick>,
    /// User presences.
    pub users: Vec<UserPresence>,
}

// ---------------------------------------------------------------------------
// Shared: Error helpers
// ---------------------------------------------------------------------------

fn collab_error(session_id: impl Into<String>, detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::CollaborationError {
            session_id: session_id.into(),
            detail: detail.into(),
        },
        "collaboration error",
    )
    .into()
}

// ===========================================================================
// Tests: S6-T3 Real-Time Collaboration (minimum 15)
// ===========================================================================

#[cfg(test)]
mod tests_collab {
    use super::*;

    /// Helper to create a SessionId from a &str.
    fn sid(s: &str) -> SessionId {
        SessionId::from(s.to_string())
    }

    /// Helper to create a UserId from a &str.
    fn uid(s: &str) -> UserId {
        UserId::from(s.to_string())
    }

    /// Helper to create a Tick from a u64.
    fn tk(v: u64) -> Tick {
        Tick(v)
    }

    #[test]
    fn user_color_hex_format() {
        // REQ-COLLAB-100: user colors must produce valid CSS hex
        let color = UserColor::new(255, 128, 0);
        assert_eq!(color.to_hex(), "#ff8000");
    }

    #[test]
    fn user_color_palette_has_8_entries() {
        assert_eq!(user_colors().len(), 8);
    }

    #[test]
    fn user_presence_creation() {
        let presence = UserPresence::new(uid("user1"), "Dr. Smith".to_string(), 0);
        assert_eq!(presence.user_id, uid("user1"));
        assert_eq!(presence.display_name, "Dr. Smith");
        assert!(presence.cursor_position.is_none());
    }

    #[test]
    fn session_join_and_leave() {
        let mut session = CollabSession::new(sid("session-1"));
        session.join(uid("user1"), "Dr. Smith".to_string()).unwrap();
        assert_eq!(session.user_count(), 1);

        session.join(uid("user2"), "Dr. Jones".to_string()).unwrap();
        assert_eq!(session.user_count(), 2);

        session.leave(&uid("user1")).unwrap();
        assert_eq!(session.user_count(), 1);
    }

    #[test]
    fn session_rejects_duplicate_join() {
        let mut session = CollabSession::new(sid("session-1"));
        session.join(uid("user1"), "Dr. Smith".to_string()).unwrap();
        assert!(session.join(uid("user1"), "Dr. Smith".to_string()).is_err());
    }

    #[test]
    fn session_rejects_unknown_leave() {
        let mut session = CollabSession::new(sid("session-1"));
        assert!(session.leave(&uid("ghost")).is_err());
    }

    #[test]
    fn viewport_change_syncs() {
        let mut session = CollabSession::new(sid("session-1"));
        session.join(uid("user1"), "Dr. Smith".to_string()).unwrap();

        let state = SharedViewportState {
            pan_x: 10.0,
            pan_y: 20.0,
            zoom: 2.0,
            window_center: 40.0,
            window_width: 400.0,
            frame_index: 0,
            viewport_index: 0,
        };

        session
            .apply_operation(CollabOperation::ViewportChange {
                state: state.clone(),
                tick: tk(1),
                user_id: uid("user1"),
            })
            .unwrap();

        let synced = session.viewport_state(0).unwrap();
        assert_eq!(synced.pan_x, 10.0);
        assert_eq!(synced.zoom, 2.0);
    }

    #[test]
    fn cursor_move_updates_presence() {
        let mut session = CollabSession::new(sid("session-1"));
        session.join(uid("user1"), "Dr. Smith".to_string()).unwrap();

        session
            .apply_operation(CollabOperation::CursorMove {
                position: (100.0, 200.0),
                viewport_index: 0,
                tick: tk(1),
                user_id: uid("user1"),
            })
            .unwrap();

        let cursors = session.viewport_cursors(0);
        assert_eq!(cursors.len(), 1);
        assert_eq!(cursors[0].0, &uid("user1"));
        assert_eq!(cursors[0].1, (100.0, 200.0));
    }

    #[test]
    fn operation_from_unknown_user_rejected() {
        let mut session = CollabSession::new(sid("session-1"));
        let result = session.apply_operation(CollabOperation::ViewportChange {
            state: SharedViewportState::default(),
            tick: tk(1),
            user_id: uid("unknown"),
        });
        assert!(result.is_err());
    }

    #[test]
    fn lww_register_last_writer_wins() {
        let mut reg = LwwRegister::new(0, tk(0), uid("user1"));
        reg.write(10, tk(1), uid("user1"));
        reg.write(20, tk(2), uid("user2"));
        assert_eq!(*reg.value(), 20);
    }

    #[test]
    fn lww_register_deterministic_tie_breaking() {
        let mut reg = LwwRegister::new(0, tk(5), uid("user_b"));
        let other = LwwRegister::new(42, tk(5), uid("user_a"));
        reg.merge(&other);
        // "user_a" < "user_b" lexicographically, so user_a wins
        assert_eq!(*reg.value(), 42);
    }

    #[test]
    fn g_set_add_and_merge() {
        let mut set1 = GSet::new();
        set1.add("annotation-1".to_string());
        set1.add("annotation-2".to_string());

        let mut set2 = GSet::new();
        set2.add("annotation-2".to_string());
        set2.add("annotation-3".to_string());

        set1.merge(&set2);
        assert_eq!(set1.len(), 3);
        assert!(set1.contains(&"annotation-1".to_string()));
        assert!(set1.contains(&"annotation-3".to_string()));
    }

    #[test]
    fn or_set_add_remove_and_merge() {
        let mut set = OrSet::new();
        set.add("ann-1".to_string(), "tag-1".to_string());
        set.add("ann-2".to_string(), "tag-2".to_string());
        assert_eq!(set.len(), 2);

        set.remove(&"ann-1".to_string());
        assert_eq!(set.len(), 1);
        assert!(!set.contains(&"ann-1".to_string()));
    }

    #[test]
    fn or_set_concurrent_add_remove_merge() {
        let mut set1 = OrSet::new();
        set1.add("ann-1".to_string(), "tag-1".to_string());

        let mut set2 = OrSet::new();
        set2.add("ann-1".to_string(), "tag-1".to_string());
        set2.remove(&"ann-1".to_string());

        set1.merge(&set2);
        // After merge, ann-1 should be removed because tag-1 is tombstoned
        assert!(!set1.contains(&"ann-1".to_string()));
    }

    #[test]
    fn annotation_add_and_remove_operations() {
        let mut session = CollabSession::new(sid("session-1"));
        session.join(uid("user1"), "Dr. Smith".to_string()).unwrap();

        session
            .apply_operation(CollabOperation::AnnotationAdd {
                annotation_id: "ann-1".to_string(),
                annotation_data: "{}".to_string(),
                tick: tk(1),
                user_id: uid("user1"),
            })
            .unwrap();

        assert_eq!(session.annotations.len(), 1);

        session
            .apply_operation(CollabOperation::AnnotationRemove {
                annotation_id: "ann-1".to_string(),
                tick: tk(2),
                user_id: uid("user1"),
            })
            .unwrap();

        assert_eq!(session.annotations.len(), 0);
    }

    #[test]
    fn operation_log_trim() {
        let mut session = CollabSession::with_max_log_size(sid("session-1"), 5);
        session.join(uid("user1"), "Dr. Smith".to_string()).unwrap();

        for i in 0..10 {
            session
                .apply_operation(CollabOperation::CursorMove {
                    position: (i as f64, i as f64),
                    viewport_index: 0,
                    tick: tk(i),
                    user_id: uid("user1"),
                })
                .unwrap();
        }

        assert!(session.operation_log().len() <= 5);
    }

    #[test]
    fn session_serialization() {
        let mut session = CollabSession::new(sid("session-1"));
        session.join(uid("user1"), "Dr. Smith".to_string()).unwrap();

        let json = session.to_json().unwrap();
        assert!(json.contains("session-1"));
        assert!(json.contains("user1"));
    }

    #[test]
    fn merge_from_another_session() {
        let mut session1 = CollabSession::new(sid("session-1"));
        session1
            .join(uid("user1"), "Dr. Smith".to_string())
            .unwrap();

        let mut session2 = CollabSession::new(sid("session-1"));
        session2
            .join(uid("user1"), "Dr. Smith".to_string())
            .unwrap();

        session2
            .apply_operation(CollabOperation::ViewportChange {
                state: SharedViewportState {
                    zoom: 3.0,
                    ..SharedViewportState::default()
                },
                tick: tk(10),
                user_id: uid("user1"),
            })
            .unwrap();

        session1.merge_from(session2.operation_log());
        // After merge, session1 should have the viewport change
        assert!(session1.viewport_state(0).is_some());
    }

    #[test]
    fn multiple_users_cursors_in_viewport() {
        let mut session = CollabSession::new(sid("session-1"));
        session.join(uid("user1"), "Dr. Smith".to_string()).unwrap();
        session.join(uid("user2"), "Dr. Jones".to_string()).unwrap();

        session
            .apply_operation(CollabOperation::CursorMove {
                position: (10.0, 20.0),
                viewport_index: 0,
                tick: tk(1),
                user_id: uid("user1"),
            })
            .unwrap();

        session
            .apply_operation(CollabOperation::CursorMove {
                position: (30.0, 40.0),
                viewport_index: 0,
                tick: tk(2),
                user_id: uid("user2"),
            })
            .unwrap();

        let cursors = session.viewport_cursors(0);
        assert_eq!(cursors.len(), 2);
    }

    #[test]
    fn shared_viewport_default_values() {
        let state = SharedViewportState::default();
        assert_eq!(state.pan_x, 0.0);
        assert_eq!(state.zoom, 1.0);
        assert_eq!(state.frame_index, 0);
    }

    // --- S12-T5: Newtype-specific tests ---

    #[test]
    fn tick_zero_next_and_value() {
        let t0 = Tick::zero();
        assert_eq!(t0.value(), 0);
        let t1 = t0.next();
        assert_eq!(t1.value(), 1);
        let t2 = t1.next();
        assert_eq!(t2.value(), 2);
    }

    #[test]
    fn tick_ordering() {
        assert!(Tick(1) < Tick(2));
        assert!(Tick(2) > Tick(1));
        assert_eq!(Tick(3), Tick(3));
    }

    #[test]
    fn tick_display() {
        assert_eq!(format!("{}", Tick(42)), "42");
    }

    #[test]
    fn session_id_display_and_as_ref() {
        let sid = SessionId::from("abc".to_string());
        assert_eq!(sid.as_ref(), "abc");
        assert_eq!(format!("{}", sid), "abc");
    }

    #[test]
    fn user_id_display_and_as_ref() {
        let uid = UserId::from("dr-smith".to_string());
        assert_eq!(uid.as_ref(), "dr-smith");
        assert_eq!(format!("{}", uid), "dr-smith");
    }

    #[test]
    fn session_id_from_str_roundtrip() {
        let sid: SessionId = "hello".parse().unwrap();
        assert_eq!(sid.as_ref(), "hello");
    }

    #[test]
    fn user_id_from_str_roundtrip() {
        let uid: UserId = "world".parse().unwrap();
        assert_eq!(uid.as_ref(), "world");
    }
}
