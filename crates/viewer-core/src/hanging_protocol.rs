//! DICOM Supplement 60 Hanging Protocol matching engine.
//!
//! Provides rule-based matching of study metadata to hanging protocol templates,
//! enabling automatic viewport arrangement based on modality, body part, laterality,
//! and study description patterns. Supports fallback protocol selection when no
//! primary match is found.

use std::collections::BTreeMap;

/// A matching rule criterion that compares against DICOM study metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchCriterion {
    /// Match against modality code (e.g., "CT", "MR", "MG").
    Modality {
        /// Expected modality code.
        code: String,
    },
    /// Match against body part examined (e.g., "CHEST", "BREAST", "ABDOMEN").
    BodyPart {
        /// Expected body part code.
        code: String,
    },
    /// Match against laterality (e.g., "L", "R", "U").
    Laterality {
        /// Expected laterality code.
        code: String,
    },
    /// Match study description against a case-insensitive substring pattern.
    StudyDescription {
        /// Substring pattern to match.
        pattern: String,
    },
    /// Match against a specific SOP Class UID.
    SopClass {
        /// Expected SOP Class UID.
        uid: String,
    },
}

/// A single image set definition within a hanging protocol.
/// Defines which images should be placed in a display set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageSetDefinition {
    /// Image set identifier within this protocol.
    pub set_id: String,
    /// Matching criteria for selecting images into this set.
    pub criteria: Vec<MatchCriterion>,
    /// Whether to select current or prior study images.
    pub time_perspective: TimePerspective,
}

/// Whether to select current or prior study images.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TimePerspective {
    /// Current (most recent) study.
    Current,
    /// Prior (previous) study.
    Prior,
}

/// A display set assignment mapping images to a viewport slot.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplaySetAssignment {
    /// Display set identifier.
    pub set_id: String,
    /// Image set to display in this slot.
    pub image_set_id: String,
    /// Viewport row position (0-indexed).
    pub row: u8,
    /// Viewport column position (0-indexed).
    pub column: u8,
    /// Optional window/level preset for this display set.
    pub window_center: Option<f64>,
    /// Optional window width preset for this display set.
    pub window_width: Option<f64>,
    /// Optional rotation in degrees (must be multiple of 90).
    pub rotation_degrees: Option<i32>,
    /// Whether to apply horizontal flip.
    pub flip_horizontal: bool,
    /// Whether to apply vertical flip.
    pub flip_vertical: bool,
}

/// A complete hanging protocol definition with matching rules and layout.
#[derive(Debug, Clone, PartialEq)]
pub struct HangingProtocol {
    /// Protocol identifier.
    pub protocol_id: String,
    /// Human-readable protocol name.
    pub name: String,
    /// Protocol priority (lower = higher priority). Used for disambiguation
    /// when multiple protocols match.
    pub priority: u32,
    /// Matching criteria that must ALL be satisfied (logical AND).
    pub match_criteria: Vec<MatchCriterion>,
    /// Image set definitions.
    pub image_sets: Vec<ImageSetDefinition>,
    /// Display set assignments mapping image sets to viewport slots.
    pub display_sets: Vec<DisplaySetAssignment>,
    /// Total grid rows.
    pub rows: u8,
    /// Total grid columns.
    pub columns: u8,
    /// Whether this protocol is a fallback (applied when no primary matches).
    pub is_fallback: bool,
}

/// Study metadata used for protocol matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudyMatchContext {
    /// Study modality codes (may be multi-modality).
    pub modalities: Vec<String>,
    /// Body part examined code.
    pub body_part: Option<String>,
    /// Laterality code.
    pub laterality: Option<String>,
    /// Study description.
    pub study_description: Option<String>,
    /// SOP Class UIDs present in the study.
    pub sop_classes: Vec<String>,
    /// Number of series in the study.
    pub series_count: usize,
    /// Number of prior studies available.
    pub prior_count: usize,
}

/// Result of applying a hanging protocol to a study context.
#[derive(Debug, Clone, PartialEq)]
pub struct HangingProtocolMatch {
    /// The matched protocol identifier.
    pub protocol_id: String,
    /// The matched protocol name.
    pub protocol_name: String,
    /// Grid layout row count.
    pub rows: u8,
    /// Grid layout column count.
    pub columns: u8,
    /// Display set assignments resolved for this study.
    pub display_sets: Vec<DisplaySetAssignment>,
    /// Whether this was a fallback match.
    pub used_fallback: bool,
    /// Match score (higher = better match, based on criteria specificity).
    pub match_score: u32,
}

/// Error type for hanging protocol operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HangingProtocolError {
    /// No protocol matched the given study context.
    NoMatch {
        /// Brief description of what was attempted.
        detail: String,
    },
    /// Protocol definition is invalid or inconsistent.
    InvalidProtocol {
        /// Protocol identifier that is invalid.
        protocol_id: String,
        /// Validation failure detail.
        detail: String,
    },
    /// Display set references a non-existent image set.
    MissingImageSet {
        /// Referenced image set identifier.
        image_set_id: String,
    },
}

