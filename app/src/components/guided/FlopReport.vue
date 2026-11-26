<template>
  <div>
    <div class="flex flex-wrap items-center gap-2">
      <span class="text-sm font-bold">Compare</span>
      <div class="seg" role="group" aria-label="How many flops">
        <button v-for="n in [8, 24, 49, 95]" :key="n" :class="count === n ? 'seg-on' : ''" :disabled="busy" @click="count = n">{{ n }} flops</button>
      </div>
      <button v-if="!busy" class="btn-quiet" :disabled="disabled" @click="run">{{ rows.length ? "Run again" : "Run the flop report" }}</button>
      <button v-else class="btn-quiet" @click="stop">Stop and keep these</button>
      <button v-if="rows.length && !busy" class="text-xs text-felt font-bold underline" @click="download">Download as a spreadsheet file</button>
    </div>
    <p class="hint mt-1.5">
      Solves this same preflop spot on {{ count }} different flops, one after another, each to within about 1% of the pot,
      and lines the answers up. {{ !busy && rows.length ? "" : perFlop ? `About ${timeText(perFlop * (count - rows.length))} left at this pace.` : "The first flop tells you how long the rest will take." }}
      It replaces the result on the Result page.
    </p>
    <div v-if="busy" class="mt-2" role="status">
      <div class="flex justify-between text-sm font-bold"><span>Flop {{ rows.length + 1 }} of {{ count }}: {{ nowFlop }}</span><span class="tabular-nums">{{ timeText(elapsed) }}</span></div>
      <div class="h-2 bg-paper rounded mt-1 overflow-hidden"><div class="h-full bg-felt transition-all" :style="{ width: (rows.length / count) * 100 + '%' }"></div></div>
    </div>
    <p v-if="error" class="mt-2 text-sm font-bold text-danger">{{ error }}</p>

    <template v-if="rows.length">
      <div class="flex flex-wrap items-center gap-1.5 mt-3 text-sm" role="group" aria-label="Show only these kinds of flop">
        <span class="text-xs text-muted mr-1">Show only</span>
        <button v-for="t in TEXTURES" :key="t.id" class="chip" :class="want.has(t.id) ? 'chip-on' : ''" :aria-pressed="want.has(t.id)" @click="toggle(t.id)">{{ t.label }} <span class="text-xs opacity-70">{{ rows.filter(t.test).length }}</span></button>
        <button v-if="want.size" class="text-xs underline text-muted ml-1" @click="want = new Set()">All flops</button>
      </div>
      <p v-if="want.size && !shown.length" class="text-sm text-muted mt-2">None of the {{ rows.length }} flops is of that kind.</p>
      <div class="overflow-x-auto mt-2">
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
              <td class="py-1" title="Weighted by how often each kind of flop is dealt">Average of {{ shown.length }}{{ want.size ? " shown" : "" }}</td>
              <td class="text-right pr-2">{{ pc(avg((r) => r.heroEq, shown)) }}</td><td class="text-right pr-2">{{ pc(avg((r) => r.heroEv, shown)) }}</td>
              <td class="text-right pr-2">{{ pc(avg((r) => r.oopBet, shown)) }}</td><td class="text-right pr-2">{{ pc(avg((r) => r.ipBet, shown)) }}</td><td></td>
            </tr>
            <tr v-if="want.size" class="text-muted">
              <td class="py-1">Average of all {{ rows.length }}</td>
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
// Larger sets: one flop from every rank pattern in turn, cycling through suit patterns, so high
// cards, pairs, connectedness and suits all stay in proportion.
function spread(n: number): string[] {
  const out = [...FLOPS]; const seen = new Set(out.map(key));
  const suits = [["s", "d", "c"], ["s", "s", "d"], ["h", "d", "h"], ["c", "h", "h"], ["d", "d", "d"]];
  let k = 0;
  for (let step = 0; out.length < n && step < 40; step++) {
    for (let a = 12; a >= 0 && out.length < n; a--) for (let b = a; b >= 0 && out.length < n; b--) for (let c = b; c >= 0 && out.length < n; c--) {
      if ((a * 7 + b * 3 + c + step * 5) % 9 !== 0 || (a === b && b === c)) continue;
      let su = suits[k++ % suits.length];
      if (a === b && su[0] === su[1]) su = [su[0], su[2] === su[0] ? "h" : su[2], su[2]];
      if (b === c && su[1] === su[2]) su = [su[0], su[1], su[0] === su[1] ? "c" : su[0]];
      if (a === b && su[0] === su[1]) continue;
      const f = RANK_CHARS[a] + su[0] + RANK_CHARS[b] + su[1] + RANK_CHARS[c] + su[2];
      if (!seen.has(key(f))) { seen.add(key(f)); out.push(f); }
    }
  }
  return out.slice(0, n);
}
function key(f: string) { const ns = new Set([f[1], f[3], f[5]]).size; return f[0] + f[2] + f[4] + ns + (ns === 2 ? (f[1] === f[3] ? "a" : f[3] === f[5] ? "b" : "c") : ""); }
const ids = (f: string) => [0, 2, 4].map((k) => RANK_CHARS.indexOf(f[k]) * 4 + SUIT_CHARS.indexOf(f[k + 1]));
interface Row extends FlopRow { ways: number; flop: string; names: string[]; cards: number[]; kind: string; high: string; suits: string; paired: boolean; joined: boolean }
function describe(f: string) {
  const r = [0, 2, 4].map((k) => RANK_CHARS.indexOf(f[k])).sort((a, b) => b - a);
  const ns = new Set([f[1], f[3], f[5]]).size;
  const paired = new Set(r).size < 3;
  const suits = ns === 1 ? "One suit" : ns === 2 ? "Two of a suit" : "Three suits";
  const high = r[0] === 12 ? "Ace high" : r[0] >= 10 ? "King or queen high" : r[0] >= 8 ? "Jack or ten high" : "Nine high or lower";
  const joined = !paired && r[0] - r[2] <= 4;
  const ways = paired ? 12 : ns === 1 ? 4 : ns === 2 ? 12 : 24; // how many real flops share this shape
  return { ways, high, suits, paired, joined, kind: [paired ? "paired" : joined ? "connected" : "", ns === 1 ? "one suit" : ns === 2 ? "two-tone" : "rainbow"].filter(Boolean).join(", ") };
}

