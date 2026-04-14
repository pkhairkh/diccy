<script setup lang="ts">
import { computed, ref } from "vue";
import ClinicalFusionPanel from "../panels/ClinicalFusionPanel.vue";
import ClinicalMeasurementPanel from "../panels/ClinicalMeasurementPanel.vue";
import ClinicalMprPanel from "../panels/ClinicalMprPanel.vue";
import ClinicalRtPanel from "../panels/ClinicalRtPanel.vue";
import ClinicalSegmentationPanel from "../panels/ClinicalSegmentationPanel.vue";
import { classTokens } from "../../styles/tokens";

type MeasurementKind = "length" | "angle" | "area";

type MeasurementRecord = {
  id: string;
  label: string;
  kind: MeasurementKind;
  sliceIndex: number;
  confidence: number;
};

type SegmentationRecord = {
  id: string;
  name: string;
  source: string;
  color: string;
  locked: boolean;
};

type Snapshot = {
  measurements: MeasurementRecord[];
  segmentations: SegmentationRecord[];
  viewportPlane: "axial" | "sagittal" | "coronal";
  linkedViewports: boolean;
  mipEnabled: boolean;
  volume3dEnabled: boolean;
  fusionAlphaPercent: number;
  fusionColormap: "hot" | "cool" | "gray";
  registrationRmse: number;
  registrationStatus: string;
  doseOpacityPercent: number;
  isoDosePercent: number;
  rtssVisible: boolean;
  rtValidationMessage: string;
  capabilityMessage: string;
  lastFocusTarget: string;
  lastExportSummary: string;
};

const measurements = ref<MeasurementRecord[]>([
  { id: "M-001", label: "Aortic diameter", kind: "length", sliceIndex: 32, confidence: 99 },
  { id: "M-002", label: "Lesion axis", kind: "angle", sliceIndex: 41, confidence: 96 },
]);
const segmentations = ref<SegmentationRecord[]>([
  { id: "SEG-001", name: "Liver", source: "manual", color: "amber", locked: false },
]);
const viewportPlane = ref<"axial" | "sagittal" | "coronal">("axial");
const linkedViewports = ref(true);
const mipEnabled = ref(false);
const volume3dEnabled = ref(false);
const supportsMip = ref(false);
const supportsVolume3d = ref(true);
const capabilityMessage = ref("");
const fusionAlphaPercent = ref(45);
const fusionColormap = ref<"hot" | "cool" | "gray">("hot");
const registrationRmse = ref(1.65);
const registrationStatus = ref("aligned");
const doseOpacityPercent = ref(55);
const isoDosePercent = ref(70);
const rtssVisible = ref(true);
const rtValidationMessage = ref("");
const lastFocusTarget = ref("M-001");
const lastExportSummary = ref("No export requested yet.");

const measurementSequence = ref(3);
const segmentationSequence = ref(2);
const undoStack = ref<Snapshot[]>([]);
const redoStack = ref<Snapshot[]>([]);

const orderedMeasurements = computed(() =>
  [...measurements.value].sort((left, right) => left.id.localeCompare(right.id)),
);
const orderedSegmentations = computed(() =>
  [...segmentations.value].sort((left, right) => left.id.localeCompare(right.id)),
);

function cloneMeasurements(rows: MeasurementRecord[]): MeasurementRecord[] {
  return rows.map((row) => ({ ...row }));
}

function cloneSegmentations(rows: SegmentationRecord[]): SegmentationRecord[] {
  return rows.map((row) => ({ ...row }));
}

function snapshotState(): Snapshot {
  return {
    measurements: cloneMeasurements(measurements.value),
    segmentations: cloneSegmentations(segmentations.value),
    viewportPlane: viewportPlane.value,
    linkedViewports: linkedViewports.value,
    mipEnabled: mipEnabled.value,
    volume3dEnabled: volume3dEnabled.value,
    fusionAlphaPercent: fusionAlphaPercent.value,
    fusionColormap: fusionColormap.value,
    registrationRmse: registrationRmse.value,
    registrationStatus: registrationStatus.value,
    doseOpacityPercent: doseOpacityPercent.value,
    isoDosePercent: isoDosePercent.value,
    rtssVisible: rtssVisible.value,
    rtValidationMessage: rtValidationMessage.value,
    capabilityMessage: capabilityMessage.value,
    lastFocusTarget: lastFocusTarget.value,
    lastExportSummary: lastExportSummary.value,
  };
}

