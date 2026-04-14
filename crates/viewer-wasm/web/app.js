import init, { WasmViewer, escapeMetadataHtml } from "./pkg/viewer_wasm.js";

const ui = {
  bootStatus: document.getElementById("boot_status"),
  ingestStatus: document.getElementById("ingest_status"),
  statWidth: document.getElementById("stat_width"),
  statHeight: document.getElementById("stat_height"),
  statZoom: document.getElementById("stat_zoom"),
  statNetwork: document.getElementById("stat_network"),
  statLoadedBytes: document.getElementById("stat_loaded_bytes"),
  statBackend: document.getElementById("stat_backend"),
  rendererBadge: document.getElementById("renderer_badge"),
  statGpuApi: document.getElementById("stat_gpu_api"),
  statGpuAdapter: document.getElementById("stat_gpu_adapter"),
  statGpuFeatures: document.getElementById("stat_gpu_features"),
  statGpuLimits: document.getElementById("stat_gpu_limits"),
  statRenderMs: document.getElementById("stat_render_ms"),
  statFrameWidth: document.getElementById("stat_frame_width"),
  statFrameHeight: document.getElementById("stat_frame_height"),
  dicomFile: document.getElementById("dicom_file"),
  dicomFolder: document.getElementById("dicom_folder"),
  seriesPrev: document.getElementById("series_prev"),
  seriesNext: document.getElementById("series_next"),
  seriesNavStatus: document.getElementById("series_nav_status"),
  comparisonGrid: document.getElementById("comparison_grid"),
  previewCanvas: document.getElementById("preview_canvas"),
  compareCanvasA: document.getElementById("compare_canvas_a"),
  compareCanvasB: document.getElementById("compare_canvas_b"),
  compareCanvasC: document.getElementById("compare_canvas_c"),
  previewStatus: document.getElementById("preview_status"),
  mprStatus: document.getElementById("mpr_status"),
  mprCrosshairX: document.getElementById("mpr_crosshair_x"),
  mprCrosshairY: document.getElementById("mpr_crosshair_y"),
  mprCrosshairZ: document.getElementById("mpr_crosshair_z"),
  mprOutputWidth: document.getElementById("mpr_output_width"),
  mprOutputHeight: document.getElementById("mpr_output_height"),
  mprSlabThickness: document.getElementById("mpr_slab_thickness"),
  mprSlabMode: document.getElementById("mpr_slab_mode"),
  modalityPreset: document.getElementById("modality_preset"),
  windowWidth: document.getElementById("window_width"),
  windowLevel: document.getElementById("window_level"),
  applyWindowLevel: document.getElementById("apply_window_level"),
  renderMpr: document.getElementById("render_mpr"),
  mprAxialCanvas: document.getElementById("mpr_axial_canvas"),
  mprCoronalCanvas: document.getElementById("mpr_coronal_canvas"),
  mprSagittalCanvas: document.getElementById("mpr_sagittal_canvas"),
  planeIndicatorAxial: document.getElementById("plane_indicator_axial"),
  planeIndicatorCoronal: document.getElementById("plane_indicator_coronal"),
  planeIndicatorSagittal: document.getElementById("plane_indicator_sagittal"),
  exportMprBundle: document.getElementById("export_mpr_bundle"),
  patientMprPlane: document.getElementById("patient_mpr_plane"),
  patientMprOffsetUm: document.getElementById("patient_mpr_offset_um"),
  runPatientMpr: document.getElementById("run_patient_mpr"),
  patientMprOutput: document.getElementById("patient_mpr_output"),
  viewportWidth: document.getElementById("viewport_width"),
  viewportHeight: document.getElementById("viewport_height"),
  viewportPreset: document.getElementById("viewport_preset"),
  applyViewportPreset: document.getElementById("apply_viewport_preset"),
  linkScrollToggle: document.getElementById("link_scroll_toggle"),
  linkWindowLevelToggle: document.getElementById("link_window_level_toggle"),
  applyResize: document.getElementById("apply_resize"),
  zoomValue: document.getElementById("zoom_value"),
  interpolationMode: document.getElementById("interpolation_mode"),
  setZoom: document.getElementById("set_zoom"),
  zoomOut: document.getElementById("zoom_out"),
  zoomIn: document.getElementById("zoom_in"),
  downloadPreview: document.getElementById("download_preview"),
  networkToggle: document.getElementById("network_toggle"),
  forceCpuToggle: document.getElementById("force_cpu_toggle"),
  metadataInput: document.getElementById("metadata_input"),
  metadataOutput: document.getElementById("metadata_output"),
  escapeMetadata: document.getElementById("escape_metadata"),
  eventLog: document.getElementById("event_log"),
  clearEventLog: document.getElementById("clear_event_log"),
  exportEventLog: document.getElementById("export_event_log"),
  exportSnapshot: document.getElementById("export_snapshot"),
  backendDiagnostics: document.getElementById("backend_diagnostics"),
  simulateDeviceLost: document.getElementById("simulate_device_lost"),
  recoverDevice: document.getElementById("recover_device"),
  studyCatalogStatus: document.getElementById("study_catalog_status"),
  studyCatalogFilter: document.getElementById("study_catalog_filter"),
  studyCatalogSort: document.getElementById("study_catalog_sort"),
  studyCatalogPageSize: document.getElementById("study_catalog_page_size"),
  studyCatalogPrevPage: document.getElementById("study_catalog_prev_page"),
  studyCatalogNextPage: document.getElementById("study_catalog_next_page"),
  studyCatalogSummary: document.getElementById("study_catalog_summary"),
  studyCatalogRefresh: document.getElementById("study_catalog_refresh"),
  studyCatalogList: document.getElementById("study_catalog_list"),
  studyCatalogPinnedList: document.getElementById("study_catalog_pinned_list"),
  studyCatalogPinnedSummary: document.getElementById("study_catalog_pinned_summary"),
  productionWebGpuToggle: document.getElementById("production_webgpu_toggle"),
  statTransitionCount: document.getElementById("stat_transition_count"),
  statLastTransitionReason: document.getElementById("stat_last_transition_reason"),
  statLastFallbackReason: document.getElementById("stat_last_fallback_reason"),
  statFallbackLatency: document.getElementById("stat_fallback_latency"),
  statFallbackBudget: document.getElementById("stat_fallback_budget"),
  statWebGpuPresent: document.getElementById("stat_webgpu_present"),
  statCpuPresent: document.getElementById("stat_cpu_present"),
  statUploadChunkBytes: document.getElementById("stat_upload_chunk_bytes"),
  backendStateMessage: document.getElementById("backend_state_message"),
  gpuRecoveryBanner: document.getElementById("gpu_recovery_banner"),
  gpuRecoveryAction: document.getElementById("gpu_recovery_action"),
  srBaseUrl: document.getElementById("sr_base_url"),
  srStudyUid: document.getElementById("sr_study_uid"),
  srSeriesUid: document.getElementById("sr_series_uid"),
  srSopUid: document.getElementById("sr_sop_uid"),
  srObserver: document.getElementById("sr_observer"),
  srKnownRefs: document.getElementById("sr_known_refs"),
  srExpectedVersion: document.getElementById("sr_expected_version"),
  srRole: document.getElementById("sr_role"),
  srItemKind: document.getElementById("sr_item_kind"),
  srNumValue: document.getElementById("sr_num_value"),
  srTextValue: document.getElementById("sr_text_value"),
  srConceptCode: document.getElementById("sr_concept_code"),
  srConceptScheme: document.getElementById("sr_concept_scheme"),
  srConceptMeaning: document.getElementById("sr_concept_meaning"),
  srUnitsCode: document.getElementById("sr_units_code"),
  srUnitsScheme: document.getElementById("sr_units_scheme"),
  srUnitsMeaning: document.getElementById("sr_units_meaning"),
  srCreate: document.getElementById("sr_create"),
  srUpdate: document.getElementById("sr_update"),
  srLoad: document.getElementById("sr_load"),
  srList: document.getElementById("sr_list"),
  srListStudyUid: document.getElementById("sr_list_study_uid"),
  srListQuery: document.getElementById("sr_list_query"),
  srStatus: document.getElementById("sr_status"),
  srOutput: document.getElementById("sr_output"),
  srListOutput: document.getElementById("sr_list_output"),
  srDetailOutput: document.getElementById("sr_detail_output"),
  srAuditTimeline: document.getElementById("sr_audit_timeline"),
  activeTool: document.getElementById("active_tool"),
  dicomwebBaseUrl: document.getElementById("dicomweb_base_url"),
  toolPan: document.getElementById("tool_pan"),
  toolMeasure: document.getElementById("tool_measure"),
  restoreSession: document.getElementById("restore_session"),
  checkConnectivity: document.getElementById("check_connectivity"),
  connectDicomweb: document.getElementById("connect_dicomweb"),
  connectWorkflow: document.getElementById("connect_workflow"),
  workstationStatus: document.getElementById("workstation_status"),
  patientSafeMode: document.getElementById("patient_safe_mode"),
  measurementList: document.getElementById("measurement_list"),
  clearMeasurements: document.getElementById("clear_measurements"),
  exportMeasurementsCsv: document.getElementById("export_measurements_csv"),
  exportMeasurementsJson: document.getElementById("export_measurements_json"),
  fusionAlpha: document.getElementById("fusion_alpha"),
  fusionColormap: document.getElementById("fusion_colormap"),
  fusionSuvScale: document.getElementById("fusion_suv_scale"),
  runFusion: document.getElementById("run_fusion"),
  exportFusionBundle: document.getElementById("export_fusion_bundle"),
  fusionCanvas: document.getElementById("fusion_canvas"),
  fusionStatus: document.getElementById("fusion_status"),
  fusionDiagOutput: document.getElementById("fusion_diag_output"),
  fusionQuantOutput: document.getElementById("fusion_quant_output"),
  onboardingStatus: document.getElementById("onboarding_status"),
};

let viewer = null;
let backendLabel = "Detecting compute path...";
let backendProbe = {
  webgpuApi: false,
  adapterAvailable: false,
  webgl2Api: false,
  maxTextureDimension2d: 0,
};
let backendCapabilities = {
  api: "Detecting compute path...",
  adapter: "-",
  features: "-",
  limits: "-",
};
let previewCtx = null;
const comparisonCtx = [
  ui.compareCanvasA?.getContext("2d") ?? null,
  ui.compareCanvasB?.getContext("2d") ?? null,
  ui.compareCanvasC?.getContext("2d") ?? null,
];
const mprCtx = {
  axial: ui.mprAxialCanvas?.getContext("2d") ?? null,
  coronal: ui.mprCoronalCanvas?.getContext("2d") ?? null,
  sagittal: ui.mprSagittalCanvas?.getContext("2d") ?? null,
};
const fusionCtx = ui.fusionCanvas?.getContext("2d") ?? null;

const PRODUCTION_WEBGPU_STORAGE_KEY = "rdvf.viewer_wasm.production_webgpu";
const STUDY_CATALOG_STORAGE_KEY = "rdvf.viewer_wasm.study_catalog.v1";
const STUDY_CATALOG_SORT_DEFAULT = "study_uid_asc";
const STUDY_CATALOG_SORT_OPTIONS = new Set([
  "study_uid_asc",
  "study_uid_desc",
  "series_uid_asc",
  "series_uid_desc",
  "sop_uid_asc",
  "sop_uid_desc",
]);
let productionWebGpuEnabled = false;
const webGpuPresenter = {
  adapter: null,
  device: null,
  context: null,
  format: null,
  pipeline: null,
  sampler: null,
  texture: null,
  bindGroup: null,
  textureWidth: 0,
  textureHeight: 0,
  failed: false,
  lastError: null,
};

const SESSION_STORAGE_KEY = "rdvf.viewer_wasm.session.v1";
const WORKFLOW_RETRYABLE = new Set([408, 425, 429, 500, 502, 503, 504]);
const DICOMWEB_DEFAULT_PORT = "8080";
const WORKFLOW_DEFAULT_PORT = "8082";
const DICOMWEB_PROXY_PATH = "/dicomweb";
const WORKFLOW_PROXY_PATH = "/workflow";
let activeTool = "pan";
let viewportPreset = "single";
let latestPreviewFrame = null;
let latestMprPlanes = null;
let latestFusionFrame = null;
let lastLoadedSrDocument = null;
let connectivityTimer = null;
let measurementDraft = null;
let measurementCounter = 0;
let pendingFirstImageStart = null;
let interactionMetrics = {
  firstImageLatencyMs: null,
  totalInteractions: 0,
  lastInteractionMs: null,
};
let patientSafeMode = false;
let studyCatalogRows = [];
let studyCatalogFilter = "";
let studyCatalogSort = STUDY_CATALOG_SORT_DEFAULT;
let studyCatalogPageSize = 10;
let studyCatalogPage = 1;
const studyCatalogPinned = new Set();
let studyCatalogLastOpenedUid = null;
const measurements = [];
let viewerOpQueue = Promise.resolve();
let loadedSeriesFiles = [];
let loadedSeriesIndex = -1;

function withViewerLock(operation) {
  const run = viewerOpQueue.then(() => operation());
  viewerOpQueue = run.catch(() => undefined);
  return run;
}

function updateSeriesNavigatorStatus() {
  if (!ui.seriesNavStatus) {
    return;
  }
  const total = loadedSeriesFiles.length;
  if (total === 0) {
    ui.seriesNavStatus.textContent = "No image stack loaded.";
    return;
  }
  if (loadedSeriesIndex < 0 || loadedSeriesIndex >= total) {
    ui.seriesNavStatus.textContent = `Stack selected (${total} files).`;
    return;
  }
  const file = loadedSeriesFiles[loadedSeriesIndex];
  ui.seriesNavStatus.textContent = `Image ${loadedSeriesIndex + 1} / ${total}: ${file?.name || "unnamed"}`;
}

function updateSeriesNavigatorButtons() {
  const total = loadedSeriesFiles.length;
  if (ui.seriesPrev) {
    ui.seriesPrev.disabled = total === 0 || loadedSeriesIndex <= 0;
  }
  if (ui.seriesNext) {
    ui.seriesNext.disabled = total === 0 || loadedSeriesIndex < 0 || loadedSeriesIndex >= total - 1;
  }
  updateSeriesNavigatorStatus();
}

function resetSeriesStack(files) {
  loadedSeriesFiles = files;
  loadedSeriesIndex = -1;
  updateSeriesNavigatorButtons();
}

async function tryLoadSeriesFileAt(index) {
  if (!viewer || index < 0 || index >= loadedSeriesFiles.length) {
    return { ok: false, unreadable: false, file: null, bytesLength: 0 };
  }
  const file = loadedSeriesFiles[index];
  let bytes;
  try {
    bytes = new Uint8Array(await file.arrayBuffer());
  } catch (_error) {
    return { ok: false, unreadable: true, file, bytesLength: 0 };
  }
  const ok = await withViewerLock(() => viewer.load_bytes(bytes));
  if (ok) {
    loadedSeriesIndex = index;
    updateSeriesNavigatorButtons();
  }
  return { ok, unreadable: false, file, bytesLength: bytes.length };
}

function parseBooleanFlag(value) {
  const normalized = (value || "").trim().toLowerCase();
  if (normalized === "1" || normalized === "true" || normalized === "on" || normalized === "prod") {
    return true;
  }
  if (normalized === "0" || normalized === "false" || normalized === "off") {
    return false;
  }
  return null;
}

function resolveInitialProductionWebGpuFlag() {
  if (typeof window !== "undefined") {
    const urlValue = new URLSearchParams(window.location.search).get("webgpu");
    const parsedFromUrl = parseBooleanFlag(urlValue);
    if (parsedFromUrl !== null) {
      return parsedFromUrl;
    }
  }
  return true;
}

function persistProductionWebGpuFlag(enabled) {
  try {
    window.localStorage.setItem(PRODUCTION_WEBGPU_STORAGE_KEY, enabled ? "true" : "false");
  } catch (_error) {
    // Ignore storage errors in restricted browser contexts.
  }
}

function normalizeHostForEndpoint(hostname) {
  return (hostname || "").trim().replace(/^\[|\]$/g, "").toLowerCase();
}

function isLoopbackHost(hostname) {
  const normalized = normalizeHostForEndpoint(hostname);
  return normalized === "localhost" || normalized === "127.0.0.1" || normalized === "::1";
}

function defaultServiceBaseUrl(proxyPath, fallbackPort) {
  if (typeof window === "undefined") {
    return `http://127.0.0.1:${fallbackPort}`;
  }
  if (window.location.origin && window.location.origin !== "null") {
    return `${window.location.origin}${proxyPath}`;
  }
  const protocol = window.location.protocol === "https:" ? "https:" : "http:";
  const host = window.location.hostname || "127.0.0.1";
  return `${protocol}//${host}:${fallbackPort}`;
}

function normalizeServiceBaseUrl(rawValue, { proxyPath, fallbackPort, legacyPort }) {
  const fallback = defaultServiceBaseUrl(proxyPath, fallbackPort).replace(/\/+$/, "");
  const candidate = (rawValue || "").trim();
  if (!candidate) {
    return fallback;
  }
  if (typeof window === "undefined") {
    return candidate.replace(/\/+$/, "");
  }

  let parsed = null;
  try {
    parsed = new URL(candidate, window.location.origin);
  } catch (_error) {
    return fallback;
  }

  const pageHost = normalizeHostForEndpoint(window.location.hostname || "");
  const targetHost = normalizeHostForEndpoint(parsed.hostname || "");

  if (targetHost && isLoopbackHost(targetHost) && pageHost && !isLoopbackHost(pageHost)) {
    return fallback;
  }

  if (targetHost && isLoopbackHost(targetHost) && parsed.port === legacyPort) {
    return fallback;
  }

  if (window.location.protocol === "https:" && parsed.protocol === "http:") {
    if (parsed.port === legacyPort && (targetHost === pageHost || isLoopbackHost(targetHost))) {
      return fallback;
    }
    parsed.protocol = "https:";
  }

  return parsed.toString().replace(/\/+$/, "");
}

