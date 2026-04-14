<script setup lang="ts">
import { classTokens } from "../../styles/tokens";

defineProps<{
  label: string;
  value: string;
}>();
</script>

<template>
  <article :class="classTokens.statCard">
    <h4 :class="classTokens.fieldLabel">{{ label }}</h4>
    <p>{{ value }}</p>
  </article>
</template>

