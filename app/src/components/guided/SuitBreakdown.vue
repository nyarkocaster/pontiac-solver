<template>
  <div>
    <p v-if="!rows.length" class="text-sm text-muted">No hands from this square get here.</p>
    <template v-else>
      <p class="text-xs text-muted">
        {{ combos.length }} exact {{ combos.length === 1 ? "hand" : "hands" }} still possible.
        <template v-if="rows.length === 1 && combos.length > 1">Every suit plays the same way here.</template>
        <template v-else-if="rows.length < combos.length">Suits that play the same way share a row.</template>
      </p>
      <div v-for="(r, k) in rows" :key="k" class="mt-2 pt-2 border-t border-rule first:border-t-0">
        <div class="flex flex-wrap gap-1">
          <span v-for="h in r.hands" :key="h" class="chip">
            <span :class="SUIT_COLOR[h[1]]">{{ h[0] }}{{ SUIT_SYMBOL[h[1]] }}</span><span :class="SUIT_COLOR[h[3]]">{{ h[2] }}{{ SUIT_SYMBOL[h[3]] }}</span>
          </span>
        </div>
        <div class="flex h-3.5 rounded overflow-hidden bg-paper mt-1.5" role="img" :aria-label="mix(r.freqs)">
          <div v-for="(f, a) in r.freqs" :key="a" :style="{ width: (f * 100).toFixed(1) + '%', background: colors[a] }"></div>
        </div>
        <div class="flex flex-wrap justify-between gap-x-3 text-xs mt-1">
          <span class="font-bold">{{ mix(r.freqs) }}</span>
          <span class="text-muted tabular-nums">
            <span title="How often this exact hand is still in the range here">here {{ Math.round(Math.min(1, r.weight) * 100) }}%</span>
            · wins {{ Math.round(r.equity * 100) }}%<template v-if="showEv"> · {{ fmtEv(r.ev) }}</template>
          </span>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { SUIT_COLOR, SUIT_SYMBOL } from "../../guided/data";

export interface ComboRow { hand: string; weight: number; freqs: number[]; equity: number; ev: number }
const props = withDefaults(defineProps<{ combos: ComboRow[]; actions: string[]; colors: string[]; fmtEv: (x: number) => string; showEv?: boolean }>(), { showEv: true });

// exact hands whose strategy is the same to within two points share a row
const rows = computed(() => {
  const groups = new Map<string, { hands: string[]; freqs: number[]; weight: number; equity: number; ev: number; n: number }>();
  for (const c of [...props.combos].sort((a, b) => b.ev - a.ev)) {
    const key = c.freqs.map((f) => Math.round(f * 50)).join(",");
    const g = groups.get(key);
    if (g) { g.hands.push(c.hand); g.n++; g.weight += c.weight; g.equity += c.equity; g.ev += c.ev; g.freqs = g.freqs.map((f, i) => f + c.freqs[i]); }
    else groups.set(key, { hands: [c.hand], freqs: [...c.freqs], weight: c.weight, equity: c.equity, ev: c.ev, n: 1 });
  }
  return [...groups.values()].map((g) => ({ hands: g.hands, freqs: g.freqs.map((f) => f / g.n), weight: g.weight / g.n, equity: g.equity / g.n, ev: g.ev / g.n }));
});
const mix = (f: number[]) => {
  const parts = f.map((x, i) => ({ x, name: props.actions[i] })).filter((p) => p.x >= 0.03).sort((a, b) => b.x - a.x);
  return parts.length === 1 ? `Always: ${parts[0].name}` : parts.map((p) => `${p.name} ${Math.round(p.x * 100)}%`).join(", ");
};
</script>

<style scoped>
.chip { @apply inline-flex px-1.5 py-0.5 rounded bg-paper border border-rule font-display font-bold text-xs; }
</style>
