<template>
  <div>
    <div class="flex flex-wrap items-center gap-2">
      <span class="text-sm font-bold">Compare</span>
      <div class="seg" role="group" aria-label="How many flops">
        <button v-for="n in [8, 16, 24]" :key="n" :class="count === n ? 'seg-on' : ''" :disabled="busy" @click="count = n">{{ n }} flops</button>
      </div>
      <button v-if="!busy" class="btn-quiet" :disabled="disabled" @click="run">{{ rows.length ? "Run again" : "Run the flop report" }}</button>
      <button v-else class="btn-quiet" @click="stop">Stop</button>
    </div>
    <p class="hint mt-1.5">
      Solves this same preflop spot on {{ count }} different kinds of flop, one after another, each to within about 1% of the pot,
      and lines the answers up. {{ !busy && rows.length ? "" : perFlop ? `About ${timeText(perFlop * (count - rows.length))} left at this pace.` : "The first flop tells you how long the rest will take." }}
      It replaces the result on the Result page.
    </p>
    <div v-if="busy" class="mt-2" role="status">
      <div class="flex justify-between text-sm font-bold"><span>Flop {{ rows.length + 1 }} of {{ count }}: {{ nowFlop }}</span><span class="tabular-nums">{{ timeText(elapsed) }}</span></div>
      <div class="h-2 bg-paper rounded mt-1 overflow-hidden"><div class="h-full bg-felt transition-all" :style="{ width: (rows.length / count) * 100 + '%' }"></div></div>
    </div>
    <p v-if="error" class="mt-2 text-sm font-bold text-danger">{{ error }}</p>

    <template v-if="rows.length">
      <div class="overflow-x-auto mt-3">
        <table class="w-full text-sm tabular-nums">
          <thead>
            <tr class="text-left text-xs text-muted">
              <th v-for="c in COLS" :key="c.id" class="font-normal py-1 pr-2 whitespace-nowrap" :class="c.id === 'flop' ? '' : 'text-right'">
                <button class="underline decoration-dotted" :class="sortBy === c.id ? 'font-bold text-ink' : ''" @click="sort(c.id)">{{ label(c.id) }}{{ sortBy === c.id ? (sortDown ? " ▾" : " ▴") : "" }}</button>
              </th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="r in sorted" :key="r.flop" class="border-t border-rule hover:bg-felt-light cursor-pointer" title="Click to put this flop on the board" @click="$emit('pick', r.cards)">
              <td class="py-1 pr-2 whitespace-nowrap">
                <span v-for="c in r.names" :key="c" class="font-bold mr-0.5" :class="SUIT_COLOR[c[1]]">{{ c[0] }}{{ SUIT_SYMBOL[c[1]] }}</span>
                <span class="text-xs text-muted ml-1">{{ r.kind }}</span>
              </td>
              <td class="text-right pr-2">{{ pc(r.heroEq) }}</td>
              <td class="text-right pr-2">{{ pc(r.heroEv) }}</td>
              <td class="pr-2"><span class="cellbar"><span :style="{ width: r.oopBet * 100 + '%' }"></span></span><span class="num">{{ pc(r.oopBet) }}</span></td>
              <td class="pr-2"><span class="cellbar"><span :style="{ width: r.ipBet * 100 + '%' }"></span></span><span class="num">{{ pc(r.ipBet) }}</span></td>
              <td class="text-right text-xs">{{ r.ipSize }}</td>
            </tr>
            <tr class="border-t-2 border-rule font-bold">
              <td class="py-1">Average of {{ rows.length }}</td>
              <td class="text-right pr-2">{{ pc(avg((r) => r.heroEq)) }}</td><td class="text-right pr-2">{{ pc(avg((r) => r.heroEv)) }}</td>
              <td class="text-right pr-2">{{ pc(avg((r) => r.oopBet)) }}</td><td class="text-right pr-2">{{ pc(avg((r) => r.ipBet)) }}</td><td></td>
            </tr>
          </tbody>
        </table>
      </div>
      <h4 v-if="groups.length" class="font-bold text-sm mt-4">By kind of flop</h4>
      <div class="grid grid-cols-2 gap-x-6 gap-y-1 mt-1 text-sm">
        <div v-for="g in groups" :key="g.name" class="flex justify-between gap-2 border-t border-rule py-1">
          <span>{{ g.name }} <span class="text-xs text-muted">({{ g.n }})</span></span>
          <span class="tabular-nums text-right">{{ hero }} wins <b>{{ pc(g.eq) }}</b> · {{ ip }} bets <b>{{ pc(g.ipBet) }}</b></span>
        </div>
      </div>
      <ul class="mt-3 space-y-1 text-sm">
        <li v-for="(t, i) in notes" :key="i" class="flex gap-2"><span class="text-felt font-bold">•</span><span>{{ t }}</span></li>
      </ul>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from "vue";
