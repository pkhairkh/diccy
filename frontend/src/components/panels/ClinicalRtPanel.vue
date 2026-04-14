<script setup lang="ts">
import { classTokens } from "../../styles/tokens";

const props = defineProps<{
  doseOpacityPercent: number;
  isoDosePercent: number;
  rtssVisible: boolean;
  validationMessage: string;
}>();

const emit = defineEmits<{
  updateDoseOpacity: [payload: { value: number }];
  updateIsoDose: [payload: { value: number }];
  toggleRtss: [];
}>();
</script>

<template>
  <section :class="classTokens.panelShell">
    <div :class="classTokens.panelBody">
      <header :class="classTokens.panelHeader">
        <h2>RT Dose and RT Structure Set</h2>
        <p :class="classTokens.panelSubtleText">Bounded numeric controls with fail-closed validation.</p>
      </header>

      <div :class="classTokens.formGrid">
        <label :class="classTokens.fieldStack">
          <span :class="classTokens.fieldLabel">Dose opacity (%)</span>
          <input
            :class="classTokens.fieldInput"
            type="number"
            min="0"
            max="100"
            :value="doseOpacityPercent"
            @change="emit('updateDoseOpacity', { value: Number(($event.target as HTMLInputElement).value) })"
          />
        </label>

        <label :class="classTokens.fieldStack">
          <span :class="classTokens.fieldLabel">Iso-dose threshold (%)</span>
          <input
            :class="classTokens.fieldInput"
            type="number"
            min="1"
            max="100"
            :value="isoDosePercent"
            @change="emit('updateIsoDose', { value: Number(($event.target as HTMLInputElement).value) })"
          />
        </label>
      </div>

      <div :class="classTokens.clinicalPanelToolbar">
        <button :class="classTokens.actionButtonSecondary" type="button" @click="emit('toggleRtss')">
          {{ rtssVisible ? "Hide" : "Show" }} RTSS
        </button>
      </div>

      <p :class="classTokens.panelSubtleText">RTSS visible: {{ rtssVisible ? "yes" : "no" }}</p>
      <p v-if="validationMessage" :class="classTokens.statusError">{{ validationMessage }}</p>
      <p v-else :class="classTokens.statusOk">RT numeric controls are within allowed bounds.</p>
    </div>
  </section>
</template>
