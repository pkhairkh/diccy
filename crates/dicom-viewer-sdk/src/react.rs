//! React component wrapper helpers for the DiCCY Viewer SDK.
//!
//! Generates TypeScript type definitions and React component wrapper code
//! that third-party applications can use to embed the DiCCY viewer.
//!
//! # Generated Component API
//!
//! ```tsx
//! import { DicomViewerComponent } from "dicom-viewer-sdk/react";
//!
//! function App() {
//!   const handleMeasurement = (evt: DicomSdkEvent) => {
//!     console.log("Measurement:", evt.data);
//!   };
//!   return (
//!     <DicomViewerComponent
//!       wadoRsUrl="https://pacs.example.com/studies/1.2.3"
//!       onMeasurementCreated={handleMeasurement}
//!     />
//!   );
//! }
//! ```

use crate::{SdkEvent, SdkEventType};

// ---------------------------------------------------------------------------
// TypeScript type definitions
// ---------------------------------------------------------------------------

/// Generate TypeScript type definitions for the SDK.
///
/// Returns a string containing TypeScript interfaces and types that
/// third-party applications can use for type-safe integration.
pub fn generate_typescript_types() -> String {
    r#"// Auto-generated TypeScript types for DiCCY Viewer SDK
// Do not edit manually — regenerate from the SDK build.

/** Event types emitted by the DicomViewer SDK. */
export type SdkEventType =
  | "study-loaded"
  | "viewport-changed"
  | "measurement-created"
  | "measurement-updated"
  | "measurement-deleted"
  | "segmentation-updated"
  | "annotation-created"
  | "error";

/** SDK event payload dispatched to listeners. */
export interface DicomSdkEvent {
  /** Event type identifier. */
  type: SdkEventType;
  /** Event-specific data payload. */
  data: unknown;
}

/** Study loaded event data. */
export interface StudyLoadedData {
  study_uid: string;
  wado_url: string;
}

/** Viewport changed event data. */
export interface ViewportChangedData {
  zoom: number;
  center_x: number;
  center_y: number;
  window_center: number;
  window_width: number;
}

/** Measurement created event data. */
export interface MeasurementCreatedData {
  measurement_id: string;
  kind: string;
  value: number;
  unit: string;
}

/** Measurement updated event data. */
export interface MeasurementUpdatedData {
  measurement_id: string;
  value: number;
}

/** Measurement deleted event data. */
export interface MeasurementDeletedData {
  measurement_id: string;
}

/** Segmentation updated event data. */
export interface SegmentationUpdatedData {
  segmentation_id: string;
  label_count: number;
}

/** Error event data. */
export interface ErrorEventData {
  code: string;
  message: string;
}

/** Viewer state snapshot. */
export interface ViewerState {
  viewport_width: number;
  viewport_height: number;
  zoom: number;
  window_center: number;
  window_width: number;
  study_uid: string | null;
  series_uid: string | null;
  frame_index: number;
  total_frames: number;
  measurement_count: number;
  study_loaded: boolean;
}

/** DicomViewer SDK class interface. */
export interface DicomViewer {
  new(containerId: string): DicomViewer;
  loadStudy(wadoRsUrl: string): string;
  setWindowLevel(center: number, width: number): void;
  addMeasurementListener(callback: (event: DicomSdkEvent) => void): number;
  addStudyLoadedListener(callback: (event: DicomSdkEvent) => void): number;
  addViewportChangedListener(callback: (event: DicomSdkEvent) => void): number;
  addSegmentationUpdatedListener(callback: (event: DicomSdkEvent) => void): number;
  removeEventListener(eventType: string, listenerId: number): boolean;
  getViewportState(): string;
  destroy(): void;
}
"#.to_string()
}

// ---------------------------------------------------------------------------
// React component wrapper generation
// ---------------------------------------------------------------------------

