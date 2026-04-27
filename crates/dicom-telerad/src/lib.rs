#![deny(missing_docs)]

//! Teleradiology gateway for bandwidth-adaptive streaming and offline mode.
//!
//! Provides:
//! - **S6-T4**: Bandwidth-adaptive streaming with JPEG 2000 progressive encoding.
//! - Low-latency interaction forwarding for remote diagnosis.
//! - Offline mode with automatic sync on reconnect.
//! - Quality-of-service adaptation based on network conditions.

use dicom_audit::{AuditEvent, AuditEventKind, AuditField, AuditValue};
use dicom_core::{Error, ErrorKind, Result};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;

// ===========================================================================
// S6-T4: Teleradiology Gateway
// ===========================================================================

/// Network bandwidth level for adaptive streaming.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BandwidthLevel {
    /// Very low bandwidth (< 1 Mbps). Uses maximum compression, reduced resolution.
    VeryLow,
    /// Low bandwidth (1-5 Mbps). Uses high compression, full resolution.
    Low,
    /// Medium bandwidth (5-15 Mbps). Uses moderate compression.
    Medium,
    /// High bandwidth (15-50 Mbps). Uses light compression.
    High,
    /// Very high bandwidth (> 50 Mbps). Uses lossless or near-lossless.
    VeryHigh,
}

impl BandwidthLevel {
    /// Classify a bandwidth measurement in bits per second.
    pub fn from_bps(bps: u64) -> Self {
        let mbps = bps / 1_000_000;
        match mbps {
            0 => BandwidthLevel::VeryLow,
            1..=4 => BandwidthLevel::Low,
            5..=14 => BandwidthLevel::Medium,
            15..=49 => BandwidthLevel::High,
            _ => BandwidthLevel::VeryHigh,
        }
    }

    /// Get the recommended JPEG 2000 quality factor for this bandwidth level.
    pub fn quality_factor(&self) -> f64 {
        match self {
            BandwidthLevel::VeryLow => 10.0,
            BandwidthLevel::Low => 30.0,
            BandwidthLevel::Medium => 60.0,
            BandwidthLevel::High => 85.0,
            BandwidthLevel::VeryHigh => 98.0,
        }
    }

    /// Get the recommended resolution scale factor (1.0 = full resolution).
    pub fn resolution_scale(&self) -> f64 {
        match self {
            BandwidthLevel::VeryLow => 0.25,
            BandwidthLevel::Low => 0.5,
            BandwidthLevel::Medium => 0.75,
            BandwidthLevel::High => 1.0,
            BandwidthLevel::VeryHigh => 1.0,
        }
    }

    /// Get the maximum frame rate for progressive loading.
    pub fn max_fps(&self) -> u32 {
        match self {
            BandwidthLevel::VeryLow => 5,
            BandwidthLevel::Low => 10,
            BandwidthLevel::Medium => 20,
            BandwidthLevel::High => 30,
            BandwidthLevel::VeryHigh => 60,
        }
    }
}

/// Streaming quality configuration for a given bandwidth level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamingConfig {
    /// Quality factor (0-100) for JPEG 2000 compression.
    pub quality_factor: f64,
    /// Resolution scale factor (0.0-1.0).
    pub resolution_scale: f64,
    /// Maximum frames per second for progressive loading.
    pub max_fps: u32,
    /// Progressive loading: number of quality layers.
    pub progressive_layers: u32,
    /// Tile size for progressive tile-based rendering.
    pub tile_size: u32,
    /// Whether to enable lossless upgrade after initial display.
    pub lossless_upgrade: bool,
}

impl Default for StreamingConfig {
    /// Default streaming configuration for medium bandwidth (S13-T5).
    fn default() -> Self {
        Self::from_bandwidth(BandwidthLevel::Medium)
    }
}

