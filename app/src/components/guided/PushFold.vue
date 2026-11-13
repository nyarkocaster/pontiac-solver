<template>
  <div class="h-full overflow-y-auto bg-paper text-ink font-body">
    <div class="max-w-[1180px] mx-auto px-6 py-6">
      <h1 class="font-display font-extrabold text-[2rem] leading-tight tracking-tight">Heads-up push or fold</h1>
      <p class="text-muted mt-1 max-w-[70ch]">
        For short stacks heads-up: the small blind (the button) either moves all-in or folds, and the big blind calls
        or folds. These are the ranges where neither player can do better by changing their own.
      </p>

      <div class="panel mt-5 flex flex-wrap items-end gap-5">
        <label class="field"><span>Shorter stack</span><input v-model.number="stack" type="number" min="0" step="any" class="input w-28" /></label>
        <label class="field"><span>Big blind</span><input v-model.number="bb" type="number" min="0" step="any" class="input w-24" /></label>
        <label class="field"><span>Small blind</span><input v-model.number="sb" type="number" min="0" step="any" class="input w-24" /></label>
        <label class="field"><span>Ante, each</span><input v-model.number="ante" type="number" min="0" step="any" class="input w-24" /></label>
        <div class="flex flex-wrap gap-1.5 pb-0.5">
          <button v-for="n in [5, 8, 10, 12, 15, 20]" :key="n" class="pill" :class="Math.abs(stackBb - n) < 0.005 ? 'pill-on' : ''" @click="setBb(n)">{{ n }} bb</button>
        </div>
        <div class="ml-auto text-right">
          <div class="text-xs text-muted">Stack in big blinds</div>
          <div class="font-display font-extrabold text-2xl tabular-nums">{{ isFinite(stackBb) ? stackBb.toFixed(1) : "…" }}</div>
        </div>
      </div>
      <p class="hint mt-2 max-w-[80ch]">
        Use chips or big blinds, as long as all four boxes use the same unit. The stack is the smaller of the two
        stacks, counted before the blinds go in. Heads-up a sit-and-go pays the winner only, so no prize adjustment is needed.
      </p>
      <p v-if="error" class="mt-3 text-sm font-bold text-[#B42318]">{{ error }}</p>

      <div v-if="res" class="grid grid-cols-2 gap-5 mt-5">
        <div v-for="side in sides" :key="side.id" class="panel">
          <div class="flex items-baseline justify-between">
            <h2 class="font-display font-extrabold text-lg">{{ side.title }}</h2>
            <span class="font-display font-extrabold text-xl tabular-nums">{{ (side.pct * 100).toFixed(1) }}%</span>
          </div>
          <p class="hint">{{ side.help }}</p>
          <div class="grid gap-[2px] mt-3 select-none" style="grid-template-columns: repeat(13, minmax(0, 1fr))">
            <div
              v-for="(f, i) in side.freqs"
              :key="i"
              class="relative aspect-square rounded-[3px] overflow-hidden cursor-pointer border"
              :class="picked && picked.side === side.id && picked.i === i ? 'border-ink ring-2 ring-ink z-10' : 'border-transparent'"
              :title="`${res.classes[i]}: ${side.verb} ${Math.round(f * 100)}%`"
              @click="picked = picked && picked.side === side.id && picked.i === i ? null : { side: side.id, i }"
            >
              <div class="absolute inset-0 bg-white"></div>
              <div class="absolute inset-y-0 left-0" :style="{ width: f * 100 + '%', background: side.color }"></div>
              <div class="absolute inset-0 flex items-center justify-center text-[10px] font-bold font-display" :class="f > 0.5 ? 'text-white' : 'text-muted'">{{ res.classes[i] }}</div>
            </div>
          </div>
          <p class="mt-3 text-sm min-h-[2.5rem]">
            <template v-if="picked && picked.side === side.id">
              <b>{{ res.classes[picked.i] }}</b>: {{ side.verb }} {{ Math.round(side.freqs[picked.i] * 100) }}% of the time.
              {{ side.edgeWord }} {{ side.edges[picked.i] >= 0 ? "gains" : "loses" }} {{ Math.abs(side.edges[picked.i]).toFixed(2) }} big blinds compared with folding{{ Math.abs(side.edges[picked.i]) < 0.03 ? ", so it is a close decision" : "" }}.
            </template>
            <span v-else class="text-muted">Click a hand to see how close the decision is.</span>
          </p>
          <details class="mt-1 text-sm">
            <summary class="cursor-pointer font-bold text-felt">Range as text</summary>
            <textarea readonly rows="4" class="input w-full mt-2 text-xs font-body" :value="side.text" @focus="($event.target as HTMLTextAreaElement).select()"></textarea>
          </details>
        </div>
      </div>

      <div v-if="res" class="panel mt-5 text-sm">
        <h3 class="font-bold">What this means</h3>
        <ul class="mt-2 space-y-1.5">
          <li class="flex gap-2"><span class="text-felt font-bold">•</span><span>On average the small blind {{ res.ev_small_blind_bb >= 0 ? "wins" : "loses" }} {{ Math.abs(res.ev_small_blind_bb).toFixed(3) }} big blinds per hand playing this way{{ res.ev_small_blind_bb < -0.02 ? ". When that loss grows, push-or-fold is leaving money behind and smaller raises or limps start to do better; that usually begins somewhere past 10 to 13 big blinds" : "" }}.</span></li>
          <li class="flex gap-2"><span class="text-felt font-bold">•</span><span>Partly filled squares are mixed: the hand is almost break-even, so either choice is fine.</span></li>
          <li class="flex gap-2"><span class="text-felt font-bold">•</span><span>These ranges assume the other player also plays this way. Against someone who calls too tight, jam wider; against someone who calls too loose, jam tighter and stick to hands that do well when called.</span></li>
          <li class="flex gap-2"><span class="text-felt font-bold">•</span><span>Limping and small raises are not part of this calculation.</span></li>
        </ul>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { pushfoldSolve, type PushFold } from "../../guided/api3";

