<script setup lang="ts">
import { classTokens } from "../../styles/tokens";

const props = defineProps<{
  alphaPercent: number;
  colormap: "hot" | "cool" | "gray";
  registrationRmse: number;
  registrationStatus: string;
}>();

const emit = defineEmits<{
  updateAlpha: [payload: { alphaPercent: number }];
  updateColormap: [payload: { colormap: "hot" | "cool" | "gray" }];
  runRegistrationDiagnostics: [];
}>();
</script>

<template>
  <section :class="classTokens.panelShell">
    <div :class="classTokens.panelBody">
      <header :class="classTokens.panelHeader">
        <h2>Fusion</h2>
        <p :class="classTokens.panelSubtleText">Alpha blend, colormap, and registration diagnostics.</p>
      </header>

      <div :class="classTokens.formGrid">
        <label :class="classTokens.fieldStack">
          <span :class="classTokens.fieldLabel">Alpha (%)</span>
          <input
            :class="classTokens.fieldInput"
            type="number"
            min="0"
            max="100"
            :value="alphaPercent"
            @change="emit('updateAlpha', { alphaPercent: Number(($event.target as HTMLInputElement).value) })"
          />
        </label>

        <label :class="classTokens.fieldStack">
          <span :class="classTokens.fieldLabel">Colormap</span>
          <select
            :class="classTokens.fieldInput"
            :value="colormap"
            @change="emit('updateColormap', { colormap: ($event.target as HTMLSelectElement).value as 'hot' | 'cool' | 'gray' })"
          >
            <option value="hot">Hot</option>
            <option value="cool">Cool</option>
            <option value="gray">Gray</option>
          </select>
        </label>
      </div>

      <div :class="classTokens.clinicalPanelToolbar">
        <button :class="classTokens.actionButtonSecondary" type="button" @click="emit('runRegistrationDiagnostics')">
          Re-run diagnostics
        </button>
      </div>

      <p :class="classTokens.statusOk">Registration status: {{ registrationStatus }}</p>
      <p :class="classTokens.panelSubtleText">RMSE: {{ registrationRmse.toFixed(2) }} mm</p>
    </div>
  </section>
</template>