impl std::fmt::Display for HangingProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HangingProtocolError::NoMatch { detail } => {
                write!(f, "no hanging protocol matched: {detail}")
            }
            HangingProtocolError::InvalidProtocol {
                protocol_id,
                detail,
            } => {
                write!(f, "invalid protocol '{protocol_id}': {detail}")
            }
            HangingProtocolError::MissingImageSet { image_set_id } => {
                write!(f, "missing image set: {image_set_id}")
            }
        }
    }
}

impl std::error::Error for HangingProtocolError {}

/// Hanging protocol registry and matching engine.
#[derive(Debug, Clone)]
pub struct HangingProtocolEngine {
    /// Registered protocols, keyed by protocol_id.
    protocols: BTreeMap<String, HangingProtocol>,
}

impl HangingProtocolEngine {
    /// Create an empty protocol engine.
    pub fn new() -> Self {
        Self {
            protocols: BTreeMap::new(),
        }
    }

    /// Register a hanging protocol.
    ///
    /// Returns an error if the protocol definition is invalid.
    pub fn register(&mut self, protocol: HangingProtocol) -> Result<(), HangingProtocolError> {
        self.validate_protocol(&protocol)?;
        self.protocols
            .insert(protocol.protocol_id.clone(), protocol);
        Ok(())
    }

    /// Remove a protocol by identifier.
    pub fn unregister(&mut self, protocol_id: &str) -> bool {
        self.protocols.remove(protocol_id).is_some()
    }

    /// Return the number of registered protocols.
    pub fn len(&self) -> usize {
        self.protocols.len()
    }

    /// Return true when no protocols are registered.
    pub fn is_empty(&self) -> bool {
        self.protocols.is_empty()
    }

    /// Match a study context against registered protocols and return the best match.
    ///
    /// Matching logic:
    /// 1. Filter protocols whose match_criteria all evaluate to true for the study context.
    /// 2. Among matching protocols, select the one with the highest match score
    ///    (computed from criteria specificity), breaking ties by priority (lower = better).
    /// 3. If no primary match, select the first fallback protocol.
    /// 4. If no fallback exists, return `NoMatch` error.
    pub fn match_protocol(
        &self,
        context: &StudyMatchContext,
    ) -> Result<HangingProtocolMatch, HangingProtocolError> {
        // Phase 1: Collect all primary matches with scores.
        let mut matches: Vec<&HangingProtocol> = Vec::new();
        let mut best_score: u32 = 0;

        for protocol in self.protocols.values() {
            if protocol.is_fallback {
                continue;
            }
            if !self.evaluate_criteria(&protocol.match_criteria, context) {
                continue;
            }
            let score = self.compute_match_score(protocol, context);
            if score > best_score {
                best_score = score;
                matches.clear();
                matches.push(protocol);
            } else if score == best_score {
                matches.push(protocol);
            }
        }

        // Phase 2: Among equal-score matches, pick lowest priority.
        if let Some(best) = matches.iter().min_by_key(|p| p.priority) {
            return Ok(HangingProtocolMatch {
                protocol_id: best.protocol_id.clone(),
                protocol_name: best.name.clone(),
                rows: best.rows,
                columns: best.columns,
                display_sets: best.display_sets.clone(),
                used_fallback: false,
                match_score: best_score,
            });
        }

        // Phase 3: Fallback.
        let fallback = self
            .protocols
            .values()
            .filter(|p| p.is_fallback)
            .min_by_key(|p| p.priority);

        match fallback {
            Some(fb) => Ok(HangingProtocolMatch {
                protocol_id: fb.protocol_id.clone(),
                protocol_name: fb.name.clone(),
                rows: fb.rows,
                columns: fb.columns,
                display_sets: fb.display_sets.clone(),
                used_fallback: true,
                match_score: 0,
            }),
            None => Err(HangingProtocolError::NoMatch {
                detail: format!(
                    "no protocol matches modality={:?}, body_part={:?}",
                    context.modalities, context.body_part
                ),
            }),
        }
    }

    /// Evaluate all criteria against a study context (logical AND).
    fn evaluate_criteria(&self, criteria: &[MatchCriterion], context: &StudyMatchContext) -> bool {
        for criterion in criteria {
            if !self.evaluate_criterion(criterion, context) {
                return false;
            }
        }
        true
    }

