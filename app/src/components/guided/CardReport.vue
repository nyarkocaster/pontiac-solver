<template>
  <div>
    <div v-if="!rows.length && !busy">
      <button class="btn-quiet" @click="run">Compare every {{ street }} card</button>
      <p class="hint mt-1.5">Shows which cards help which player, and how often the next player bets on each. Takes a few seconds.</p>
    </div>
    <p v-else-if="busy" class="text-sm font-bold text-felt" role="status">Looking at each card… {{ done }} of {{ cards.length }}</p>
    <template v-else>
      <div class="flex flex-wrap items-center justify-between gap-2">
        <div class="seg" role="group" aria-label="What the cards show">
          <button :class="metric === 'bet' ? 'seg-on' : ''" @click="metric = 'bet'">How often {{ actor }} bets</button>
          <button :class="metric === 'eq' ? 'seg-on' : ''" @click="metric = 'eq'">{{ names[hero] }}'s chance to win</button>
        </div>
        <button class="text-xs text-felt font-bold underline" @click="table = !table">{{ table ? "Show the cards" : "Show as a table" }}</button>
      </div>
      <div v-if="!table" class="mt-3 grid gap-[3px]" style="grid-template-columns: repeat(13, minmax(0, 1fr))">
        <template v-for="s in [3, 2, 1, 0]" :key="s">
          <template v-for="r in [12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]" :key="r">
            <button v-if="byId.get(r * 4 + s)" class="cell" :title="tip(byId.get(r * 4 + s)!)" @click="$emit('pick', r * 4 + s)">
              <span :class="SUIT_COLOR[SUIT_CHARS[s]]" class="font-bold">{{ RANK_CHARS[r] }}{{ SUIT_SYMBOL[SUIT_CHARS[s]] }}</span>
              <span class="tabular-nums text-[10px] leading-none text-muted">{{ Math.round(val(byId.get(r * 4 + s)!) * 100) }}</span>
              <span class="bar"><span :style="{ width: val(byId.get(r * 4 + s)!) * 100 + '%' }"></span></span>
            </button>
            <span v-else class="cell cell-off" aria-hidden="true"></span>
          </template>
        </template>
      </div>
      <table v-else class="w-full text-sm mt-2 tabular-nums">
        <thead><tr class="text-left text-xs text-muted"><th class="font-normal py-1">Card</th><th class="font-normal text-right">{{ actor }} bets</th><th v-for="(n, i) in names" :key="i" class="font-normal text-right">{{ n }} wins</th></tr></thead>
        <tbody>
          <tr v-for="r in sorted" :key="r.id" class="border-t border-rule">
            <td class="py-0.5 font-bold" :class="SUIT_COLOR[r.label[1]]">{{ r.label[0] }}{{ SUIT_SYMBOL[r.label[1]] }}</td>
            <td class="text-right">{{ pc(r.bet) }}</td><td v-for="(_n, i) in names" :key="i" class="text-right">{{ r.equity[i] === undefined ? "–" : pc(r.equity[i]) }}</td>
          </tr>
        </tbody>
      </table>
      <ul class="mt-3 space-y-1 text-sm">
        <li v-for="(t, i) in notes" :key="i" class="flex gap-2"><span class="text-felt font-bold">•</span><span>{{ t }}</span></li>
      </ul>
      <p class="hint mt-1.5">The number and bar on each card are percentages. Click a card to deal it.</p>
    </template>
    <p v-if="error" class="mt-2 text-sm font-bold text-danger">{{ error }}</p>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { RANK_CHARS, SUIT_CHARS, SUIT_SYMBOL, SUIT_COLOR } from "../../guided/data";

export interface CardRow { actor: string; bet: number; equity: number[] }
const props = defineProps<{ cards: [string, number][]; names: string[]; hero: number; street: string; resetKey: string; load: (id: number) => Promise<CardRow> }>();
defineEmits<{ (e: "pick", id: number): void }>();
interface Row extends CardRow { id: number; label: string }
const rows = ref<Row[]>([]); const busy = ref(false); const done = ref(0); const error = ref("");
const metric = ref<"bet" | "eq">("bet"); const table = ref(false);
let token = 0;
watch(() => props.resetKey, () => { token++; rows.value = []; busy.value = false; error.value = ""; });
async function run() {
  const mine = ++token; busy.value = true; done.value = 0; error.value = ""; const out: Row[] = [];
  try {
    for (const [label, id] of props.cards) { const r = await props.load(id); if (mine !== token) return; out.push({ ...r, id, label }); done.value++; }
    rows.value = out;
  } catch (e) { if (mine === token) error.value = String((e as Error).message ?? e); }
  finally { if (mine === token) busy.value = false; }
}
const byId = computed(() => new Map(rows.value.map((r) => [r.id, r])));
const actor = computed(() => rows.value[0]?.actor ?? "the next player");
const val = (r: Row) => (metric.value === "bet" ? r.bet : r.equity[props.hero] ?? 0);
const pc = (v: number) => Math.round(v * 100) + "%";
const sorted = computed(() => [...rows.value].sort((a, b) => val(b) - val(a)));
const tip = (r: Row) => `${r.label}: ${r.actor} bets ${pc(r.bet)}. ` + props.names.map((n, i) => (r.equity[i] === undefined ? "" : `${n} wins ${pc(r.equity[i])}`)).filter(Boolean).join(", ");
const names5 = (list: Row[]) => list.map((r) => r.label[0] + SUIT_SYMBOL[r.label[1]]).join(" ");
const notes = computed(() => {
  const rs = rows.value; if (rs.length < 4) return [];
  const eq = [...rs].sort((a, b) => (b.equity[props.hero] ?? 0) - (a.equity[props.hero] ?? 0));
  const bet = [...rs].sort((a, b) => b.bet - a.bet);
  const k = Math.min(5, Math.floor(rs.length / 2));
  const avgBet = rs.reduce((a, r) => a + r.bet, 0) / rs.length;
  return [
    `Best cards for ${props.names[props.hero]}: ${names5(eq.slice(0, k))} (wins ${pc(eq[0].equity[props.hero] ?? 0)} on the best). Worst: ${names5(eq.slice(-k).reverse())} (${pc(eq[eq.length - 1].equity[props.hero] ?? 0)}).`,
    `${actor.value} bets ${pc(avgBet)} of the time on an average card; most on ${names5(bet.slice(0, k))} (${pc(bet[0].bet)}), least on ${names5(bet.slice(-k).reverse())} (${pc(bet[bet.length - 1].bet)}).`,
  ];
});
</script>

<style scoped>
.cell { @apply flex flex-col items-center gap-[2px] rounded border border-rule bg-surface pt-1 pb-[3px] px-[2px] text-[11px] leading-none hover:border-felt transition-colors; }
.cell-off { @apply border-dashed bg-transparent opacity-40 min-h-[2.4rem]; }
.bar { @apply block w-full h-[4px] rounded-sm bg-paper overflow-hidden; }
.bar > span { @apply block h-full bg-felt; }
.hint { @apply text-xs text-muted; }
.btn-quiet { @apply px-3 py-1.5 rounded-lg border border-rule bg-surface text-sm font-bold hover:border-felt; }
.seg { @apply inline-flex rounded-lg border border-rule overflow-hidden text-xs font-bold; }
.seg button { @apply px-2.5 py-1 bg-surface; }
.seg .seg-on { @apply bg-felt text-white; }
</style>