impl StreamingConfig {
    /// Create a streaming configuration from a bandwidth level.
    pub fn from_bandwidth(level: BandwidthLevel) -> Self {
        let (quality, scale, fps, layers, tile, upgrade) = match level {
            BandwidthLevel::VeryLow => (10.0, 0.25, 5, 2, 128, true),
            BandwidthLevel::Low => (30.0, 0.5, 10, 3, 256, true),
            BandwidthLevel::Medium => (60.0, 0.75, 20, 4, 256, true),
            BandwidthLevel::High => (85.0, 1.0, 30, 5, 512, true),
            BandwidthLevel::VeryHigh => (98.0, 1.0, 60, 6, 512, false),
        };
        Self {
            quality_factor: quality,
            resolution_scale: scale,
            max_fps: fps,
            progressive_layers: layers,
            tile_size: tile,
            lossless_upgrade: upgrade,
        }
    }
}

/// A frame in the progressive streaming pipeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamFrame {
    /// Frame sequence number.
    pub sequence: u64,
    /// Quality layer index (0 = lowest quality, higher = better).
    pub quality_layer: u32,
    /// Tile x index.
    pub tile_x: u32,
    /// Tile y index.
    pub tile_y: u32,
    /// Compressed frame data (JPEG 2000 codestream fragment).
    pub data: Vec<u8>,
    /// Original image width before tiling.
    pub image_width: u32,
    /// Original image height before tiling.
    pub image_height: u32,
    /// Study Instance UID this frame belongs to.
    pub study_uid: String,
    /// Series Instance UID this frame belongs to.
    pub series_uid: String,
    /// SOP Instance UID this frame belongs to.
    pub sop_instance_uid: String,
}

/// Interaction event for low-latency forwarding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InteractionEvent {
    /// Pan event with offset.
    Pan {
        /// Horizontal offset in viewport coordinates.
        dx: f64,
        /// Vertical offset in viewport coordinates.
        dy: f64,
    },
    /// Zoom event with factor.
    Zoom {
        /// Zoom factor (1.0 = no zoom, >1.0 = zoom in).
        factor: f64,
        /// Center point for zoom (x, y).
        center: (f64, f64),
    },
    /// Window/level adjustment.
    WindowLevel {
        /// New window center.
        center: f64,
        /// New window width.
        width: f64,
    },
    /// Scroll to frame.
    Scroll {
        /// Target frame index.
        frame_index: u32,
    },
    /// Measurement started.
    MeasureStart {
        /// Measurement point (x, y).
        point: (f64, f64),
        /// Measurement type.
        measure_type: String,
    },
    /// Measurement updated.
    MeasureUpdate {
        /// Current measurement point (x, y).
        point: (f64, f64),
    },
}

impl InteractionEvent {
    /// Estimate the serialized size of this event in bytes.
    pub fn estimated_size(&self) -> usize {
        match self {
            InteractionEvent::Pan { .. } => 32,
            InteractionEvent::Zoom { .. } => 48,
            InteractionEvent::WindowLevel { .. } => 32,
            InteractionEvent::Scroll { .. } => 16,
            InteractionEvent::MeasureStart { .. } => 64,
            InteractionEvent::MeasureUpdate { .. } => 32,
        }
    }
}

/// Offline operation stored for later sync.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OfflineOperation {
    /// Unique operation identifier.
    pub operation_id: String,
    /// The interaction event that occurred offline.
    pub event: InteractionEvent,
    /// Timestamp when the event was created.
    pub timestamp: u64,
    /// Study UID associated with the event.
    pub study_uid: String,
    /// Whether this operation has been synced.
    pub synced: bool,
}

/// Sync status for offline mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncStatus {
    /// Online and connected.
    Online,
    /// Offline with pending operations.
    Offline {
        /// Number of pending operations.
        pending_count: usize,
    },
    /// Syncing offline operations back to server.
    Syncing {
        /// Total operations to sync.
        total: usize,
        /// Operations already synced.
        completed: usize,
    },
}