    /// Evaluate a single match criterion against a study context.
    fn evaluate_criterion(&self, criterion: &MatchCriterion, context: &StudyMatchContext) -> bool {
        match criterion {
            MatchCriterion::Modality { code } => context
                .modalities
                .iter()
                .any(|m| m.eq_ignore_ascii_case(code)),
            MatchCriterion::BodyPart { code } => context
                .body_part
                .as_ref()
                .map(|bp| bp.eq_ignore_ascii_case(code))
                .unwrap_or(false),
            MatchCriterion::Laterality { code } => context
                .laterality
                .as_ref()
                .map(|lat| lat.eq_ignore_ascii_case(code))
                .unwrap_or(false),
            MatchCriterion::StudyDescription { pattern } => context
                .study_description
                .as_ref()
                .map(|desc| desc.to_lowercase().contains(&pattern.to_lowercase()))
                .unwrap_or(false),
            MatchCriterion::SopClass { uid } => context
                .sop_classes
                .iter()
                .any(|s| s.eq_ignore_ascii_case(uid)),
        }
    }

    /// Compute a match score based on how specific the criteria are.
    ///
    /// Scoring heuristics:
    /// - Each criterion adds a base score of 10.
    /// - Modality match adds +5.
    /// - Body part match adds +5.
    /// - Laterality match adds +3.
    /// - Study description match adds +2.
    /// - SOP class match adds +5.
    fn compute_match_score(&self, protocol: &HangingProtocol, _context: &StudyMatchContext) -> u32 {
        let mut score = 0u32;
        for criterion in &protocol.match_criteria {
            score += 10; // base score per criterion
            match criterion {
                MatchCriterion::Modality { .. } => score += 5,
                MatchCriterion::BodyPart { .. } => score += 5,
                MatchCriterion::Laterality { .. } => score += 3,
                MatchCriterion::StudyDescription { .. } => score += 2,
                MatchCriterion::SopClass { .. } => score += 5,
            }
        }
        // Bonus for having multiple image sets (indicates richer protocol).
        score += (protocol.image_sets.len().min(4) as u32) * 2;
        score
    }

    /// Validate a protocol definition for internal consistency.
    fn validate_protocol(&self, protocol: &HangingProtocol) -> Result<(), HangingProtocolError> {
        if protocol.protocol_id.is_empty() {
            return Err(HangingProtocolError::InvalidProtocol {
                protocol_id: String::new(),
                detail: "protocol_id must not be empty".to_string(),
            });
        }
        if protocol.rows == 0 || protocol.columns == 0 {
            return Err(HangingProtocolError::InvalidProtocol {
                protocol_id: protocol.protocol_id.clone(),
                detail: "rows and columns must be > 0".to_string(),
            });
        }
        // Verify display set image_set_id references are valid.
        let image_set_ids: Vec<&str> = protocol
            .image_sets
            .iter()
            .map(|is| is.set_id.as_str())
            .collect();
        for ds in &protocol.display_sets {
            if !image_set_ids.contains(&ds.image_set_id.as_str()) {
                return Err(HangingProtocolError::MissingImageSet {
                    image_set_id: ds.image_set_id.clone(),
                });
            }
            if ds.row >= protocol.rows {
                return Err(HangingProtocolError::InvalidProtocol {
                    protocol_id: protocol.protocol_id.clone(),
                    detail: format!(
                        "display set '{}' row {} exceeds grid rows {}",
                        ds.set_id, ds.row, protocol.rows
                    ),
                });
            }
            if ds.column >= protocol.columns {
                return Err(HangingProtocolError::InvalidProtocol {
                    protocol_id: protocol.protocol_id.clone(),
                    detail: format!(
                        "display set '{}' column {} exceeds grid columns {}",
                        ds.set_id, ds.column, protocol.columns
                    ),
                });
            }
        }
        Ok(())
    }
}

