<script setup lang="ts">
import { ref } from "vue";
import { classTokens } from "../../styles/tokens";

type SegmentationRecord = {
  id: string;
  name: string;
  source: string;
  color: string;
  locked: boolean;
};

const props = defineProps<{
  segmentations: SegmentationRecord[];
}>();

const emit = defineEmits<{
  createSegmentation: [payload: { name: string; color: string }];
  importSegmentation: [payload: { source: string }];
  styleSegmentation: [payload: { id: string; color: string }];
  toggleLock: [payload: { id: string }];
  removeSegmentation: [payload: { id: string }];
}>();

const segmentationName = ref("Liver region");
const segmentationColor = ref("amber");
const importSource = ref("/imports/segmentation.nii.gz");

function createSegmentation() {
  const name = segmentationName.value.trim();
  emit("createSegmentation", {
    name: name.length > 0 ? name : "New segmentation",
    color: segmentationColor.value,
  });
}

function importSegmentation() {
  const source = importSource.value.trim();
  emit("importSegmentation", {
    source: source.length > 0 ? source : "inline-buffer",
  });
}

function nextColor(color: string) {
  if (color === "amber") {
    return "cyan";
  }
  if (color === "cyan") {
    return "rose";
  }
  return "amber";
}
</script>

<template>
  <section :class="classTokens.panelShell">
    <div :class="classTokens.panelBody">
      <header :class="classTokens.panelHeader">
        <h2>Segmentation</h2>
        <p :class="classTokens.panelSubtleText">Create, import, style, lock, and remove overlays.</p>
      </header>

      <div :class="classTokens.clinicalPanelSection">
        <div :class="classTokens.formGrid">
          <label :class="classTokens.fieldStack">
            <span :class="classTokens.fieldLabel">Name</span>
            <input v-model="segmentationName" :class="classTokens.fieldInput" type="text" />
          </label>
          <label :class="classTokens.fieldStack">
            <span :class="classTokens.fieldLabel">Style color</span>
            <select v-model="segmentationColor" :class="classTokens.fieldInput">
              <option value="amber">Amber</option>
              <option value="cyan">Cyan</option>
              <option value="rose">Rose</option>
            </select>
          </label>
        </div>

        <div :class="classTokens.clinicalPanelToolbar">
          <button :class="classTokens.actionButton" type="button" @click="createSegmentation">Create</button>
          <button :class="classTokens.actionButtonSecondary" type="button" @click="importSegmentation">Import</button>
        </div>
      </div>

      <label :class="classTokens.fieldStack">
        <span :class="classTokens.fieldLabel">Import source</span>
        <input v-model="importSource" :class="classTokens.fieldInput" type="text" />
      </label>

      <div :class="classTokens.clinicalPanelList">
        <article v-for="seg in segmentations" :key="seg.id" :class="classTokens.clinicalPanelListItem">
          <p>
            <strong>{{ seg.name }}</strong>
            <span> · {{ seg.source }} · color {{ seg.color }} · locked {{ seg.locked ? "yes" : "no" }}</span>
          </p>
          <div :class="classTokens.clinicalPanelToolbar">
            <button
              :class="classTokens.actionButtonSecondary"
              type="button"
              @click="emit('styleSegmentation', { id: seg.id, color: nextColor(seg.color) })"
            >
              Style
            </button>
            <button :class="classTokens.actionButtonSecondary" type="button" @click="emit('toggleLock', { id: seg.id })">
              {{ seg.locked ? "Unlock" : "Lock" }}
            </button>
            <button :class="classTokens.actionButtonDanger" type="button" @click="emit('removeSegmentation', { id: seg.id })">
              Remove
            </button>
          </div>
        </article>
      </div>
    </div>
  </section>
</template>
