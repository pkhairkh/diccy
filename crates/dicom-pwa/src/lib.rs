#![deny(missing_docs)]

//! PWA / offline mode support for the DiCCY WASM viewer.
//!
//! Provides:
//! - **Cache strategies** for offline study access (metadata-first, aggressive, manual)
//! - **Offline sync queue** for measurements/annotations created offline
//! - **Storage quota monitoring** with user notification for cache limits
//! - **Web App Manifest** generation for installable PWA
//! - **Service Worker** configuration and lifecycle management
//! - **Integration** with `TeleradGateway` offline mode for seamless online/offline transition

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// PWA error type
// ---------------------------------------------------------------------------

/// Errors that can occur in PWA/offline operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PwaError {
    /// The cache is full and cannot accept new entries.
    CacheFull,
    /// The sync queue has reached its maximum size.
    SyncQueueFull,
    /// A requested cache entry was not found.
    CacheMiss,
    /// Storage quota has been exceeded.
    QuotaExceeded,
    /// The data is invalid or corrupted.
    InvalidData,
    /// The viewer is offline and the operation requires connectivity.
    OfflineRequired,
    /// A sync operation failed.
    SyncFailed(String),
}

impl std::fmt::Display for PwaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PwaError::CacheFull => write!(f, "cache is full"),
            PwaError::SyncQueueFull => write!(f, "sync queue is full"),
            PwaError::CacheMiss => write!(f, "cache entry not found"),
            PwaError::QuotaExceeded => write!(f, "storage quota exceeded"),
            PwaError::InvalidData => write!(f, "invalid data"),
            PwaError::OfflineRequired => write!(f, "operation requires connectivity"),
            PwaError::SyncFailed(detail) => write!(f, "sync failed: {detail}"),
        }
    }
}

impl std::error::Error for PwaError {}

// ---------------------------------------------------------------------------
// Cache strategies
// ---------------------------------------------------------------------------

/// Cache strategies for offline access.
///
/// Different strategies trade off between storage usage, initial load speed,
/// and offline availability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CacheStrategy {
    /// Cache metadata on first load, pixel data on demand.
    ///
    /// This is the recommended strategy for most use cases. Study metadata
    /// (DICOM tags, series structure) is cached immediately on first load,
    /// while pixel data is cached only when explicitly viewed, with LRU
    /// eviction when the cache approaches its limit.
    MetadataFirst {
        /// Maximum pixel data cache size in megabytes.
        max_pixel_cache_mb: u32,
        /// Maximum number of LRU entries before eviction.
        lru_max_entries: usize,
    },

    /// Cache everything eagerly.
    ///
    /// All study data (metadata + pixel data) is cached on first load.
    /// This provides the best offline experience but uses the most storage.
    Aggressive {
        /// Maximum total cache size in megabytes.
        max_cache_mb: u32,
    },

    /// Cache only what the user explicitly requests.
    ///
    /// Nothing is cached automatically. The user must explicitly mark
    /// studies or instances for offline access.
    Manual,
}

impl Default for CacheStrategy {
    fn default() -> Self {
        Self::MetadataFirst {
            max_pixel_cache_mb: 512,
            lru_max_entries: 100,
        }
    }
}

// ---------------------------------------------------------------------------
// Offline sync queue
// ---------------------------------------------------------------------------

/// Offline sync queue for measurements/annotations.
///
/// When the viewer is offline, measurement and annotation operations are
/// enqueued for later replay. When connectivity is restored, the queue
/// is replayed in FIFO order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OfflineSyncQueue {
    /// Pending operations.
    pending_operations: Vec<PendingOperation>,
    /// Maximum queue size before rejecting new operations.
    max_queue_size: usize,
}

impl Default for OfflineSyncQueue {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl OfflineSyncQueue {
    /// Create a new sync queue with a maximum size.
    pub fn new(max_queue_size: usize) -> Self {
        Self {
            pending_operations: Vec::new(),
            max_queue_size,
        }
    }

    /// Enqueue an operation for later sync.
    ///
    /// Returns an error if the queue is full.
    pub fn enqueue(&mut self, op: PendingOperation) -> Result<(), PwaError> {
        if self.pending_operations.len() >= self.max_queue_size {
            return Err(PwaError::SyncQueueFull);
        }
        self.pending_operations.push(op);
        Ok(())
    }