/// Network measurement sample.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NetworkSample {
    /// Round-trip time in milliseconds.
    pub rtt_ms: f64,
    /// Measured bandwidth in bits per second.
    pub bandwidth_bps: u64,
    /// Packet loss percentage (0.0-100.0).
    pub packet_loss_pct: f64,
    /// Timestamp of this sample.
    pub timestamp: u64,
}

/// Teleradiology gateway managing streaming, interactions, and offline sync.
pub struct TeleradGateway {
    /// Current bandwidth level.
    bandwidth: BandwidthLevel,
    /// Current streaming configuration.
    config: StreamingConfig,
    /// Pending interaction events for forwarding.
    interaction_queue: VecDeque<InteractionEvent>,
    /// Offline operation queue.
    offline_queue: VecDeque<OfflineOperation>,
    /// Current sync status.
    sync_status: SyncStatus,
    /// Network measurement history.
    network_history: VecDeque<NetworkSample>,
    /// Maximum network history size.
    max_history: usize,
    /// Total bytes sent.
    bytes_sent: u64,
    /// Total bytes received.
    bytes_received: u64,
    /// Audit callback.
    audit: Option<AuditCallback>,
}

/// Audit callback for teleradiology operations.
pub type AuditCallback = Arc<dyn Fn(AuditEvent) -> Result<()> + Send + Sync>;

impl fmt::Debug for TeleradGateway {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TeleradGateway")
            .field("bandwidth", &self.bandwidth)
            .field("sync_status", &self.sync_status)
            .field("bytes_sent", &self.bytes_sent)
            .field("bytes_received", &self.bytes_received)
            .finish()
    }
}

impl Default for TeleradGateway {
    fn default() -> Self {
        Self::new()
    }
}

impl TeleradGateway {
    /// Create a new teleradiology gateway with default (Medium) bandwidth.
    pub fn new() -> Self {
        Self::with_bandwidth(BandwidthLevel::Medium)
    }

    /// Create a teleradiology gateway with a specific bandwidth level.
    pub fn with_bandwidth(bandwidth: BandwidthLevel) -> Self {
        let config = StreamingConfig::from_bandwidth(bandwidth);
        Self {
            bandwidth,
            config,
            interaction_queue: VecDeque::new(),
            offline_queue: VecDeque::new(),
            sync_status: SyncStatus::Online,
            network_history: VecDeque::new(),
            max_history: 100,
            bytes_sent: 0,
            bytes_received: 0,
            audit: None,
        }
    }

    /// Set the audit callback.
    pub fn set_audit(&mut self, audit: Option<AuditCallback>) {
        self.audit = audit;
    }

    /// Return the current bandwidth level.
    pub fn bandwidth(&self) -> BandwidthLevel {
        self.bandwidth
    }

    /// Return the current streaming configuration.
    pub fn config(&self) -> &StreamingConfig {
        &self.config
    }

    /// Return the current sync status.
    pub fn sync_status(&self) -> &SyncStatus {
        &self.sync_status
    }

    /// Record a network measurement sample and adapt streaming quality.
    pub fn record_network_sample(&mut self, sample: NetworkSample) {
        self.bandwidth = BandwidthLevel::from_bps(sample.bandwidth_bps);
        self.config = StreamingConfig::from_bandwidth(self.bandwidth);

        self.network_history.push_back(sample);
        if self.network_history.len() > self.max_history {
            self.network_history.pop_front();
        }

        // If we detect connectivity, move from offline to online
        if matches!(self.sync_status, SyncStatus::Offline { .. }) {
            if !self.offline_queue.is_empty() {
                self.sync_status = SyncStatus::Syncing {
                    total: self.offline_queue.len(),
                    completed: 0,
                };
            } else {
                self.sync_status = SyncStatus::Online;
            }
        }
    }

