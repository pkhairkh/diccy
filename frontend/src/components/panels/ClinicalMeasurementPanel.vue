<script setup lang="ts">
import { computed, ref } from "vue";
import { classTokens } from "../../styles/tokens";

type MeasurementKind = "length" | "angle" | "area";

type MeasurementRecord = {
  id: string;
  label: string;
  kind: MeasurementKind;
  sliceIndex: number;
  confidence: number;
};

const props = defineProps<{
  measurements: MeasurementRecord[];
  undoDepth: number;
  redoDepth: number;
  lastFocusTarget: string;
  lastExportSummary: string;
}>();

const emit = defineEmits<{
  createMeasurement: [payload: { label: string; kind: MeasurementKind }];
  updateMeasurement: [payload: { id: string; label: string }];
  removeMeasurement: [payload: { id: string }];
  jumpToMeasurement: [payload: { id: string }];
  exportMeasurements: [payload: { format: "csv" | "json" }];
  undo: [];
  redo: [];
}>();

const draftLabel = ref("Length");
const draftKind = ref<MeasurementKind>("length");
const canUndo = computed(() => props.undoDepth > 0);
const canRedo = computed(() => props.redoDepth > 0);

function createMeasurement() {
  const normalizedLabel = draftLabel.value.trim();
  emit("createMeasurement", {
    label: normalizedLabel.length > 0 ? normalizedLabel : "New measurement",
    kind: draftKind.value,
  });
}

function renameMeasurement(id: string, currentLabel: string) {
  emit("updateMeasurement", { id, label: `${currentLabel} (edited)` });
}
</script>

<template>
  <section :class="classTokens.panelShell">
    <div :class="classTokens.panelBody">
      <header :class="classTokens.panelHeader">
        <h2>Measurements</h2>
        <p :class="classTokens.panelSubtleText">Deterministic ordering by measurement identifier.</p>
      </header>

      <div :class="classTokens.clinicalPanelSection">
        <div :class="classTokens.formGrid">
          <label :class="classTokens.fieldStack">
            <span :class="classTokens.fieldLabel">Label</span>
            <input v-model="draftLabel" :class="classTokens.fieldInput" type="text" />
          </label>
          <label :class="classTokens.fieldStack">
            <span :class="classTokens.fieldLabel">Type</span>
            <select v-model="draftKind" :class="classTokens.fieldInput">
              <option value="length">Length</option>
              <option value="angle">Angle</option>
              <option value="area">Area</option>
            </select>
          </label>
        </div>

        <div :class="classTokens.clinicalPanelToolbar">
          <button :class="classTokens.actionButton" type="button" @click="createMeasurement">Create</button>
          <button :class="classTokens.actionButtonSecondary" type="button" :disabled="!canUndo" @click="emit('undo')">Undo</button>
          <button :class="classTokens.actionButtonSecondary" type="button" :disabled="!canRedo" @click="emit('redo')">Redo</button>
          <button :class="classTokens.actionButtonSecondary" type="button" @click="emit('exportMeasurements', { format: 'csv' })">Export CSV</button>
          <button :class="classTokens.actionButtonSecondary" type="button" @click="emit('exportMeasurements', { format: 'json' })">Export JSON</button>
        </div>
      </div>

      <div :class="classTokens.clinicalPanelList">
        <article v-for="measurement in measurements" :key="measurement.id" :class="classTokens.clinicalPanelListItem">
          <p>
            <strong>{{ measurement.label }}</strong>
            <span> · {{ measurement.kind }} · slice {{ measurement.sliceIndex }} · confidence {{ measurement.confidence }}%</span>
          </p>
          <div :class="classTokens.clinicalPanelToolbar">
            <button
              :class="classTokens.actionButtonSecondary"
              type="button"
              @click="renameMeasurement(measurement.id, measurement.label)"
            >
              Rename
            </button>
            <button
              :class="classTokens.actionButtonSecondary"
              type="button"
              @click="emit('jumpToMeasurement', { id: measurement.id })"
            >
              Jump to viewport
            </button>
            <button
              :class="classTokens.actionButtonDanger"
              type="button"
              @click="emit('removeMeasurement', { id: measurement.id })"
            >
              Remove
            </button>
          </div>
        </article>
      </div>

      <p :class="classTokens.statusOk">Viewport focus: {{ lastFocusTarget }}</p>
      <p :class="classTokens.panelSubtleText">{{ lastExportSummary }}</p>
    </div>
  </section>
</template>