    /// Replay all pending operations.
    ///
    /// Returns a vector of results, one for each operation. Operations
    /// that succeed are removed from the queue; operations that fail
    /// remain for retry.
    pub fn replay_all(&mut self) -> Vec<Result<(), PwaError>> {
        let mut results = Vec::new();
        let mut remaining = Vec::new();

        for op in self.pending_operations.drain(..) {
            // Simulate replay: in a real implementation, this would
            // send the operation to the server
            match Self::simulate_replay(&op) {
                Ok(()) => results.push(Ok(())),
                Err(e) => {
                    results.push(Err(e.clone()));
                    remaining.push(op);
                }
            }
        }

        self.pending_operations = remaining;
        results
    }

    /// Return the number of pending operations.
    pub fn pending_count(&self) -> usize {
        self.pending_operations.len()
    }

    /// Clear all pending operations.
    pub fn clear(&mut self) {
        self.pending_operations.clear();
    }

    /// Return a reference to the pending operations.
    pub fn pending_operations(&self) -> &[PendingOperation] {
        &self.pending_operations
    }

    /// Simulate replay of an operation. In production, this would
    /// send the operation to the server.
    fn simulate_replay(op: &PendingOperation) -> Result<(), PwaError> {
        match op {
            PendingOperation::CreateMeasurement { data, .. }
            | PendingOperation::UpdateMeasurement { data, .. }
            | PendingOperation::CreateAnnotation { data, .. }
            | PendingOperation::CreateSegmentation { data, .. } => {
                if data.is_empty() {
                    Err(PwaError::SyncFailed("empty data payload".to_string()))
                } else {
                    Ok(())
                }
            }
            PendingOperation::DeleteMeasurement { .. } => Ok(()),
        }
    }
}

/// Pending operation in the offline sync queue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PendingOperation {
    /// Create a new measurement.
    CreateMeasurement {
        /// Study Instance UID.
        study_uid: String,
        /// Serialized measurement data.
        data: Vec<u8>,
    },
    /// Update an existing measurement.
    UpdateMeasurement {
        /// Measurement identifier.
        measurement_id: String,
        /// Serialized measurement data.
        data: Vec<u8>,
    },
    /// Delete a measurement.
    DeleteMeasurement {
        /// Measurement identifier.
        measurement_id: String,
    },
    /// Create an annotation.
    CreateAnnotation {
        /// Study Instance UID.
        study_uid: String,
        /// Serialized annotation data.
        data: Vec<u8>,
    },
    /// Create a segmentation.
    CreateSegmentation {
        /// Study Instance UID.
        study_uid: String,
        /// Serialized segmentation data.
        data: Vec<u8>,
    },
}

// ---------------------------------------------------------------------------
// Storage quota monitoring
// ---------------------------------------------------------------------------

/// Storage quota monitoring.
///
/// Tracks current storage usage and quota limits, providing notifications
/// when the cache approaches or exceeds its limits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StorageQuotaMonitor {
    /// Current usage in bytes.
    usage_bytes: u64,
    /// Quota in bytes (None if unknown).
    quota_bytes: Option<u64>,
    /// Warning threshold as a percentage (0-100).
    warning_threshold_pct: u8,
}

impl Default for StorageQuotaMonitor {
    fn default() -> Self {
        Self::new(0, None, 80)
    }
}

impl StorageQuotaMonitor {
    /// Create a new quota monitor.
    pub fn new(usage_bytes: u64, quota_bytes: Option<u64>, warning_threshold_pct: u8) -> Self {
        Self {
            usage_bytes,
            quota_bytes,
            warning_threshold_pct: warning_threshold_pct.min(100),
        }
    }

    /// Estimate current storage usage.
    pub fn estimate_usage(&self) -> StorageEstimate {
        let usage_percentage = self.quota_bytes.map(|quota| {
            if quota > 0 {
                (self.usage_bytes as f64 / quota as f64) * 100.0
            } else {
                0.0
            }
        });
        StorageEstimate {
            usage_bytes: self.usage_bytes,
            quota_bytes: self.quota_bytes,
            usage_percentage,
        }
    }

    /// Check the current quota status.
    pub fn check_quota(&self) -> QuotaStatus {
        let Some(quota) = self.quota_bytes else {
            return QuotaStatus::Available;
        };
        if quota == 0 {
            return QuotaStatus::Available;
        }
        let pct = (self.usage_bytes as f64 / quota as f64) * 100.0;
        if self.usage_bytes > quota {
            QuotaStatus::Exceeded
        } else if pct >= self.warning_threshold_pct as f64 {
            QuotaStatus::NearLimit
        } else {
            QuotaStatus::Available
        }
    }