    /// Enqueue an interaction event for forwarding.
    pub fn enqueue_interaction(&mut self, event: InteractionEvent) {
        match self.sync_status {
            SyncStatus::Online => {
                let size = event.estimated_size();
                self.interaction_queue.push_back(event);
                self.bytes_sent += size as u64;
            }
            SyncStatus::Offline { .. } | SyncStatus::Syncing { .. } => {
                let op = OfflineOperation {
                    operation_id: format!("op-{}", self.offline_queue.len()),
                    event,
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0),
                    study_uid: String::new(),
                    synced: false,
                };
                self.offline_queue.push_back(op);
            }
        }
    }

    /// Dequeue the next interaction event for forwarding.
    pub fn dequeue_interaction(&mut self) -> Option<InteractionEvent> {
        self.interaction_queue.pop_front()
    }

    /// Go offline — all interactions will be queued for later sync.
    pub fn go_offline(&mut self) {
        self.sync_status = SyncStatus::Offline {
            pending_count: self.offline_queue.len(),
        };
        self.record_audit("go_offline");
    }

    /// Go online and start syncing offline operations.
    pub fn go_online(&mut self) {
        if self.offline_queue.is_empty() {
            self.sync_status = SyncStatus::Online;
        } else {
            self.sync_status = SyncStatus::Syncing {
                total: self.offline_queue.len(),
                completed: 0,
            };
        }
        self.record_audit("go_online");
    }

    /// Process the next offline sync batch. Returns the number of operations synced.
    pub fn sync_next_batch(&mut self, batch_size: usize) -> usize {
        let mut synced = 0;
        for _ in 0..batch_size {
            if let Some(mut op) = self.offline_queue.pop_front() {
                op.synced = true;
                synced += 1;
            } else {
                break;
            }
        }

        if self.offline_queue.is_empty() {
            self.sync_status = SyncStatus::Online;
        } else if let SyncStatus::Syncing { total, completed } = &self.sync_status {
            self.sync_status = SyncStatus::Syncing {
                total: *total,
                completed: completed + synced,
            };
        }

        self.record_audit("sync_batch");
        synced
    }

    /// Compute a simulated JPEG 2000 progressive frame for streaming.
    ///
    /// This is a stub that produces deterministic output suitable for testing.
    /// A real implementation would use an actual JPEG 2000 encoder.
    pub fn encode_progressive_frame(
        &self,
        image_data: &[u8],
        width: u32,
        height: u32,
        quality_layer: u32,
    ) -> Vec<u8> {
        let scale = self.config.resolution_scale;
        let scaled_w = ((width as f64 * scale).round() as u32).max(1);
        let scaled_h = ((height as f64 * scale).round() as u32).max(1);

        // Simulate progressive quality: each layer adds more detail bytes
        let base_size = (scaled_w * scaled_h / 4) as usize;
        let layer_size = base_size / self.config.progressive_layers as usize;
        let target_size = base_size.min(layer_size * (quality_layer as usize + 1));

        // Deterministic output: first N bytes of input (padded with zeros)
        let mut output = vec![0u8; target_size];
        let copy_len = target_size.min(image_data.len());
        output[..copy_len].copy_from_slice(&image_data[..copy_len]);
        output
    }

    /// Return total bytes sent since creation.
    pub fn bytes_sent(&self) -> u64 {
        self.bytes_sent
    }

    /// Return total bytes received since creation.
    pub fn bytes_received(&self) -> u64 {
        self.bytes_received
    }

    /// Record bytes received from a streaming response.
    pub fn record_bytes_received(&mut self, bytes: u64) {
        self.bytes_received += bytes;
    }

    /// Return the number of pending offline operations.
    pub fn pending_offline_count(&self) -> usize {
        self.offline_queue.iter().filter(|op| !op.synced).count()
    }

    /// Return average network RTT from recent samples.
    pub fn average_rtt(&self) -> f64 {
        if self.network_history.is_empty() {
            return 0.0;
        }
        self.network_history.iter().map(|s| s.rtt_ms).sum::<f64>()
            / self.network_history.len() as f64
    }

    fn record_audit(&self, operation: &'static str) {
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
                    key: "bandwidth",
                    value: AuditValue::Plain(format!("{:?}", self.bandwidth)),
                },
            ],
        });
    }
}

