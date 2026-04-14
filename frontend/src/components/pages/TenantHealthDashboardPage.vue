<script setup lang="ts">
import { computed } from "vue";
import ConnectorOpsPanel from "../panels/ConnectorOpsPanel.vue";
import { classTokens } from "../../styles/tokens";
// Tailwind Plus UI Blocks parity: page shell + status header + panel composition.

const props = defineProps<{
  tenantId: string;
  role: "admin" | "operator" | "viewer";
}>();

const canViewOperationalPanel = computed(
  () => props.role === "admin" || props.role === "operator" || props.role === "viewer",
);
const canMutateOperationalPanel = computed(() => props.role === "admin" || props.role === "operator");
</script>

<template>
  <main :class="classTokens.pageShell">
    <section :class="classTokens.pageHeroSection">
      <h1 :class="classTokens.pageTitle">Tenant Integration Health</h1>
      <p :class="classTokens.pageSubtitle">Tenant: {{ props.tenantId }}</p>
      <p v-if="!canMutateOperationalPanel" :class="classTokens.pageNoticeText">
        Read-only mode active for this role.
      </p>
    </section>
    <section :class="classTokens.pageBodySection">
      <ConnectorOpsPanel
        v-if="canViewOperationalPanel"
        :tenant-id="props.tenantId"
        :role="props.role"
      />
    </section>
  </main>
</template>