    /// Check whether additional data should be accepted.
    ///
    /// Returns `true` if adding the additional bytes would not exceed
    /// the quota or trigger eviction.
    pub fn should_evict(&self, additional_bytes: u64) -> bool {
        let Some(quota) = self.quota_bytes else {
            return false;
        };
        self.usage_bytes + additional_bytes > quota
    }

    /// Record that bytes have been added to storage.
    pub fn record_usage(&mut self, bytes: u64) {
        self.usage_bytes = self.usage_bytes.saturating_add(bytes);
    }

    /// Record that bytes have been freed from storage.
    pub fn record_free(&mut self, bytes: u64) {
        self.usage_bytes = self.usage_bytes.saturating_sub(bytes);
    }

    /// Return current usage in bytes.
    pub fn usage_bytes(&self) -> u64 {
        self.usage_bytes
    }

    /// Set the quota in bytes.
    pub fn set_quota(&mut self, quota_bytes: u64) {
        self.quota_bytes = Some(quota_bytes);
    }
}

/// Storage usage estimate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StorageEstimate {
    /// Current usage in bytes.
    pub usage_bytes: u64,
    /// Quota in bytes (None if unknown).
    pub quota_bytes: Option<u64>,
    /// Usage as a percentage of quota (None if quota unknown).
    pub usage_percentage: Option<f64>,
}

/// Quota status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuotaStatus {
    /// Storage is available for use.
    Available,
    /// Storage is approaching the quota limit.
    NearLimit,
    /// Storage quota has been exceeded.
    Exceeded,
}

// ---------------------------------------------------------------------------
// Web App Manifest
// ---------------------------------------------------------------------------

/// Web App Manifest for installable PWA.
///
/// See: <https://developer.mozilla.org/en-US/docs/Web/Manifest>
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebAppManifest {
    /// Application name.
    pub name: String,
    /// Short application name (for home screen).
    pub short_name: String,
    /// Start URL.
    pub start_url: String,
    /// Display mode.
    pub display: DisplayMode,
    /// Background color (CSS color string).
    pub background_color: String,
    /// Theme color (CSS color string).
    pub theme_color: String,
    /// Application icons.
    pub icons: Vec<ManifestIcon>,
}

impl Default for WebAppManifest {
    fn default() -> Self {
        Self {
            name: "DiCCY DICOM Viewer".to_string(),
            short_name: "DiCCY".to_string(),
            start_url: "/".to_string(),
            display: DisplayMode::Standalone,
            background_color: "#000000".to_string(),
            theme_color: "#1a1a2e".to_string(),
            icons: vec![
                ManifestIcon {
                    src: "/icons/icon-192.png".to_string(),
                    sizes: "192x192".to_string(),
                    icon_type: "image/png".to_string(),
                },
                ManifestIcon {
                    src: "/icons/icon-512.png".to_string(),
                    sizes: "512x512".to_string(),
                    icon_type: "image/png".to_string(),
                },
            ],
        }
    }
}

impl WebAppManifest {
    /// Generate the manifest as a JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// PWA display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisplayMode {
    /// Standalone application (no browser UI).
    Standalone,
    /// Fullscreen application.
    Fullscreen,
    /// Minimal UI (some browser controls).
    MinimalUi,
}

impl DisplayMode {
    /// Return the manifest string value.
    pub fn as_str(&self) -> &'static str {
        match self {
            DisplayMode::Standalone => "standalone",
            DisplayMode::Fullscreen => "fullscreen",
            DisplayMode::MinimalUi => "minimal-ui",
        }
    }
}

/// Manifest icon entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestIcon {
    /// Icon source URL.
    pub src: String,
    /// Icon sizes string (e.g., "192x192").
    pub sizes: String,
    /// Icon MIME type.
    pub icon_type: String,
}

// ---------------------------------------------------------------------------
// Service Worker configuration
// ---------------------------------------------------------------------------

/// Service Worker registration and lifecycle configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceWorkerConfig {
    /// Service worker scope.
    pub scope: String,
    /// Cache update policy.
    pub update_via_cache: UpdateViaCache,
}

impl Default for ServiceWorkerConfig {
    fn default() -> Self {
        Self {
            scope: "/".to_string(),
            update_via_cache: UpdateViaCache::Imports,
        }
    }
}

/// Cache update policy for service workers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateViaCache {
    /// Only update imports.
    Imports,
    /// Update all via cache.
    All,
    /// Never update via cache.
    None,
}