import { RANK_CHARS, SUIT_CHARS, SUIT_SYMBOL, SUIT_COLOR } from "../../guided/data";

export interface FlopRow { heroEq: number; heroEv: number; oopBet: number; ipBet: number; ipSize: string }
const props = defineProps<{ hero: string; oop: string; ip: string; disabled?: boolean; solve: (flop: number[]) => Promise<FlopRow>; cancel: () => void }>();
const emit = defineEmits<{ (e: "pick", cards: number[]): void; (e: "busy", on: boolean): void }>();

// a spread of flops: the first 8 cover the main kinds, the next groups fill in between
const FLOPS = ["As7d2c", "Ks8s3d", "QhJh6c", "Ts9s8d", "7s6d5c", "KsKd4c", "Jh9h5h", "8s3d2c",
  "AsKdTc", "Ah8h5c", "Qs7d2c", "Jd7d4s", "Ts6d2c", "9c6c4d", "7s7d4d", "As5d5c",
  "KsQsJd", "KcTc6c", "Ad4d3s", "Qs9d8c", "Js6d2c", "Th5h4c", "6s5s2d", "QsQd7c"];
const ids = (f: string) => [0, 2, 4].map((k) => RANK_CHARS.indexOf(f[k]) * 4 + SUIT_CHARS.indexOf(f[k + 1]));
interface Row extends FlopRow { flop: string; names: string[]; cards: number[]; kind: string; high: string; suits: string; paired: boolean; joined: boolean }
function describe(f: string) {
  const r = [0, 2, 4].map((k) => RANK_CHARS.indexOf(f[k])).sort((a, b) => b - a);
  const ns = new Set([f[1], f[3], f[5]]).size;
  const paired = new Set(r).size < 3;
  const suits = ns === 1 ? "One suit" : ns === 2 ? "Two of a suit" : "Three suits";
  const high = r[0] === 12 ? "Ace high" : r[0] >= 10 ? "King or queen high" : r[0] >= 8 ? "Jack or ten high" : "Nine high or lower";
  const joined = !paired && r[0] - r[2] <= 4;
  return { high, suits, paired, joined, kind: [paired ? "paired" : joined ? "connected" : "", ns === 1 ? "one suit" : ns === 2 ? "two-tone" : "rainbow"].filter(Boolean).join(", ") };
}

const count = ref(8); const rows = ref<Row[]>([]); const busy = ref(false); const error = ref("");
const nowFlop = ref(""); const elapsed = ref(0); const perFlop = ref(0);
let token = 0; let timer = 0;
async function run() {
  const mine = ++token; rows.value = []; error.value = ""; busy.value = true; emit("busy", true); perFlop.value = 0;
  const began = Date.now(); elapsed.value = 0;
  timer = window.setInterval(() => (elapsed.value = Math.round((Date.now() - began) / 1000)), 500);
  try {
    for (const f of FLOPS.slice(0, count.value)) {
      nowFlop.value = [0, 2, 4].map((k) => f[k] + SUIT_SYMBOL[f[k + 1]]).join(" ");
      const r = await props.solve(ids(f));
      if (mine !== token) return;
      rows.value = [...rows.value, { ...r, flop: f, names: [f.slice(0, 2), f.slice(2, 4), f.slice(4, 6)], cards: ids(f), ...describe(f) }];
      perFlop.value = (Date.now() - began) / 1000 / rows.value.length;
    }
  } catch (e) { if (mine === token) error.value = String((e as Error).message ?? e); }
  finally { if (mine === token) { window.clearInterval(timer); busy.value = false; emit("busy", false); } }
}
function stop() { token++; props.cancel(); window.clearInterval(timer); busy.value = false; emit("busy", false); }
onBeforeUnmount(() => window.clearInterval(timer));