const count = ref(8); const rows = ref<Row[]>([]); const busy = ref(false); const error = ref("");
const nowFlop = ref(""); const elapsed = ref(0); const perFlop = ref(0);
let token = 0; let timer = 0;
async function run() {
  const mine = ++token; rows.value = []; error.value = ""; busy.value = true; emit("busy", true); perFlop.value = 0;
  const began = Date.now(); elapsed.value = 0;
  timer = window.setInterval(() => (elapsed.value = Math.round((Date.now() - began) / 1000)), 500);
  try {
    for (const f of spread(count.value)) {
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

function download() {
  const head = ["Flop", "Kind", `${props.hero} wins %`, `${props.hero} share of pot %`, `${props.oop} bets first %`, `${props.ip} bets when checked to %`, "Favourite size"];
  const lines = rows.value.map((r) => [r.flop, r.kind, (r.heroEq * 100).toFixed(1), (r.heroEv * 100).toFixed(1), (r.oopBet * 100).toFixed(1), (r.ipBet * 100).toFixed(1), r.ipSize].map((x) => `"${x}"`).join(","));
  const a = document.createElement("a");
  a.href = URL.createObjectURL(new Blob([[head.map((x) => `"${x}"`).join(","), ...lines].join("\r\n")], { type: "text/csv" }));
  a.download = "flop-report.csv"; a.click(); URL.revokeObjectURL(a.href);
}
const COLS = [{ id: "flop" }, { id: "heroEq" }, { id: "heroEv" }, { id: "oopBet" }, { id: "ipBet" }, { id: "ipSize" }] as const;
type ColId = (typeof COLS)[number]["id"];
const label = (id: ColId) => ({ flop: "Flop", heroEq: `${props.hero} wins`, heroEv: `${props.hero}'s share of the pot`, oopBet: `${props.oop} bets first`, ipBet: `${props.ip} bets when checked to`, ipSize: "Favourite size" })[id];
const sortBy = ref<ColId>("flop"); const sortDown = ref(true);
const sort = (id: ColId) => { if (sortBy.value === id) sortDown.value = !sortDown.value; else { sortBy.value = id; sortDown.value = true; } };
// kinds of flop to show; within one line (suits, pairing, height) picks widen, across lines they narrow
const TEXTURES: { id: string; group: string; label: string; test: (r: Row) => boolean }[] = [
  { id: "paired", group: "shape", label: "Paired", test: (r) => r.paired },
  { id: "connected", group: "shape", label: "Connected", test: (r) => r.joined },
  { id: "dry", group: "shape", label: "Unpaired, not connected", test: (r) => !r.paired && !r.joined },
  { id: "mono", group: "suits", label: "One suit", test: (r) => r.suits === "One suit" },
  { id: "two", group: "suits", label: "Two-tone", test: (r) => r.suits === "Two of a suit" },
  { id: "rainbow", group: "suits", label: "Rainbow", test: (r) => r.suits === "Three suits" },
  { id: "ace", group: "high", label: "Ace high", test: (r) => r.high === "Ace high" },
  { id: "kq", group: "high", label: "King or queen high", test: (r) => r.high === "King or queen high" },
  { id: "jt", group: "high", label: "Jack or ten high", test: (r) => r.high === "Jack or ten high" },
  { id: "low", group: "high", label: "Nine high or lower", test: (r) => r.high === "Nine high or lower" },
];
const want = ref(new Set<string>());
function toggle(id: string) { const n = new Set(want.value); if (n.has(id)) n.delete(id); else n.add(id); want.value = n; }
const shown = computed(() => {
  if (!want.value.size) return rows.value;
  const groups = new Map<string, ((r: Row) => boolean)[]>();
  for (const t of TEXTURES) if (want.value.has(t.id)) groups.set(t.group, [...(groups.get(t.group) ?? []), t.test]);
  return rows.value.filter((r) => [...groups.values()].every((tests) => tests.some((f) => f(r))));
});
const sorted = computed(() => {
  const id = sortBy.value; if (id === "flop" || id === "ipSize") return sortDown.value ? shown.value : [...shown.value].reverse();
  return [...shown.value].sort((a, b) => (sortDown.value ? b[id] - a[id] : a[id] - b[id]));
});
const pc = (v: number) => Math.round(v * 100) + "%";
const avg = (f: (r: Row) => number, list: Row[] = rows.value) => { const w = list.reduce((a, r) => a + r.ways, 0); return w ? list.reduce((a, r) => a + f(r) * r.ways, 0) / w : 0; };
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
.chip { @apply px-2 py-0.5 rounded-full border border-rule bg-surface hover:border-felt; }
.chip-on { @apply border-felt bg-felt text-white; }
.hint { @apply text-xs text-muted; }
.btn-quiet { @apply px-3 py-1.5 rounded-lg border border-rule bg-surface text-sm font-bold hover:border-felt disabled:opacity-50; }
.seg { @apply inline-flex rounded-lg border border-rule overflow-hidden text-xs font-bold; }
.seg button { @apply px-2.5 py-1.5 bg-surface; }
.seg .seg-on { @apply bg-felt text-white; }
.cellbar { @apply inline-block align-middle w-16 h-2 rounded-sm bg-paper overflow-hidden mr-1.5; }
.cellbar > span { @apply block h-full bg-felt; }
.num { @apply inline-block w-9 text-right; }
</style>
