<script setup lang="ts">
import { classTokens } from "../../styles/tokens";

const props = defineProps<{
  viewportPlane: "axial" | "sagittal" | "coronal";
  linkedViewports: boolean;
  mipEnabled: boolean;
  volume3dEnabled: boolean;
  supportsMip: boolean;
  supportsVolume3d: boolean;
  capabilityMessage: string;
}>();

const emit = defineEmits<{
  updatePlane: [payload: { plane: "axial" | "sagittal" | "coronal" }];
  toggleLinked: [];
  toggleMip: [];
  toggleVolume3d: [];
}>();
</script>

<template>
  <section :class="classTokens.panelShell">
    <div :class="classTokens.panelBody">
      <header :class="classTokens.panelHeader">
        <h2>MPR and Viewport Linkage</h2>
        <p :class="classTokens.panelSubtleText">Cross-plane controls for synchronized viewport navigation.</p>
      </header>

      <div :class="classTokens.clinicalPanelToolbar">
        <button :class="classTokens.actionButtonSecondary" type="button" @click="emit('updatePlane', { plane: 'axial' })">
          Axial
        </button>
        <button
          :class="classTokens.actionButtonSecondary"
          type="button"
          @click="emit('updatePlane', { plane: 'sagittal' })"
        >
          Sagittal
        </button>
        <button
          :class="classTokens.actionButtonSecondary"
          type="button"
          @click="emit('updatePlane', { plane: 'coronal' })"
        >
          Coronal
        </button>
        <button :class="classTokens.actionButtonSecondary" type="button" @click="emit('toggleLinked')">
          {{ linkedViewports ? "Unlink" : "Link" }} viewports
        </button>
      </div>

      <p :class="classTokens.statusOk">Active plane: {{ viewportPlane }}</p>
      <p :class="classTokens.panelSubtleText">Linked: {{ linkedViewports ? "on" : "off" }}</p>

      <div :class="classTokens.clinicalPanelToolbar">
        <button
          :class="classTokens.actionButtonSecondary"
          type="button"
          :disabled="!supportsMip"
          @click="emit('toggleMip')"
        >
          {{ mipEnabled ? "Disable" : "Enable" }} MIP
        </button>
        <button
          :class="classTokens.actionButtonSecondary"
          type="button"
          :disabled="!supportsVolume3d"
          @click="emit('toggleVolume3d')"
        >
          {{ volume3dEnabled ? "Disable" : "Enable" }} 3D
        </button>
      </div>

      <p v-if="!supportsMip || !supportsVolume3d" :class="classTokens.statusWarn">Capability-gated controls are disabled for this tenant profile.</p>
      <p v-if="capabilityMessage" :class="classTokens.alertBanner">{{ capabilityMessage }}</p>
    </div>
  </section>
</template>
