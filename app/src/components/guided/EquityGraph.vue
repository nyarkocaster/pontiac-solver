<template>
  <div>
    <div class="flex flex-wrap items-center justify-between gap-2">
      <div class="flex flex-wrap gap-x-4 gap-y-1 text-sm">
        <span v-for="l in lines" :key="l.i" class="inline-flex items-center gap-1.5">
          <span class="inline-block w-4 h-[3px] rounded" :style="{ background: l.color }"></span>
          <span class="font-bold">{{ l.name }}</span><span class="text-muted tabular-nums">wins {{ pc(l.avg) }} on average</span>
        </span>
      </div>
      <button class="text-xs text-felt font-bold underline" @click="table = !table">{{ table ? "Show the graph" : "Show as a table" }}</button>
    </div>

    <div v-if="!table" class="relative mt-2">
      <svg :viewBox="`0 0 ${W} ${H}`" class="w-full select-none" role="img" :aria-label="summary.join(' ')" @mousemove="move" @mouseleave="hover = null">
        <g v-for="t in [0, 25, 50, 75, 100]" :key="t">
          <line :x1="L" :x2="W - R" :y1="y(t / 100)" :y2="y(t / 100)" stroke="rgb(var(--rule))" :stroke-width="t === 50 ? 1.2 : 0.7" :stroke-dasharray="t === 50 ? '' : '2 3'" />
          <text :x="L - 6" :y="y(t / 100) + 3.5" text-anchor="end" class="tick">{{ t }}%</text>
          <text :x="x(t / 100)" :y="H - B + 14" text-anchor="middle" class="tick">{{ t }}%</text>
        </g>
        <text :x="L" :y="H - 3" class="tick">◂ strongest hands</text>
        <text :x="W - R" :y="H - 3" text-anchor="end" class="tick">weakest hands ▸</text>
        <text :x="(L + W - R) / 2" :y="H - 3" text-anchor="middle" class="tick">share of the range</text>
        <path v-for="l in lines" :key="l.i" :d="l.path" fill="none" :stroke="l.color" stroke-width="2" stroke-linejoin="round" stroke-linecap="round" />
        <text v-for="l in lines" :key="'n' + l.i" :x="W - R + 5" :y="l.labelY + 3.5" class="lab">{{ l.name }}</text>
        <g v-if="hover !== null">
          <line :x1="x(hover)" :x2="x(hover)" :y1="T" :y2="H - B" stroke="rgb(var(--muted))" stroke-width="0.8" />
          <circle v-for="l in lines" :key="'c' + l.i" :cx="x(hover)" :cy="y(at(l, hover))" r="4" :fill="l.color" stroke="rgb(var(--surface))" stroke-width="2" />
        </g>
      </svg>
      <div v-if="hover !== null" class="tip" :style="{ left: Math.min(72, Math.max(2, hover * 100 - 8)) + '%' }">
        <div class="text-muted">{{ hover < 0.005 ? "The very best hand" : `${Math.round(hover * 100)}% of the way down the range` }}</div>
        <div v-for="l in lines" :key="l.i" class="flex items-center gap-1.5">
          <span class="inline-block w-3 h-[3px] rounded" :style="{ background: l.color }"></span>
          <span class="flex-1">{{ l.name }}</span><b class="tabular-nums">wins {{ pc(at(l, hover)) }}</b>
        </div>
      </div>
    </div>

    <table v-else class="w-full text-sm mt-2 tabular-nums">
      <thead><tr class="text-left text-xs text-muted"><th class="font-normal py-1">Part of the range</th><th v-for="l in lines" :key="l.i" class="font-normal text-right">{{ l.name }}</th></tr></thead>
      <tbody>
        <tr v-for="row in ROWS" :key="row.label" class="border-t border-rule">
          <td class="py-1">{{ row.label }}</td><td v-for="l in lines" :key="l.i" class="text-right font-bold">{{ pc(mean(l, row.from, row.to)) }}</td>
        </tr>
        <tr class="border-t border-rule"><td class="py-1">Hands that win 4 times in 5</td><td v-for="l in lines" :key="l.i" class="text-right font-bold">{{ pc(l.strong) }}</td></tr>
        <tr class="border-t border-rule"><td class="py-1">Hands that win under 1 time in 3</td><td v-for="l in lines" :key="l.i" class="text-right font-bold">{{ pc(l.weak) }}</td></tr>
      </tbody>
    </table>

    <ul class="mt-2 space-y-1 text-sm">
      <li v-for="(s, i) in summary" :key="i" class="flex gap-2"><span class="text-felt font-bold">•</span><span>{{ s }}</span></li>
    </ul>
    <p class="hint mt-1.5">Each line lines a player's hands up from best to worst and shows how often each wins at showdown. A line that sits higher on the left has more very strong hands; higher across the middle means a stronger range overall.</p>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { useStore } from "../../store";

