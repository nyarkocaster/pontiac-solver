<template>
  <!-- every exact hand the deciding player still holds, sortable -->
  <div>
    <div class="flex items-center gap-3 text-sm">
      <input v-model="filter" class="border border-rule rounded-lg px-2 py-1 w-40 bg-surface" placeholder="Filter: AK, 7h, s…" aria-label="Filter hands" />
      <span class="text-muted">{{ rows.length }} of {{ combos.length }} hands</span>
    </div>
    <div class="overflow-auto max-h-[26rem] mt-2 border border-rule rounded-lg">
      <table class="w-full text-sm tabular-nums">
        <thead class="sticky top-0 bg-surface z-10">
          <tr class="text-left text-xs text-muted border-b border-rule">
            <th v-for="c in columns" :key="c.key" class="py-1.5 px-2 font-normal whitespace-nowrap" :class="c.key === 'hand' ? '' : 'text-right'" :aria-sort="sortKey === c.key ? (sortDown ? 'descending' : 'ascending') : 'none'">
              <button class="hover:text-ink" :class="sortKey === c.key ? 'font-bold text-ink' : ''" :title="c.title" @click="sortBy(c.key)">
                <span v-if="c.color" class="inline-block w-2.5 h-2.5 rounded-sm mr-1 align-middle" :style="{ background: c.color }"></span>{{ c.label }}<span v-if="sortKey === c.key" aria-hidden="true"> {{ sortDown ? "▾" : "▴" }}</span>
              </button>
            </th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="r in shown" :key="r.hand" class="border-b border-rule last:border-0 cursor-pointer hover:bg-paper" :class="r.cell === selected ? 'bg-paper font-bold' : ''" @click="emit('select', r.cell)">
            <td class="py-1 px-2 whitespace-nowrap">
              <span v-for="(c, i) in [r.hand.slice(0, 2), r.hand.slice(2)]" :key="i" :style="{ color: SUIT_COLOR[c[1]] }">{{ c[0] === "T" ? "10" : c[0] }}{{ SUIT_SYMBOL[c[1]] }}</span>
            </td>
            <td class="py-1 px-2 text-right">{{ pct(r.weight) }}</td>
            <td v-for="(f, a) in r.freqs" :key="a" class="py-1 px-2 text-right" :style="f >= 0.005 ? { background: tint(colors[a], f) } : {}">{{ f >= 0.005 ? pct(f) : "" }}</td>
            <td class="py-1 px-2 text-right">{{ fmtEv(Math.round(r.ev * 10) / 10) }}</td>
            <td class="py-1 px-2 text-right">{{ pct(r.equity) }}</td>
            <td class="py-1 px-2 text-right">{{ r.realised === null ? "–" : pct(r.realised) }}</td>
          </tr>
        </tbody>
      </table>
    </div>
    <button v-if="rows.length > shown.length" class="mt-2 text-sm text-felt font-bold underline" @click="all = true">Show all {{ rows.length }}</button>
    <p class="hint mt-2">
      "In range" is how much of the hand is still here after the actions so far. "Share kept" is the hand's value against what its chance to win would be worth if the pot were simply shared out: over 100% means the hand does better than its raw strength, under 100% worse.
    </p>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { SUIT_COLOR, SUIT_SYMBOL } from "../../guided/data";

interface Combo { hand: string; cell: number; weight: number; freqs: number[]; equity: number; ev: number }
const props = defineProps<{ combos: Combo[]; actions: string[]; colors: string[]; pot: number; selected: number | null; fmtEv: (chips: number) => string; /** when given, only these exact hands are listed */ only?: Set<string> | null }>();
const emit = defineEmits<{ (e: "select", cell: number): void }>();

const filter = ref(""); const all = ref(false);
const sortKey = ref("ev"); const sortDown = ref(true);
const pct = (x: number) => `${Math.round(x * 100)}%`;
/** an action's colour, lighter the less often the hand takes it */
const tint = (color: string, f: number) => `color-mix(in srgb, ${color} ${Math.round(12 + f * 43)}%, transparent)`;

const columns = computed(() => [
  { key: "hand", label: "Hand", title: "Sort by rank", color: "" },
  { key: "weight", label: "In range", title: "How much of this hand is still in the range here", color: "" },
  ...props.actions.map((a, i) => ({ key: "a" + i, label: a, title: `How often the hand chooses: ${a}`, color: props.colors[i] })),
  { key: "ev", label: "Worth", title: "What the hand collects from the pot on average", color: "" },
  { key: "equity", label: "Wins", title: "Chance to win at showdown against the other range", color: "" },
  { key: "realised", label: "Share kept", title: "Value divided by (chance to win x pot)", color: "" },
]);

const RANKS = "23456789TJQKA";
const rankValue = (hand: string) => RANKS.indexOf(hand[0]) * 13 + RANKS.indexOf(hand[2]);
const rows = computed(() => {
  // filter words: ranks ("AK", "77"), a card ("7h") or a suit letter ("s")
  const words = filter.value.trim().split(/[\s,]+/).filter(Boolean);
  const match = (hand: string) => words.every((w) => {
    const u = w[0].toUpperCase() + w.slice(1).toLowerCase();
    if (/^[shdc]$/i.test(w)) return hand[1] === w.toLowerCase() || hand[3] === w.toLowerCase();
    if (/^[2-9TJQKA][shdc]$/i.test(w)) return hand.slice(0, 2) === u || hand.slice(2) === u;
    const ranks = w.toUpperCase().replace(/10/g, "T").replace(/[^2-9TJQKA]/g, "");
    if (ranks.length === 1) return hand[0] === ranks || hand[2] === ranks;
    if (ranks.length === 2) return (hand[0] === ranks[0] && hand[2] === ranks[1]) || (hand[0] === ranks[1] && hand[2] === ranks[0]);
    return true;
  });
  const list = props.combos.filter((c) => c.weight > 0.0005 && (!props.only || props.only.has(c.hand)) && match(c.hand)).map((c) => ({ ...c, realised: c.equity > 0.005 && props.pot > 0 ? c.ev / (c.equity * props.pot) : null as number | null }));
  const k = sortKey.value;
  const value = (r: (typeof list)[number]): number => k === "hand" ? rankValue(r.hand) : k === "weight" ? r.weight : k === "ev" ? r.ev : k === "equity" ? r.equity : k === "realised" ? r.realised ?? -1 : r.freqs[Number(k.slice(1))] ?? 0;
  return list.sort((x, y) => (sortDown.value ? value(y) - value(x) : value(x) - value(y)) || rankValue(y.hand) - rankValue(x.hand));
});
const shown = computed(() => (all.value ? rows.value : rows.value.slice(0, 60)));
function sortBy(k: string) { if (sortKey.value === k) sortDown.value = !sortDown.value; else { sortKey.value = k; sortDown.value = true; } }
// a different decision has different actions: an action column that no longer exists falls back to value
watch(() => props.actions.length, (n) => { if (sortKey.value.startsWith("a") && Number(sortKey.value.slice(1)) >= n) sortKey.value = "ev"; });
</script>

<style scoped>
.hint { @apply text-sm text-muted; }
</style>