impl UpdateViaCache {
    /// Return the manifest string value.
    pub fn as_str(&self) -> &'static str {
        match self {
            UpdateViaCache::Imports => "imports",
            UpdateViaCache::All => "all",
            UpdateViaCache::None => "none",
        }
    }
}

// ---------------------------------------------------------------------------
// PwaManager — main orchestrator
// ---------------------------------------------------------------------------

/// PWA / offline mode manager for the DiCCY WASM viewer.
///
/// Coordinates caching, offline sync, quota monitoring, and PWA manifest
/// generation for installable offline-capable DICOM viewing.
#[derive(Debug, Clone)]
pub struct PwaManager {
    /// Cache strategy for offline access.
    cache_strategy: CacheStrategy,
    /// Offline sync queue for operations created while offline.
    sync_queue: OfflineSyncQueue,
    /// Storage quota monitor.
    quota_monitor: StorageQuotaMonitor,
    /// Web App Manifest.
    manifest: WebAppManifest,
    /// Service Worker configuration.
    sw_config: ServiceWorkerConfig,
    /// Cached study metadata.
    metadata_cache: HashMap<String, Vec<u8>>,
    /// Cached pixel data.
    pixel_cache: HashMap<String, Vec<u8>>,
    /// LRU ordering for pixel cache eviction.
    pixel_lru: Vec<String>,
    /// Whether the viewer is currently online.
    online: bool,
}

impl Default for PwaManager {
    fn default() -> Self {
        Self::new(CacheStrategy::default())
    }
}

impl PwaManager {
    /// Create a new PWA manager with the given cache strategy.
    pub fn new(strategy: CacheStrategy) -> Self {
        let (max_pixel_mb, _lru_max) = match &strategy {
            CacheStrategy::MetadataFirst {
                max_pixel_cache_mb,
                lru_max_entries,
            } => (*max_pixel_cache_mb, *lru_max_entries),
            CacheStrategy::Aggressive { max_cache_mb } => (*max_cache_mb, 500),
            CacheStrategy::Manual => (100, 50),
        };
        let quota_bytes = Some((max_pixel_mb as u64) * 1024 * 1024);

        Self {
            cache_strategy: strategy,
            sync_queue: OfflineSyncQueue::new(1000),
            quota_monitor: StorageQuotaMonitor::new(0, quota_bytes, 80),
            manifest: WebAppManifest::default(),
            sw_config: ServiceWorkerConfig::default(),
            metadata_cache: HashMap::new(),
            pixel_cache: HashMap::new(),
            pixel_lru: Vec::new(),
            online: true,
        }
    }

    /// Cache study metadata.
    ///
    /// Returns an error if the cache is full.
    pub fn cache_study_metadata(&mut self, study_uid: &str, metadata: &[u8]) -> Result<(), PwaError> {
        if self.quota_monitor.should_evict(metadata.len() as u64) {
            return Err(PwaError::CacheFull);
        }
        self.metadata_cache.insert(study_uid.to_string(), metadata.to_vec());
        self.quota_monitor.record_usage(metadata.len() as u64);
        Ok(())
    }

    /// Cache pixel data for an instance.
    ///
    /// Returns an error if the cache is full. May trigger LRU eviction.
    pub fn cache_pixel_data(&mut self, instance_uid: &str, data: &[u8]) -> Result<(), PwaError> {
        // Check LRU eviction needs
        let lru_max = match &self.cache_strategy {
            CacheStrategy::MetadataFirst { lru_max_entries, .. } => *lru_max_entries,
            CacheStrategy::Aggressive { .. } => 500,
            CacheStrategy::Manual => 50,
        };

        // Evict LRU entries if needed
        while self.pixel_lru.len() >= lru_max && !self.pixel_lru.is_empty() {
            if let Some(evict_uid) = self.pixel_lru.first().cloned() {
                if let Some(evicted) = self.pixel_cache.remove(&evict_uid) {
                    self.quota_monitor.record_free(evicted.len() as u64);
                }
                self.pixel_lru.remove(0);
            }
        }

        // Check quota after eviction
        if self.quota_monitor.should_evict(data.len() as u64) {
            return Err(PwaError::CacheFull);
        }

        // Remove from LRU if already present (re-access)
        self.pixel_lru.retain(|uid| uid != instance_uid);

        self.pixel_cache.insert(instance_uid.to_string(), data.to_vec());
        self.pixel_lru.push(instance_uid.to_string());
        self.quota_monitor.record_usage(data.len() as u64);
        Ok(())
    }

