<template>
  <div>
    <p v-if="!made.length" class="text-sm text-muted">No hands reach this point.</p>
    <template v-else>
      <div class="rowhead"><span>Type of hand</span><span class="text-right">Share</span><span>What it does</span></div>
      <div v-for="r in made" :key="r.name" class="row" :class="rowClass(r.name)" :title="tip(r)" role="button" tabindex="0" :aria-pressed="picked === r.name" @click="pick(r.name)" @keydown.enter="pick(r.name)">
        <span class="font-bold truncate">{{ r.name }}</span>
        <span class="text-right tabular-nums">{{ share(r.share) }}</span>
        <span>
          <span class="flex h-3.5 rounded overflow-hidden bg-paper" role="img" :aria-label="tip(r)">
            <span v-for="(f, a) in r.freqs" :key="a" :style="{ width: (f * 100).toFixed(1) + '%', background: colors[a], marginRight: f > 0.005 && a < r.freqs.length - 1 ? '2px' : '0' }"></span>
          </span>
          <span class="block text-[11px] text-muted leading-tight mt-0.5">{{ lead(r) }} · wins {{ Math.round(r.equity * 100) }}%</span>
        </span>
      </div>
      <template v-if="draws.length">
        <div class="rowhead mt-3"><span>Draws</span><span class="text-right">Share</span><span>Counted in the rows above as well</span></div>
        <div v-for="r in draws" :key="r.name" class="row" :class="rowClass(r.name)" :title="tip(r)" role="button" tabindex="0" :aria-pressed="picked === r.name" @click="pick(r.name)" @keydown.enter="pick(r.name)">
          <span class="font-bold truncate">{{ r.name }}</span>
          <span class="text-right tabular-nums">{{ share(r.share) }}</span>
          <span>
            <span class="flex h-3.5 rounded overflow-hidden bg-paper" role="img" :aria-label="tip(r)">
              <span v-for="(f, a) in r.freqs" :key="a" :style="{ width: (f * 100).toFixed(1) + '%', background: colors[a], marginRight: f > 0.005 && a < r.freqs.length - 1 ? '2px' : '0' }"></span>
            </span>
            <span class="block text-[11px] text-muted leading-tight mt-0.5">{{ lead(r) }} · wins {{ Math.round(r.equity * 100) }}%</span>
          </span>
        </div>
      </template>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { classify, MADE, DRAWS } from "../../guided/handClass";

interface Combo { hand: string; weight: number; freqs: number[]; equity: number }
const props = defineProps<{ combos: Combo[]; board: string[]; actions: string[]; colors: string[]; /** the type being highlighted in the chart, if any */ picked?: string | null }>();
const emit = defineEmits<{ (e: "pick", name: string | null): void }>();
/** clicking a row highlights that type in the chart; clicking it again shows everything */
const pick = (name: string) => emit("pick", props.picked === name ? null : name);
const rowClass = (name: string) => (props.picked === undefined ? "" : "cursor-pointer hover:bg-paper " + (props.picked === name ? "bg-paper ring-1 ring-felt rounded" : ""));
interface Row { name: string; share: number; freqs: number[]; equity: number }

const groups = computed(() => {
  const acc = new Map<string, { w: number; f: number[]; e: number }>();
  let total = 0;
  const add = (name: string, c: Combo) => {
    const g = acc.get(name) ?? { w: 0, f: props.actions.map(() => 0), e: 0 };
    g.w += c.weight; g.e += c.weight * c.equity; c.freqs.forEach((f, a) => (g.f[a] += c.weight * f)); acc.set(name, g);
  };
  for (const c of props.combos) {
    if (c.weight <= 0 || c.hand.length !== 4) continue;
    total += c.weight;
    const k = classify(c.hand, props.board);
    add(k.made, c); k.draws.forEach((d) => add(d, c));
  }
  const row = (name: string): Row | null => { const g = acc.get(name); return g && total > 0 && g.w / total >= 0.0005 ? { name, share: g.w / total, freqs: g.f.map((v) => v / g.w), equity: g.e / g.w } : null; };
  return { made: MADE.map(row).filter((r): r is Row => !!r), draws: DRAWS.map(row).filter((r): r is Row => !!r) };
});
const made = computed(() => groups.value.made);
const draws = computed(() => groups.value.draws);
const share = (v: number) => (v < 0.01 ? "<1%" : Math.round(v * 100) + "%");
const tip = (r: Row) => `${r.name}: ` + r.freqs.map((f, a) => `${props.actions[a]} ${Math.round(f * 100)}%`).filter((_s, a) => r.freqs[a] >= 0.005).join(", ");
const lead = (r: Row) => { const a = r.freqs.indexOf(Math.max(...r.freqs)); return r.freqs[a] > 0.9 ? `Almost always ${props.actions[a].toLowerCase()}` : `Mostly ${props.actions[a].toLowerCase()} (${Math.round(r.freqs[a] * 100)}%)`; };
</script>

<style scoped>
.rowhead { @apply grid gap-x-3 text-[11px] text-muted pb-1; grid-template-columns: 9.5rem 2.75rem 1fr; }
.row { @apply grid gap-x-3 items-start text-sm py-1.5 border-t border-rule; grid-template-columns: 9.5rem 2.75rem 1fr; }
</style>