function readServiceBaseUrl(inputElement, options) {
  const normalized = normalizeServiceBaseUrl(inputElement?.value, options);
  if (inputElement && inputElement.value !== normalized) {
    inputElement.value = normalized;
  }
  return normalized;
}

function normalizeServiceEndpointInputs() {
  readServiceBaseUrl(ui.dicomwebBaseUrl, {
    proxyPath: DICOMWEB_PROXY_PATH,
    fallbackPort: DICOMWEB_DEFAULT_PORT,
    legacyPort: DICOMWEB_DEFAULT_PORT,
  });
  readServiceBaseUrl(ui.srBaseUrl, {
    proxyPath: WORKFLOW_PROXY_PATH,
    fallbackPort: WORKFLOW_DEFAULT_PORT,
    legacyPort: WORKFLOW_DEFAULT_PORT,
  });
}

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}

function deterministicHash(input) {
  let hash = 0x811c9dc5;
  for (let i = 0; i < input.length; i += 1) {
    hash ^= input.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash.toString(16).padStart(8, "0");
}

function recordInteractionEvent() {
  interactionMetrics.totalInteractions += 1;
  interactionMetrics.lastInteractionMs = new Date().toISOString();
}

function downloadBlob(filename, blob) {
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.rel = "noopener";
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  URL.revokeObjectURL(url);
}

function downloadText(filename, text, mime = "text/plain;charset=utf-8") {
  downloadBlob(filename, new Blob([text], { type: mime }));
}

function srRole() {
  return (ui.srRole?.value || "writer").trim().toLowerCase();
}

function currentWindowLevel() {
  const width = clamp(Number.parseFloat(ui.windowWidth?.value || "400"), 1, 8192);
  const level = clamp(Number.parseFloat(ui.windowLevel?.value || "40"), -4096, 4096);
  return { width, level };
}

function setWindowLevel(width, level) {
  if (ui.windowWidth) {
    ui.windowWidth.value = String(Math.round(width));
  }
  if (ui.windowLevel) {
    ui.windowLevel.value = String(Math.round(level));
  }
}

function isPatientSafeMode() {
  return Boolean(patientSafeMode);
}

function applyModalityPreset() {
  const preset = (ui.modalityPreset?.value || "ct_soft").trim();
  if (preset === "ct_lung") {
    setWindowLevel(1500, -600);
  } else if (preset === "mr_t1") {
    setWindowLevel(450, 100);
  } else if (preset === "mr_t2") {
    setWindowLevel(650, 180);
  } else if (preset === "pet_overlay") {
    setWindowLevel(900, 300);
  } else {
    setWindowLevel(400, 40);
  }
}

function applyWindowLevelRgba(rgba, width, level) {
  const out = new Uint8Array(rgba.length);
  const low = level - width / 2;
  const high = level + width / 2;
  const denom = Math.max(1, high - low);
  for (let i = 0; i < rgba.length; i += 4) {
    const value = rgba[i];
    const normalized = clamp((value - low) / denom, 0, 1);
    const mapped = Math.round(normalized * 255);
    out[i] = mapped;
    out[i + 1] = mapped;
    out[i + 2] = mapped;
    out[i + 3] = rgba[i + 3];
  }
  return out;
}

function channelRange(rgba) {
  if (!rgba || rgba.length < 4) {
    return 0;
  }
  let min = 255;
  let max = 0;
  for (let i = 0; i < rgba.length; i += 4) {
    const value = rgba[i];
    if (value < min) {
      min = value;
    }
    if (value > max) {
      max = value;
    }
  }
  return max - min;
}

function shouldFallbackToRawPreview(rawRgba, wlRgba) {
  // If WL output is effectively flat but raw frame has contrast, keep the raw image visible.
  return channelRange(wlRgba) <= 2 && channelRange(rawRgba) > 2;
}

function nowTag() {
  return new Date().toISOString().replace(/[:.]/g, "-");
}

function stableArtifactSuffix() {
  const seed = JSON.stringify({
    sop: ui.srSopUid?.value || "",
    preset: viewportPreset,
    wl: currentWindowLevel(),
    slab: {
      thickness: ui.mprSlabThickness?.value ?? "1",
      mode: ui.mprSlabMode?.value ?? "average",
    },
    fusion: {
      alpha: ui.fusionAlpha?.value ?? "0.45",
      colormap: ui.fusionColormap?.value ?? "hotiron",
      suvScale: ui.fusionSuvScale?.value ?? "5.0",
    },
  });
  return deterministicHash(seed);
}

function alignTo(value, alignment) {
  return Math.ceil(value / alignment) * alignment;
}

function packRgbaRowsForWebGpu(rgba, width, height) {
  const bytesPerRow = width * 4;
  const alignedBytesPerRow = alignTo(bytesPerRow, 256);
  if (alignedBytesPerRow === bytesPerRow) {
    return { data: rgba, bytesPerRow: alignedBytesPerRow };
  }
  const packed = new Uint8Array(alignedBytesPerRow * height);
  for (let y = 0; y < height; y += 1) {
    const srcOffset = y * bytesPerRow;
    const dstOffset = y * alignedBytesPerRow;
    packed.set(rgba.subarray(srcOffset, srcOffset + bytesPerRow), dstOffset);
  }
  return { data: packed, bytesPerRow: alignedBytesPerRow };
}

async function ensureWebGpuPresenter(width, height) {
  if (!viewer || !ui.previewCanvas || typeof navigator === "undefined" || typeof navigator.gpu !== "object") {
    return false;
  }
  if (webGpuPresenter.failed) {
    return false;
  }
  try {
    if (!webGpuPresenter.device) {
      webGpuPresenter.adapter = await navigator.gpu.requestAdapter();
      if (!webGpuPresenter.adapter) {
        throw new Error("No compatible WebGPU adapter");
      }
      webGpuPresenter.device = await webGpuPresenter.adapter.requestDevice();
      webGpuPresenter.context = ui.previewCanvas.getContext("webgpu");
      if (!webGpuPresenter.context) {
        throw new Error("WebGPU canvas context unavailable");
      }
      const preferredFormat = navigator.gpu.getPreferredCanvasFormat
        ? navigator.gpu.getPreferredCanvasFormat()
        : "bgra8unorm";
      webGpuPresenter.format = preferredFormat;
      webGpuPresenter.context.configure({
        device: webGpuPresenter.device,
        format: webGpuPresenter.format,
        alphaMode: "premultiplied",
      });
      const shaderSource = viewer.csp_safe_shader_source();
      const shaderModule = webGpuPresenter.device.createShaderModule({ code: shaderSource });
      webGpuPresenter.pipeline = webGpuPresenter.device.createRenderPipeline({
        layout: "auto",
        vertex: {
          module: shaderModule,
          entryPoint: "vs_main",
        },
        fragment: {
          module: shaderModule,
          entryPoint: "fs_main",
          targets: [{ format: webGpuPresenter.format }],
        },
        primitive: {
          topology: "triangle-list",
        },
      });
      webGpuPresenter.sampler = webGpuPresenter.device.createSampler({
        magFilter: "linear",
        minFilter: "linear",
      });
      webGpuPresenter.device.lost.then((info) => {
        webGpuPresenter.failed = true;
        webGpuPresenter.lastError = `device lost: ${info?.message ?? info?.reason ?? "unknown"}`;
      });
    }

    if (webGpuPresenter.texture && (webGpuPresenter.textureWidth !== width || webGpuPresenter.textureHeight !== height)) {
      webGpuPresenter.texture.destroy();
      webGpuPresenter.texture = null;
      webGpuPresenter.bindGroup = null;
    }

    if (!webGpuPresenter.texture) {
      webGpuPresenter.texture = webGpuPresenter.device.createTexture({
        size: { width, height, depthOrArrayLayers: 1 },
        format: "rgba8unorm",
        usage: GPUTextureUsage.COPY_DST | GPUTextureUsage.TEXTURE_BINDING,
      });
      const textureView = webGpuPresenter.texture.createView();
      webGpuPresenter.bindGroup = webGpuPresenter.device.createBindGroup({
        layout: webGpuPresenter.pipeline.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: webGpuPresenter.sampler },
          { binding: 1, resource: textureView },
        ],
      });
      webGpuPresenter.textureWidth = width;
      webGpuPresenter.textureHeight = height;
    }
    webGpuPresenter.lastError = null;
    return true;
  } catch (error) {
    webGpuPresenter.failed = true;
    webGpuPresenter.lastError = error instanceof Error ? error.message : String(error);
    return false;
  }
}

async function drawPreviewWithWebGpu(rgba, width, height) {
  const ready = await ensureWebGpuPresenter(width, height);
  if (!ready) {
    return false;
  }
  try {
    const upload = packRgbaRowsForWebGpu(rgba, width, height);
    webGpuPresenter.device.queue.writeTexture(
      { texture: webGpuPresenter.texture },
      upload.data,
      {
        offset: 0,
        bytesPerRow: upload.bytesPerRow,
        rowsPerImage: height,
      },
      {
        width,
        height,
        depthOrArrayLayers: 1,
      },
    );
    const encoder = webGpuPresenter.device.createCommandEncoder();
    const pass = encoder.beginRenderPass({
      colorAttachments: [
        {
          view: webGpuPresenter.context.getCurrentTexture().createView(),
          clearValue: { r: 0, g: 0, b: 0, a: 1 },
          loadOp: "clear",
          storeOp: "store",
        },
      ],
    });
    pass.setPipeline(webGpuPresenter.pipeline);
    pass.setBindGroup(0, webGpuPresenter.bindGroup);
    pass.draw(3);
    pass.end();
    webGpuPresenter.device.queue.submit([encoder.finish()]);
    webGpuPresenter.lastError = null;
    return true;
  } catch (error) {
    webGpuPresenter.lastError = error instanceof Error ? error.message : String(error);
    webGpuPresenter.failed = true;
    return false;
  }
}

function getPreview2dContext() {
  if (previewCtx) {
    return previewCtx;
  }
  if (!ui.previewCanvas) {
    return null;
  }
  previewCtx = ui.previewCanvas.getContext("2d");
  return previewCtx;
}

function ensurePreviewBindings(boundary) {
  const required = [
    "has_preview",
    "preview_width",
    "preview_height",
    "set_interpolation_mode",
    "interpolation_mode",
    "recenter_from_viewport_fraction",
    "render_viewport_rgba_bytes_for_size",
    "configure_renderer_backend",
    "active_renderer_backend",
    "backend_capability_probe_json",
    "backend_selection_metrics_json",
    "set_force_cpu_renderer",
    "set_production_webgpu_renderer",
    "production_webgpu_renderer_enabled",
    "set_render_frame_interval_ms",
    "recover_gpu_device",
    "csp_safe_shader_source",
    "simulate_gpu_device_lost",
    "set_mpr_plane_index",
    "set_mpr_crosshair",
    "tri_planar_state_json",
    "mpr_plane_rgba_bytes",
    "set_mpr_slab",
    "mpr_slab_json",
    "mpr_reslice_patient_json",
  ];
  for (const name of required) {
    if (typeof boundary[name] !== "function") {
      throw new Error(
        `WASM package is stale (missing ${name}). Rebuild viewer-wasm package and reload.`,
      );
    }
  }
}

const statusCodeByKind = {
  ok: "UI-OK",
  pending: "UI-PENDING",
  error: "UI-ERR",
};

function pushEvent(kind, code, message) {
  if (!ui.eventLog) {
    return;
  }
  const entry = document.createElement("li");
  entry.className = `event event-${kind}`;
  entry.textContent = `[${code}] ${message}`;
  ui.eventLog.appendChild(entry);
  while (ui.eventLog.children.length > 20) {
    ui.eventLog.removeChild(ui.eventLog.firstChild);
  }
}

function setStatus(target, kind, message, code) {
  const finalCode = code || statusCodeByKind[kind] || "UI-UNKNOWN";
  const label = `[${finalCode}] ${message}`;
  if (target) {
    target.className = `status status-${kind}`;
    target.textContent = label;
    pushEvent(kind, finalCode, `${target.id}: ${message}`);
  } else {
    pushEvent(kind, finalCode, `missing target: ${message}`);
  }
}

function setBackendStateMessage(message, kind = "pending", code = "UI-BACKEND-00") {
  if (!ui.backendStateMessage) {
    return;
  }
  setStatus(ui.backendStateMessage, kind, message, code);
}

function setGpuRecoveryBanner(visible) {
  if (!ui.gpuRecoveryBanner) {
    return;
  }
  if (visible) {
    ui.gpuRecoveryBanner.classList.remove("hidden-banner");
  } else {
    ui.gpuRecoveryBanner.classList.add("hidden-banner");
  }
}

function studyCatalogIdFromValue(raw) {
  if (!raw || typeof raw !== "object") {
    return null;
  }
  const candidate = (
    raw.StudyInstanceUID ||
    raw.study_instance_uid ||
    raw.studyUid ||
    raw.study_uid ||
    ""
  )
    .toString()
    .trim();
  return candidate || null;
}

function normalizeStudyCatalogRow(raw) {
  const study_uid = studyCatalogIdFromValue(raw);
  if (!study_uid) {
    return null;
  }
  return {
    study_uid,
    series_uid: (raw.SeriesInstanceUID || raw.series_instance_uid || raw.series_uid || "").toString().trim(),
    sop_uid: (raw.SOPInstanceUID || raw.sop_instance_uid || raw.sop_uid || "").toString().trim(),
  };
}

function clampStudyCatalogPage(page) {
  const value = Number.parseInt(page, 10);
  if (!Number.isFinite(value) || value < 1) {
    return 1;
  }
  return value;
}

function clampStudyCatalogPageSize(size) {
  const parsed = Number.parseInt(size, 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return 10;
  }
  return Math.max(5, Math.min(100, parsed));
}

function normalizeStudyCatalogRows(payload) {
  if (!Array.isArray(payload)) {
    return [];
  }
  const dedupe = new Map();
  for (const item of payload) {
    const normalized = normalizeStudyCatalogRow(item);
    if (!normalized) {
      continue;
    }
    if (!dedupe.has(normalized.study_uid)) {
      dedupe.set(normalized.study_uid, normalized);
    }
  }
  return [...dedupe.values()];
}

function studyCatalogSortableValue(row, field) {
  if (field === "series_uid") {
    return row.series_uid || "";
  }
  if (field === "sop_uid") {
    return row.sop_uid || "";
  }
  return row.study_uid || "";
}

function sortStudyCatalogRows(rows) {
  const [field, dir] = studyCatalogSort.split("_");
  const asc = dir !== "desc";
  const sorted = rows.slice().sort((a, b) => {
    const av = studyCatalogSortableValue(a, field);
    const bv = studyCatalogSortableValue(b, field);
    if (av < bv) {
      return asc ? -1 : 1;
    }
    if (av > bv) {
      return asc ? 1 : -1;
    }
    if (a.study_uid === b.study_uid) {
      if (a.series_uid === b.series_uid) {
        return (a.sop_uid || "").localeCompare(b.sop_uid || "");
      }
      return (a.series_uid || "").localeCompare(b.series_uid || "");
    }
    return a.study_uid.localeCompare(b.study_uid);
  });
  return sorted;
}

function applyStudyCatalogFilter(rows) {
  const needle = studyCatalogFilter.trim().toLowerCase();
  if (!needle) {
    return rows.slice();
  }
  return rows.filter((row) => {
    const haystack = `${row.study_uid} ${row.series_uid} ${row.sop_uid}`.toLowerCase();
    return haystack.includes(needle);
  });
}

function ensureStudyCatalogPageInRange(totalPages) {
  const normalizedTotal = Math.max(1, totalPages);
  if (studyCatalogPage > normalizedTotal) {
    studyCatalogPage = normalizedTotal;
  }
}

function currentStudyCatalogRange(totalCount) {
  const pageSize = Math.max(1, studyCatalogPageSize || 1);
  const totalPages = Math.max(1, Math.ceil(totalCount / pageSize));
  ensureStudyCatalogPageInRange(totalPages);
  const start = (studyCatalogPage - 1) * pageSize;
  const end = Math.min(start + pageSize, totalCount);
  return {
    pageSize,
    totalPages,
    start,
    end,
    page: studyCatalogPage,
  };
}

function isPinnedStudy(studyUid) {
  return studyCatalogPinned.has(studyUid);
}

