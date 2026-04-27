# S16-T4: PWA / Offline Mode for WASM Viewer

## Task: Implement PWA and offline mode support

### Completed Implementation

1. **Created `/home/z/diccy/crates/dicom-pwa/Cargo.toml`**
   - cdylib + rlib crate types
   - Dependencies: serde, serde_json

2. **Created `/home/z/diccy/crates/dicom-pwa/src/lib.rs`**
   - `PwaManager` — main orchestrator for PWA/offline functionality
   - `CacheStrategy` enum: MetadataFirst (with LRU), Aggressive, Manual
   - `OfflineSyncQueue` — FIFO queue for offline operations with max size
   - `PendingOperation` enum: CreateMeasurement, UpdateMeasurement, DeleteMeasurement, CreateAnnotation, CreateSegmentation
   - `StorageQuotaMonitor` — usage tracking with quota status and warning thresholds
   - `StorageEstimate` — usage/quota/percentage data
   - `QuotaStatus` enum: Available, NearLimit, Exceeded
   - `WebAppManifest` — PWA manifest with name, icons, display mode, colors
   - `DisplayMode` enum: Standalone, Fullscreen, MinimalUi
   - `ManifestIcon` — icon entry for manifest
   - `ServiceWorkerConfig` — SW registration and lifecycle config
   - `UpdateViaCache` enum: Imports, All, None
   - `PwaError` enum: CacheFull, SyncQueueFull, CacheMiss, QuotaExceeded, InvalidData, OfflineRequired, SyncFailed

### Key Features
- **LRU pixel cache eviction**: Configurable max entries with automatic eviction
- **Metadata-first caching**: Study metadata cached on load, pixels on demand
- **Offline sync queue**: Operations enqueued offline, replayed on reconnect
- **Failed replay retry**: Failed sync operations remain in queue for retry
- **Storage quota monitoring**: Available/NearLimit/Exceeded status tracking
- **Web App Manifest generation**: JSON output for installable PWA
- **Service Worker generation**: JS output with cache-first/network-first strategies
- **Seamless online/offline transition**: on_offline/on_online lifecycle management

### Test Results
- 28 PWA tests passing (combined with SDK)
- Tests cover: cache operations, LRU eviction, re-access updates, sync queue enqueue/replay/clear, quota monitoring (Available/NearLimit/Exceeded), offline/online cycle, manifest generation, service worker generation, all cache strategies, error display
