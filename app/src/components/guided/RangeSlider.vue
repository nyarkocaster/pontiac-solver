<template>
  <div class="flex items-center gap-2 text-xs mt-1.5">
    <button class="rs-btn" title="Remove the weakest hands" :disabled="pct <= 0.5" @click="nudge(-1)">Tighter</button>
    <input
      type="range" min="0" max="100" step="0.5" :value="pct" class="flex-1 accent-[#0E6B5C]"
      :aria-label="`Range width for ${label || 'this player'}, percent of all hands`"
      @input="slide(($event.target as HTMLInputElement).valueAsNumber)"
    />
    <button class="rs-btn" title="Add the next strongest hands" :disabled="pct >= 99.5" @click="nudge(1)">Wider</button>
    <span class="tabular-nums w-[5.5rem] text-right" :title="`${Math.round(pct * 13.26)} of 1,326 hands`"><b>{{ pct.toFixed(1) }}%</b> of hands</span>
    <button v-if="changed" class="text-felt underline font-bold" title="Back to the range as it was before the slider moved" @click="reset">Undo</button>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { formatRange, parseRange, rangePercent, resize } from "../../guided/rangeTools";

const props = defineProps<{ modelValue: string; label?: string }>();
const emit = defineEmits<{ (e: "update:modelValue", v: string): void }>();

// The range the slider works from: whatever was in the box when the slider was first moved.
// Typing in the box or loading a preset makes that the new starting point.
const base = ref(props.modelValue);
let mine = props.modelValue;
watch(() => props.modelValue, (v) => { if (v !== mine) { base.value = v; mine = v; } });

const pct = computed(() => rangePercent(parseRange(props.modelValue)));
const changed = computed(() => props.modelValue !== base.value);
function slide(target: number) {
  mine = formatRange(resize(parseRange(base.value), target));
  emit("update:modelValue", mine);
}
function nudge(dir: number) { slide(Math.max(0, Math.min(100, Math.round(pct.value) + dir * 2))); }
function reset() { mine = base.value; emit("update:modelValue", mine); }
</script>

<style scoped>
.rs-btn { @apply px-2 py-0.5 rounded border border-rule bg-white font-bold hover:border-felt disabled:opacity-40; }
</style>