const stack = ref(10);
const bb = ref(1);
const sb = ref(0.5);
const ante = ref(0);
const res = ref<PushFold | null>(null);
const error = ref("");
const picked = ref<{ side: string; i: number } | null>(null);
const stackBb = computed(() => stack.value / bb.value);
function setBb(n: number) { stack.value = Math.round(n * bb.value * 1000) / 1000; }

const text = (r: PushFold, f: number[]) => r.classes.map((c, i) => (f[i] >= 1 ? c : f[i] > 0 ? `${c}:${f[i]}` : "")).filter(Boolean).join(",");
const sides = computed(() => {
  const r = res.value; if (!r) return [];
  return [
    { id: "jam", title: "Small blind moves all-in with", help: "Everything else folds.", verb: "jams", edgeWord: "Jamming", pct: r.jam_pct, freqs: r.jam, edges: r.jam_edge_bb, color: "#0E6B5C", text: text(r, r.jam) },
    { id: "call", title: "Big blind calls the all-in with", help: "Everything else folds.", verb: "calls", edgeWord: "Calling", pct: r.call_pct, freqs: r.call, edges: r.call_edge_bb, color: "#2F5FB3", text: text(r, r.call) },
  ];
});

let timer: number | undefined; let token = 0;
async function run() {
  const t = ++token;
  if (!(bb.value > 0) || !(stack.value > 0) || !(sb.value > 0)) { error.value = "Enter a stack, a big blind and a small blind above zero."; return; }
  try {
    const r = await pushfoldSolve(stackBb.value, sb.value / bb.value, (ante.value || 0) / bb.value);
    if (t === token) { res.value = r; error.value = ""; }
  } catch (e) { if (t === token) error.value = String((e as Error).message ?? e); }
}
watch([stack, bb, sb, ante], () => { window.clearTimeout(timer); timer = window.setTimeout(run, 200); });
onMounted(run);
</script>

<style scoped>
.pill { @apply px-3 py-1.5 rounded-full border border-rule bg-white text-sm hover:border-felt transition-colors; }
.pill-on { @apply border-felt bg-felt text-white hover:border-felt; }
.field { @apply flex flex-col text-sm gap-1 font-bold; }
.input { @apply rounded-md border-rule text-sm font-normal focus:border-felt focus:ring-felt; }
.hint { @apply text-sm text-muted; }
.panel { @apply bg-white rounded-xl border border-rule p-4; }
button:focus-visible, input:focus-visible, textarea:focus-visible, summary:focus-visible { @apply outline-none ring-2 ring-felt ring-offset-1; }
</style>