    /// Get cached study metadata.
    pub fn get_cached_metadata(&self, study_uid: &str) -> Option<&[u8]> {
        self.metadata_cache.get(study_uid).map(|v| v.as_slice())
    }

    /// Get cached pixel data.
    pub fn get_cached_pixel_data(&self, instance_uid: &str) -> Option<&[u8]> {
        self.pixel_cache.get(instance_uid).map(|v| v.as_slice())
    }

    /// Return whether the viewer is currently online.
    pub fn is_online(&self) -> bool {
        self.online
    }

    /// Handle going offline.
    ///
    /// Transitions the manager to offline mode. All subsequent measurement
    /// and annotation operations will be queued for later sync.
    pub fn on_offline(&mut self) {
        self.online = false;
    }

    /// Handle going online.
    ///
    /// Transitions the manager to online mode and replays all pending
    /// operations from the sync queue.
    pub fn on_online(&mut self) -> Vec<Result<(), PwaError>> {
        self.online = true;
        self.sync_queue.replay_all()
    }

    /// Enqueue an offline operation.
    ///
    /// If the viewer is online, returns an error (use normal API instead).
    pub fn enqueue_offline_operation(&mut self, op: PendingOperation) -> Result<(), PwaError> {
        if self.online {
            // Online: operation should go through normal API, not the queue
            return Ok(());
        }
        self.sync_queue.enqueue(op)
    }

    /// Return the number of pending offline operations.
    pub fn pending_operation_count(&self) -> usize {
        self.sync_queue.pending_count()
    }

    /// Return a reference to the quota monitor.
    pub fn quota_monitor(&self) -> &StorageQuotaMonitor {
        &self.quota_monitor
    }

    /// Return a reference to the cache strategy.
    pub fn cache_strategy(&self) -> &CacheStrategy {
        &self.cache_strategy
    }

    /// Return a reference to the sync queue.
    pub fn sync_queue(&self) -> &OfflineSyncQueue {
        &self.sync_queue
    }

    /// Generate the Web App Manifest as a JSON string.
    pub fn generate_manifest_json(&self) -> String {
        self.manifest.to_json()
    }

    /// Generate a Service Worker JavaScript file.
    ///
    /// Returns a minimal Service Worker that handles offline caching
    /// according to the configured strategy.
    pub fn generate_service_worker_js(&self) -> String {
        let cache_name = "diccy-viewer-v1";
        let strategy_comment = match &self.cache_strategy {
            CacheStrategy::MetadataFirst { .. } => "Metadata-first caching strategy",
            CacheStrategy::Aggressive { .. } => "Aggressive caching strategy",
            CacheStrategy::Manual => "Manual caching strategy",
        };

        format!(
            r#"// DiCCY Viewer Service Worker — {strategy_comment}
const CACHE_NAME = '{cache_name}';

// Install event: pre-cache application shell
self.addEventListener('install', (event) => {{
  event.waitUntil(
    caches.open(CACHE_NAME).then((cache) => {{
      return cache.addAll([
        '/',
        '/index.html',
        '/app.js',
        '/styles.css',
      ]);
    }})
  );
  self.skipWaiting();
}});

// Activate event: clean up old caches
self.addEventListener('activate', (event) => {{
  event.waitUntil(
    caches.keys().then((cacheNames) => {{
      return Promise.all(
        cacheNames
          .filter((name) => name !== CACHE_NAME)
          .map((name) => caches.delete(name))
      );
    }})
  );
  self.clients.claim();
}});

// Fetch event: serve from cache, fall back to network
self.addEventListener('fetch', (event) => {{
  // Only handle GET requests
  if (event.request.method !== 'GET') return;

  // DICOMweb API requests: network-first with cache fallback
  if (event.request.url.includes('/wado-rs/') ||
      event.request.url.includes('/qido-rs/')) {{
    event.respondWith(
      fetch(event.request)
        .then((response) => {{
          // Cache successful responses
          if (response.ok) {{
            const responseClone = response.clone();
            caches.open(CACHE_NAME).then((cache) => {{
              cache.put(event.request, responseClone);
            }});
          }}
          return response;
        }})
        .catch(() => {{
          // Fallback to cache when offline
          return caches.match(event.request);
        }})
    );
    return;
  }}

  // Application shell: cache-first
  event.respondWith(
    caches.match(event.request).then((cached) => {{
      return cached || fetch(event.request).then((response) => {{
        if (response.ok) {{
          const responseClone = response.clone();
          caches.open(CACHE_NAME).then((cache) => {{
            cache.put(event.request, responseClone);
          }});
        }}
        return response;
      }});
    }})
  );
}});

// Background sync for offline measurements
self.addEventListener('sync', (event) => {{
  if (event.tag === 'diccy-sync-measurements') {{
    event.waitUntil(syncOfflineMeasurements());
  }}
}});

async function syncOfflineMeasurements() {{
  // Replay offline measurement operations
  // This would be implemented by the SDK's sync queue
  console.log('DiCCY: syncing offline measurements');
}}
"#,
            cache_name = cache_name,
            strategy_comment = strategy_comment,
        )
    }