function restoreStudyCatalogState() {
  let state = null;
  try {
    state = JSON.parse(window.localStorage.getItem(STUDY_CATALOG_STORAGE_KEY) || "null");
  } catch (_error) {
    state = null;
  }
  if (!state || typeof state !== "object") {
    state = {};
  }
  studyCatalogFilter = typeof state.filter === "string" ? state.filter : "";
  studyCatalogSort = STUDY_CATALOG_SORT_OPTIONS.has(state.sort) ? state.sort : STUDY_CATALOG_SORT_DEFAULT;
  studyCatalogPageSize = clampStudyCatalogPageSize(state.pageSize || 10);
  studyCatalogPage = clampStudyCatalogPage(state.page || 1);
  studyCatalogLastOpenedUid =
    typeof state.lastOpenedStudyUid === "string" && state.lastOpenedStudyUid.trim()
      ? state.lastOpenedStudyUid.trim()
      : null;
  studyCatalogPinned.clear();
  if (Array.isArray(state.pinnedStudyUids)) {
    for (const uid of state.pinnedStudyUids) {
      if (typeof uid === "string" && uid.trim()) {
        studyCatalogPinned.add(uid.trim());
      }
    }
  }
  if (ui.studyCatalogFilter) {
    ui.studyCatalogFilter.value = studyCatalogFilter;
  }
  if (ui.studyCatalogSort) {
    ui.studyCatalogSort.value = studyCatalogSort;
  }
  if (ui.studyCatalogPageSize) {
    ui.studyCatalogPageSize.value = String(studyCatalogPageSize);
  }
}

function persistStudyCatalogState() {
  try {
    window.localStorage.setItem(
      STUDY_CATALOG_STORAGE_KEY,
      JSON.stringify({
        filter: studyCatalogFilter,
        sort: studyCatalogSort,
        pageSize: studyCatalogPageSize,
        page: studyCatalogPage,
        pinnedStudyUids: [...studyCatalogPinned],
        lastOpenedStudyUid: studyCatalogLastOpenedUid,
      }),
    );
  } catch (_error) {
    // Ignore storage failures in restricted browser contexts.
  }
}

function renderStudyCatalogSummary(filteredCount, range) {
  if (!ui.studyCatalogSummary) {
    return;
  }
  const startDisplay = filteredCount === 0 ? 0 : range.start + 1;
  ui.studyCatalogSummary.textContent = `${filteredCount} studies • page ${range.page}/${range.totalPages} (${startDisplay}-${range.end})`;
}

function toggleStudyPin(studyUid) {
  if (!studyUid) {
    return;
  }
  if (studyCatalogPinned.has(studyUid)) {
    studyCatalogPinned.delete(studyUid);
  } else {
    studyCatalogPinned.add(studyUid);
  }
  persistStudyCatalogState();
  renderStudyCatalogRows();
  renderPinnedStudies();
}

function openStudy(studyUid) {
  const resolved = studyCatalogRows.find((row) => row.study_uid === studyUid);
  if (!resolved) {
    setStatus(ui.workstationStatus, "error", "Selected study no longer exists in catalog cache.", "UI-CATALOG-02");
    return;
  }
  if (ui.srStudyUid) {
    ui.srStudyUid.value = resolved.study_uid;
  }
  if (ui.srSeriesUid) {
    ui.srSeriesUid.value = resolved.series_uid || "";
  }
  if (ui.srSopUid) {
    ui.srSopUid.value = resolved.sop_uid || "";
  }
  if (ui.srListStudyUid) {
    ui.srListStudyUid.value = resolved.study_uid;
  }
  if (ui.srListQuery) {
    ui.srListQuery.value = resolved.study_uid;
  }
  studyCatalogLastOpenedUid = resolved.study_uid;
  persistSessionState();
  persistStudyCatalogState();
  setStatus(
    ui.workstationStatus,
    "ok",
    `Study selected: ${resolved.study_uid}. SR workflow fields populated from catalog row.`,
    "UI-CATALOG-03",
  );
  renderStudyCatalogRows();
  renderPinnedStudies();
}

function restoreLastOpenedStudyFromCatalog() {
  if (!studyCatalogLastOpenedUid) {
    return false;
  }
  const resolved = studyCatalogRows.find((row) => row.study_uid === studyCatalogLastOpenedUid);
  if (!resolved) {
    return false;
  }
  openStudy(resolved.study_uid);
  return true;
}

function appendStudyCatalogRowElement(container, row, isPinnedEntry = false) {
  const rowEl = document.createElement("li");
  rowEl.className = "study-catalog-item";
  if (studyCatalogLastOpenedUid && row.study_uid === studyCatalogLastOpenedUid) {
    rowEl.classList.add("study-catalog-item-active");
  }
  if (isPinnedEntry) {
    rowEl.className = `${rowEl.className} study-catalog-item-pinned`;
  }
  const rowTitle = document.createElement("h4");
  rowTitle.textContent = `Study ${row.study_uid}`;
  const rowMeta = document.createElement("div");
  rowMeta.className = "study-catalog-meta";
  rowMeta.innerHTML =
    `<div><span>Series UID</span><code>${row.series_uid || "—"}</code></div>` +
    `<div><span>SOP UID</span><code>${row.sop_uid || "—"}</code></div>`;
  const actions = document.createElement("div");
  actions.className = "study-catalog-item-actions";
  const openButton = document.createElement("button");
  openButton.type = "button";
  openButton.textContent = "Open study";
  openButton.addEventListener("click", () => openStudy(row.study_uid));
  const pinButton = document.createElement("button");
  const pinned = isPinnedStudy(row.study_uid);
  pinButton.type = "button";
  pinButton.textContent = pinned ? "Unpin" : "Pin";
  pinButton.className = pinned ? "study-catalog-unpin" : "study-catalog-pin";
  pinButton.addEventListener("click", () => toggleStudyPin(row.study_uid));
  actions.appendChild(openButton);
  actions.appendChild(pinButton);
  rowEl.appendChild(rowTitle);
  rowEl.appendChild(rowMeta);
  rowEl.appendChild(actions);
  container.appendChild(rowEl);
}

function renderPinnedStudies() {
  if (!ui.studyCatalogPinnedList) {
    return;
  }
  ui.studyCatalogPinnedList.innerHTML = "";
  const pinnedRows = [];
  const rowByUid = new Map(studyCatalogRows.map((row) => [row.study_uid, row]));
  for (const uid of studyCatalogPinned) {
    const row = rowByUid.get(uid) || { study_uid: uid, series_uid: "", sop_uid: "" };
    pinnedRows.push(row);
  }
  if (pinnedRows.length === 0) {
    const empty = document.createElement("li");
    empty.className = "event event-pending";
    empty.textContent = "No pinned studies.";
    ui.studyCatalogPinnedList.appendChild(empty);
    if (ui.studyCatalogPinnedSummary) {
      ui.studyCatalogPinnedSummary.textContent = "Pinned: 0";
    }
    return;
  }
  if (ui.studyCatalogPinnedSummary) {
    ui.studyCatalogPinnedSummary.textContent = `Pinned: ${pinnedRows.length}`;
  }
  pinnedRows.forEach((row) => appendStudyCatalogRowElement(ui.studyCatalogPinnedList, row, true));
}

function renderStudyCatalogRows() {
  if (!ui.studyCatalogList) {
    return;
  }
  const filtered = applyStudyCatalogFilter(studyCatalogRows);
  const sorted = sortStudyCatalogRows(filtered);
  const range = currentStudyCatalogRange(sorted.length);
  const visible = sorted.slice(range.start, range.end);
  ui.studyCatalogList.innerHTML = "";
  if (ui.studyCatalogPrevPage) {
    ui.studyCatalogPrevPage.disabled = range.page <= 1;
  }
  if (ui.studyCatalogNextPage) {
    ui.studyCatalogNextPage.disabled = range.page >= range.totalPages;
  }
  if (visible.length === 0) {
    const empty = document.createElement("li");
    empty.className = "event event-pending";
    empty.textContent = "No studies match the current filters.";
    ui.studyCatalogList.appendChild(empty);
    renderStudyCatalogSummary(0, range);
    return;
  }
  visible.forEach((row) => appendStudyCatalogRowElement(ui.studyCatalogList, row, false));
  renderStudyCatalogSummary(sorted.length, range);
}

async function refreshStudyCatalog() {
  const baseUrl = dicomwebBaseUrl();
  if (!baseUrl) {
    setStatus(ui.studyCatalogStatus, "error", "DICOMweb base URL is required to load study catalog.", "UI-CATALOG-04");
    return;
  }
  setStatus(ui.studyCatalogStatus, "pending", "Loading study catalog from DICOMweb...", "UI-CATALOG-05");
  try {
    const response = await fetch(`${baseUrl}/studies`, { method: "GET" });
    if (!response.ok) {
      setStatus(ui.studyCatalogStatus, "error", `Catalog load failed (${response.status}).`, "UI-CATALOG-06");
      studyCatalogRows = [];
      renderStudyCatalogRows();
      renderPinnedStudies();
      return;
    }
    const payloadRaw = await response.text();
    let payload = [];
    try {
      payload = JSON.parse(payloadRaw);
    } catch (_error) {
      payload = [];
    }
    studyCatalogRows = normalizeStudyCatalogRows(payload);
    studyCatalogPage = Math.max(1, studyCatalogPage);
    if (studyCatalogLastOpenedUid && !studyCatalogRows.some((row) => row.study_uid === studyCatalogLastOpenedUid)) {
      studyCatalogLastOpenedUid = null;
    }
    renderStudyCatalogRows();
    renderPinnedStudies();
    if (studyCatalogLastOpenedUid) {
      restoreLastOpenedStudyFromCatalog();
    }
    if (ui.studyCatalogFilter) {
      ui.studyCatalogFilter.value = studyCatalogFilter;
    }
    persistStudyCatalogState();
    setStatus(ui.studyCatalogStatus, "ok", `Loaded ${studyCatalogRows.length} studies.`, "UI-CATALOG-07");
  } catch (error) {
    setStatus(
      ui.studyCatalogStatus,
      "error",
      `Catalog load failed: ${error instanceof Error ? error.message : String(error)}`,
      "UI-CATALOG-08",
    );
    studyCatalogRows = [];
    renderStudyCatalogRows();
    renderPinnedStudies();
  }
}

function updateActiveTool(tool) {
  activeTool = tool === "measure" ? "measure" : "pan";
  if (ui.activeTool) {
    ui.activeTool.value = activeTool;
  }
  if (ui.previewCanvas) {
    ui.previewCanvas.style.cursor = activeTool === "measure" ? "crosshair" : "grab";
  }
  setStatus(
    ui.workstationStatus,
    "ok",
    `Active tool set to ${activeTool}.`,
    activeTool === "measure" ? "UI-TOOL-02" : "UI-TOOL-01",
  );
}

function setViewportPreset(nextPreset) {
  const preset = nextPreset === "dual" || nextPreset === "quad" ? nextPreset : "single";
  viewportPreset = preset;
  if (ui.viewportPreset) {
    ui.viewportPreset.value = preset;
  }
  if (ui.comparisonGrid) {
    ui.comparisonGrid.classList.remove("preset-single", "preset-dual", "preset-quad");
    ui.comparisonGrid.classList.add(`preset-${preset}`);
  }
}

function updateMeasurementList() {
  if (!ui.measurementList) {
    return;
  }
  ui.measurementList.innerHTML = "";
  for (const measurement of measurements) {
    const item = document.createElement("li");
    item.className = "event event-ok";
    const status = measurement.reviewed ? "reviewed" : "unreviewed";
    item.textContent =
      `#${measurement.id} ${measurement.kind} length=${measurement.length_px.toFixed(2)} px ` +
      `status=${status} start=(${measurement.start.x},${measurement.start.y}) end=(${measurement.end.x},${measurement.end.y})`;
    const actions = document.createElement("div");
    actions.className = "button-row";
    const reviewButton = document.createElement("button");
    reviewButton.type = "button";
    reviewButton.textContent = measurement.reviewed ? "Unreview" : "Mark reviewed";
    reviewButton.addEventListener("click", () => {
      measurement.reviewed = !measurement.reviewed;
      persistSessionState();
      updateMeasurementList();
    });
    const deleteButton = document.createElement("button");
    deleteButton.type = "button";
    deleteButton.textContent = "Delete";
    deleteButton.addEventListener("click", () => {
      const index = measurements.findIndex((item) => item.id === measurement.id);
      if (index >= 0) {
        measurements.splice(index, 1);
        persistSessionState();
        updateMeasurementList();
      }
    });
    actions.appendChild(reviewButton);
    actions.appendChild(deleteButton);
    item.appendChild(actions);
    ui.measurementList.appendChild(item);
  }
  if (measurements.length === 0) {
    const empty = document.createElement("li");
    empty.className = "event event-pending";
    empty.textContent = "No measurements captured.";
    ui.measurementList.appendChild(empty);
  }
}

function addMeasurement(start, end) {
  measurementCounter += 1;
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const lengthPx = Math.sqrt(dx * dx + dy * dy);
  measurements.push({
    id: measurementCounter,
    kind: "line",
    created_at: new Date().toISOString(),
    reviewed: false,
    start,
    end,
    length_px: lengthPx,
  });
  persistSessionState();
  updateMeasurementList();
}

function clearMeasurements() {
  measurementDraft = null;
  measurements.splice(0, measurements.length);
  persistSessionState();
  updateMeasurementList();
  setStatus(ui.workstationStatus, "ok", "Measurements cleared.", "UI-MEASURE-03");
}

function exportMeasurements(format) {
  if (measurements.length === 0) {
    setStatus(ui.workstationStatus, "pending", "No measurements available for export.", "UI-MEASURE-04");
    return;
  }
  const suffix = stableArtifactSuffix();
  if (format === "csv") {
    const rows = [
      "id,kind,created_at,reviewed,start_x,start_y,end_x,end_y,length_px",
      ...measurements.map((m) =>
        [
          m.id,
          m.kind,
          m.created_at,
          m.reviewed ? "1" : "0",
          m.start.x,
          m.start.y,
          m.end.x,
          m.end.y,
          m.length_px.toFixed(3),
        ].join(","),
      ),
    ];
    downloadText(`rdvf-measurements-${suffix}.csv`, `${rows.join("\n")}\n`, "text/csv;charset=utf-8");
  } else {
    downloadText(
      `rdvf-measurements-${suffix}.json`,
      `${JSON.stringify({ measurements }, null, 2)}\n`,
      "application/json;charset=utf-8",
    );
  }
  setStatus(ui.workstationStatus, "ok", `Measurements exported (${format}).`, "UI-MEASURE-05");
}

function sessionPayload() {
  return {
    timestamp: new Date().toISOString(),
    ui: {
      patientSafeMode,
    },
    study: {
      lastOpenedStudyUid: studyCatalogLastOpenedUid,
    },
    viewportPreset,
    activeTool,
    viewport: {
      width: ui.viewportWidth?.value ?? "1024",
      height: ui.viewportHeight?.value ?? "768",
      zoom: ui.zoomValue?.value ?? "1.0",
      interpolation: ui.interpolationMode?.value ?? "linear",
    },
    windowLevel: {
      width: ui.windowWidth?.value ?? "400",
      level: ui.windowLevel?.value ?? "40",
      modalityPreset: ui.modalityPreset?.value ?? "ct_soft",
    },
    crosshair: {
      x: ui.mprCrosshairX?.value ?? "0",
      y: ui.mprCrosshairY?.value ?? "0",
      z: ui.mprCrosshairZ?.value ?? "0",
    },
    mpr: {
      outputWidth: ui.mprOutputWidth?.value ?? "256",
      outputHeight: ui.mprOutputHeight?.value ?? "256",
      slabThickness: ui.mprSlabThickness?.value ?? "1",
      slabMode: ui.mprSlabMode?.value ?? "average",
    },
    sr: {
      baseUrl: srBaseUrl(),
      studyUid: ui.srStudyUid?.value ?? "",
      seriesUid: ui.srSeriesUid?.value ?? "",
      sopUid: ui.srSopUid?.value ?? "",
      expectedVersion: ui.srExpectedVersion?.value ?? "1",
      role: ui.srRole?.value ?? "writer",
    },
    connectivity: {
      dicomweb: dicomwebBaseUrl(),
    },
    measurements,
  };
}

function persistSessionState() {
  try {
    window.localStorage.setItem(SESSION_STORAGE_KEY, JSON.stringify(sessionPayload()));
  } catch (_error) {
    // Ignore storage failures in restricted contexts.
  }
}

