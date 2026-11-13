<template>
  <figure>
    <figcaption class="text-sm text-ink font-bold mb-1">
      {{ title }}
      <span class="font-normal text-muted">{{ pctText }}</span>
    </figcaption>
    <div class="grid gap-px bg-rule p-px rounded" style="grid-template-columns: repeat(13, minmax(0, 1fr))">
      <div
        v-for="(w, i) in weights"
        :key="i"
        class="aspect-square"
        :style="{ background: w > 0 ? `rgba(14,107,92,${0.25 + 0.75 * w})` : '#fff' }"
        :title="label(i) + (w > 0 ? ` — played ${Math.round(w * 100)}% of the time` : ' — not played')"
      ></div>
    </div>
  </figure>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import * as api from "../../guided/api";

const props = defineProps<{ title: string; range: string }>();
const weights = ref<number[]>(Array(169).fill(0));
const R = "AKQJT98765432";
const label = (i: number) => {
  const r = Math.floor(i / 13), c = i % 13;
  return r === c ? R[r] + R[c] : r < c ? R[r] + R[c] + "s" : R[c] + R[r] + "o";
};
watch(
  () => props.range,
  async (r) => {
    try { weights.value = await api.studyRangeGrid(r); } catch { weights.value = Array(169).fill(0); }
  },
  { immediate: true }
);
const pctText = computed(() => {
  let combos = 0;
  weights.value.forEach((w, i) => {
    const r = Math.floor(i / 13), c = i % 13;
    combos += w * (r === c ? 6 : r < c ? 4 : 12);
  });
  return `${((combos / 1326) * 100).toFixed(1)}% of hands`;
});
</script>