// ---------------------------------------------------------------------------
// Shared: Error helpers
// ---------------------------------------------------------------------------

fn telerad_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::DecodeError {
            stage: "dicom-telerad".to_string(),
            detail: detail.into(),
        },
        "teleradiology error",
    )
    .into()
}

// ===========================================================================
// Tests: S6-T4 Teleradiology Gateway (minimum 12)
// ===========================================================================

#[cfg(test)]
mod tests_telerad {
    use super::*;

    #[test]
    fn bandwidth_classification() {
        // REQ-TELERAD-100: bandwidth level classification must be deterministic
        assert_eq!(BandwidthLevel::from_bps(500_000), BandwidthLevel::VeryLow);
        assert_eq!(BandwidthLevel::from_bps(3_000_000), BandwidthLevel::Low);
        assert_eq!(BandwidthLevel::from_bps(10_000_000), BandwidthLevel::Medium);
        assert_eq!(BandwidthLevel::from_bps(30_000_000), BandwidthLevel::High);
        assert_eq!(
            BandwidthLevel::from_bps(100_000_000),
            BandwidthLevel::VeryHigh
        );
    }

    #[test]
    fn quality_factor_increases_with_bandwidth() {
        let q_very_low = BandwidthLevel::VeryLow.quality_factor();
        let q_low = BandwidthLevel::Low.quality_factor();
        let q_medium = BandwidthLevel::Medium.quality_factor();
        let q_high = BandwidthLevel::High.quality_factor();
        let q_very_high = BandwidthLevel::VeryHigh.quality_factor();

        assert!(q_very_low < q_low);
        assert!(q_low < q_medium);
        assert!(q_medium < q_high);
        assert!(q_high < q_very_high);
    }

    #[test]
    fn resolution_scale_for_very_low_bandwidth() {
        assert_eq!(BandwidthLevel::VeryLow.resolution_scale(), 0.25);
        assert_eq!(BandwidthLevel::VeryHigh.resolution_scale(), 1.0);
    }

    #[test]
    fn streaming_config_from_bandwidth() {
        let config = StreamingConfig::from_bandwidth(BandwidthLevel::Medium);
        assert_eq!(config.quality_factor, 60.0);
        assert_eq!(config.resolution_scale, 0.75);
        assert_eq!(config.max_fps, 20);
        assert!(config.lossless_upgrade);
    }

    #[test]
    fn gateway_creation_default() {
        let gateway = TeleradGateway::new();
        assert_eq!(gateway.bandwidth(), BandwidthLevel::Medium);
        assert!(matches!(gateway.sync_status(), SyncStatus::Online));
    }

    #[test]
    fn gateway_bandwidth_adaptation() {
        let mut gateway = TeleradGateway::new();
        gateway.record_network_sample(NetworkSample {
            rtt_ms: 100.0,
            bandwidth_bps: 2_000_000,
            packet_loss_pct: 1.0,
            timestamp: 1,
        });
        assert_eq!(gateway.bandwidth(), BandwidthLevel::Low);
        assert_eq!(gateway.config().quality_factor, 30.0);
    }

    #[test]
    fn interaction_enqueue_dequeue_online() {
        let mut gateway = TeleradGateway::new();
        gateway.enqueue_interaction(InteractionEvent::Pan { dx: 10.0, dy: 20.0 });
        gateway.enqueue_interaction(InteractionEvent::Zoom {
            factor: 2.0,
            center: (100.0, 100.0),
        });

        let event1 = gateway.dequeue_interaction().unwrap();
        assert!(matches!(event1, InteractionEvent::Pan { .. }));
        let event2 = gateway.dequeue_interaction().unwrap();
        assert!(matches!(event2, InteractionEvent::Zoom { .. }));
        assert!(gateway.dequeue_interaction().is_none());
    }