function restoreSessionState() {
  let parsed = null;
  try {
    parsed = JSON.parse(window.localStorage.getItem(SESSION_STORAGE_KEY) || "null");
  } catch (_error) {
    parsed = null;
  }
  if (!parsed || typeof parsed !== "object") {
    setStatus(ui.workstationStatus, "pending", "No stored session snapshot found.", "UI-SESSION-01");
    return false;
  }
  if (ui.viewportWidth) ui.viewportWidth.value = parsed.viewport?.width ?? ui.viewportWidth.value;
  if (ui.viewportHeight) ui.viewportHeight.value = parsed.viewport?.height ?? ui.viewportHeight.value;
  if (ui.zoomValue) ui.zoomValue.value = parsed.viewport?.zoom ?? ui.zoomValue.value;
  if (ui.interpolationMode) ui.interpolationMode.value = parsed.viewport?.interpolation ?? ui.interpolationMode.value;
  if (ui.windowWidth) ui.windowWidth.value = parsed.windowLevel?.width ?? ui.windowWidth.value;
  if (ui.windowLevel) ui.windowLevel.value = parsed.windowLevel?.level ?? ui.windowLevel.value;
  if (ui.modalityPreset) ui.modalityPreset.value = parsed.windowLevel?.modalityPreset ?? ui.modalityPreset.value;
  if (ui.mprCrosshairX) ui.mprCrosshairX.value = parsed.crosshair?.x ?? ui.mprCrosshairX.value;
  if (ui.mprCrosshairY) ui.mprCrosshairY.value = parsed.crosshair?.y ?? ui.mprCrosshairY.value;
  if (ui.mprCrosshairZ) ui.mprCrosshairZ.value = parsed.crosshair?.z ?? ui.mprCrosshairZ.value;
  if (ui.mprOutputWidth) ui.mprOutputWidth.value = parsed.mpr?.outputWidth ?? ui.mprOutputWidth.value;
  if (ui.mprOutputHeight) ui.mprOutputHeight.value = parsed.mpr?.outputHeight ?? ui.mprOutputHeight.value;
  if (ui.mprSlabThickness) ui.mprSlabThickness.value = parsed.mpr?.slabThickness ?? ui.mprSlabThickness.value;
  if (ui.mprSlabMode) ui.mprSlabMode.value = parsed.mpr?.slabMode ?? ui.mprSlabMode.value;
  if (ui.srBaseUrl) ui.srBaseUrl.value = parsed.sr?.baseUrl ?? ui.srBaseUrl.value;
  if (ui.srStudyUid) ui.srStudyUid.value = parsed.sr?.studyUid ?? ui.srStudyUid.value;
  if (ui.srSeriesUid) ui.srSeriesUid.value = parsed.sr?.seriesUid ?? ui.srSeriesUid.value;
  if (ui.srSopUid) ui.srSopUid.value = parsed.sr?.sopUid ?? ui.srSopUid.value;
  if (ui.srExpectedVersion) {
    ui.srExpectedVersion.value = parsed.sr?.expectedVersion ?? ui.srExpectedVersion.value;
  }
  if (ui.srRole) ui.srRole.value = parsed.sr?.role ?? ui.srRole.value;
  if (ui.dicomwebBaseUrl) {
    ui.dicomwebBaseUrl.value = parsed.connectivity?.dicomweb ?? ui.dicomwebBaseUrl.value;
  }
  normalizeServiceEndpointInputs();
  patientSafeMode = parsed.ui?.patientSafeMode === true;
  if (ui.patientSafeMode) {
    ui.patientSafeMode.checked = patientSafeMode;
  }
  if (typeof parsed.study?.lastOpenedStudyUid === "string") {
    const next = parsed.study.lastOpenedStudyUid.trim();
    studyCatalogLastOpenedUid = next || null;
  }
  setViewportPreset(parsed.viewportPreset ?? viewportPreset);
  updateActiveTool(parsed.activeTool ?? activeTool);
  const restoredWidth = parsePositiveInteger(parsed.viewport?.width);
  const restoredHeight = parsePositiveInteger(parsed.viewport?.height);
  if (restoredWidth && restoredHeight && viewer) {
    viewer.resize(restoredWidth, restoredHeight);
  }
  const restoredZoom = Number(parsed.viewport?.zoom);
  if (viewer && Number.isFinite(restoredZoom) && restoredZoom > 0) {
    viewer.set_zoom(restoredZoom);
    if (ui.zoomValue) {
      ui.zoomValue.value = restoredZoom.toFixed(2);
    }
  }
  if (viewer && (parsed.viewport?.interpolation === "nearest" || parsed.viewport?.interpolation === "linear")) {
    const mode = parsed.viewport.interpolation;
    viewer.set_interpolation_mode(mode);
    if (ui.interpolationMode) {
      ui.interpolationMode.value = mode;
    }
  }
  refreshSnapshot();
  if (Array.isArray(parsed.measurements)) {
    const restored = parsed.measurements
      .filter((item) => item && typeof item === "object")
      .map((item) => ({
        id: Number.parseInt(item.id, 10) || 0,
        kind: item.kind || "line",
        created_at: item.created_at || new Date().toISOString(),
        reviewed: item.reviewed === true,
        start: {
          x: Number.parseFloat(item.start?.x) || 0,
          y: Number.parseFloat(item.start?.y) || 0,
        },
        end: {
          x: Number.parseFloat(item.end?.x) || 0,
          y: Number.parseFloat(item.end?.y) || 0,
        },
        length_px: Number.parseFloat(item.length_px) || 0,
      }));
    measurements.splice(0, measurements.length, ...restored);
    measurementCounter = measurements.reduce((maxId, item) => Math.max(maxId, Number(item.id) || 0), 0);
  }
  updateMeasurementList();
  setStatus(ui.workstationStatus, "ok", "Session restored from local storage.", "UI-SESSION-02");
  return true;
}

function interpolationMode() {
  return ui.interpolationMode.value === "nearest" ? "nearest" : "linear";
}

function updateOnboardingStatus() {
  const dicomweb = ui.connectDicomweb?.textContent || "unknown";
  const workflow = ui.connectWorkflow?.textContent || "unknown";
  if (dicomweb.startsWith("ok") && workflow.startsWith("ok")) {
    setStatus(ui.onboardingStatus, "ok", "All required local services are reachable.", "UI-ONBOARD-03");
    return;
  }
  if (dicomweb.startsWith("error") || workflow.startsWith("error")) {
    setStatus(ui.onboardingStatus, "error", "One or more required services are unavailable.", "UI-ONBOARD-02");
    return;
  }
  setStatus(ui.onboardingStatus, "pending", "Run connectivity check to validate evaluator setup.", "UI-ONBOARD-01");
}

async function checkEndpoint(url, requestInit = {}) {
  try {
    const response = await fetch(url, requestInit);
    return {
      ok: response.ok,
      status: response.status,
    };
  } catch (_error) {
    return {
      ok: false,
      status: 0,
    };
  }
}

async function runConnectivityChecks() {
  const dicomwebBase = dicomwebBaseUrl();
  const workflowBase = srBaseUrl();
  if (!dicomwebBase || !workflowBase) {
    if (ui.connectDicomweb) {
      ui.connectDicomweb.textContent = "error (missing URL)";
    }
    if (ui.connectWorkflow) {
      ui.connectWorkflow.textContent = "error (missing URL)";
    }
    updateOnboardingStatus();
    return;
  }
  const [dicomweb, workflow] = await Promise.all([
    checkEndpoint(`${dicomwebBase}/studies`, { method: "GET" }),
    checkEndpoint(`${workflowBase}/sr/documents`, { method: "GET" }),
  ]);
  if (ui.connectDicomweb) {
    ui.connectDicomweb.textContent = dicomweb.ok
      ? `ok (${dicomweb.status})`
      : `error (${dicomweb.status || "unreachable"})`;
  }
  if (ui.connectWorkflow) {
    ui.connectWorkflow.textContent = workflow.ok
      ? `ok (${workflow.status})`
      : `error (${workflow.status || "unreachable"})`;
  }
  setStatus(
    ui.workstationStatus,
    dicomweb.ok && workflow.ok ? "ok" : "error",
    dicomweb.ok && workflow.ok
      ? "Connectivity checks passed for DICOMweb and workflow backends."
      : "Connectivity checks failed for one or more backends.",
    dicomweb.ok && workflow.ok ? "UI-CONN-02" : "UI-CONN-03",
  );
  updateOnboardingStatus();
}

function startConnectivityPolling() {
  if (connectivityTimer !== null) {
    window.clearInterval(connectivityTimer);
  }
  connectivityTimer = window.setInterval(() => {
    void runConnectivityChecks();
  }, 20_000);
}

function webGpuBlockedByInsecureContext() {
  if (typeof window === "undefined") {
    return false;
  }
  const host = window.location.hostname || "";
  const isLoopback = host === "localhost" || host === "127.0.0.1" || host === "[::1]";
  return !window.isSecureContext && !isLoopback;
}

function detectBackendLabel() {
  const hasWebGPU = typeof navigator !== "undefined" && typeof navigator.gpu === "object";
  const hasWebGL2 =
    typeof window !== "undefined" &&
    Boolean(document.createElement("canvas").getContext("webgl2"));

  backendProbe.webgpuApi = hasWebGPU;
  backendProbe.webgl2Api = hasWebGL2;
  backendProbe.adapterAvailable = false;
  backendProbe.maxTextureDimension2d = 0;

  if (hasWebGPU) {
    backendCapabilities = {
      api: "WebGPU API available",
      adapter: "Not inspected",
      features: "Not inspected",
      limits: "Not inspected",
    };
    return "WebGPU API available";
  }
  if (webGpuBlockedByInsecureContext()) {
    backendCapabilities = {
      api: "WebGPU blocked by insecure origin",
      adapter: "Use https:// or localhost",
      features: "Unavailable",
      limits: "Unavailable",
    };
    return "WebGPU unavailable on insecure origin";
  }
  if (hasWebGL2) {
    backendCapabilities = {
      api: "WebGL2 available",
      adapter: "Renderer not queried in this harness",
      features: "Not inspected",
      limits: "Not inspected",
    };
    return "WebGL2 API available";
  }
  backendCapabilities = {
    api: "CPU fallback",
    adapter: "No GPU backend detected",
    features: "N/A",
    limits: "N/A",
  };
  return "CPU fallback path";
}

async function resolveBackendLabel() {
  const hasWebGPU = typeof navigator !== "undefined" && typeof navigator.gpu === "object";
  backendProbe.webgpuApi = hasWebGPU;
  backendProbe.webgl2Api =
    typeof window !== "undefined" &&
    Boolean(document.createElement("canvas").getContext("webgl2"));
  backendProbe.adapterAvailable = false;
  backendProbe.maxTextureDimension2d = 0;
  if (!hasWebGPU) {
    const label = detectBackendLabel();
    backendLabel = label;
    return label;
  }
  try {
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) {
      backendProbe.adapterAvailable = false;
      backendLabel = "WebGPU API available, adapter unavailable";
      backendCapabilities = {
        api: "WebGPU API available",
        adapter: "No compatible adapter",
        features: "Unavailable",
        limits: "Unavailable",
      };
      return backendLabel;
    }
    backendProbe.adapterAvailable = true;
    const featureCount = adapter.features?.size ?? 0;
    const featureList = Array.from(adapter.features ?? [])
      .slice(0, 6)
      .join(", ");
    const limits = adapter.limits;
    const maxTextureDimension2D = limits?.maxTextureDimension2D ?? 0;
    backendProbe.maxTextureDimension2d = maxTextureDimension2D;
    const maxStorageBufferBindingSize = limits?.maxStorageBufferBindingSize;
    const maxBindGroups = limits?.maxBindGroups;
    const maxBufferSize = limits?.maxBufferSize;
    const limitsSummary =
      maxTextureDimension2D && maxStorageBufferBindingSize && maxBufferSize
        ? `tex2D:${maxTextureDimension2D}, bindGroups:${maxBindGroups}, buf:${maxBufferSize}, storage:${maxStorageBufferBindingSize}`
        : "Partial or unavailable";

    backendCapabilities = {
      api: "WebGPU adapter",
      adapter: adapter.name ?? "Detected adapter",
      features: `${featureCount} features (${featureList || "none"})`,
      limits: limitsSummary,
    };
    backendLabel = `WebGPU probe ready (${backendCapabilities.adapter})`;
    return backendLabel;
  } catch (error) {
    const detail = error instanceof Error ? error.message : "unknown error";
    backendProbe.adapterAvailable = false;
    backendLabel = `WebGPU detection failed (${detail}); using CPU fallback`;
    backendCapabilities = {
      api: "WebGPU detection failed",
      adapter: "Unavailable",
      features: "Unavailable",
      limits: "Unavailable",
    };
    return backendLabel;
  }
}

function applyBackendSelection() {
  return withViewerLock(() => {
    if (!viewer) {
      return;
    }
    const productionRequested = true;
    viewer.set_production_webgpu_renderer(productionRequested);
    productionWebGpuEnabled = Boolean(viewer.production_webgpu_renderer_enabled());
    persistProductionWebGpuFlag(productionWebGpuEnabled);
    if (ui.productionWebGpuToggle) {
      ui.productionWebGpuToggle.checked = true;
      ui.productionWebGpuToggle.disabled = true;
    }
    const forceCpu = false;
    if (ui.forceCpuToggle) {
      ui.forceCpuToggle.checked = false;
      ui.forceCpuToggle.disabled = true;
    }
    viewer.set_force_cpu_renderer(forceCpu);
    const preferred = forceCpu ? "cpu" : "webgpu";
    const active = viewer.configure_renderer_backend(
      preferred,
      backendProbe.webgpuApi,
      backendProbe.adapterAvailable,
      backendProbe.webgl2Api,
      backendProbe.maxTextureDimension2d,
    );
    backendLabel = `${active} active`;
    if (active === "CPU" && !forceCpu) {
      backendLabel = productionWebGpuEnabled
        ? "CPU active (automatic fallback)"
        : "CPU active (production WebGPU flag disabled)";
    }
    if (active === "WebGPU" && productionWebGpuEnabled) {
      backendLabel = "WebGPU active (production draw path)";
    }
    if (active === "WebGPU" && productionWebGpuEnabled) {
      setBackendStateMessage("WebGPU active.", "ok", "UI-BACKEND-12");
      setGpuRecoveryBanner(false);
    } else if (active === "CPU" && productionWebGpuEnabled) {
      if (!backendProbe.webgpuApi && webGpuBlockedByInsecureContext()) {
        setBackendStateMessage(
          "CPU fallback active: WebGPU requires https:// or localhost in this browser.",
          "error",
          "UI-BACKEND-18",
        );
      } else {
        setBackendStateMessage("CPU fallback active (WebGPU fallback).", "error", "UI-BACKEND-13");
      }
      setGpuRecoveryBanner(true);
    } else if (active === "CPU" && !productionWebGpuEnabled) {
      setBackendStateMessage(
        "CPU active (WebGPU disabled by policy).",
        "pending",
        "UI-BACKEND-14",
      );
      setGpuRecoveryBanner(false);
    } else {
      setBackendStateMessage(`${active} active.`, "pending", "UI-BACKEND-15");
      setGpuRecoveryBanner(false);
    }
  });
}

async function refreshSnapshot() {
  return withViewerLock(async () => {
    if (!viewer) {
      return;
    }
    ui.statWidth.textContent = String(viewer.viewport_width());
    ui.statHeight.textContent = String(viewer.viewport_height());
    ui.statZoom.textContent = viewer.viewport_zoom().toFixed(2);
    ui.statNetwork.textContent = viewer.network_enabled() ? "enabled" : "disabled";
    ui.statLoadedBytes.textContent = String(viewer.loaded_input_bytes());

    const activeBackend = viewer.active_renderer_backend();
    if (ui.statBackend) {
      ui.statBackend.textContent = `${activeBackend} (${backendLabel})`;
    }
    if (ui.rendererBadge) {
      ui.rendererBadge.textContent = activeBackend;
      ui.rendererBadge.className = `backend-badge backend-${activeBackend.toLowerCase()}`;
    }

  if (ui.statGpuApi) {
    ui.statGpuApi.textContent = backendCapabilities.api;
  }
  if (ui.statGpuAdapter) {
    ui.statGpuAdapter.textContent = backendCapabilities.adapter;
  }
  if (ui.statGpuFeatures) {
    ui.statGpuFeatures.textContent = backendCapabilities.features;
  }
  if (ui.statGpuLimits) {
    ui.statGpuLimits.textContent = backendCapabilities.limits;
  }

  if (ui.backendDiagnostics) {
    try {
      const probe = JSON.parse(viewer.backend_capability_probe_json());
      const metrics = JSON.parse(viewer.backend_selection_metrics_json());
      if (ui.statTransitionCount) {
        ui.statTransitionCount.textContent = String(metrics.backend_transition_count ?? "-");
      }
      if (ui.statLastTransitionReason) {
        ui.statLastTransitionReason.textContent = String(metrics.last_transition_reason ?? "-");
      }
      if (ui.statLastFallbackReason) {
        ui.statLastFallbackReason.textContent = String(metrics.last_fallback_reason ?? "-");
      }
      if (ui.statFallbackLatency) {
        ui.statFallbackLatency.textContent = String(metrics.last_fallback_latency_ms ?? "-");
      }
      if (ui.statFallbackBudget) {
        ui.statFallbackBudget.textContent = String(metrics.fallback_latency_budget_ms ?? "-");
      }
      if (ui.statWebGpuPresent) {
        ui.statWebGpuPresent.textContent = String(metrics.webgpu_present_count ?? "-");
      }
      if (ui.statCpuPresent) {
        ui.statCpuPresent.textContent = String(metrics.cpu_present_count ?? "-");
      }
      if (ui.statUploadChunkBytes) {
        ui.statUploadChunkBytes.textContent = String(metrics.last_upload_chunk_bytes ?? "-");
      }
      const shaderSource = viewer.csp_safe_shader_source();
      ui.backendDiagnostics.textContent = JSON.stringify(
        {
          probe,
          metrics,
          productionWebGpuEnabled,
          presenter: {
            failed: webGpuPresenter.failed,
            lastError: webGpuPresenter.lastError,
          },
          shader: {
            source: "inline-csp-safe",
            length: shaderSource.length,
          },
        },
        null,
        2,
      );
    } catch (error) {
      ui.backendDiagnostics.textContent = `Failed to parse diagnostics: ${error}`;
      if (ui.statTransitionCount) {
        ui.statTransitionCount.textContent = "-";
      }
      if (ui.statLastTransitionReason) {
        ui.statLastTransitionReason.textContent = "-";
      }
      if (ui.statLastFallbackReason) {
        ui.statLastFallbackReason.textContent = "-";
      }
      if (ui.statFallbackLatency) {
        ui.statFallbackLatency.textContent = "-";
      }
      if (ui.statFallbackBudget) {
        ui.statFallbackBudget.textContent = "-";
      }
      if (ui.statWebGpuPresent) {
        ui.statWebGpuPresent.textContent = "-";
      }
      if (ui.statCpuPresent) {
        ui.statCpuPresent.textContent = "-";
      }
      if (ui.statUploadChunkBytes) {
        ui.statUploadChunkBytes.textContent = "-";
      }
    }
  }

    if (viewer.has_preview()) {
      ui.statRenderMs.textContent = "-";
      ui.statFrameWidth.textContent = String(viewer.preview_width());
      ui.statFrameHeight.textContent = String(viewer.preview_height());
    } else {
      if (ui.statRenderMs) {
        ui.statRenderMs.textContent = "-";
      }
      ui.statFrameWidth.textContent = "-";
      ui.statFrameHeight.textContent = "-";
    }
    await renderPreviewUnlocked();
  });
}

