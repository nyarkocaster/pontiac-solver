<template>
  <div>
    <p class="text-muted mt-3 max-w-[78ch]">
      Heads-up before the flop with every option open: the small blind (the button) can fold, limp, raise or move all-in, and the big blind
      can check, call, re-raise or move all-in. Use it for stacks where push-or-fold is too blunt, roughly 12 big blinds and up.
    </p>
    <div class="panel mt-4 flex flex-wrap items-end gap-x-6 gap-y-3">
      <label class="block"><span class="lbl">Shorter stack (big blinds)</span><input v-model.number="stack" type="number" min="2" max="250" step="1" class="input w-28" /></label>
      <div class="flex gap-1.5">
        <button v-for="n in [12, 15, 20, 25, 30, 40]" :key="n" class="pill" :class="stack === n ? 'pill-on' : ''" @click="stack = n">{{ n }} bb</button>
      </div>
      <div>
        <span class="lbl">Button may open to</span>
        <div class="flex gap-1.5">
          <button v-for="s in [2, 2.5, 3]" :key="s" class="pill" :class="opens.includes(s) ? 'pill-on' : ''" :aria-pressed="opens.includes(s)" @click="toggleOpen(s)">{{ s }} bb</button>
        </div>
      </div>
      <label class="flex items-center gap-2 text-sm pb-2"><input v-model="limp" type="checkbox" class="rounded text-felt" />Button may limp</label>
      <label class="block"><span class="lbl">Re-raise size</span>
        <select v-model.number="reraise" class="input w-36"><option :value="2.5">2.5 × the raise</option><option :value="3">3 × the raise</option><option :value="3.5">3.5 × the raise</option></select>
      </label>
      <label class="block"><span class="lbl">Ante, each</span><input v-model.number="ante" type="number" min="0" step="0.05" class="input w-24" /></label>
      <button class="btn-solve" :disabled="busy || !opens.length && !limp" @click="run">{{ busy ? "Solving…" : res ? "Solve again" : "Solve" }}</button>
    </div>
    <div class="mt-3 flex flex-wrap items-center gap-x-4 gap-y-2 text-sm">
      <label class="flex items-center gap-2" :class="hasSolved ? '' : 'text-muted'" :title="hasSolved ? '' : 'Ready-made accurate solutions exist for ' + SOLVED_STACKS.join(', ') + ' big blinds with the standard options (open to 2, limps allowed, 3 × re-raise, no ante)'">
        <input v-model="useSolved" type="checkbox" class="rounded text-felt" :disabled="!hasSolved" />Use the accurate solution for this stack (flops solved, not estimated)
      </label>
      <label class="text-felt font-bold underline cursor-pointer">Open a result file<input type="file" accept=".json,application/json" class="hidden" @change="openFile" /></label>
    </div>
    <p v-if="error" class="mt-3 text-sm font-bold text-danger">{{ error }}</p>

    <template v-if="res">
      <div v-if="res.accurate" class="mt-4 rounded-xl border border-felt bg-felt-light p-3 text-sm">
        <b>Accurate solution.</b> All-in pots are exact, and the common lines that see a flop take their values from real flop solves
        ({{ res.accurate[0]?.lines[0]?.flops ?? 0 }} flops per line, {{ res.accurate.length }} rounds of feeding the results back).
        The flop solves use one bet size per street and a sample of flops, so read differences of a few points as noise.
        <details class="mt-1"><summary class="cursor-pointer font-bold">How it settled, round by round</summary>
          <table class="mt-1 text-xs tabular-nums"><tbody>
            <tr v-for="p in res.accurate" :key="p.pass"><td class="pr-3 py-0.5 align-top">Round {{ p.pass }}</td><td>{{ p.root_actions.map((a, i) => `${a.toLowerCase()} ${pc(p.root_freqs[i])}`).join(", ") }}<span class="text-muted"> · {{ p.lines.map((l) => `${plain(l.path)}: button's share ${pc(l.share_before)} going in, flops gave ${pc(l.share_solved)}`).join("; ") }}</span></td></tr>
          </tbody></table>
        </details>
      </div>
      <div v-else class="mt-4 rounded-xl border border-mine bg-mine-light p-3 text-sm">
        <b>How far to trust this.</b> Pots where all the chips go in before the flop are exact. Pots that see a flop are estimated, not solved:
        each hand gets its all-in share of the pot, shifted toward the button (who acts last), the last raiser, and hands that play well after the flop.
        Use these ranges as a strong starting point, and solve the flops that matter with the buttons under "Ways to see a flop".
      </div>
      <p class="text-sm text-muted mt-3">
        Button averages <b class="text-ink">{{ signed(res.ev_button_bb) }} bb</b> a hand, big blind <b class="text-ink">{{ signed(res.ev_big_blind_bb) }} bb</b>.
        Inside this model neither player can gain more than {{ res.max_gain_bb.toFixed(3) }} bb by changing strategy.
      </p>

      <div class="grid gap-5 mt-4 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)] items-start">
        <section class="panel">
          <div class="flex flex-wrap items-center gap-1.5 text-sm">
            <button class="crumb" :class="trail.length ? '' : 'crumb-on'" @click="trail = []">Start</button>
            <template v-for="(t, k) in trail" :key="k"><span class="text-muted">›</span><button class="crumb" :class="k === trail.length - 1 ? 'crumb-on' : ''" @click="trail = trail.slice(0, k + 1)">{{ t.label }}</button></template>
          </div>
          <h2 class="font-display font-extrabold text-xl mt-3">{{ node.situation }}</h2>
          <p class="text-sm text-muted">Happens in {{ pc(node.reach) }} of hands. Click an action to follow it.</p>
          <div class="mt-3 space-y-1.5">
            <component :is="node.next[a] >= 0 ? 'button' : 'div'" v-for="(act, a) in node.actions" :key="a" class="action-row" :class="node.next[a] >= 0 ? 'action-go' : ''" @click="go(a)">
              <span class="w-3.5 h-3.5 rounded-sm shrink-0" :style="{ background: colors[a] }"></span>
              <span class="w-36 font-bold text-left">{{ act }}{{ act === "All-in" ? ` (${node.amounts_bb[a]} bb)` : "" }}</span>
              <span class="flex-1 h-2.5 bg-paper rounded-full overflow-hidden"><span class="block h-full rounded-full" :style="{ width: node.freqs[a] * 100 + '%', background: colors[a] }"></span></span>
              <span class="w-12 text-right tabular-nums font-bold">{{ pc(node.freqs[a]) }}</span>
              <span class="w-24 text-right text-xs text-muted">{{ node.next[a] >= 0 ? "other player acts ›" : ends(act) }}</span>
            </component>
          </div>
          <ul class="mt-3 space-y-1 text-sm">
            <li v-for="(t, i) in notes" :key="i" class="flex gap-2"><span class="text-felt font-bold">•</span><span>{{ t }}</span></li>
          </ul>
        </section>

        <section class="panel">
          <div class="flex flex-wrap items-center justify-between gap-2">
            <h3 class="font-bold">{{ node.player }}'s hands here</h3>
            <ul class="flex flex-wrap gap-x-3 gap-y-1 text-xs text-muted" aria-label="Colour key">
              <li v-for="(act, a) in node.actions" :key="a" class="flex items-center gap-1"><span class="w-2.5 h-2.5 rounded-sm" :style="{ background: colors[a] }"></span>{{ act }}</li>
            </ul>
          </div>
          <div class="grid gap-[2px] mt-2 select-none" style="grid-template-columns: repeat(13, minmax(0, 1fr))">
            <div v-for="(name, i) in res.classes" :key="i" class="relative aspect-square rounded-[3px] overflow-hidden bg-paper" :title="tip(i)">
              <div v-if="node.range[i] > 0.005" class="absolute inset-x-0 bottom-0 flex" :style="{ height: Math.max(6, node.range[i] * 100) + '%' }">
                <div v-for="(_act, a) in node.actions" :key="a" :style="{ width: node.strategy[a][i] * 100 + '%', background: colors[a] }"></div>
              </div>
              <div class="absolute inset-0 flex items-center justify-center text-[10px] font-bold font-display" :class="node.range[i] > 0.5 ? 'text-[#1B1417]' : node.range[i] > 0.005 ? 'text-ink' : 'text-muted opacity-60'" :style="node.range[i] > 0.5 ? 'text-shadow: 0 0 2px #fff, 0 0 2px #fff' : ''">{{ name }}</div>
            </div>
          </div>
          <p class="hint mt-1.5">Split colours mean the hand mixes. The fill height shows how much of the hand is still in the range at this point.</p>
        </section>
      </div>

      <section class="panel mt-5">
        <h3 class="font-bold">Ways to see a flop</h3>
        <p class="hint">Each line with both players' ranges. "Study on a flop" loads it into Study a spot, where the flop is solved properly.</p>
        <div class="overflow-x-auto mt-2">
          <table class="w-full text-sm tabular-nums">
            <thead><tr class="text-left text-xs text-muted"><th class="font-normal py-1">Line</th><th class="font-normal text-right">How often</th><th class="font-normal text-right">Pot</th><th class="font-normal text-right">Behind</th><th class="font-normal text-right" title="The model's estimate of the button's share of the pot">Button's share</th><th></th></tr></thead>
            <tbody>
              <tr v-for="l in res.lines" :key="l.path" class="border-t border-rule">
                <td class="py-1.5">{{ plain(l.path) }} <span v-if="res.accurate" class="text-xs" :class="l.solved ? 'text-felt font-bold' : 'text-muted'">{{ l.solved ? "solved" : "estimated" }}</span></td><td class="text-right">{{ pc(l.freq) }}</td><td class="text-right">{{ l.pot_bb }} bb</td><td class="text-right">{{ l.behind_bb }} bb</td><td class="text-right">{{ pc(l.button_share) }}</td>
                <td class="text-right whitespace-nowrap">
                  <button class="btn-quiet" @click="study(l)">Study on a flop</button>
                  <button class="text-xs text-felt font-bold underline ml-2" @click="copy(l)">{{ copied === l.path ? "Copied" : "Copy ranges" }}</button>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { preflopSolve, type Preflop, type FlopLine } from "../../guided/api3";
