<template>
  <div class="flex items-center gap-2 text-xs mt-1.5">
    <button class="rs-btn" title="Remove the weakest hands" :disabled="pct <= 0.5" @click="nudge(-1)">Tighter</button>
    <input
      type="range" min="0" max="100" step="0.5" :value="pct" class="flex-1 accent-[#0E6B5C]"
      :aria-label="`Range width for ${label || 'this player'}, percent of all hands`"
      @input="slide(($event.target as HTMLInputElement).valueAsNumber)"
    />
    <button class="rs-btn" title="Add the next hands" :disabled="pct >= 99.5" @click="nudge(1)">Wider</button>
    <span class="tabular-nums w-[5.5rem] text-right" :title="`${Math.round(pct * 13.26)} of 1,326 hands`"><b>{{ pct.toFixed(1) }}%</b> of hands</span>
    <select v-model="kind" class="rs-kind" :title="kind === 'call' ? 'Adds hands a player would call with; hands that normally re-raise (QQ+, AK) come last' : 'Adds the strongest hands first'" aria-label="How the slider adds hands">
      <option value="open">raising range</option>
      <option value="call">calling range</option>
    </select>
    <button v-if="changed" class="text-felt underline font-bold" title="Back to the range as it was before the slider moved" @click="reset">Undo</button>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { formatRange, guessKind, parseRange, rangePercent, resize, type RangeKind } from "../../guided/rangeTools";

const props = defineProps<{ modelValue: string; label?: string }>();
const emit = defineEmits<{ (e: "update:modelValue", v: string): void }>();

// The range the slider works from: whatever was in the box when the slider was first moved.
// Typing in the box or loading a preset makes that the new starting point.
const base = ref(props.modelValue);
let mine = props.modelValue;
// A range with no AA or KK is taken to be a calling range; the menu overrides the guess.
const kind = ref<RangeKind>(guessKind(parseRange(props.modelValue)));
watch(() => props.modelValue, (v) => { if (v !== mine) { base.value = v; mine = v; kind.value = guessKind(parseRange(v)); } });

const pct = computed(() => rangePercent(parseRange(props.modelValue)));
const changed = computed(() => props.modelValue !== base.value);
function slide(target: number) {
  mine = formatRange(resize(parseRange(base.value), target, kind.value));
  emit("update:modelValue", mine);
}
function nudge(dir: number) { slide(Math.max(0, Math.min(100, Math.round(pct.value) + dir * 2))); }
function reset() { mine = base.value; emit("update:modelValue", mine); }
</script>

<style scoped>
.rs-kind { @apply rounded border-rule text-xs py-0.5 pl-1.5 pr-6 font-bold focus:border-felt focus:ring-felt; }
.rs-btn { @apply px-2 py-0.5 rounded border border-rule bg-white font-bold hover:border-felt disabled:opacity-40; }
</style>