function syncComparisonViewports(frame) {
  if (!frame) {
    return;
  }
  const activeCount = viewportPreset === "quad" ? 3 : viewportPreset === "dual" ? 1 : 0;
  for (let index = 0; index < comparisonCtx.length; index += 1) {
    const ctx = comparisonCtx[index];
    const canvas =
      index === 0 ? ui.compareCanvasA : index === 1 ? ui.compareCanvasB : ui.compareCanvasC;
    if (!ctx || !canvas) {
      continue;
    }
    if (index >= activeCount) {
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      continue;
    }
    canvas.width = frame.width;
    canvas.height = frame.height;
    canvas.style.width = `${Math.max(1, viewer?.viewport_width?.() ?? frame.width)}px`;
    canvas.style.height = `${Math.max(1, viewer?.viewport_height?.() ?? frame.height)}px`;
    const linkedWl = Boolean(ui.linkWindowLevelToggle?.checked);
    const source = linkedWl ? frame.rgba : frame.rawRgba;
    const imageData = new ImageData(new Uint8ClampedArray(source), frame.width, frame.height);
    ctx.putImageData(imageData, 0, 0);
  }
}

async function renderPreviewUnlocked() {
  if (!viewer) {
    return;
  }
  try {

  const viewportWidth = Math.max(1, viewer.viewport_width());
  const viewportHeight = Math.max(1, viewer.viewport_height());
  const dpr = Math.max(1, window.devicePixelRatio || 1);
  const outputWidth = Math.max(1, Math.round(viewportWidth * dpr));
  const outputHeight = Math.max(1, Math.round(viewportHeight * dpr));
  ui.previewCanvas.width = outputWidth;
  ui.previewCanvas.height = outputHeight;
  ui.previewCanvas.style.width = `${viewportWidth}px`;
  ui.previewCanvas.style.height = `${viewportHeight}px`;

  if (!viewer.has_preview()) {
    setStatus(ui.previewStatus, "pending", "No decoded frame available.", "UI-PREVIEW-02");
    return;
  }

  const frameWidth = viewer.preview_width();
  const frameHeight = viewer.preview_height();
  const renderStart = performance.now();
  const rawRgba = viewer.render_viewport_rgba_bytes_for_size(outputWidth, outputHeight);
  const { width: wlWidth, level: wlLevel } = currentWindowLevel();
  const wlRgba = applyWindowLevelRgba(rawRgba, wlWidth, wlLevel);
  const rgba = shouldFallbackToRawPreview(rawRgba, wlRgba) ? rawRgba : wlRgba;
  const renderMs = Math.max(0, performance.now() - renderStart);
  if (pendingFirstImageStart !== null && interactionMetrics.firstImageLatencyMs === null) {
    interactionMetrics.firstImageLatencyMs = Math.max(0, performance.now() - pendingFirstImageStart);
    pendingFirstImageStart = null;
  }
  if (ui.statRenderMs) {
    ui.statRenderMs.textContent = `${renderMs.toFixed(2)} ms`;
  }
  const expectedBytes = outputWidth * outputHeight * 4;

  if (rawRgba.length !== expectedBytes) {
    setStatus(
      ui.previewStatus,
      "error",
      "Viewport re-render failed (output size mismatch).",
      "UI-PREVIEW-03",
    );
    return;
  }

  const activeBackend = viewer.active_renderer_backend();
  let presentationPath = "CPU/canvas";
  let drewWithWebGpu = false;
  if (activeBackend === "WebGPU" && productionWebGpuEnabled) {
    drewWithWebGpu = await drawPreviewWithWebGpu(rgba, outputWidth, outputHeight);
    if (drewWithWebGpu) {
      presentationPath = "WebGPU";
    } else {
      const fallbackStart = performance.now();
      const triggered = viewer.simulate_gpu_device_lost();
      if (triggered) {
        if (ui.forceCpuToggle) {
          ui.forceCpuToggle.checked = true;
        }
        applyBackendSelection();
      }
      const fallbackLatency = Math.max(0, performance.now() - fallbackStart);
      setStatus(
        ui.bootStatus,
        "error",
        `WebGPU presentation failed (${webGpuPresenter.lastError || "unknown error"}); switched to CPU fallback in ${fallbackLatency.toFixed(2)}ms.`,
        "UI-GPU-04",
      );
      setBackendStateMessage("CPU fallback activated after WebGPU presentation failure.", "error", "UI-BACKEND-16");
      setGpuRecoveryBanner(true);
    }
  }

  if (!drewWithWebGpu) {
    const ctx = getPreview2dContext();
    if (!ctx) {
      setStatus(
        ui.previewStatus,
        "error",
        "Canvas 2D context is unavailable.",
        "UI-PREVIEW-01",
      );
      return;
    }
    const imageData = new ImageData(new Uint8ClampedArray(rgba), outputWidth, outputHeight);
    ctx.putImageData(imageData, 0, 0);
  }

  const mode = viewer.interpolation_mode();
  const zoom = viewer.viewport_zoom();
  latestPreviewFrame = {
    width: outputWidth,
    height: outputHeight,
    rgba,
    rawRgba,
  };
  syncComparisonViewports(latestPreviewFrame);
  setStatus(
    ui.previewStatus,
    "ok",
    `Rendered from DICOM ${frameWidth}x${frameHeight} source at zoom ${zoom.toFixed(2)} (${mode}) in ${renderMs.toFixed(2)}ms via ${presentationPath}. Click preview to recenter ROI.`,
    "UI-PREVIEW-04",
  );
  persistSessionState();
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    setStatus(
      ui.previewStatus,
      "error",
      `Preview render failed: ${detail}`,
      "UI-PREVIEW-99",
    );
  }
}

async function renderPreview() {
  return withViewerLock(() => renderPreviewUnlocked());
}

function parseNonNegativeInteger(value) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed) || parsed < 0) {
    return null;
  }
  return parsed;
}

function drawMprPlane(plane, width, height, rgba) {
  const target =
    plane === "axial" ? mprCtx.axial : plane === "coronal" ? mprCtx.coronal : mprCtx.sagittal;
  if (!target) {
    return false;
  }
  const expected = width * height * 4;
  if (rgba.length !== expected) {
    return false;
  }
  const canvas = target.canvas;
  canvas.width = width;
  canvas.height = height;
  const imageData = new ImageData(new Uint8ClampedArray(rgba), width, height);
  target.putImageData(imageData, 0, 0);
  return true;
}

function applyMprSlabSettings() {
  if (!viewer) {
    return false;
  }
  const thickness = parsePositiveInteger(ui.mprSlabThickness?.value ?? "1");
  if (thickness === null || thickness > 64) {
    setStatus(ui.mprStatus, "error", "Slab thickness must be an integer between 1 and 64.", "UI-MPR-06");
    return false;
  }
  const mode = ui.mprSlabMode?.value === "max" ? "max" : "average";
  const ok = viewer.set_mpr_slab(thickness, mode);
  if (!ok) {
    setStatus(ui.mprStatus, "error", "Failed to apply slab settings.", "UI-MPR-07");
    return false;
  }
  return true;
}

function renderTriPlanarMpr() {
  void withViewerLock(async () => {
  if (!viewer || !ui.mprStatus) {
    return;
  }
  if (!viewer.has_preview()) {
    setStatus(ui.mprStatus, "pending", "Load a DICOM frame before running tri-planar MPR.", "UI-MPR-01");
    return;
  }
  const x = parseNonNegativeInteger(ui.mprCrosshairX.value);
  const y = parseNonNegativeInteger(ui.mprCrosshairY.value);
  const z = parseNonNegativeInteger(ui.mprCrosshairZ.value);
  const outWidth = parsePositiveInteger(ui.mprOutputWidth.value);
  const outHeight = parsePositiveInteger(ui.mprOutputHeight.value);
  if (x === null || y === null || z === null || outWidth === null || outHeight === null) {
    setStatus(ui.mprStatus, "error", "Tri-planar inputs must be valid integers.", "UI-MPR-02");
    return;
  }
  if (!applyMprSlabSettings()) {
    return;
  }
  if (!viewer.set_mpr_crosshair(x, y, z)) {
    setStatus(ui.mprStatus, "error", "Crosshair is outside the current volume bounds.", "UI-MPR-03");
    return;
  }

  const wl = currentWindowLevel();
  const axialRaw = viewer.mpr_plane_rgba_bytes("axial", outWidth, outHeight);
  const coronalRaw = viewer.mpr_plane_rgba_bytes("coronal", outWidth, outHeight);
  const sagittalRaw = viewer.mpr_plane_rgba_bytes("sagittal", outWidth, outHeight);
  const axial = applyWindowLevelRgba(axialRaw, wl.width, wl.level);
  const coronal = applyWindowLevelRgba(coronalRaw, wl.width, wl.level);
  const sagittal = applyWindowLevelRgba(sagittalRaw, wl.width, wl.level);
  const rendered =
    drawMprPlane("axial", outWidth, outHeight, axial) &&
    drawMprPlane("coronal", outWidth, outHeight, coronal) &&
    drawMprPlane("sagittal", outWidth, outHeight, sagittal);
  if (!rendered) {
    setStatus(ui.mprStatus, "error", "Tri-planar MPR render failed.", "UI-MPR-04");
    return;
  }

  let triState = null;
  try {
    triState = JSON.parse(viewer.tri_planar_state_json());
  } catch (_error) {
    triState = null;
  }
  if (triState) {
    ui.mprCrosshairX.value = String(triState.crosshair_voxel?.[0] ?? x);
    ui.mprCrosshairY.value = String(triState.crosshair_voxel?.[1] ?? y);
    ui.mprCrosshairZ.value = String(triState.crosshair_voxel?.[2] ?? z);
    if (ui.planeIndicatorAxial) {
      ui.planeIndicatorAxial.textContent = `L/R • P/A • z=${triState.axial_index ?? z}`;
    }
    if (ui.planeIndicatorCoronal) {
      ui.planeIndicatorCoronal.textContent = `L/R • I/S • y=${triState.coronal_index ?? y}`;
    }
    if (ui.planeIndicatorSagittal) {
      ui.planeIndicatorSagittal.textContent = `P/A • I/S • x=${triState.sagittal_index ?? x}`;
    }
  }
  latestMprPlanes = {
    width: outWidth,
    height: outHeight,
    axial,
    coronal,
    sagittal,
    axialRaw,
    coronalRaw,
    sagittalRaw,
  };
  setStatus(
    ui.mprStatus,
    "ok",
    `Tri-planar MPR rendered at ${outWidth}x${outHeight} (slab ${ui.mprSlabThickness?.value || 1}, mode ${ui.mprSlabMode?.value || "average"}).`,
    "UI-MPR-05",
  );
  persistSessionState();
  });
}

function onMprCanvasClick(plane, event) {
  if (!viewer) {
    return;
  }
  let state = null;
  try {
    state = JSON.parse(viewer.tri_planar_state_json());
  } catch (_error) {
    state = null;
  }
  if (!state) {
    return;
  }
  const dims = state.volume_dimensions ?? [1, 1, 1];
  const crosshair = [...(state.crosshair_voxel ?? [0, 0, 0])];
  const rect = event.currentTarget.getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0) {
    return;
  }
  const u = Math.max(0, Math.min(1, (event.clientX - rect.left) / rect.width));
  const v = Math.max(0, Math.min(1, (event.clientY - rect.top) / rect.height));

  if (plane === "axial") {
    crosshair[0] = Math.round(u * Math.max(0, dims[0] - 1));
    crosshair[1] = Math.round(v * Math.max(0, dims[1] - 1));
  } else if (plane === "coronal") {
    crosshair[0] = Math.round(u * Math.max(0, dims[0] - 1));
    crosshair[2] = Math.round(v * Math.max(0, dims[2] - 1));
  } else {
    crosshair[1] = Math.round(u * Math.max(0, dims[1] - 1));
    crosshair[2] = Math.round(v * Math.max(0, dims[2] - 1));
  }

  ui.mprCrosshairX.value = String(crosshair[0]);
  ui.mprCrosshairY.value = String(crosshair[1]);
  ui.mprCrosshairZ.value = String(crosshair[2]);
  renderTriPlanarMpr();
}

function onApplyWindowLevel() {
  recordInteractionEvent();
  const wl = currentWindowLevel();
  if (!Number.isFinite(wl.width) || wl.width < 1) {
    setStatus(ui.mprStatus, "error", "Window width must be >= 1.", "UI-WL-01");
    return;
  }
  setStatus(
    ui.mprStatus,
    "ok",
    `Window/level applied (W=${Math.round(wl.width)}, L=${Math.round(wl.level)}).`,
    "UI-WL-02",
  );
  refreshSnapshot();
  if (viewer?.has_preview()) {
    renderTriPlanarMpr();
  }
  persistSessionState();
}

function onApplyViewportPreset() {
  recordInteractionEvent();
  setViewportPreset(ui.viewportPreset?.value || "single");
  setStatus(ui.workstationStatus, "ok", `Viewport preset set to ${viewportPreset}.`, "UI-VIEW-03");
  syncComparisonViewports(latestPreviewFrame);
  persistSessionState();
}

function panViewportByDelta(dxFraction, dyFraction) {
  if (!viewer || !viewer.has_preview()) {
    return;
  }
  const state = snapshotPayload();
  const zoom = Number(state.viewport?.zoom ?? 1);
  const sx = clamp(0.5 + dxFraction / Math.max(zoom, 0.1), 0, 1);
  const sy = clamp(0.5 + dyFraction / Math.max(zoom, 0.1), 0, 1);
  const ok = viewer.recenter_from_viewport_fraction(sx, sy);
  if (ok) {
    setStatus(ui.bootStatus, "ok", "Viewport panned.", "UI-PAN-01");
    refreshSnapshot();
  }
}

function stepCrosshair(delta) {
  const z = parseNonNegativeInteger(ui.mprCrosshairZ?.value ?? "0");
  if (z === null) {
    return;
  }
  const next = Math.max(0, z + delta);
  if (ui.mprCrosshairZ) {
    ui.mprCrosshairZ.value = String(next);
  }
  if (ui.linkScrollToggle?.checked) {
    renderTriPlanarMpr();
  }
}

async function onRunPatientMpr() {
  if (!viewer) {
    return;
  }
  const plane = (ui.patientMprPlane?.value || "axial").trim();
  const offsetUm = Number.parseInt(ui.patientMprOffsetUm?.value || "0", 10);
  const width = parsePositiveInteger(ui.mprOutputWidth?.value || "256") ?? 256;
  const height = parsePositiveInteger(ui.mprOutputHeight?.value || "256") ?? 256;
  const payload = viewer.mpr_reslice_patient_json(
    plane,
    Number.isFinite(offsetUm) ? offsetUm : 0,
    width,
    height,
  );
  if (ui.patientMprOutput) {
    ui.patientMprOutput.textContent = payload;
  }
  setStatus(ui.mprStatus, "ok", "Patient-space MPR request executed.", "UI-MPR-08");
}

function exportMprBundle() {
  if (!latestMprPlanes) {
    setStatus(ui.mprStatus, "error", "Render MPR before export.", "UI-MPR-09");
    return;
  }
  const suffix = stableArtifactSuffix();
  const payload = {
    exported_at: new Date().toISOString(),
    suffix,
    crosshair: {
      x: Number(ui.mprCrosshairX?.value ?? "0"),
      y: Number(ui.mprCrosshairY?.value ?? "0"),
      z: Number(ui.mprCrosshairZ?.value ?? "0"),
    },
    slab: {
      thickness: Number(ui.mprSlabThickness?.value ?? "1"),
      mode: ui.mprSlabMode?.value ?? "average",
    },
    window_level: currentWindowLevel(),
    dimensions: {
      width: latestMprPlanes.width,
      height: latestMprPlanes.height,
    },
  };
  downloadText(
    `rdvf-mpr-bundle-${suffix}.json`,
    `${JSON.stringify(payload, null, 2)}\n`,
    "application/json;charset=utf-8",
  );
  setStatus(ui.mprStatus, "ok", `MPR bundle exported (rdvf-mpr-bundle-${suffix}.json).`, "UI-MPR-10");
}

function fusionDiagnostics() {
  let triState = null;
  try {
    triState = JSON.parse(viewer?.tri_planar_state_json?.() ?? "{}");
  } catch (_error) {
    triState = null;
  }
  const dims = triState?.volume_dimensions ?? [0, 0, 0];
  const alpha = Number.parseFloat(ui.fusionAlpha?.value || "0");
  const suvScale = Number.parseFloat(ui.fusionSuvScale?.value || "0");
  const diagnostics = {
    has_preview: Boolean(viewer?.has_preview?.()),
    has_mpr_planes: Boolean(latestMprPlanes),
    volume_dimensions: dims,
    frame_of_reference_match: "unknown-in-host-prototype",
    geometry_ready: dims.every((value) => Number(value) > 0),
    alpha_in_range: Number.isFinite(alpha) && alpha >= 0 && alpha <= 1,
    suv_scale_in_range: Number.isFinite(suvScale) && suvScale >= 0.1 && suvScale <= 20,
    status: "pending",
  };
  diagnostics.status =
    diagnostics.has_preview &&
    diagnostics.has_mpr_planes &&
    diagnostics.geometry_ready &&
    diagnostics.alpha_in_range &&
    diagnostics.suv_scale_in_range
      ? "ready"
      : "blocked";
  if (ui.fusionDiagOutput) {
    ui.fusionDiagOutput.textContent = JSON.stringify(diagnostics, null, 2);
  }
  return diagnostics;
}