import { useStore } from "../../store";

const store = useStore();
const stack = ref(20); const opens = ref<number[]>([2]); const limp = ref(true); const reraise = ref(3); const ante = ref(0);
const res = ref<Preflop | null>(null); const busy = ref(false); const error = ref("");
const trail = ref<{ id: number; label: string }[]>([]);
const toggleOpen = (s: number) => { opens.value = opens.value.includes(s) ? opens.value.filter((x) => x !== s) : [...opens.value, s].sort((a, b) => a - b); };
// ready-made accurate solutions, made with `pontiac preflop --accurate` (hours of flop solves each)
const SOLVED = import.meta.glob("../../guided/preflopSolved/*.json");
const SOLVED_STACKS = Object.keys(SOLVED).map((k) => Number(k.match(/(\d+)\.json$/)?.[1])).filter((n) => n > 0).sort((a, b) => a - b);
const hasSolved = computed(() => SOLVED_STACKS.includes(stack.value) && opens.value.join() === "2" && limp.value && reraise.value === 3 && !ante.value);
const useSolved = ref(true);
function openFile(ev: Event) {
  const f = (ev.target as HTMLInputElement).files?.[0]; if (!f) return;
  f.text().then((t) => { const r = JSON.parse(t) as Preflop; if (!r.nodes?.length || !r.classes) throw new Error(); res.value = r; stack.value = r.stack_bb; trail.value = []; error.value = ""; })
    .catch(() => { error.value = "That file is not a preflop result from Pontiac."; });
  (ev.target as HTMLInputElement).value = "";
}
async function run() {
  busy.value = true; error.value = "";
  if (hasSolved.value && useSolved.value) {
    try { const key = Object.keys(SOLVED).find((k) => k.endsWith(`/${stack.value}.json`))!; res.value = ((await SOLVED[key]()) as { default: Preflop }).default; trail.value = []; }
    catch (e) { error.value = String((e as Error).message ?? e); }
    busy.value = false; return;
  }
  try { res.value = await preflopSolve({ stack: stack.value, open_sizes: opens.value, limp: limp.value, reraise: reraise.value, ante: ante.value, small_blind: 0.5 }); trail.value = []; }
  catch (e) { error.value = String((e as Error).message ?? e); }
  finally { busy.value = false; }
}
const node = computed(() => res.value!.nodes[trail.value.length ? trail.value[trail.value.length - 1].id : 0]);
const PALETTE: Record<string, string> = { Fold: "#7FA8E0", Check: "#6FCF97", Call: "#6FCF97", Limp: "#9BD6B4", "All-in": "#B4232F" };
const colors = computed(() => { let k = 0; return node.value.actions.map((a) => PALETTE[a] ?? ["#F2A65A", "#E8743B"][k++ % 2]); });
const pc = (v: number) => (v > 0 && v < 0.005 ? "<1%" : Math.round(v * 100) + "%");
const signed = (v: number) => (v >= 0 ? "+" : "−") + Math.abs(v).toFixed(2);
const ends = (a: string) => (a === "Fold" ? "hand over" : a === "Check" || node.value.amounts_bb[node.value.actions.indexOf(a)] < stack.value - ante.value - 1e-6 ? "to the flop" : "cards on their backs");
function go(a: number) { const id = node.value.next[a]; if (id < 0) return; trail.value = [...trail.value, { id, label: `${node.value.player === "Small blind" ? "Button" : "Big blind"}: ${node.value.actions[a].toLowerCase()}` }]; }
const tip = (i: number) => {
  const n = node.value; if (n.range[i] <= 0.005) return `${res.value!.classes[i]}: not in range here`;
  return `${res.value!.classes[i]}: ` + n.actions.map((a, k) => (n.strategy[k][i] >= 0.005 ? `${a.toLowerCase()} ${Math.round(n.strategy[k][i] * 100)}%` : "")).filter(Boolean).join(", ");
};
const notes = computed(() => {
  const n = node.value; const out: string[] = [];
  const top = n.freqs.indexOf(Math.max(...n.freqs));
  out.push(`${n.player} mostly ${n.actions[top].toLowerCase()}s here (${pc(n.freqs[top])}).`.replace("all-ins", "moves all-in").replace("checks", "checks").replace(/re-raise to ([\d.]+)s/, "re-raises to $1").replace(/raise to ([\d.]+)s/, "raises to $1"));
  const jam = n.actions.indexOf("All-in");
  if (jam >= 0 && n.freqs[jam] >= 0.02) {
    const hands = res.value!.classes.map((c, i) => ({ c, w: n.range[i] * n.strategy[jam][i] })).filter((x) => x.w > 0.6).map((x) => x.c);
    if (hands.length) out.push(`All-in hands include ${hands.slice(0, 10).join(", ")}${hands.length > 10 ? " and more" : ""}.`);
  }
  return out;
});
const plain = (p: string) => p.replace(/SB/g, "Button").replace(/BB/g, "big blind").replace(/ > /g, ", ").replace(/raise ([\d.]+)/g, "raises to $1").replace(/\blimp\b/g, "limps").replace(/\bcheck\b/g, "checks").replace(/\bcall\b/g, "calls");
const copied = ref("");
async function copy(l: FlopLine) { try { await navigator.clipboard.writeText(`Button: ${l.button_range}\nBig blind: ${l.big_blind_range}`); copied.value = l.path; } catch { /* clipboard not available */ } }
function study(l: FlopLine) { store.preflopLine = { stack: stack.value, ante: ante.value, path: l.path, button: l.button_range, bigBlind: l.big_blind_range }; store.navView = "builder"; }
</script>

<style scoped>
.panel { @apply bg-surface border border-rule rounded-xl p-4; }
.lbl { @apply block text-xs font-bold text-muted mb-1; }
.input { @apply rounded-lg border border-rule bg-surface px-3 py-2 text-sm; }
.hint { @apply text-xs text-muted; }
.pill { @apply px-3 py-1.5 rounded-full border border-rule bg-surface text-sm hover:border-felt transition-colors; }
.pill-on { @apply border-felt bg-felt text-white hover:border-felt; }
.btn-solve { @apply px-6 py-2 rounded-lg bg-felt text-white font-bold disabled:opacity-50; }
.btn-quiet { @apply px-3 py-1 rounded-lg border border-rule bg-surface text-sm font-bold hover:border-felt; }
.crumb { @apply px-2 py-0.5 rounded border border-rule; }
.crumb-on { @apply border-felt bg-felt text-white; }
.action-row { @apply w-full flex items-center gap-3 px-3 py-2 rounded-lg border border-rule bg-surface text-sm; }
.action-go { @apply hover:border-felt cursor-pointer; }
</style>