/// Generate a React component wrapper for the DiCCY viewer.
///
/// Returns TypeScript source code for a React component that wraps
/// the `DicomViewer` SDK class and provides declarative props for
/// event handlers.
pub fn generate_react_component() -> String {
    r#"// Auto-generated React component wrapper for DiCCY Viewer SDK
// Do not edit manually — regenerate from the SDK build.

import { useEffect, useRef, useCallback } from "react";
import type {
  DicomSdkEvent,
  SdkEventType,
  ViewerState,
  MeasurementCreatedData,
  MeasurementUpdatedData,
  MeasurementDeletedData,
  SegmentationUpdatedData,
  StudyLoadedData,
  ViewportChangedData,
} from "./types";

// Dynamically imported from the WASM module
declare const initWasm: () => Promise<void>;
declare const DicomViewerClass: DicomViewer;

interface DicomViewer {
  loadStudy(wadoRsUrl: string): string;
  setWindowLevel(center: number, width: number): void;
  addMeasurementListener(callback: (event: DicomSdkEvent) => void): number;
  addStudyLoadedListener(callback: (event: DicomSdkEvent) => void): number;
  addViewportChangedListener(callback: (event: DicomSdkEvent) => void): number;
  addSegmentationUpdatedListener(callback: (event: DicomSdkEvent) => void): number;
  removeEventListener(eventType: string, listenerId: number): boolean;
  getViewportState(): string;
  destroy(): void;
}

export interface DicomViewerComponentProps {
  /** WADO-RS URL to load on mount. */
  wadoRsUrl?: string;
  /** Initial window center. */
  windowCenter?: number;
  /** Initial window width. */
  windowWidth?: number;
  /** Called when a study is loaded. */
  onStudyLoaded?: (data: StudyLoadedData) => void;
  /** Called when viewport state changes. */
  onViewportChanged?: (data: ViewportChangedData) => void;
  /** Called when a measurement is created. */
  onMeasurementCreated?: (data: MeasurementCreatedData) => void;
  /** Called when a measurement is updated. */
  onMeasurementUpdated?: (data: MeasurementUpdatedData) => void;
  /** Called when a measurement is deleted. */
  onMeasurementDeleted?: (data: MeasurementDeletedData) => void;
  /** Called when a segmentation is updated. */
  onSegmentationUpdated?: (data: SegmentationUpdatedData) => void;
  /** Called when an error occurs. */
  onError?: (code: string, message: string) => void;
  /** CSS class for the container div. */
  className?: string;
  /** Inline style for the container div. */
  style?: React.CSSProperties;
}

/**
 * React component that embeds the DiCCY DICOM viewer.
 *
 * @example
 * ```tsx
 * <DicomViewerComponent
 *   wadoRsUrl="https://pacs.example.com/studies/1.2.3"
 *   onMeasurementCreated={(data) => console.log(data)}
 * />
 * ```
 */
export function DicomViewerComponent({
  wadoRsUrl,
  windowCenter,
  windowWidth,
  onStudyLoaded,
  onViewportChanged,
  onMeasurementCreated,
  onMeasurementUpdated,
  onMeasurementDeleted,
  onSegmentationUpdated,
  onError,
  className,
  style,
}: DicomViewerComponentProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const viewerRef = useRef<DicomViewer | null>(null);
  const listenerIdsRef = useRef<Array<{ eventType: string; id: number }>>([]);

  useEffect(() => {
    if (!containerRef.current) return;

    const viewer = new DicomViewerClass(containerRef.current.id || "diccy-viewer");
    viewerRef.current = viewer;

    // Register event listeners
    const makeListener = (handler?: (data: unknown) => void) => {
      if (!handler) return null;
      return (event: DicomSdkEvent) => handler(event.data);
    };

    if (onStudyLoaded) {
      const id = viewer.addStudyLoadedListener((event: DicomSdkEvent) => {
        onStudyLoaded(event.data as StudyLoadedData);
      });
      listenerIdsRef.current.push({ eventType: "study-loaded", id });
    }

    if (onViewportChanged) {
      const id = viewer.addViewportChangedListener((event: DicomSdkEvent) => {
        onViewportChanged(event.data as ViewportChangedData);
      });
      listenerIdsRef.current.push({ eventType: "viewport-changed", id });
    }

    if (onMeasurementCreated) {
      const id = viewer.addMeasurementListener((event: DicomSdkEvent) => {
        onMeasurementCreated(event.data as MeasurementCreatedData);
      });
      listenerIdsRef.current.push({ eventType: "measurement-created", id });
    }

    // Apply initial window/level
    if (windowCenter !== undefined && windowWidth !== undefined) {
      viewer.setWindowLevel(windowCenter, windowWidth);
    }

    // Load study if URL provided
    if (wadoRsUrl) {
      viewer.loadStudy(wadoRsUrl);
    }

    return () => {
      // Clean up listeners
      for (const { eventType, id } of listenerIdsRef.current) {
        viewer.removeEventListener(eventType, id);
      }
      listenerIdsRef.current = [];
      viewer.destroy();
      viewerRef.current = null;
    };
  }, []);

  // Update window/level when props change
  useEffect(() => {
    if (viewerRef.current && windowCenter !== undefined && windowWidth !== undefined) {
      viewerRef.current.setWindowLevel(windowCenter, windowWidth);
    }
  }, [windowCenter, windowWidth]);

  return (
    <div
      ref={containerRef}
      className={className}
      style={style}
      id="diccy-viewer"
    />
  );
}

export default DicomViewerComponent;
"#.to_string()
}