function restoreSnapshot(snapshot: Snapshot) {
  measurements.value = cloneMeasurements(snapshot.measurements);
  segmentations.value = cloneSegmentations(snapshot.segmentations);
  viewportPlane.value = snapshot.viewportPlane;
  linkedViewports.value = snapshot.linkedViewports;
  mipEnabled.value = snapshot.mipEnabled;
  volume3dEnabled.value = snapshot.volume3dEnabled;
  fusionAlphaPercent.value = snapshot.fusionAlphaPercent;
  fusionColormap.value = snapshot.fusionColormap;
  registrationRmse.value = snapshot.registrationRmse;
  registrationStatus.value = snapshot.registrationStatus;
  doseOpacityPercent.value = snapshot.doseOpacityPercent;
  isoDosePercent.value = snapshot.isoDosePercent;
  rtssVisible.value = snapshot.rtssVisible;
  rtValidationMessage.value = snapshot.rtValidationMessage;
  capabilityMessage.value = snapshot.capabilityMessage;
  lastFocusTarget.value = snapshot.lastFocusTarget;
  lastExportSummary.value = snapshot.lastExportSummary;
}

function commitMutation(mutation: () => void) {
  undoStack.value.push(snapshotState());
  if (undoStack.value.length > 64) {
    undoStack.value.shift();
  }
  redoStack.value = [];
  mutation();
}

function undo() {
  const snapshot = undoStack.value.pop();
  if (!snapshot) {
    return;
  }
  redoStack.value.push(snapshotState());
  restoreSnapshot(snapshot);
}

function redo() {
  const snapshot = redoStack.value.pop();
  if (!snapshot) {
    return;
  }
  undoStack.value.push(snapshotState());
  restoreSnapshot(snapshot);
}

function createMeasurement(payload: { label: string; kind: MeasurementKind }) {
  commitMutation(() => {
    const id = `M-${measurementSequence.value.toString().padStart(3, "0")}`;
    measurementSequence.value += 1;
    measurements.value.push({
      id,
      label: payload.label,
      kind: payload.kind,
      sliceIndex: 20 + measurements.value.length,
      confidence: 95,
    });
  });
}

function updateMeasurement(payload: { id: string; label: string }) {
  commitMutation(() => {
    const row = measurements.value.find((measurement) => measurement.id === payload.id);
    if (row) {
      row.label = payload.label;
    }
  });
}

function removeMeasurement(payload: { id: string }) {
  commitMutation(() => {
    measurements.value = measurements.value.filter((measurement) => measurement.id !== payload.id);
    if (lastFocusTarget.value === payload.id) {
      lastFocusTarget.value = "none";
    }
  });
}

function jumpToMeasurement(payload: { id: string }) {
  commitMutation(() => {
    const measurement = measurements.value.find((row) => row.id === payload.id);
    if (!measurement) {
      return;
    }
    lastFocusTarget.value = measurement.id;
    viewportPlane.value = measurement.sliceIndex % 2 === 0 ? "axial" : "sagittal";
    linkedViewports.value = true;
  });
}

function measurementCsvPayload(rows: MeasurementRecord[]): string {
  const header = "id,label,kind,slice_index,confidence";
  const body = rows
    .map((row) => `${row.id},${row.label},${row.kind},${row.sliceIndex},${row.confidence}`)
    .join("\n");
  return `${header}\n${body}`;
}

function exportMeasurements(payload: { format: "csv" | "json" }) {
  const ordered = orderedMeasurements.value;
  if (payload.format === "csv") {
    const csv = measurementCsvPayload(ordered);
    lastExportSummary.value = `Exported CSV (${csv.length} bytes)`;
    return;
  }
  const json = JSON.stringify(ordered, null, 2);
  lastExportSummary.value = `Exported JSON (${json.length} bytes)`;
}

function createSegmentation(payload: { name: string; color: string }) {
  commitMutation(() => {
    const id = `SEG-${segmentationSequence.value.toString().padStart(3, "0")}`;
    segmentationSequence.value += 1;
    segmentations.value.push({
      id,
      name: payload.name,
      source: "manual",
      color: payload.color,
      locked: false,
    });
  });
}

function importSegmentation(payload: { source: string }) {
  commitMutation(() => {
    const id = `SEG-${segmentationSequence.value.toString().padStart(3, "0")}`;
    segmentationSequence.value += 1;
    segmentations.value.push({
      id,
      name: "Imported segmentation",
      source: payload.source,
      color: "cyan",
      locked: true,
    });
  });
}

function styleSegmentation(payload: { id: string; color: string }) {
  commitMutation(() => {
    const row = segmentations.value.find((segmentation) => segmentation.id === payload.id);
    if (row) {
      row.color = payload.color;
    }
  });
}

function toggleLock(payload: { id: string }) {
  commitMutation(() => {
    const row = segmentations.value.find((segmentation) => segmentation.id === payload.id);
    if (row) {
      row.locked = !row.locked;
    }
  });
}

function removeSegmentation(payload: { id: string }) {
  commitMutation(() => {
    segmentations.value = segmentations.value.filter((segmentation) => segmentation.id !== payload.id);
  });
}

function updatePlane(payload: { plane: "axial" | "sagittal" | "coronal" }) {
  commitMutation(() => {
    viewportPlane.value = payload.plane;
  });
}

