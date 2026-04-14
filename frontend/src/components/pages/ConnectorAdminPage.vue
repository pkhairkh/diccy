<script setup lang="ts">
import { computed } from "vue";
import ConnectorOpsPanel from "../panels/ConnectorOpsPanel.vue";
import { classTokens } from "../../styles/tokens";
// Tailwind Plus UI Blocks parity: page shell + header + panel composition.

const props = defineProps<{
  role: "admin" | "operator" | "viewer";
}>();

const canViewAdminControls = computed(() => props.role === "admin" || props.role === "operator");
</script>

<template>
  <main :class="classTokens.pageShell">
    <section :class="classTokens.pageHeroSection">
      <h1 :class="classTokens.pageTitle">Connector Configuration</h1>
      <p :class="classTokens.pageSubtitle">Connector rollout controls and integration policy state.</p>
    </section>
    <section :class="classTokens.pageBodySection">
      <ConnectorOpsPanel v-if="canViewAdminControls" :role="role" />
      <p v-else :class="classTokens.pageNoticeText">Role does not permit connector configuration access.</p>
    </section>
  </main>
</template>