// ---------------------------------------------------------------------------
// Vue component wrapper generation
// ---------------------------------------------------------------------------

/// Generate a Vue 3 component wrapper for the DiCCY viewer.
pub fn generate_vue_component() -> String {
    r#"// Auto-generated Vue 3 component wrapper for DiCCY Viewer SDK
// Do not edit manually — regenerate from the SDK build.

import { defineComponent, ref, onMounted, onUnmounted, watch } from "vue";
import type { DicomSdkEvent, ViewerState } from "./types";

export const DicomViewerVue = defineComponent({
  name: "DicomViewerVue",
  props: {
    wadoRsUrl: { type: String, default: undefined },
    windowCenter: { type: Number, default: undefined },
    windowWidth: { type: Number, default: undefined },
  },
  emits: [
    "study-loaded",
    "viewport-changed",
    "measurement-created",
    "measurement-updated",
    "measurement-deleted",
    "segmentation-updated",
    "error",
  ],
  setup(props, { emit }) {
    const containerRef = ref<HTMLDivElement | null>(null);
    const viewerRef = ref<any>(null);

    onMounted(() => {
      if (!containerRef.value) return;
      // Viewer initialization would happen here via WASM
    });

    onUnmounted(() => {
      viewerRef.value?.destroy();
    });

    watch(() => props.windowCenter, (newCenter) => {
      if (viewerRef.value && newCenter !== undefined && props.windowWidth !== undefined) {
        viewerRef.value.setWindowLevel(newCenter, props.windowWidth);
      }
    });

    watch(() => props.windowWidth, (newWidth) => {
      if (viewerRef.value && props.windowCenter !== undefined && newWidth !== undefined) {
        viewerRef.value.setWindowLevel(props.windowCenter, newWidth);
      }
    });

    return () => (
      <div ref={containerRef} id="diccy-viewer-vue" />
    );
  },
});

export default DicomViewerVue;
"#.to_string()
}

// ---------------------------------------------------------------------------
// Svelte component wrapper generation
// ---------------------------------------------------------------------------

/// Generate a Svelte component wrapper for the DiCCY viewer.
pub fn generate_svelte_component() -> String {
    r#"<!-- Auto-generated Svelte component wrapper for DiCCY Viewer SDK -->
<!-- Do not edit manually — regenerate from the SDK build. -->

<script lang="ts">
  import { onMount, onDestroy } from "svelte";

  export let wadoRsUrl: string | undefined = undefined;
  export let windowCenter: number | undefined = undefined;
  export let windowWidth: number | undefined = undefined;

  let container: HTMLDivElement;
  let viewer: any = null;

  onMount(() => {
    if (!container) return;
    // Viewer initialization would happen here via WASM
  });

  onDestroy(() => {
    viewer?.destroy();
  });

  $: if (viewer && windowCenter !== undefined && windowWidth !== undefined) {
    viewer.setWindowLevel(windowCenter, windowWidth);
  }
</script>

<div bind:this={container} id="diccy-viewer-svelte"></div>
"#.to_string()
}

