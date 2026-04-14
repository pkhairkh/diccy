<script setup lang="ts">
import ConnectorStatusBadge from "../components/ConnectorStatusBadge.vue";
import { classTokens } from "../../styles/tokens";

defineProps<{
  connector: string;
  status: "healthy" | "degraded" | "offline";
  latencyMs: number;
}>();
</script>

<template>
  <article :class="classTokens.compositeCard">
    <header>
      <h3>{{ connector }}</h3>
      <ConnectorStatusBadge :status="status" />
    </header>
    <p>Latency: {{ latencyMs }} ms</p>
  </article>
</template>

