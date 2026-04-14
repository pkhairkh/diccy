<script setup lang="ts">
import { computed } from "vue";
import { classTokens } from "../../styles/tokens";

const props = defineProps<{
  status: "healthy" | "degraded" | "offline";
}>();

const statusLabel = computed(() => {
  if (props.status === "healthy") return "Healthy";
  if (props.status === "degraded") return "Degraded";
  return "Offline";
});

const statusState = computed(() => `status-${props.status}`);
</script>

<template>
  <span :class="classTokens.statusBadgeBase" :data-status="statusState">
    {{ statusLabel }}
  </span>
</template>