function colorizeFusionIntensity(value, colormap) {
  const v = clamp(value, 0, 1);
  if (colormap === "icefire") {
    return [
      Math.round(40 + 180 * v),
      Math.round(120 * (1 - v) + 40),
      Math.round(255 * (1 - v)),
    ];
  }
  if (colormap === "grayscale") {
    const mapped = Math.round(v * 255);
    return [mapped, mapped, mapped];
  }
  // hotiron default
  return [Math.round(255 * v), Math.round(180 * v * v), Math.round(90 * (1 - v))];
}

function renderFusion() {
  const diagnostics = fusionDiagnostics();
  if (diagnostics.status !== "ready") {
    setStatus(ui.fusionStatus, "error", "Fusion preconditions are not satisfied.", "UI-FUSION-01");
    return;
  }
  if (!latestMprPlanes || !fusionCtx || !ui.fusionCanvas) {
    setStatus(ui.fusionStatus, "error", "Fusion plane data unavailable.", "UI-FUSION-02");
    return;
  }
  const alpha = clamp(Number.parseFloat(ui.fusionAlpha?.value || "0.45"), 0, 1);
  const suvScale = clamp(Number.parseFloat(ui.fusionSuvScale?.value || "5.0"), 0.1, 20);
  const colormap = (ui.fusionColormap?.value || "hotiron").trim();
  const width = latestMprPlanes.width;
  const height = latestMprPlanes.height;
  const rgba = new Uint8Array(width * height * 4);

  let roiCount = 0;
  let roiSum = 0;
  const roiRadius = Math.min(width, height) * 0.15;
  const cx = width * 0.5;
  const cy = height * 0.5;
  for (let i = 0; i < width * height; i += 1) {
    const px = i % width;
    const py = Math.floor(i / width);
    const src = i * 4;
    const base = latestMprPlanes.axial[src];
    const suv = clamp(base / 255, 0, 1) * suvScale;
    const normalized = clamp(suv / suvScale, 0, 1);
    const overlay = colorizeFusionIntensity(normalized, colormap);
    rgba[src] = Math.round(base * (1 - alpha) + overlay[0] * alpha);
    rgba[src + 1] = Math.round(base * (1 - alpha) + overlay[1] * alpha);
    rgba[src + 2] = Math.round(base * (1 - alpha) + overlay[2] * alpha);
    rgba[src + 3] = 255;
    const dist = Math.sqrt((px - cx) ** 2 + (py - cy) ** 2);
    if (dist <= roiRadius) {
      roiCount += 1;
      roiSum += suv;
    }
  }

  latestFusionFrame = {
    width,
    height,
    rgba,
    alpha,
    colormap,
    suvScale,
  };

  ui.fusionCanvas.width = width;
  ui.fusionCanvas.height = height;
  fusionCtx.putImageData(new ImageData(new Uint8ClampedArray(rgba), width, height), 0, 0);

  const quant = {
    roi_pixel_count: roiCount,
    roi_mean_suv: roiCount > 0 ? roiSum / roiCount : 0,
    alpha,
    colormap,
    suv_scale: suvScale,
  };
  if (ui.fusionQuantOutput) {
    ui.fusionQuantOutput.textContent = JSON.stringify(quant, null, 2);
  }
  setStatus(ui.fusionStatus, "ok", "Fusion preview rendered with bounded parameters.", "UI-FUSION-03");
}

function exportFusionBundle() {
  if (!latestFusionFrame) {
    setStatus(ui.fusionStatus, "error", "Render fusion before export.", "UI-FUSION-04");
    return;
  }
  const suffix = stableArtifactSuffix();
  const payload = {
    exported_at: new Date().toISOString(),
    suffix,
    dimensions: {
      width: latestFusionFrame.width,
      height: latestFusionFrame.height,
    },
    alpha: latestFusionFrame.alpha,
    colormap: latestFusionFrame.colormap,
    suv_scale: latestFusionFrame.suvScale,
    diagnostics: ui.fusionDiagOutput?.textContent ? JSON.parse(ui.fusionDiagOutput.textContent) : null,
    quantification: ui.fusionQuantOutput?.textContent ? JSON.parse(ui.fusionQuantOutput.textContent) : null,
  };
  downloadText(
    `rdvf-fusion-bundle-${suffix}.json`,
    `${JSON.stringify(payload, null, 2)}\n`,
    "application/json;charset=utf-8",
  );
  if (ui.fusionCanvas) {
    ui.fusionCanvas.toBlob((blob) => {
      if (blob) {
        downloadBlob(`rdvf-fusion-${suffix}.png`, blob);
      }
    }, "image/png");
  }
  setStatus(ui.fusionStatus, "ok", `Fusion bundle exported (suffix ${suffix}).`, "UI-FUSION-05");
}

function parsePositiveInteger(value) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed) || parsed < 1) {
    return null;
  }
  return parsed;
}

function parsePositiveNumber(value) {
  const parsed = Number.parseFloat(value);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return null;
  }
  return parsed;
}

async function onDicomSelected(event) {
  await ingestFileBatch(event.target?.files, "file");
}

async function onDicomFolderSelected(event) {
  await ingestFileBatch(event.target?.files, "folder");
}

async function onDicomDrop(event) {
  event.preventDefault();
  event.stopPropagation();
  await ingestFileBatch(event.dataTransfer?.files, "drop");
}

function normalizeIngestFiles(fileList) {
  if (!fileList) {
    return [];
  }
  const files = [...fileList].filter((file) => file && typeof file.arrayBuffer === "function");
  return files.sort((a, b) => (a.name || "").localeCompare(b.name || "", undefined, { sensitivity: "base" }));
}

async function ingestFileBatch(fileList, sourceKind) {
  const files = normalizeIngestFiles(fileList);
  if (files.length === 0) {
    const msg = sourceKind === "drop" ? "Drop event did not include files." : "No files selected for ingest.";
    setStatus(ui.ingestStatus, "error", msg, "UI-INGEST-03");
    return;
  }
  if (!viewer) {
    setStatus(ui.ingestStatus, "error", "WASM viewer not initialized yet.", "UI-INGEST-05");
    return;
  }
  resetSeriesStack(files);
  interactionMetrics.firstImageLatencyMs = null;
  pendingFirstImageStart = performance.now();
  let acceptedFile = null;
  let acceptedBytes = 0;
  let acceptedIndex = -1;
  let readFailures = 0;

  for (let index = 0; index < files.length; index += 1) {
    const loaded = await tryLoadSeriesFileAt(index);
    if (loaded.unreadable) {
      readFailures += 1;
      continue;
    }
    if (!loaded.ok) {
      continue;
    }
    acceptedFile = loaded.file;
    acceptedBytes = loaded.bytesLength;
    acceptedIndex = index;
    break;
  }

  if (!acceptedFile) {
    const failureDetail = readFailures > 0 ? ` (${readFailures} unreadable files skipped)` : "";
    setStatus(
      ui.ingestStatus,
      "error",
      `No decodable DICOM instances found in ${files.length} selected files${failureDetail}.`,
      "UI-INGEST-02",
    );
    await refreshSnapshot();
    return;
  }

  loadedSeriesIndex = acceptedIndex;
  updateSeriesNavigatorButtons();
  const studyLabel = patientSafeMode ? "selected DICOM content" : acceptedFile.name;
  const sourceLabel = sourceKind === "folder" ? "folder" : sourceKind === "drop" ? "drop selection" : "selection";
  setStatus(
    ui.ingestStatus,
    "ok",
    `Accepted ${studyLabel} (${acceptedBytes} bytes) from ${sourceLabel}; scanned ${files.length} file(s).`,
    "UI-INGEST-01",
  );
  await refreshSnapshot();
  setStatus(
    ui.workstationStatus,
    "ok",
    "Workflow path advanced: ingest complete -> viewer open. Capture measurements and draft SR next.",
    "UI-WORKFLOW-01",
  );
  renderTriPlanarMpr();
  fusionDiagnostics();
  persistSessionState();
}

async function onSeriesStep(direction) {
  if (!viewer) {
    return;
  }
  if (loadedSeriesFiles.length === 0 || loadedSeriesIndex < 0) {
    setStatus(ui.ingestStatus, "pending", "Load a file/folder before image navigation.", "UI-INGEST-08");
    return;
  }
  const step = direction < 0 ? -1 : 1;
  let index = loadedSeriesIndex + step;
  let readFailures = 0;
  while (index >= 0 && index < loadedSeriesFiles.length) {
    const loaded = await tryLoadSeriesFileAt(index);
    if (loaded.unreadable) {
      readFailures += 1;
      index += step;
      continue;
    }
    if (!loaded.ok) {
      index += step;
      continue;
    }
    const sourceLabel = step > 0 ? "next" : "previous";
    setStatus(
      ui.ingestStatus,
      "ok",
      `Accepted ${patientSafeMode ? "selected DICOM content" : loaded.file.name} (${loaded.bytesLength} bytes) from ${sourceLabel} stack navigation.`,
      "UI-INGEST-01",
    );
    await refreshSnapshot();
    renderTriPlanarMpr();
    fusionDiagnostics();
    persistSessionState();
    return;
  }
  const boundary = step > 0 ? "end" : "start";
  const unreadableNote = readFailures > 0 ? ` (${readFailures} unreadable files skipped)` : "";
  setStatus(
    ui.ingestStatus,
    "pending",
    `Reached ${boundary} of stack with no additional decodable DICOM files${unreadableNote}.`,
    "UI-INGEST-07",
  );
  updateSeriesNavigatorButtons();
}

function onSeriesPrev() {
  void onSeriesStep(-1);
}

function onSeriesNext() {
  void onSeriesStep(1);
}

function onResizeClick() {
  recordInteractionEvent();
  if (!viewer) {
    return;
  }
  const width = parsePositiveInteger(ui.viewportWidth.value);
  const height = parsePositiveInteger(ui.viewportHeight.value);
  if (width === null || height === null) {
    setStatus(
      ui.bootStatus,
      "error",
      "Viewport size must be positive integers.",
      "UI-VIEW-01",
    );
    return;
  }
  viewer.resize(width, height);
  setStatus(ui.bootStatus, "ok", `Viewport resized to ${width}x${height}.`, "UI-VIEW-02");
  refreshSnapshot();
  persistSessionState();
}

function applyZoomValue(nextZoom) {
  if (!viewer) {
    return;
  }
  const ok = viewer.set_zoom(nextZoom);
  if (!ok) {
    setStatus(
      ui.bootStatus,
      "error",
      "Zoom must be a finite value greater than 0.",
      "UI-ZOOM-02",
    );
    return;
  }
  ui.zoomValue.value = nextZoom.toFixed(2);
  setStatus(ui.bootStatus, "ok", `Zoom updated to ${nextZoom.toFixed(2)}x.`, "UI-ZOOM-01");
  refreshSnapshot();
  persistSessionState();
}

function onSetZoomClick() {
  recordInteractionEvent();
  const zoom = parsePositiveNumber(ui.zoomValue.value);
  if (zoom === null) {
    setStatus(
      ui.bootStatus,
      "error",
      "Zoom must be a finite value greater than 0.",
      "UI-ZOOM-03",
    );
    return;
  }
  applyZoomValue(zoom);
}

function onZoomDelta(delta) {
  recordInteractionEvent();
  if (!viewer) {
    return;
  }
  const ok = viewer.zoom_by(delta);
  if (!ok) {
    setStatus(
      ui.bootStatus,
      "error",
      "Zoom delta produced an invalid value and was rejected.",
      "UI-ZOOM-04",
    );
    return;
  }
  ui.zoomValue.value = viewer.viewport_zoom().toFixed(2);
  setStatus(
    ui.bootStatus,
    "ok",
    `Zoom updated to ${viewer.viewport_zoom().toFixed(2)}x.`,
    "UI-ZOOM-01",
  );
  refreshSnapshot();
  persistSessionState();
}

function onNetworkToggle() {
  if (!viewer) {
    return;
  }
  const enabled = ui.networkToggle.checked;
  const ok = viewer.set_network_enabled(enabled);
  if (!ok) {
    ui.networkToggle.checked = false;
    setStatus(
      ui.bootStatus,
      "error",
      "Network feature is disabled in this build. Rebuild with --features network.",
      "UI-NET-01",
    );
  } else {
    const state = enabled ? "enabled" : "disabled";
    setStatus(ui.bootStatus, "ok", `Network mode ${state}.`, "UI-NET-02");
  }
  refreshSnapshot();
  persistSessionState();
}

function onForceCpuToggle() {
  if (!viewer) {
    return;
  }
  applyBackendSelection();
  const state = ui.forceCpuToggle.checked ? "forced CPU" : "auto backend";
  setStatus(ui.bootStatus, "ok", `Renderer mode updated (${state}).`, "UI-BACKEND-01");
  refreshSnapshot();
  persistSessionState();
}

function onProductionWebGpuToggle() {
  if (!viewer) {
    return;
  }
  applyBackendSelection();
  const state = productionWebGpuEnabled ? "enabled" : "disabled";
  setStatus(ui.bootStatus, "ok", `Production WebGPU path ${state}.`, "UI-BACKEND-02");
  refreshSnapshot();
  persistSessionState();
}

function onSimulateDeviceLost() {
  if (!viewer) {
    return;
  }
  const triggered = viewer.simulate_gpu_device_lost();
  if (!triggered) {
    setStatus(ui.bootStatus, "pending", "Device loss simulation ignored (backend is CPU).", "UI-GPU-01");
  } else {
    setStatus(ui.bootStatus, "error", "Simulated device loss. Renderer fell back to CPU.", "UI-GPU-02");
    setBackendStateMessage("CPU fallback active after simulated device loss.", "error", "UI-BACKEND-17");
    setGpuRecoveryBanner(true);
  }
  refreshSnapshot();
}

function onRecoverDevice() {
  if (!viewer) {
    return;
  }
  if (ui.forceCpuToggle?.checked) {
    ui.forceCpuToggle.checked = false;
  }
  applyBackendSelection();
  const backend = viewer.recover_gpu_device();
  setStatus(ui.bootStatus, "ok", `Recovery attempted; active backend: ${backend}.`, "UI-GPU-03");
  setGpuRecoveryBanner(false);
  refreshSnapshot();
}

function onInterpolationModeChange() {
  recordInteractionEvent();
  if (!viewer) {
    return;
  }
  const mode = interpolationMode();
  const ok = viewer.set_interpolation_mode(mode);
  if (!ok) {
    setStatus(
      ui.bootStatus,
      "error",
      `Unsupported interpolation mode: ${mode}`,
      "UI-INTERP-01",
    );
    return;
  }
  setStatus(ui.bootStatus, "ok", `Interpolation mode set to ${mode}.`, "UI-INTERP-02");
  refreshSnapshot();
  persistSessionState();
}

function onPreviewCanvasClick(event) {
  recordInteractionEvent();
  if (!viewer || !viewer.has_preview()) {
    return;
  }
  const rect = ui.previewCanvas.getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0) {
    return;
  }
  const u = Math.max(0, Math.min(1, (event.clientX - rect.left) / rect.width));
  const v = Math.max(0, Math.min(1, (event.clientY - rect.top) / rect.height));
  if (activeTool === "measure") {
    const point = {
      x: Math.round(u * Math.max(1, viewer.viewport_width())),
      y: Math.round(v * Math.max(1, viewer.viewport_height())),
    };
    if (!measurementDraft) {
      measurementDraft = point;
      setStatus(
        ui.workstationStatus,
        "pending",
        `Measurement start recorded at (${point.x}, ${point.y}); click endpoint to complete.`,
        "UI-MEASURE-01",
      );
      return;
    }
    addMeasurement(measurementDraft, point);
    measurementDraft = null;
    setStatus(
      ui.workstationStatus,
      "ok",
      `Measurement captured. Total measurements: ${measurements.length}.`,
      "UI-MEASURE-02",
    );
    persistSessionState();
    return;
  }
  const ok = viewer.recenter_from_viewport_fraction(u, v);
  if (!ok) {
    setStatus(ui.bootStatus, "error", "ROI recenter request was rejected.", "UI-ROI-02");
    return;
  }
  setStatus(ui.bootStatus, "ok", "ROI center updated.", "UI-ROI-01");
  refreshSnapshot();
  persistSessionState();
}

function onViewportWheel(event) {
  recordInteractionEvent();
  if (!ui.linkScrollToggle?.checked) {
    return;
  }
  event.preventDefault();
  const delta = event.deltaY > 0 ? 1 : -1;
  stepCrosshair(delta);
}

function onEscapeMetadataClick() {
  if (!ui.metadataInput) {
    return;
  }
  const value = isPatientSafeMode()
    ? "Metadata escaping disabled in patient-safe mode to avoid leaking embedded identifiers."
    : ui.metadataInput.value;
  const escaped = escapeMetadataHtml(value);
  ui.metadataOutput.textContent = escaped;
}

function onResetViewCenter() {
  recordInteractionEvent();
  if (!viewer) {
    return;
  }
  viewer.reset_view_center();
  setStatus(ui.bootStatus, "ok", "ROI view center reset.", "UI-ROI-03");
  refreshSnapshot();
  persistSessionState();
}