    #[test]
    fn offline_mode_queues_operations() {
        let mut gateway = TeleradGateway::new();
        gateway.go_offline();

        gateway.enqueue_interaction(InteractionEvent::WindowLevel {
            center: 40.0,
            width: 400.0,
        });

        assert!(matches!(gateway.sync_status(), SyncStatus::Offline { .. }));
        assert_eq!(gateway.pending_offline_count(), 1);
    }

    #[test]
    fn offline_sync_recovery() {
        let mut gateway = TeleradGateway::new();
        gateway.go_offline();

        gateway.enqueue_interaction(InteractionEvent::Pan { dx: 5.0, dy: 5.0 });
        gateway.enqueue_interaction(InteractionEvent::Scroll { frame_index: 10 });

        assert_eq!(gateway.pending_offline_count(), 2);

        // Go online triggers sync
        gateway.go_online();
        assert!(matches!(gateway.sync_status(), SyncStatus::Syncing { .. }));

        // Sync one batch
        let synced = gateway.sync_next_batch(1);
        assert_eq!(synced, 1);
        assert_eq!(gateway.pending_offline_count(), 1);

        // Sync remaining
        let synced = gateway.sync_next_batch(10);
        assert_eq!(synced, 1);
        assert!(matches!(gateway.sync_status(), SyncStatus::Online));
    }

    #[test]
    fn progressive_frame_encoding() {
        let gateway = TeleradGateway::with_bandwidth(BandwidthLevel::Medium);
        let image_data = vec![42u8; 1024];

        let low_quality = gateway.encode_progressive_frame(&image_data, 512, 512, 0);
        let high_quality = gateway.encode_progressive_frame(&image_data, 512, 512, 3);

        // Higher quality layer should produce larger or equal output
        assert!(high_quality.len() >= low_quality.len());
    }

    #[test]
    fn interaction_event_size_estimates() {
        let pan = InteractionEvent::Pan { dx: 1.0, dy: 1.0 };
        let wl = InteractionEvent::WindowLevel {
            center: 0.0,
            width: 0.0,
        };
        assert!(pan.estimated_size() > 0);
        assert!(wl.estimated_size() > 0);
    }

    #[test]
    fn bytes_sent_tracked() {
        let mut gateway = TeleradGateway::new();
        gateway.enqueue_interaction(InteractionEvent::Pan { dx: 1.0, dy: 1.0 });
        assert!(gateway.bytes_sent() > 0);
    }

    #[test]
    fn average_rtt_with_no_samples() {
        let gateway = TeleradGateway::new();
        assert_eq!(gateway.average_rtt(), 0.0);
    }

    #[test]
    fn average_rtt_with_samples() {
        let mut gateway = TeleradGateway::new();
        gateway.record_network_sample(NetworkSample {
            rtt_ms: 50.0,
            bandwidth_bps: 10_000_000,
            packet_loss_pct: 0.0,
            timestamp: 1,
        });
        gateway.record_network_sample(NetworkSample {
            rtt_ms: 100.0,
            bandwidth_bps: 10_000_000,
            packet_loss_pct: 0.0,
            timestamp: 2,
        });
        assert_eq!(gateway.average_rtt(), 75.0);
    }

    #[test]
    fn network_history_trimming() {
        let mut gateway = TeleradGateway::new();
        for i in 0..200 {
            gateway.record_network_sample(NetworkSample {
                rtt_ms: i as f64,
                bandwidth_bps: 10_000_000,
                packet_loss_pct: 0.0,
                timestamp: i,
            });
        }
        // Should be trimmed to max_history (100)
        assert!(gateway.network_history.len() <= 100);
    }
}