const props = defineProps<{ curves: [number, number][][]; names: string[] }>();
const store = useStore();
// one colour per seat, checked for colour-blind separation on each surface
const PALETTE = { light: ["#1F6FB5", "#B25E00", "#6B4BC4"], dark: ["#3492CF", "#C47A1E", "#937BE4"] };
const W = 560, H = 250, L = 40, R = 74, T = 8, B = 32, N = 100;
const x = (f: number) => L + f * (W - L - R);
const y = (e: number) => T + (1 - e) * (H - T - B);
const pc = (v: number) => Math.round(v * 100) + "%";
const ROWS = [
  { label: "Best tenth", from: 0, to: 0.1 }, { label: "Best quarter", from: 0, to: 0.25 },
  { label: "Middle half", from: 0.25, to: 0.75 }, { label: "Worst quarter", from: 0.75, to: 1 },
];

interface Line { i: number; name: string; color: string; pts: number[]; avg: number; strong: number; weak: number; path: string; labelY: number }
const lines = computed<Line[]>(() => {
  const pal = PALETTE[store.theme === "dark" ? "dark" : "light"];
  const out: Line[] = [];
  props.curves.forEach((c, i) => {
    const hands = c.filter((h) => h[1] > 0).sort((a, b) => b[0] - a[0]);
    const total = hands.reduce((a, h) => a + h[1], 0);
    if (!hands.length || total <= 0) return;
    const pts: number[] = []; let k = 0, cum = 0;
    for (let n = 0; n <= N; n++) {
      const target = (n / N) * total;
      while (k < hands.length - 1 && cum + hands[k][1] < target) { cum += hands[k][1]; k++; }
      pts.push(hands[k][0]);
    }
    const avg = hands.reduce((a, h) => a + h[0] * h[1], 0) / total;
    const share = (ok: (e: number) => boolean) => hands.reduce((a, h) => a + (ok(h[0]) ? h[1] : 0), 0) / total;
    out.push({ i, name: props.names[i] ?? `Player ${i + 1}`, color: pal[i % pal.length], pts, avg, strong: share((e) => e >= 0.8), weak: share((e) => e < 1 / 3),
      path: pts.map((e, n) => `${n ? "L" : "M"}${x(n / N).toFixed(1)},${y(e).toFixed(1)}`).join(""), labelY: y(pts[N]) });
  });
  // keep the end labels from sitting on top of each other
  const sorted = [...out].sort((a, b) => a.labelY - b.labelY);
  for (let n = 1; n < sorted.length; n++) if (sorted[n].labelY - sorted[n - 1].labelY < 13) sorted[n].labelY = sorted[n - 1].labelY + 13;
  const over = sorted.length ? sorted[sorted.length - 1].labelY - (H - B) : 0;
  if (over > 0) sorted.forEach((l) => (l.labelY -= over));
  return out;
});
const at = (l: Line, f: number) => l.pts[Math.round(f * N)];
const mean = (l: Line, from: number, to: number) => { const a = Math.round(from * N), b = Math.round(to * N); let s = 0; for (let n = a; n <= b; n++) s += l.pts[n]; return s / (b - a + 1); };

const summary = computed(() => {
  const ls = lines.value; if (ls.length < 2) return ls.length ? [`Only ${ls[0].name} is still in the hand.`] : [];
  const out: string[] = [];
  const byAvg = [...ls].sort((a, b) => b.avg - a.avg);
  const gap = byAvg[0].avg - byAvg[1].avg;
  out.push(gap < 0.02 ? `The ranges are about level overall: ${byAvg.map((l) => `${l.name} wins ${pc(l.avg)}`).join(", ")}.`
    : `${byAvg[0].name} has the stronger range overall, winning ${pc(byAvg[0].avg)} against ${byAvg.slice(1).map((l) => `${pc(l.avg)} for ${l.name}`).join(" and ")}.`);
  const byTop = [...ls].sort((a, b) => b.strong - a.strong);
  if (byTop[0].strong - byTop[1].strong >= 0.03) out.push(`${byTop[0].name} holds more of the very best hands: ${pc(byTop[0].strong)} of that range wins at least four times in five, against ${pc(byTop[1].strong)} for ${byTop[1].name}. That is what lets a player bet big.`);
  else out.push(`Neither side has many more of the very best hands (${byTop.map((l) => `${l.name} ${pc(l.strong)}`).join(", ")}), so big bets are harder to back up.`);
  const byWeak = [...ls].sort((a, b) => b.weak - a.weak);
  if (byWeak[0].weak - byWeak[byWeak.length - 1].weak >= 0.08) out.push(`${byWeak[0].name} carries the most weak hands (${pc(byWeak[0].weak)} win less than one time in three), which is where the folds and bluffs come from.`);
  return out;
});

const hover = ref<number | null>(null);
const table = ref(false);
function move(ev: MouseEvent) {
  const box = (ev.currentTarget as SVGElement).getBoundingClientRect();
  const px = ((ev.clientX - box.left) / box.width) * W;
  hover.value = px < L - 4 || px > W - R + 4 ? null : Math.min(1, Math.max(0, (px - L) / (W - L - R)));
}
</script>

<style scoped>
.tick { font-size: 10px; fill: rgb(var(--muted)); }
.lab { font-size: 11px; font-weight: 700; fill: rgb(var(--ink)); }
.tip { @apply absolute top-1 z-10 min-w-[11rem] rounded-lg border border-rule bg-surface px-2.5 py-1.5 text-xs shadow-lg pointer-events-none space-y-0.5; }
.hint { @apply text-xs text-muted; }
</style>
