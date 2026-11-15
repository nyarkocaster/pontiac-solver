<template>
  <div class="seg" role="group" aria-label="What the chart shows">
    <button v-for="m in MODES" :key="m.id" :class="modelValue === m.id ? 'seg-on' : ''" :title="m.help" @click="$emit('update:modelValue', m.id)">{{ m.label }}</button>
  </div>
</template>
<script setup lang="ts">
defineProps<{ modelValue: "strategy" | "ev" | "equity" }>();
defineEmits<{ (e: "update:modelValue", v: "strategy" | "ev" | "equity"): void }>();
const MODES = [
  { id: "strategy", label: "Actions", help: "How each hand plays" },
  { id: "ev", label: "Value", help: "What each hand is worth on average from here (EV)" },
  { id: "equity", label: "Chance to win", help: "How often each hand wins at showdown (equity)" },
] as const;
</script>
<style scoped>
.seg { @apply inline-flex rounded-lg border border-rule overflow-hidden text-xs font-bold; }
.seg button { @apply px-2.5 py-1 bg-surface; }
.seg .seg-on { @apply bg-felt text-white; }
</style>