function onToggleInterpolation() {
  recordInteractionEvent();
  const current = ui.interpolationMode.value === "nearest" ? "linear" : "nearest";
  ui.interpolationMode.value = current;
  onInterpolationModeChange();
}

function onDownloadPreview() {
  if (!viewer) {
    return;
  }
  if (!viewer.has_preview()) {
    setStatus(ui.previewStatus, "error", "No preview available to export.", "UI-PREVIEW-05");
    return;
  }
  if (!ui.previewCanvas) {
    setStatus(
      ui.previewStatus,
      "error",
      "Preview canvas is unavailable for export.",
      "UI-PREVIEW-06",
    );
    return;
  }
  const filename = `rdvf-preview-${stableArtifactSuffix()}.png`;
  ui.previewCanvas.toBlob((blob) => {
    if (!blob) {
      setStatus(
        ui.previewStatus,
        "error",
        "Preview export failed: browser rejected image encoding.",
        "UI-PREVIEW-07",
      );
      return;
    }
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = filename;
    link.rel = "noopener";
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(url);
    setStatus(ui.previewStatus, "ok", `Preview exported as ${filename}.`, "UI-PREVIEW-08");
  }, "image/png");
}

function onClearEventLog() {
  if (!ui.eventLog) {
    return;
  }
  ui.eventLog.innerHTML = "";
  setStatus(ui.bootStatus, "ok", "Event log cleared.", "UI-EVENT-01");
}

function onExportEventLog() {
  if (!ui.eventLog || ui.eventLog.children.length === 0) {
    setStatus(ui.bootStatus, "pending", "Event log is empty; nothing to export.", "UI-EVENT-02");
    return;
  }
  const payload = [...ui.eventLog.children]
    .map((entry) => entry.textContent)
    .join("\n");
  const blob = new Blob([payload], { type: "text/plain;charset=utf-8" });
  const filename = `rdvf-event-log-${new Date().toISOString().replace(/[:.]/g, "-")}.txt`;
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.rel = "noopener";
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  URL.revokeObjectURL(url);
  setStatus(ui.bootStatus, "ok", `Event log exported as ${filename}.`, "UI-EVENT-03");
}

function snapshotPayload() {
  return {
    timestamp: new Date().toISOString(),
    interactionMetrics,
    viewport: {
      width: viewer?.viewport_width() ?? null,
      height: viewer?.viewport_height() ?? null,
      zoom: viewer?.viewport_zoom() ?? null,
    },
    networkEnabled: Boolean(viewer?.network_enabled()),
    loadedBytes: viewer?.loaded_input_bytes() ?? null,
    interpolationMode: viewer?.interpolation_mode() ?? null,
    hasPreview: Boolean(viewer?.has_preview()),
    previewSize: {
      width: viewer?.preview_width() ?? null,
      height: viewer?.preview_height() ?? null,
    },
    triPlanar: (() => {
      try {
        return JSON.parse(viewer?.tri_planar_state_json?.() ?? "{}");
      } catch (_error) {
        return null;
      }
    })(),
    mprSlab: (() => {
      try {
        return JSON.parse(viewer?.mpr_slab_json?.() ?? "{}");
      } catch (_error) {
        return null;
      }
    })(),
    backendCapabilities,
    backendProbe,
    backend: backendLabel,
    productionWebGpuEnabled,
    activeBackend: viewer?.active_renderer_backend?.() ?? null,
    backendMetrics: (() => {
      try {
        return JSON.parse(viewer?.backend_selection_metrics_json?.() ?? "{}");
      } catch (_error) {
        return null;
      }
    })(),
    presenter: {
      failed: webGpuPresenter.failed,
      lastError: webGpuPresenter.lastError,
      textureWidth: webGpuPresenter.textureWidth,
      textureHeight: webGpuPresenter.textureHeight,
    },
    cspShaderSource: viewer?.csp_safe_shader_source?.() ?? null,
    ui: {
      zoomInput: ui.zoomValue?.value ?? null,
      interpolationInput: ui.interpolationMode?.value ?? null,
      ingestState: ui.ingestStatus ? ui.ingestStatus.textContent : null,
      previewStatus: ui.previewStatus ? ui.previewStatus.textContent : null,
      bootStatus: ui.bootStatus ? ui.bootStatus.textContent : null,
      srStatus: ui.srStatus ? ui.srStatus.textContent : null,
    },
    srWorkflow: {
      baseUrl: srBaseUrl() || null,
      sopUid: ui.srSopUid?.value ?? null,
      expectedVersion: ui.srExpectedVersion?.value ?? null,
      role: srRole(),
    },
    viewportPreset,
    activeTool,
    windowLevel: currentWindowLevel(),
    measurements,
    connectivity: {
      dicomweb: ui.connectDicomweb?.textContent ?? null,
      workflow: ui.connectWorkflow?.textContent ?? null,
    },
    mprPlanes: latestMprPlanes
      ? {
          width: latestMprPlanes.width,
          height: latestMprPlanes.height,
        }
      : null,
    fusion: latestFusionFrame
      ? {
          width: latestFusionFrame.width,
          height: latestFusionFrame.height,
          alpha: latestFusionFrame.alpha,
          colormap: latestFusionFrame.colormap,
          suvScale: latestFusionFrame.suvScale,
        }
      : null,
    userAgent: navigator.userAgent,
    devicePixelRatio: window.devicePixelRatio,
  };
}

function onExportSnapshot() {
  if (!viewer) {
    setStatus(ui.bootStatus, "error", "Cannot export snapshot before WASM is initialized.", "UI-SNAP-01");
    return;
  }
  const blob = new Blob([JSON.stringify(snapshotPayload(), null, 2)], {
    type: "application/json;charset=utf-8",
  });
  const filename = `rdvf-snapshot-${stableArtifactSuffix()}.json`;
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.rel = "noopener";
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  URL.revokeObjectURL(url);
  setStatus(ui.bootStatus, "ok", `Runtime snapshot exported as ${filename}.`, "UI-SNAP-02");
}

function dicomwebBaseUrl() {
  return readServiceBaseUrl(ui.dicomwebBaseUrl, {
    proxyPath: DICOMWEB_PROXY_PATH,
    fallbackPort: DICOMWEB_DEFAULT_PORT,
    legacyPort: DICOMWEB_DEFAULT_PORT,
  });
}

function srBaseUrl() {
  return readServiceBaseUrl(ui.srBaseUrl, {
    proxyPath: WORKFLOW_PROXY_PATH,
    fallbackPort: WORKFLOW_DEFAULT_PORT,
    legacyPort: WORKFLOW_DEFAULT_PORT,
  });
}

function srIdempotencyKey(operation) {
  const params = new URLSearchParams();
  appendSrBasePayload(params);
  appendSrItemPayload(params);
  if (operation === "update") {
    params.set("expected_version", String(ui.srExpectedVersion?.value || ""));
  }
  const canonical = `${operation}|${(ui.srSopUid?.value || "").trim()}|${params.toString()}`;
  return `${operation}-${deterministicHash(canonical)}`;
}

function srAuditHash(value) {
  return deterministicHash(String(value || ""));
}

function pushSrAuditEntry(operation, outcome, details = "") {
  if (!ui.srAuditTimeline) {
    return;
  }
  const item = document.createElement("li");
  item.className = `event ${outcome === "ok" ? "event-ok" : outcome === "pending" ? "event-pending" : "event-error"}`;
  item.textContent = `${new Date().toISOString()} op=${operation} outcome=${outcome} sop_hash=${srAuditHash(
    ui.srSopUid?.value,
  )} principal_hash=${srAuditHash(ui.srObserver?.value)} detail=${details}`.slice(0, 360);
  ui.srAuditTimeline.prepend(item);
  while (ui.srAuditTimeline.children.length > 20) {
    ui.srAuditTimeline.removeChild(ui.srAuditTimeline.lastChild);
  }
}

function validateSrForm() {
  const code = (ui.srConceptCode?.value || "").trim();
  const scheme = (ui.srConceptScheme?.value || "").trim();
  const meaning = (ui.srConceptMeaning?.value || "").trim();
  if (!code || !scheme || !meaning) {
    return "Concept code/scheme/meaning are required.";
  }
  const itemKind = (ui.srItemKind?.value || "num").trim();
  if (itemKind === "num") {
    const numValue = Number.parseFloat(ui.srNumValue?.value || "");
    if (!Number.isFinite(numValue)) {
      return "NUM value must be numeric.";
    }
    const unitsCode = (ui.srUnitsCode?.value || "").trim();
    const unitsScheme = (ui.srUnitsScheme?.value || "").trim();
    const unitsMeaning = (ui.srUnitsMeaning?.value || "").trim();
    if (!unitsCode || !unitsScheme || !unitsMeaning) {
      return "NUM units code/scheme/meaning are required.";
    }
  }
  if (itemKind === "text") {
    const textValue = (ui.srTextValue?.value || "").trim();
    if (!textValue) {
      return "TEXT value is required.";
    }
  }
  return null;
}

async function fetchWithRetry(url, init, maxAttempts = 3) {
  let lastResponse = null;
  for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
    // Keep deterministic idempotency per attempt sequence.
    const headers = new Headers(init.headers || {});
    headers.set("x-retry-attempt", String(attempt));
    const response = await fetch(url, {
      ...init,
      headers,
    });
    lastResponse = response;
    if (response.ok || !WORKFLOW_RETRYABLE.has(response.status) || attempt === maxAttempts) {
      return response;
    }
  }
  return lastResponse;
}

function appendSrBasePayload(params) {
  params.set("study_instance_uid", (ui.srStudyUid?.value || "").trim());
  params.set("series_instance_uid", (ui.srSeriesUid?.value || "").trim());
  params.set("sop_instance_uid", (ui.srSopUid?.value || "").trim());
  params.set("observer", (ui.srObserver?.value || "web-prototype").trim());
  params.set("known_refs", (ui.srKnownRefs?.value || "").trim());
  params.set("concept_code_value", (ui.srConceptCode?.value || "121071").trim());
  params.set("concept_scheme", (ui.srConceptScheme?.value || "DCM").trim());
  params.set("concept_meaning", (ui.srConceptMeaning?.value || "Finding").trim());
}

function appendSrItemPayload(params) {
  const itemKind = (ui.srItemKind?.value || "num").trim();
  params.set("item_kind", itemKind);
  const knownRefsRaw = (ui.srKnownRefs?.value || "").split(",").map((value) => value.trim()).filter(Boolean);
  if (knownRefsRaw.length > 0) {
    params.set("referenced_sop_instance_uid", knownRefsRaw[0]);
  }
  if (itemKind === "num") {
    const numeric = Number.parseFloat(ui.srNumValue?.value || "0");
    params.set("num_value", Number.isFinite(numeric) ? numeric.toString() : "0");
    params.set("units_code_value", (ui.srUnitsCode?.value || "mm").trim());
    params.set("units_scheme", (ui.srUnitsScheme?.value || "UCUM").trim());
    params.set("units_meaning", (ui.srUnitsMeaning?.value || "millimeter").trim());
  } else {
    params.set("text_value", (ui.srTextValue?.value || "").trim());
  }
}

async function onSrCreate() {
  const baseUrl = srBaseUrl();
  if (!baseUrl) {
    setStatus(ui.srStatus, "error", "Workflow base URL is required.", "UI-SR-01");
    return;
  }
  if (srRole() !== "writer") {
    setStatus(ui.srStatus, "error", "Create requires x-sr-role=writer.", "UI-SR-14");
    pushSrAuditEntry("create", "error", "role-mismatch");
    return;
  }
  const validationError = validateSrForm();
  if (validationError) {
    setStatus(ui.srStatus, "error", validationError, "UI-SR-15");
    pushSrAuditEntry("create", "error", validationError);
    return;
  }
  const params = new URLSearchParams();
  appendSrBasePayload(params);
  appendSrItemPayload(params);
  params.set("authored_epoch_ms", `${Date.now()}`);
  const principal = (ui.srObserver?.value || "web-prototype").trim();
  const response = await fetchWithRetry(`${baseUrl}/sr/documents`, {
    method: "POST",
    headers: {
      "content-type": "application/x-www-form-urlencoded",
      "x-sr-principal": principal,
      "x-sr-role": srRole(),
      "x-idempotency-key": srIdempotencyKey("create"),
    },
    body: params.toString(),
  });
  const payload = await response.text();
  if (ui.srOutput) {
    ui.srOutput.textContent = payload;
  }
  if (!response.ok) {
    setStatus(ui.srStatus, "error", `SR create failed (${response.status}).`, "UI-SR-02");
    pushSrAuditEntry("create", "error", `status=${response.status}`);
    return;
  }
  try {
    const parsed = JSON.parse(payload);
    if (ui.srExpectedVersion && Number.isFinite(parsed?.version)) {
      ui.srExpectedVersion.value = String(parsed.version);
    }
    lastLoadedSrDocument = parsed;
    renderSrDetail(parsed);
  } catch (_error) {
    // Keep raw response in output panel.
  }
  setStatus(ui.srStatus, "ok", "SR create committed through workflow service.", "UI-SR-03");
  setStatus(ui.workstationStatus, "ok", "Workflow path advanced: SR draft created.", "UI-WORKFLOW-02");
  pushSrAuditEntry("create", "ok", "committed");
  persistSessionState();
}

async function onSrUpdate() {
  const baseUrl = srBaseUrl();
  const sopUid = (ui.srSopUid?.value || "").trim();
  if (!baseUrl || !sopUid) {
    setStatus(ui.srStatus, "error", "Workflow base URL and SR SOP UID are required.", "UI-SR-04");
    return;
  }
  if (srRole() !== "writer") {
    setStatus(ui.srStatus, "error", "Update requires x-sr-role=writer.", "UI-SR-16");
    pushSrAuditEntry("update", "error", "role-mismatch");
    return;
  }
  const validationError = validateSrForm();
  if (validationError) {
    setStatus(ui.srStatus, "error", validationError, "UI-SR-15");
    pushSrAuditEntry("update", "error", validationError);
    return;
  }
  const expectedVersion = Number.parseInt(ui.srExpectedVersion?.value || "0", 10);
  if (!Number.isFinite(expectedVersion) || expectedVersion < 1) {
    setStatus(ui.srStatus, "error", "Expected version must be >= 1.", "UI-SR-05");
    return;
  }
  const params = new URLSearchParams();
  appendSrBasePayload(params);
  appendSrItemPayload(params);
  params.set("expected_version", String(expectedVersion));
  const principal = (ui.srObserver?.value || "web-prototype").trim();
  const response = await fetchWithRetry(`${baseUrl}/sr/documents/${encodeURIComponent(sopUid)}/updates`, {
    method: "POST",
    headers: {
      "content-type": "application/x-www-form-urlencoded",
      "x-sr-principal": principal,
      "x-sr-role": srRole(),
      "x-idempotency-key": srIdempotencyKey("update"),
    },
    body: params.toString(),
  });
  const payload = await response.text();
  if (ui.srOutput) {
    ui.srOutput.textContent = payload;
  }
  if (!response.ok) {
    let conflictHint = "";
    try {
      const parsed = JSON.parse(payload);
      if (response.status === 409) {
        const observed = parsed?.version ?? parsed?.observed_version ?? "unknown";
        conflictHint = ` Version conflict: expected ${expectedVersion}, observed ${observed}.`;
      }
    } catch (_error) {
      // Keep plain status.
    }
    setStatus(
      ui.srStatus,
      "error",
      `SR update failed (${response.status}).${conflictHint}`,
      "UI-SR-06",
    );
    pushSrAuditEntry("update", "error", `status=${response.status}`);
    return;
  }
  try {
    const parsed = JSON.parse(payload);
    if (ui.srExpectedVersion && Number.isFinite(parsed?.version)) {
      ui.srExpectedVersion.value = String(parsed.version);
    }
    lastLoadedSrDocument = parsed;
    renderSrDetail(parsed);
  } catch (_error) {
    // Keep raw response in output panel.
  }
  setStatus(ui.srStatus, "ok", "SR update committed through workflow service.", "UI-SR-07");
  setStatus(ui.workstationStatus, "ok", "Workflow path advanced: SR draft updated.", "UI-WORKFLOW-03");
  pushSrAuditEntry("update", "ok", "committed");
  persistSessionState();
}

async function onSrLoad() {
  const baseUrl = srBaseUrl();
  const sopUid = (ui.srSopUid?.value || "").trim();
  if (!baseUrl || !sopUid) {
    setStatus(ui.srStatus, "error", "Workflow base URL and SR SOP UID are required.", "UI-SR-08");
    return;
  }
  const response = await fetch(`${baseUrl}/sr/documents/${encodeURIComponent(sopUid)}`, {
    method: "GET",
  });
  const payload = await response.text();
  if (ui.srOutput) {
    ui.srOutput.textContent = payload;
  }
  if (!response.ok) {
    setStatus(ui.srStatus, "error", `SR load failed (${response.status}).`, "UI-SR-09");
    pushSrAuditEntry("retrieve", "error", `status=${response.status}`);
    return;
  }
  try {
    const parsed = JSON.parse(payload);
    if (ui.srExpectedVersion && Number.isFinite(parsed?.version)) {
      ui.srExpectedVersion.value = String(parsed.version);
    }
    lastLoadedSrDocument = parsed;
    renderSrDetail(parsed);
  } catch (_error) {
    // Keep raw response in output panel.
  }
  setStatus(ui.srStatus, "ok", "SR document loaded from workflow service.", "UI-SR-10");
  setStatus(ui.workstationStatus, "ok", "Workflow path complete: SR retrieve succeeded.", "UI-WORKFLOW-04");
  pushSrAuditEntry("retrieve", "ok", "loaded");
  persistSessionState();
}