impl Default for HangingProtocolEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a built-in mammography hanging protocol (CC/MLO, current/prior).
///
/// This implements the standard four-view mammography layout:
/// - Top-left: Left CC current
/// - Top-right: Right CC current
/// - Bottom-left: Left MLO current
/// - Bottom-right: Right MLO current
pub fn mammography_protocol() -> HangingProtocol {
    HangingProtocol {
        protocol_id: "builtin.mammography_4view".to_string(),
        name: "Mammography 4-View (CC/MLO)".to_string(),
        priority: 10,
        match_criteria: vec![MatchCriterion::Modality {
            code: "MG".to_string(),
        }],
        image_sets: vec![
            ImageSetDefinition {
                set_id: "l_cc".to_string(),
                criteria: vec![
                    MatchCriterion::Laterality {
                        code: "L".to_string(),
                    },
                    MatchCriterion::StudyDescription {
                        pattern: "CC".to_string(),
                    },
                ],
                time_perspective: TimePerspective::Current,
            },
            ImageSetDefinition {
                set_id: "r_cc".to_string(),
                criteria: vec![
                    MatchCriterion::Laterality {
                        code: "R".to_string(),
                    },
                    MatchCriterion::StudyDescription {
                        pattern: "CC".to_string(),
                    },
                ],
                time_perspective: TimePerspective::Current,
            },
            ImageSetDefinition {
                set_id: "l_mlo".to_string(),
                criteria: vec![
                    MatchCriterion::Laterality {
                        code: "L".to_string(),
                    },
                    MatchCriterion::StudyDescription {
                        pattern: "MLO".to_string(),
                    },
                ],
                time_perspective: TimePerspective::Current,
            },
            ImageSetDefinition {
                set_id: "r_mlo".to_string(),
                criteria: vec![
                    MatchCriterion::Laterality {
                        code: "R".to_string(),
                    },
                    MatchCriterion::StudyDescription {
                        pattern: "MLO".to_string(),
                    },
                ],
                time_perspective: TimePerspective::Current,
            },
        ],
        display_sets: vec![
            DisplaySetAssignment {
                set_id: "ds_l_cc".to_string(),
                image_set_id: "l_cc".to_string(),
                row: 0,
                column: 0,
                window_center: None,
                window_width: None,
                rotation_degrees: None,
                flip_horizontal: false,
                flip_vertical: false,
            },
            DisplaySetAssignment {
                set_id: "ds_r_cc".to_string(),
                image_set_id: "r_cc".to_string(),
                row: 0,
                column: 1,
                window_center: None,
                window_width: None,
                rotation_degrees: None,
                flip_horizontal: true,
                flip_vertical: false,
            },
            DisplaySetAssignment {
                set_id: "ds_l_mlo".to_string(),
                image_set_id: "l_mlo".to_string(),
                row: 1,
                column: 0,
                window_center: None,
                window_width: None,
                rotation_degrees: None,
                flip_horizontal: false,
                flip_vertical: false,
            },
            DisplaySetAssignment {
                set_id: "ds_r_mlo".to_string(),
                image_set_id: "r_mlo".to_string(),
                row: 1,
                column: 1,
                window_center: None,
                window_width: None,
                rotation_degrees: None,
                flip_horizontal: true,
                flip_vertical: false,
            },
        ],
        rows: 2,
        columns: 2,
        is_fallback: false,
    }
}

/// Create a built-in CT chest/abdomen hanging protocol.
pub fn ct_chest_abdomen_protocol() -> HangingProtocol {
    HangingProtocol {
        protocol_id: "builtin.ct_chest_abdomen".to_string(),
        name: "CT Chest/Abdomen".to_string(),
        priority: 20,
        match_criteria: vec![
            MatchCriterion::Modality {
                code: "CT".to_string(),
            },
            MatchCriterion::BodyPart {
                code: "CHEST".to_string(),
            },
        ],
        image_sets: vec![
            ImageSetDefinition {
                set_id: "lung_windows".to_string(),
                criteria: vec![MatchCriterion::StudyDescription {
                    pattern: "lung".to_string(),
                }],
                time_perspective: TimePerspective::Current,
            },
            ImageSetDefinition {
                set_id: "soft_windows".to_string(),
                criteria: vec![MatchCriterion::StudyDescription {
                    pattern: "soft".to_string(),
                }],
                time_perspective: TimePerspective::Current,
            },
        ],
        display_sets: vec![
            DisplaySetAssignment {
                set_id: "ds_lung".to_string(),
                image_set_id: "lung_windows".to_string(),
                row: 0,
                column: 0,
                window_center: Some(-600.0),
                window_width: Some(1500.0),
                rotation_degrees: None,
                flip_horizontal: false,
                flip_vertical: false,
            },
            DisplaySetAssignment {
                set_id: "ds_soft".to_string(),
                image_set_id: "soft_windows".to_string(),
                row: 0,
                column: 1,
                window_center: Some(40.0),
                window_width: Some(400.0),
                rotation_degrees: None,
                flip_horizontal: false,
                flip_vertical: false,
            },
        ],
        rows: 1,
        columns: 2,
        is_fallback: false,
    }
}

/// Create a built-in default fallback protocol (single viewport).
pub fn default_fallback_protocol() -> HangingProtocol {
    HangingProtocol {
        protocol_id: "builtin.default_fallback".to_string(),
        name: "Default (Single Viewport)".to_string(),
        priority: 999,
        match_criteria: vec![],
        image_sets: vec![ImageSetDefinition {
            set_id: "all_images".to_string(),
            criteria: vec![],
            time_perspective: TimePerspective::Current,
        }],
        display_sets: vec![DisplaySetAssignment {
            set_id: "ds_main".to_string(),
            image_set_id: "all_images".to_string(),
            row: 0,
            column: 0,
            window_center: None,
            window_width: None,
            rotation_degrees: None,
            flip_horizontal: false,
            flip_vertical: false,
        }],
        rows: 1,
        columns: 1,
        is_fallback: true,
    }
}