function toggleLinked() {
  commitMutation(() => {
    linkedViewports.value = !linkedViewports.value;
  });
}

function toggleMip() {
  commitMutation(() => {
    if (!supportsMip.value) {
      capabilityMessage.value = "MIP is disabled for this profile; upgrade capability settings to enable it.";
      return;
    }
    mipEnabled.value = !mipEnabled.value;
    capabilityMessage.value = "";
  });
}

function toggleVolume3d() {
  commitMutation(() => {
    if (!supportsVolume3d.value) {
      capabilityMessage.value = "3D volume rendering is disabled for this profile.";
      return;
    }
    volume3dEnabled.value = !volume3dEnabled.value;
    capabilityMessage.value = "";
  });
}

function updateFusionAlpha(payload: { alphaPercent: number }) {
  commitMutation(() => {
    if (payload.alphaPercent < 0 || payload.alphaPercent > 100 || Number.isNaN(payload.alphaPercent)) {
      return;
    }
    fusionAlphaPercent.value = Math.round(payload.alphaPercent);
  });
}

function updateFusionColormap(payload: { colormap: "hot" | "cool" | "gray" }) {
  commitMutation(() => {
    fusionColormap.value = payload.colormap;
  });
}

function runRegistrationDiagnostics() {
  commitMutation(() => {
    registrationRmse.value = Number((registrationRmse.value + 0.05).toFixed(2));
    registrationStatus.value = registrationRmse.value <= 2.0 ? "aligned" : "review_required";
  });
}

function updateDoseOpacity(payload: { value: number }) {
  commitMutation(() => {
    if (payload.value < 0 || payload.value > 100 || Number.isNaN(payload.value)) {
      rtValidationMessage.value = "Dose opacity must be within 0..100.";
      return;
    }
    doseOpacityPercent.value = Math.round(payload.value);
    rtValidationMessage.value = "";
  });
}

function updateIsoDose(payload: { value: number }) {
  commitMutation(() => {
    if (payload.value < 1 || payload.value > 100 || Number.isNaN(payload.value)) {
      rtValidationMessage.value = "Iso-dose threshold must be within 1..100.";
      return;
    }
    isoDosePercent.value = Math.round(payload.value);
    rtValidationMessage.value = "";
  });
}

function toggleRtss() {
  commitMutation(() => {
    rtssVisible.value = !rtssVisible.value;
  });
}
</script>

<template>
  <main :class="classTokens.clinicalPageShell">
    <section :class="classTokens.clinicalPageHero">
      <h1 :class="classTokens.pageTitle">Clinical Workflow Shell</h1>
      <p :class="classTokens.pageSubtitle">
        Governed workflow state for measurement, segmentation, MPR, fusion, and RT interactions.
      </p>
    </section>

    <section :class="classTokens.clinicalPageBody">
      <ClinicalMeasurementPanel
        :measurements="orderedMeasurements"
        :undo-depth="undoStack.length"
        :redo-depth="redoStack.length"
        :last-focus-target="lastFocusTarget"
        :last-export-summary="lastExportSummary"
        @create-measurement="createMeasurement"
        @update-measurement="updateMeasurement"
        @remove-measurement="removeMeasurement"
        @jump-to-measurement="jumpToMeasurement"
        @export-measurements="exportMeasurements"
        @undo="undo"
        @redo="redo"
      />

      <ClinicalSegmentationPanel
        :segmentations="orderedSegmentations"
        @create-segmentation="createSegmentation"
        @import-segmentation="importSegmentation"
        @style-segmentation="styleSegmentation"
        @toggle-lock="toggleLock"
        @remove-segmentation="removeSegmentation"
      />

      <ClinicalMprPanel
        :viewport-plane="viewportPlane"
        :linked-viewports="linkedViewports"
        :mip-enabled="mipEnabled"
        :volume3d-enabled="volume3dEnabled"
        :supports-mip="supportsMip"
        :supports-volume3d="supportsVolume3d"
        :capability-message="capabilityMessage"
        @update-plane="updatePlane"
        @toggle-linked="toggleLinked"
        @toggle-mip="toggleMip"
        @toggle-volume3d="toggleVolume3d"
      />

      <ClinicalFusionPanel
        :alpha-percent="fusionAlphaPercent"
        :colormap="fusionColormap"
        :registration-rmse="registrationRmse"
        :registration-status="registrationStatus"
        @update-alpha="updateFusionAlpha"
        @update-colormap="updateFusionColormap"
        @run-registration-diagnostics="runRegistrationDiagnostics"
      />

      <ClinicalRtPanel
        :dose-opacity-percent="doseOpacityPercent"
        :iso-dose-percent="isoDosePercent"
        :rtss-visible="rtssVisible"
        :validation-message="rtValidationMessage"
        @update-dose-opacity="updateDoseOpacity"
        @update-iso-dose="updateIsoDose"
        @toggle-rtss="toggleRtss"
      />
    </section>
  </main>
</template>