function renderSrDetail(documentPayload) {
  if (!ui.srDetailOutput) {
    return;
  }
  const doc = documentPayload || lastLoadedSrDocument;
  if (!doc) {
    ui.srDetailOutput.textContent = "";
    return;
  }
  const detail = {
    sop_instance_uid: doc.sop_instance_uid ?? doc.provenance?.sop_instance_uid ?? null,
    version: doc.version ?? null,
    observer: doc.provenance?.observer ?? null,
    authored_epoch_ms: doc.provenance?.authored_epoch_ms ?? null,
    study_instance_uid: doc.provenance?.study_instance_uid ?? null,
    series_instance_uid: doc.provenance?.series_instance_uid ?? null,
    item_count: Array.isArray(doc.items) ? doc.items.length : doc.item_count ?? null,
  };
  ui.srDetailOutput.textContent = JSON.stringify(detail, null, 2);
}

async function onSrList() {
  const baseUrl = srBaseUrl();
  if (!baseUrl) {
    setStatus(ui.srStatus, "error", "Workflow base URL is required.", "UI-SR-17");
    return;
  }
  const params = new URLSearchParams();
  const studyUid = (ui.srListStudyUid?.value || "").trim();
  if (studyUid) {
    params.set("study_instance_uid", studyUid);
  }
  const endpoint = `${baseUrl}/sr/documents${params.toString() ? `?${params.toString()}` : ""}`;
  const response = await fetch(endpoint, { method: "GET" });
  const payload = await response.text();
  if (!response.ok) {
    setStatus(ui.srStatus, "error", `SR list failed (${response.status}).`, "UI-SR-18");
    if (ui.srListOutput) {
      ui.srListOutput.textContent = payload;
    }
    return;
  }
  let rows = [];
  try {
    rows = JSON.parse(payload);
  } catch (_error) {
    rows = [];
  }
  const query = (ui.srListQuery?.value || "").trim();
  if (query) {
    rows = rows.filter((item) => String(item?.sop_instance_uid || "").startsWith(query));
  }
  if (ui.srListOutput) {
    ui.srListOutput.textContent = JSON.stringify(rows, null, 2);
  }
  setStatus(ui.srStatus, "ok", `SR list loaded (${rows.length} rows).`, "UI-SR-19");
  pushSrAuditEntry("list", "ok", `rows=${rows.length}`);
}

function updateSrRoleControls() {
  const writer = srRole() === "writer";
  if (ui.srCreate) {
    ui.srCreate.disabled = !writer;
  }
  if (ui.srUpdate) {
    ui.srUpdate.disabled = !writer;
  }
  if (!writer) {
    setStatus(ui.srStatus, "pending", "SR write actions disabled for non-writer role.", "UI-SR-20");
  }
}

function onKeyShortcut(event) {
  if (!viewer) {
    return;
  }
  if (event.target.closest("input, textarea, select, button")) {
    return;
  }
  let handled = true;
  switch (event.key) {
    case "+":
    case "=":
    case "NumpadAdd":
      onZoomDelta(0.1);
      break;
    case "-":
    case "NumpadSubtract":
      onZoomDelta(-0.1);
      break;
    case "0":
    case "Numpad0":
      applyZoomValue(1.0);
      break;
    case "ArrowUp":
      panViewportByDelta(0, -0.08);
      break;
    case "ArrowDown":
      panViewportByDelta(0, 0.08);
      break;
    case "ArrowLeft":
      panViewportByDelta(-0.08, 0);
      break;
    case "ArrowRight":
      panViewportByDelta(0.08, 0);
      break;
    case "[":
      setWindowLevel(currentWindowLevel().width, currentWindowLevel().level - 25);
      onApplyWindowLevel();
      break;
    case "]":
      setWindowLevel(currentWindowLevel().width, currentWindowLevel().level + 25);
      onApplyWindowLevel();
      break;
    case "{":
      setWindowLevel(currentWindowLevel().width - 50, currentWindowLevel().level);
      onApplyWindowLevel();
      break;
    case "}":
      setWindowLevel(currentWindowLevel().width + 50, currentWindowLevel().level);
      onApplyWindowLevel();
      break;
    case "m":
    case "M":
      updateActiveTool(activeTool === "measure" ? "pan" : "measure");
      break;
    case "r":
    case "R":
      onResetViewCenter();
      break;
    case "i":
    case "I":
      onToggleInterpolation();
      break;
    case "d":
    case "D":
      onDownloadPreview();
      break;
    case "s":
    case "S":
      onExportSnapshot();
      break;
    case "e":
    case "E":
      onExportEventLog();
      break;
    case "c":
    case "C":
      onClearEventLog();
      break;
    default:
      handled = false;
      break;
  }
  if (handled) {
    recordInteractionEvent();
    event.preventDefault();
  }
}

function wireEvents() {
  ui.dicomFile.addEventListener("change", onDicomSelected);
  if (ui.dicomFolder) {
    ui.dicomFolder.addEventListener("change", onDicomFolderSelected);
  }
  if (ui.seriesPrev) {
    ui.seriesPrev.addEventListener("click", onSeriesPrev);
  }
  if (ui.seriesNext) {
    ui.seriesNext.addEventListener("click", onSeriesNext);
  }
  document.addEventListener("dragover", (event) => {
    event.preventDefault();
  });
  document.addEventListener("drop", onDicomDrop);
  ui.applyResize.addEventListener("click", onResizeClick);
  if (ui.applyViewportPreset) {
    ui.applyViewportPreset.addEventListener("click", onApplyViewportPreset);
  }
  if (ui.linkWindowLevelToggle) {
    ui.linkWindowLevelToggle.addEventListener("change", () => {
      syncComparisonViewports(latestPreviewFrame);
      setStatus(
        ui.workstationStatus,
        "ok",
        `Linked window/level ${ui.linkWindowLevelToggle.checked ? "enabled" : "disabled"}.`,
        "UI-VIEW-04",
      );
    });
  }
  if (ui.linkScrollToggle) {
    ui.linkScrollToggle.addEventListener("change", () => {
      setStatus(
        ui.workstationStatus,
        "ok",
        `Linked scrolling ${ui.linkScrollToggle.checked ? "enabled" : "disabled"}.`,
        "UI-VIEW-05",
      );
    });
  }
  ui.setZoom.addEventListener("click", onSetZoomClick);
  ui.interpolationMode.addEventListener("change", onInterpolationModeChange);
  if (ui.modalityPreset) {
    ui.modalityPreset.addEventListener("change", () => {
      applyModalityPreset();
      onApplyWindowLevel();
    });
  }
  if (ui.applyWindowLevel) {
    ui.applyWindowLevel.addEventListener("click", onApplyWindowLevel);
  }
  ui.zoomOut.addEventListener("click", () => onZoomDelta(-0.1));
  ui.zoomIn.addEventListener("click", () => onZoomDelta(0.1));
  ui.networkToggle.addEventListener("change", onNetworkToggle);
  if (ui.forceCpuToggle) {
    ui.forceCpuToggle.addEventListener("change", onForceCpuToggle);
  }
  if (ui.productionWebGpuToggle) {
    ui.productionWebGpuToggle.addEventListener("change", onProductionWebGpuToggle);
  }
  if (ui.simulateDeviceLost) {
    ui.simulateDeviceLost.addEventListener("click", onSimulateDeviceLost);
  }
  if (ui.recoverDevice) {
    ui.recoverDevice.addEventListener("click", onRecoverDevice);
  }
  ui.previewCanvas.addEventListener("click", onPreviewCanvasClick);
  ui.previewCanvas.addEventListener("wheel", onViewportWheel, { passive: false });
  if (ui.compareCanvasA) {
    ui.compareCanvasA.addEventListener("wheel", onViewportWheel, { passive: false });
  }
  if (ui.compareCanvasB) {
    ui.compareCanvasB.addEventListener("wheel", onViewportWheel, { passive: false });
  }
  if (ui.compareCanvasC) {
    ui.compareCanvasC.addEventListener("wheel", onViewportWheel, { passive: false });
  }
  ui.escapeMetadata.addEventListener("click", onEscapeMetadataClick);
  if (ui.downloadPreview) {
    ui.downloadPreview.addEventListener("click", onDownloadPreview);
  }
  if (ui.renderMpr) {
    ui.renderMpr.addEventListener("click", renderTriPlanarMpr);
  }
  if (ui.exportMprBundle) {
    ui.exportMprBundle.addEventListener("click", exportMprBundle);
  }
  if (ui.runPatientMpr) {
    ui.runPatientMpr.addEventListener("click", () => {
      void onRunPatientMpr();
    });
  }
  if (ui.mprSlabThickness) {
    ui.mprSlabThickness.addEventListener("change", renderTriPlanarMpr);
  }
  if (ui.mprSlabMode) {
    ui.mprSlabMode.addEventListener("change", renderTriPlanarMpr);
  }
  if (ui.mprAxialCanvas) {
    ui.mprAxialCanvas.addEventListener("click", (event) => onMprCanvasClick("axial", event));
  }
  if (ui.mprCoronalCanvas) {
    ui.mprCoronalCanvas.addEventListener("click", (event) => onMprCanvasClick("coronal", event));
  }
  if (ui.mprSagittalCanvas) {
    ui.mprSagittalCanvas.addEventListener("click", (event) => onMprCanvasClick("sagittal", event));
  }
  if (ui.clearEventLog) {
    ui.clearEventLog.addEventListener("click", onClearEventLog);
  }
  if (ui.exportEventLog) {
    ui.exportEventLog.addEventListener("click", onExportEventLog);
  }
  if (ui.exportSnapshot) {
    ui.exportSnapshot.addEventListener("click", onExportSnapshot);
  }
  if (ui.srCreate) {
    ui.srCreate.addEventListener("click", () => {
      onSrCreate().catch((error) => {
        const details = error instanceof Error ? error.message : String(error);
        setStatus(ui.srStatus, "error", `SR create failed: ${details}`, "UI-SR-11");
      });
    });
  }
  if (ui.srUpdate) {
    ui.srUpdate.addEventListener("click", () => {
      onSrUpdate().catch((error) => {
        const details = error instanceof Error ? error.message : String(error);
        setStatus(ui.srStatus, "error", `SR update failed: ${details}`, "UI-SR-12");
      });
    });
  }
  if (ui.srLoad) {
    ui.srLoad.addEventListener("click", () => {
      onSrLoad().catch((error) => {
        const details = error instanceof Error ? error.message : String(error);
        setStatus(ui.srStatus, "error", `SR load failed: ${details}`, "UI-SR-13");
      });
    });
  }
  if (ui.srList) {
    ui.srList.addEventListener("click", () => {
      onSrList().catch((error) => {
        const details = error instanceof Error ? error.message : String(error);
        setStatus(ui.srStatus, "error", `SR list failed: ${details}`, "UI-SR-21");
      });
    });
  }
  if (ui.srRole) {
    ui.srRole.addEventListener("change", updateSrRoleControls);
  }
  if (ui.toolPan) {
    ui.toolPan.addEventListener("click", () => updateActiveTool("pan"));
  }
  if (ui.toolMeasure) {
    ui.toolMeasure.addEventListener("click", () => updateActiveTool("measure"));
  }
  if (ui.restoreSession) {
    ui.restoreSession.addEventListener("click", () => {
      restoreSessionState();
      onApplyViewportPreset();
      onApplyWindowLevel();
      refreshSnapshot();
      if (viewer?.has_preview()) {
        renderTriPlanarMpr();
      }
    });
  }
  if (ui.patientSafeMode) {
    ui.patientSafeMode.addEventListener("change", () => {
      patientSafeMode = ui.patientSafeMode.checked === true;
      persistSessionState();
      setStatus(
        ui.workstationStatus,
        "ok",
        `Patient-safe mode ${patientSafeMode ? "enabled" : "disabled"}.`,
        patientSafeMode ? "UI-PATIENT-02" : "UI-PATIENT-01",
      );
    });
  }
  if (ui.checkConnectivity) {
    ui.checkConnectivity.addEventListener("click", () => {
      void runConnectivityChecks();
    });
  }
  if (ui.dicomwebBaseUrl) {
    ui.dicomwebBaseUrl.addEventListener("change", () => {
      dicomwebBaseUrl();
      persistSessionState();
    });
  }
  if (ui.srBaseUrl) {
    ui.srBaseUrl.addEventListener("change", () => {
      srBaseUrl();
      persistSessionState();
    });
  }
  if (ui.studyCatalogFilter) {
    ui.studyCatalogFilter.addEventListener("input", () => {
      studyCatalogFilter = ui.studyCatalogFilter.value || "";
      studyCatalogPage = 1;
      persistStudyCatalogState();
      renderStudyCatalogRows();
      renderPinnedStudies();
    });
  }
  if (ui.studyCatalogSort) {
    ui.studyCatalogSort.addEventListener("change", () => {
      const nextSort = ui.studyCatalogSort.value;
      if (STUDY_CATALOG_SORT_OPTIONS.has(nextSort)) {
        studyCatalogSort = nextSort;
      } else {
        studyCatalogSort = STUDY_CATALOG_SORT_DEFAULT;
      }
      studyCatalogPage = 1;
      persistStudyCatalogState();
      renderStudyCatalogRows();
      renderPinnedStudies();
    });
  }
  if (ui.studyCatalogPageSize) {
    ui.studyCatalogPageSize.addEventListener("change", () => {
      studyCatalogPageSize = clampStudyCatalogPageSize(ui.studyCatalogPageSize.value);
      studyCatalogPage = 1;
      persistStudyCatalogState();
      renderStudyCatalogRows();
      renderPinnedStudies();
    });
  }
  if (ui.studyCatalogPrevPage) {
    ui.studyCatalogPrevPage.addEventListener("click", () => {
      studyCatalogPage = clampStudyCatalogPage(studyCatalogPage - 1);
      persistStudyCatalogState();
      renderStudyCatalogRows();
      renderPinnedStudies();
    });
  }
  if (ui.studyCatalogNextPage) {
    ui.studyCatalogNextPage.addEventListener("click", () => {
      studyCatalogPage = clampStudyCatalogPage(studyCatalogPage + 1);
      persistStudyCatalogState();
      renderStudyCatalogRows();
      renderPinnedStudies();
    });
  }
  if (ui.studyCatalogRefresh) {
    ui.studyCatalogRefresh.addEventListener("click", () => {
      void refreshStudyCatalog();
    });
  }
  if (ui.clearMeasurements) {
    ui.clearMeasurements.addEventListener("click", clearMeasurements);
  }
  if (ui.exportMeasurementsCsv) {
    ui.exportMeasurementsCsv.addEventListener("click", () => exportMeasurements("csv"));
  }
  if (ui.exportMeasurementsJson) {
    ui.exportMeasurementsJson.addEventListener("click", () => exportMeasurements("json"));
  }
  if (ui.runFusion) {
    ui.runFusion.addEventListener("click", renderFusion);
  }
  if (ui.exportFusionBundle) {
    ui.exportFusionBundle.addEventListener("click", exportFusionBundle);
  }
  if (ui.fusionAlpha) {
    ui.fusionAlpha.addEventListener("change", fusionDiagnostics);
  }
  if (ui.fusionSuvScale) {
    ui.fusionSuvScale.addEventListener("change", fusionDiagnostics);
  }
  if (ui.fusionColormap) {
    ui.fusionColormap.addEventListener("change", fusionDiagnostics);
  }
  if (ui.gpuRecoveryAction) {
    ui.gpuRecoveryAction.addEventListener("click", onRecoverDevice);
  }
  document.addEventListener("keydown", onKeyShortcut);
}

if (typeof window !== "undefined") {
  window.__RDVF_GET_SNAPSHOT = () => snapshotPayload();
}

async function bootstrap() {
  try {
    await init();
    await resolveBackendLabel();
    viewer = new WasmViewer(1024, 768);
    ensurePreviewBindings(viewer);
    productionWebGpuEnabled = resolveInitialProductionWebGpuFlag();
    if (ui.productionWebGpuToggle) {
      ui.productionWebGpuToggle.checked = productionWebGpuEnabled;
    }
    viewer.set_production_webgpu_renderer(productionWebGpuEnabled);
    productionWebGpuEnabled = Boolean(viewer.production_webgpu_renderer_enabled());
    viewer.set_interpolation_mode(interpolationMode());
    viewer.set_mpr_slab(1, "average");
    viewer.set_render_frame_interval_ms(16);
    applyModalityPreset();
    setViewportPreset(ui.viewportPreset?.value || "single");
    updateActiveTool("pan");
    updateMeasurementList();
    updateSrRoleControls();
    await applyBackendSelection();
    wireEvents();
    updateSeriesNavigatorButtons();
    normalizeServiceEndpointInputs();
    restoreStudyCatalogState();
    renderStudyCatalogRows();
    renderPinnedStudies();
    restoreSessionState();
    await refreshSnapshot();
    fusionDiagnostics();
    startConnectivityPolling();
    void refreshStudyCatalog();
    void runConnectivityChecks();
    setStatus(ui.bootStatus, "ok", `WASM module loaded (${backendLabel}).`, "UI-BOOT-01");
    onEscapeMetadataClick();
  } catch (error) {
    const details = error instanceof Error ? error.message : String(error);
    setStatus(ui.bootStatus, "error", `Failed to initialize WASM module: ${details}`, "UI-BOOT-02");
  }
}

void bootstrap();