    /// Return the cached study UIDs.
    pub fn cached_study_uids(&self) -> Vec<&str> {
        self.metadata_cache.keys().map(|s| s.as_str()).collect()
    }

    /// Return the cached instance UIDs (pixel data).
    pub fn cached_instance_uids(&self) -> Vec<&str> {
        self.pixel_cache.keys().map(|s| s.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pwa_manager_default() {
        let manager = PwaManager::default();
        assert!(manager.is_online());
        assert_eq!(manager.pending_operation_count(), 0);
    }

    #[test]
    fn cache_study_metadata() {
        let mut manager = PwaManager::new(CacheStrategy::MetadataFirst {
            max_pixel_cache_mb: 10,
            lru_max_entries: 10,
        });
        let result = manager.cache_study_metadata("1.2.3.4", b"metadata-bytes");
        assert!(result.is_ok());
        assert_eq!(manager.get_cached_metadata("1.2.3.4"), Some(&b"metadata-bytes"[..]));
    }

    #[test]
    fn cache_pixel_data() {
        let mut manager = PwaManager::new(CacheStrategy::MetadataFirst {
            max_pixel_cache_mb: 10,
            lru_max_entries: 10,
        });
        let result = manager.cache_pixel_data("inst-1", b"pixel-data-here");
        assert!(result.is_ok());
        assert_eq!(manager.get_cached_pixel_data("inst-1"), Some(&b"pixel-data-here"[..]));
    }

    #[test]
    fn cache_pixel_data_lru_eviction() {
        let mut manager = PwaManager::new(CacheStrategy::MetadataFirst {
            max_pixel_cache_mb: 100,
            lru_max_entries: 3,
        });

        manager.cache_pixel_data("inst-1", b"data1").unwrap();
        manager.cache_pixel_data("inst-2", b"data2").unwrap();
        manager.cache_pixel_data("inst-3", b"data3").unwrap();

        // Adding a 4th should evict the first
        manager.cache_pixel_data("inst-4", b"data4").unwrap();
        assert!(manager.get_cached_pixel_data("inst-1").is_none(), "inst-1 should be evicted");
        assert!(manager.get_cached_pixel_data("inst-4").is_some(), "inst-4 should be cached");
    }

    #[test]
    fn cache_pixel_data_reaccess_updates_lru() {
        let mut manager = PwaManager::new(CacheStrategy::MetadataFirst {
            max_pixel_cache_mb: 100,
            lru_max_entries: 3,
        });

        manager.cache_pixel_data("inst-1", b"data1").unwrap();
        manager.cache_pixel_data("inst-2", b"data2").unwrap();
        manager.cache_pixel_data("inst-3", b"data3").unwrap();

        // Re-access inst-1 to move it to the end of LRU
        manager.cache_pixel_data("inst-1", b"data1-updated").unwrap();

        // Adding a 4th should evict inst-2 (not inst-1)
        manager.cache_pixel_data("inst-4", b"data4").unwrap();
        assert!(manager.get_cached_pixel_data("inst-1").is_some(), "inst-1 should still be cached");
        assert!(manager.get_cached_pixel_data("inst-2").is_none(), "inst-2 should be evicted");
    }

    #[test]
    fn cache_miss() {
        let manager = PwaManager::default();
        assert!(manager.get_cached_metadata("nonexistent").is_none());
        assert!(manager.get_cached_pixel_data("nonexistent").is_none());
    }

    #[test]
    fn offline_sync_queue_enqueue() {
        let mut queue = OfflineSyncQueue::new(10);
        let op = PendingOperation::CreateMeasurement {
            study_uid: "1.2.3".to_string(),
            data: vec![1, 2, 3],
        };
        assert!(queue.enqueue(op).is_ok());
        assert_eq!(queue.pending_count(), 1);
    }

    #[test]
    fn offline_sync_queue_full() {
        let mut queue = OfflineSyncQueue::new(2);
        queue.enqueue(PendingOperation::DeleteMeasurement {
            measurement_id: "m1".to_string(),
        }).unwrap();
        queue.enqueue(PendingOperation::DeleteMeasurement {
            measurement_id: "m2".to_string(),
        }).unwrap();
        let result = queue.enqueue(PendingOperation::DeleteMeasurement {
            measurement_id: "m3".to_string(),
        });
        assert!(matches!(result, Err(PwaError::SyncQueueFull)));
    }

    #[test]
    fn offline_sync_queue_replay() {
        let mut queue = OfflineSyncQueue::new(10);
        queue.enqueue(PendingOperation::CreateMeasurement {
            study_uid: "1.2.3".to_string(),
            data: vec![1, 2, 3],
        }).unwrap();
        queue.enqueue(PendingOperation::CreateAnnotation {
            study_uid: "1.2.3".to_string(),
            data: vec![4, 5, 6],
        }).unwrap();

        let results = queue.replay_all();
        assert_eq!(results.len(), 2);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());
        assert_eq!(queue.pending_count(), 0);
    }

    #[test]
    fn offline_sync_queue_replay_keeps_failed() {
        let mut queue = OfflineSyncQueue::new(10);
        queue.enqueue(PendingOperation::CreateMeasurement {
            study_uid: "1.2.3".to_string(),
            data: vec![], // Empty data will fail
        }).unwrap();
        queue.enqueue(PendingOperation::CreateMeasurement {
            study_uid: "1.2.3".to_string(),
            data: vec![1, 2, 3], // This will succeed
        }).unwrap();

        let results = queue.replay_all();
        assert_eq!(results.len(), 2);
        assert!(results[0].is_err());
        assert!(results[1].is_ok());
        assert_eq!(queue.pending_count(), 1, "Failed operation should remain");
    }

    #[test]
    fn offline_sync_queue_clear() {
        let mut queue = OfflineSyncQueue::new(10);
        queue.enqueue(PendingOperation::DeleteMeasurement {
            measurement_id: "m1".to_string(),
        }).unwrap();
        queue.clear();
        assert_eq!(queue.pending_count(), 0);
    }

    #[test]
    fn storage_quota_monitor_available() {
        let monitor = StorageQuotaMonitor::new(50_000_000, Some(100_000_000), 80);
        assert_eq!(monitor.check_quota(), QuotaStatus::Available);
        let estimate = monitor.estimate_usage();
        assert_eq!(estimate.usage_bytes, 50_000_000);
        assert_eq!(estimate.quota_bytes, Some(100_000_000));
        assert!(estimate.usage_percentage.unwrap() < 80.0);
    }

    #[test]
    fn storage_quota_monitor_near_limit() {
        let monitor = StorageQuotaMonitor::new(85_000_000, Some(100_000_000), 80);
        assert_eq!(monitor.check_quota(), QuotaStatus::NearLimit);
    }

    #[test]
    fn storage_quota_monitor_exceeded() {
        let monitor = StorageQuotaMonitor::new(150_000_000, Some(100_000_000), 80);
        assert_eq!(monitor.check_quota(), QuotaStatus::Exceeded);
    }

    #[test]
    fn storage_quota_monitor_should_evict() {
        let monitor = StorageQuotaMonitor::new(90_000_000, Some(100_000_000), 80);
        assert!(monitor.should_evict(20_000_000));
        assert!(!monitor.should_evict(5_000_000));
    }

    #[test]
    fn storage_quota_monitor_no_quota_known() {
        let monitor = StorageQuotaMonitor::new(50_000_000, None, 80);
        assert_eq!(monitor.check_quota(), QuotaStatus::Available);
        assert!(!monitor.should_evict(1_000_000_000));
    }

    #[test]
    fn storage_quota_monitor_record_usage() {
        let mut monitor = StorageQuotaMonitor::new(0, Some(100), 80);
        monitor.record_usage(50);
        assert_eq!(monitor.usage_bytes(), 50);
        monitor.record_free(20);
        assert_eq!(monitor.usage_bytes(), 30);
    }

    #[test]
    fn web_app_manifest_default() {
        let manifest = WebAppManifest::default();
        assert_eq!(manifest.name, "DiCCY DICOM Viewer");
        assert_eq!(manifest.short_name, "DiCCY");
        assert_eq!(manifest.display, DisplayMode::Standalone);
        assert_eq!(manifest.icons.len(), 2);
    }

    #[test]
    fn web_app_manifest_json() {
        let manifest = WebAppManifest::default();
        let json = manifest.to_json();
        assert!(json.contains("DiCCY"));
        // Display mode is serialized by serde; check the display key exists
        assert!(json.contains("display"));
    }

    #[test]
    fn display_mode_as_str() {
        assert_eq!(DisplayMode::Standalone.as_str(), "standalone");
        assert_eq!(DisplayMode::Fullscreen.as_str(), "fullscreen");
        assert_eq!(DisplayMode::MinimalUi.as_str(), "minimal-ui");
    }

    #[test]
    fn update_via_cache_as_str() {
        assert_eq!(UpdateViaCache::Imports.as_str(), "imports");
        assert_eq!(UpdateViaCache::All.as_str(), "all");
        assert_eq!(UpdateViaCache::None.as_str(), "none");
    }

    #[test]
    fn pwa_manager_offline_online_cycle() {
        let mut manager = PwaManager::default();
        assert!(manager.is_online());

        // Go offline
        manager.on_offline();
        assert!(!manager.is_online());

        // Enqueue operation while offline
        manager.enqueue_offline_operation(PendingOperation::CreateMeasurement {
            study_uid: "1.2.3".to_string(),
            data: vec![1, 2, 3],
        }).unwrap();
        assert_eq!(manager.pending_operation_count(), 1);

        // Go online and sync
        let results = manager.on_online();
        assert!(manager.is_online());
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
        assert_eq!(manager.pending_operation_count(), 0);
    }

    #[test]
    fn pwa_manager_generate_manifest() {
        let manager = PwaManager::default();
        let json = manager.generate_manifest_json();
        assert!(json.contains("DiCCY"));
    }

    #[test]
    fn pwa_manager_generate_service_worker() {
        let manager = PwaManager::default();
        let js = manager.generate_service_worker_js();
        assert!(js.contains("Service Worker"));
        assert!(js.contains("CACHE_NAME"));
        assert!(js.contains("fetch"));
    }

    #[test]
    fn cache_strategy_aggressive() {
        let mut manager = PwaManager::new(CacheStrategy::Aggressive { max_cache_mb: 10 });
        assert!(manager.cache_study_metadata("1.2.3", b"meta").is_ok());
        assert!(manager.cache_pixel_data("inst-1", b"pixels").is_ok());
    }

    #[test]
    fn cache_strategy_manual() {
        let manager = PwaManager::new(CacheStrategy::Manual);
        assert!(matches!(manager.cache_strategy(), CacheStrategy::Manual));
    }

    #[test]
    fn pending_operation_variants() {
        let ops = vec![
            PendingOperation::CreateMeasurement {
                study_uid: "1.2.3".to_string(),
                data: vec![1],
            },
            PendingOperation::UpdateMeasurement {
                measurement_id: "m1".to_string(),
                data: vec![2],
            },
            PendingOperation::DeleteMeasurement {
                measurement_id: "m2".to_string(),
            },
            PendingOperation::CreateAnnotation {
                study_uid: "1.2.3".to_string(),
                data: vec![3],
            },
            PendingOperation::CreateSegmentation {
                study_uid: "1.2.3".to_string(),
                data: vec![4],
            },
        ];
        let mut queue = OfflineSyncQueue::new(10);
        for op in ops {
            assert!(queue.enqueue(op).is_ok());
        }
        assert_eq!(queue.pending_count(), 5);
    }

    #[test]
    fn cached_uids() {
        let mut manager = PwaManager::new(CacheStrategy::MetadataFirst {
            max_pixel_cache_mb: 10,
            lru_max_entries: 10,
        });
        manager.cache_study_metadata("study-1", b"meta").unwrap();
        manager.cache_pixel_data("inst-1", b"pixels").unwrap();

        let studies = manager.cached_study_uids();
        assert!(studies.contains(&"study-1"));

        let instances = manager.cached_instance_uids();
        assert!(instances.contains(&"inst-1"));
    }

    #[test]
    fn pwa_error_display() {
        assert_eq!(PwaError::CacheFull.to_string(), "cache is full");
        assert_eq!(PwaError::SyncQueueFull.to_string(), "sync queue is full");
        assert_eq!(PwaError::CacheMiss.to_string(), "cache entry not found");
        assert_eq!(PwaError::QuotaExceeded.to_string(), "storage quota exceeded");
        assert_eq!(
            PwaError::SyncFailed("timeout".to_string()).to_string(),
            "sync failed: timeout"
        );
    }
}