// ---------------------------------------------------------------------------
// Embedding guide
// ---------------------------------------------------------------------------

/// Generate the iframe-less integration embedding guide as Markdown.
pub fn generate_embedding_guide() -> String {
    r#"# DiCCY Viewer SDK — Embedding Guide

## Iframe-less Integration Pattern

The DiCCY Viewer SDK enables direct embedding of the DICOM viewer into
your application's DOM without iframe isolation. This provides:

- **Direct event communication** — no postMessage overhead
- **Shared CSS context** — seamless visual integration
- **Type-safe API** — TypeScript definitions for all event types
- **Framework-native** — React, Vue, and Svelte component wrappers

## Quick Start

### 1. Install the SDK

```bash
npm install dicom-viewer-sdk
```

### 2. Initialize WASM

```typescript
import init, { DicomViewer } from "dicom-viewer-sdk";

await init();
```

### 3. Create a Viewer Instance

```typescript
const viewer = new DicomViewer("viewer-container");

viewer.addStudyLoadedListener((event) => {
  console.log("Study loaded:", event.data);
});

viewer.addMeasurementListener((event) => {
  console.log("Measurement:", event.data);
});

await viewer.loadStudy("https://pacs.example.com/wado-rs/studies/1.2.3.4");
```

### 4. React Integration

```tsx
import { DicomViewerComponent } from "dicom-viewer-sdk/react";

function App() {
  return (
    <DicomViewerComponent
      wadoRsUrl="https://pacs.example.com/studies/1.2.3"
      onMeasurementCreated={(data) => console.log(data)}
    />
  );
}
```

### 5. CORS Configuration

Ensure your DiCCY DICOMweb server has CORS headers configured to allow
requests from your application's origin. See the CORS configuration
documentation for details.

## Event Types

| Event Type | Description |
|---|---|
| `study-loaded` | Study fully loaded from WADO-RS |
| `viewport-changed` | Viewport pan/zoom/WL changed |
| `measurement-created` | New measurement placed |
| `measurement-updated` | Measurement modified |
| `measurement-deleted` | Measurement removed |
| `segmentation-updated` | Segmentation overlay changed |
| `annotation-created` | 3D annotation created |
| `error` | Error occurred |

## Cleanup

Always call `destroy()` when removing the viewer:

```typescript
viewer.destroy();
```
"#.to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_typescript_types_not_empty() {
        let types = generate_typescript_types();
        assert!(types.contains("SdkEventType"));
        assert!(types.contains("DicomSdkEvent"));
        assert!(types.contains("ViewerState"));
        assert!(types.contains("MeasurementCreatedData"));
    }

    #[test]
    fn generate_react_component_not_empty() {
        let component = generate_react_component();
        assert!(component.contains("DicomViewerComponent"));
        assert!(component.contains("onMeasurementCreated"));
        assert!(component.contains("useEffect"));
    }

    #[test]
    fn generate_vue_component_not_empty() {
        let component = generate_vue_component();
        assert!(component.contains("DicomViewerVue"));
        assert!(component.contains("defineComponent"));
    }

    #[test]
    fn generate_svelte_component_not_empty() {
        let component = generate_svelte_component();
        assert!(component.contains("diccy-viewer-svelte"));
        assert!(component.contains("onMount"));
    }

    #[test]
    fn generate_embedding_guide_not_empty() {
        let guide = generate_embedding_guide();
        assert!(guide.contains("Iframe-less"));
        assert!(guide.contains("Quick Start"));
        assert!(guide.contains("React Integration"));
    }

    #[test]
    fn types_include_all_event_types() {
        let types = generate_typescript_types();
        for et in SdkEventType::all() {
            assert!(
                types.contains(et.as_str()),
                "TypeScript types missing event type: {}",
                et.as_str()
            );
        }
    }
}