const COLS = [{ id: "flop" }, { id: "heroEq" }, { id: "heroEv" }, { id: "oopBet" }, { id: "ipBet" }, { id: "ipSize" }] as const;
type ColId = (typeof COLS)[number]["id"];
const label = (id: ColId) => ({ flop: "Flop", heroEq: `${props.hero} wins`, heroEv: `${props.hero}'s share of the pot`, oopBet: `${props.oop} bets first`, ipBet: `${props.ip} bets when checked to`, ipSize: "Favourite size" })[id];
const sortBy = ref<ColId>("flop"); const sortDown = ref(true);
const sort = (id: ColId) => { if (sortBy.value === id) sortDown.value = !sortDown.value; else { sortBy.value = id; sortDown.value = true; } };
const sorted = computed(() => {
  const id = sortBy.value; if (id === "flop" || id === "ipSize") return sortDown.value ? rows.value : [...rows.value].reverse();
  return [...rows.value].sort((a, b) => (sortDown.value ? b[id] - a[id] : a[id] - b[id]));
});
const pc = (v: number) => Math.round(v * 100) + "%";
const avg = (f: (r: Row) => number, list: Row[] = rows.value) => (list.length ? list.reduce((a, r) => a + f(r), 0) / list.length : 0);
const timeText = (s: number) => (s < 90 ? `${Math.max(1, Math.round(s))} seconds` : `${Math.round(s / 60)} minutes`);
const groups = computed(() => {
  const out: { name: string; n: number; eq: number; ipBet: number }[] = [];
  const add = (name: string, list: Row[]) => { if (list.length && list.length < rows.value.length) out.push({ name, n: list.length, eq: avg((r) => r.heroEq, list), ipBet: avg((r) => r.ipBet, list) }); };
  for (const h of ["Ace high", "King or queen high", "Jack or ten high", "Nine high or lower"]) add(h, rows.value.filter((r) => r.high === h));
  add("Paired", rows.value.filter((r) => r.paired)); add("Connected", rows.value.filter((r) => r.joined));
  for (const s of ["Three suits", "Two of a suit", "One suit"]) add(s, rows.value.filter((r) => r.suits === s));
  return out;
});
const notes = computed(() => {
  const rs = rows.value; if (rs.length < 4 || busy.value) return [];
  const show = (r: Row) => r.names.map((c) => c[0] + SUIT_SYMBOL[c[1]]).join("");
  const byEq = [...rs].sort((a, b) => b.heroEq - a.heroEq); const byBet = [...rs].sort((a, b) => b.ipBet - a.ipBet);
  return [
    `Best flops for ${props.hero}: ${byEq.slice(0, 3).map(show).join(", ")} (wins ${pc(byEq[0].heroEq)} on the best). Worst: ${byEq.slice(-3).reverse().map(show).join(", ")} (${pc(byEq[byEq.length - 1].heroEq)}).`,
    `${props.ip} bets most when checked to on ${byBet.slice(0, 3).map(show).join(", ")} (${pc(byBet[0].ipBet)}) and least on ${byBet.slice(-3).reverse().map(show).join(", ")} (${pc(byBet[byBet.length - 1].ipBet)}).`,
    `These are quick solves of a sample of flops, so read them for the pattern. Solve a single flop in full for exact numbers.`,
  ];
});
</script>

<style scoped>
.hint { @apply text-xs text-muted; }
.btn-quiet { @apply px-3 py-1.5 rounded-lg border border-rule bg-surface text-sm font-bold hover:border-felt disabled:opacity-50; }
.seg { @apply inline-flex rounded-lg border border-rule overflow-hidden text-xs font-bold; }
.seg button { @apply px-2.5 py-1.5 bg-surface; }
.seg .seg-on { @apply bg-felt text-white; }
.cellbar { @apply inline-block align-middle w-16 h-2 rounded-sm bg-paper overflow-hidden mr-1.5; }
.cellbar > span { @apply block h-full bg-felt; }
.num { @apply inline-block w-9 text-right; }
</style>
