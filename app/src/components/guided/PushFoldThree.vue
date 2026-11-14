<template>
  <div>
    <p class="text-muted mt-3 max-w-[72ch]">
      Three players left at the table: button, small blind and big blind. Each may only move all-in (or call all-in)
      or fold. Add the prizes to play for prize money rather than chips, which is what makes players call tighter near
      the money.
    </p>

    <div class="panel mt-5">
      <div class="flex flex-wrap items-end gap-5">
        <label class="field"><span>Button's stack</span><input v-model.number="stacks[0]" type="number" min="0" step="any" class="input w-28" /></label>
        <label class="field"><span>Small blind's stack</span><input v-model.number="stacks[1]" type="number" min="0" step="any" class="input w-28" /></label>
        <label class="field"><span>Big blind's stack</span><input v-model.number="stacks[2]" type="number" min="0" step="any" class="input w-28" /></label>
        <label class="field"><span>Big blind</span><input v-model.number="bb" type="number" min="0" step="any" class="input w-24" /></label>
        <label class="field"><span>Small blind</span><input v-model.number="sb" type="number" min="0" step="any" class="input w-24" /></label>
      </div>
      <div class="flex flex-wrap items-end gap-5 mt-4">
        <label class="field">
          <span>Antes</span>
          <select v-model="anteKind" class="input w-56">
            <option value="none">None</option>
            <option value="bb">Big-blind ante (big blind pays one)</option>
            <option value="each">Everyone pays an ante</option>
          </select>
        </label>
        <label v-if="anteKind !== 'none'" class="field"><span>{{ anteKind === "bb" ? "Ante size" : "Ante, each" }}</span><input v-model.number="ante" type="number" min="0" step="any" class="input w-24" /></label>
        <label class="field">
          <span>Play for</span>
          <select v-model="playFor" class="input w-44">
            <option value="prizes">Prize money</option>
            <option value="chips">Chips only</option>
          </select>
        </label>
        <label v-if="playFor === 'prizes'" class="field"><span>Prizes, first place first</span><input v-model="payoutText" class="input w-44" placeholder="50, 30, 20" /></label>
        <label v-if="playFor === 'prizes'" class="field"><span>Other players' stacks (not in this hand)</span><input v-model="othersText" class="input w-56" placeholder="none, or e.g. 1800, 2400" /></label>
      </div>
      <div class="flex flex-wrap items-center gap-4 mt-4">
        <button class="btn-go" :disabled="busy || !!problem" @click="run">{{ busy ? "Working…" : "Find the ranges" }}</button>
        <span v-if="problem" class="text-sm font-bold text-[#B42318]">{{ problem }}</span>
        <span v-else-if="busy" class="text-sm text-muted">Usually 5 to 20 seconds.</span>
        <span v-else class="text-sm text-muted">Stacks in big blinds: {{ stackBb.map((x) => x.toFixed(1)).join(" / ") }}</span>
      </div>
      <p class="hint mt-3 max-w-[85ch]">
        Use chips or big blinds, as long as stacks, blinds and antes share the unit. Stacks are counted before antes and
        blinds go in. Prizes can be percentages or money. List other stacks when more players are left than the three
        in the hand, for example a fourth player who already folded on the bubble.
      </p>
    </div>
    <p v-if="error" class="mt-3 text-sm font-bold text-[#B42318]">{{ error }}</p>

    <template v-if="res">
      <div class="panel mt-5">
        <h2 class="font-display font-extrabold text-lg">{{ res.playing_for === "chips" ? "Average result per hand" : "What each stack is worth" }}</h2>
        <div class="grid grid-cols-3 gap-4 mt-2">
          <div v-for="(n, i) in ['Button', 'Small blind', 'Big blind']" :key="n">
            <div class="text-sm text-muted">{{ n }} · {{ res.stacks_bb[i].toFixed(1) }} bb</div>
            <div class="font-display font-extrabold text-xl tabular-nums">{{ res.playing_for === "chips" ? signed(res.value[i]) + " bb" : res.value[i].toFixed(2) }}</div>
            <div v-if="res.value_before" class="text-xs text-muted tabular-nums">before the hand: {{ res.value_before[i].toFixed(2) }} ({{ signed(res.value[i] - res.value_before[i]) }})</div>
          </div>
        </div>
        <p v-if="res.value_before" class="hint mt-2">Figures are expected prize money, in the units you typed the prizes in. "Before the hand" is the value of the stack before antes and blinds; the difference is what this seat costs or earns on average.</p>
      </div>

      <div class="grid grid-cols-2 gap-5 mt-5">
        <div v-for="(d, di) in res.decisions" :key="d.id" class="panel">
          <div class="flex items-baseline justify-between gap-3">
            <h2 class="font-display font-extrabold text-base leading-snug">{{ d.title }}</h2>
            <span class="font-display font-extrabold text-xl tabular-nums">{{ (d.pct * 100).toFixed(1) }}%</span>
          </div>
          <div class="grid gap-[2px] mt-3 select-none" style="grid-template-columns: repeat(13, minmax(0, 1fr))">
            <div
              v-for="(f, i) in d.freq" :key="i"
              class="relative aspect-square rounded-[3px] overflow-hidden cursor-pointer border"
              :class="picked && picked.d === di && picked.i === i ? 'border-ink ring-2 ring-ink z-10' : 'border-transparent'"
              :title="`${res.classes[i]}: ${Math.round(f * 100)}%`"
              @click="picked = picked && picked.d === di && picked.i === i ? null : { d: di, i }"
            >
              <div class="absolute inset-0 bg-white"></div>
              <div class="absolute inset-y-0 left-0" :style="{ width: f * 100 + '%', background: COLORS[di] }"></div>
              <div class="absolute inset-0 flex items-center justify-center text-[10px] font-bold font-display" :class="f > 0.5 ? 'text-white' : 'text-muted'">{{ res.classes[i] }}</div>
            </div>
          </div>
          <p class="mt-3 text-sm min-h-[2.5rem]">
            <template v-if="picked && picked.d === di">
              <b>{{ res.classes[picked.i] }}</b>: {{ d.id.includes("jam") ? "moves all-in" : "calls" }} {{ Math.round(d.freq[picked.i] * 100) }}% of the time.
              Doing so {{ d.edge[picked.i] >= 0 ? "gains" : "loses" }} {{ edgeText(d.edge[picked.i]) }} compared with folding.
            </template>
            <span v-else class="text-muted">Click a hand to see how close the decision is.</span>
          </p>
          <details class="mt-1 text-sm">
            <summary class="cursor-pointer font-bold text-felt">Range as text</summary>
            <textarea readonly rows="3" class="input w-full mt-2 text-xs font-body" :value="d.range" @focus="($event.target as HTMLTextAreaElement).select()"></textarea>
          </details>
        </div>
      </div>

      <div class="panel mt-5 text-sm">
        <h3 class="font-bold">Reading this</h3>
        <ul class="mt-2 space-y-1.5">
          <li class="flex gap-2"><span class="text-felt font-bold">•</span><span>No player can gain more than {{ gainText }} by changing only their own ranges, so this is as close to balanced play as the model gets.</span></li>
          <li v-if="res.playing_for !== 'chips'" class="flex gap-2"><span class="text-felt font-bold">•</span><span>Prize values use the Independent Chip Model: your chance of each finishing place follows from chip counts alone. It ignores skill, position and rising blinds, and it tends to make very short stacks a little too tight.</span></li>
          <li class="flex gap-2"><span class="text-felt font-bold">•</span><span>All-in results are exact: every board was counted for every pair and trio of hands, with split pots handled as split pots.</span></li>
          <li class="flex gap-2"><span class="text-felt font-bold">•</span><span>A call is treated as all-in for the caller's whole stack. Limps and small raises are not part of this calculation.</span></li>
        </ul>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { pushfold3Solve, type PushFold3 } from "../../guided/api3";

const stacks = reactive<number[]>([12, 8, 15]);
const bb = ref(1);
const sb = ref(0.5);
const anteKind = ref<"none" | "bb" | "each">("none");
const ante = ref(1);
const playFor = ref<"prizes" | "chips">("prizes");
const payoutText = ref("50, 30, 20");
const othersText = ref("");
const res = ref<PushFold3 | null>(null);
const busy = ref(false);
const error = ref("");
const picked = ref<{ d: number; i: number } | null>(null);
const COLORS = ["#0E6B5C", "#2F5FB3", "#0E6B5C", "#2F5FB3", "#2F5FB3", "#7A4FB5"];

const nums = (t: string) => t.split(/[,\s]+/).map((x) => x.trim()).filter(Boolean).map(Number);
const stackBb = computed(() => stacks.map((x) => x / bb.value));
const problem = computed(() => {
  if (!(bb.value > 0) || !(sb.value > 0) || stacks.some((x) => !(x > 0))) return "Enter three stacks and both blinds above zero.";
  if (playFor.value === "prizes") {
    const p = nums(payoutText.value);
    if (!p.length || p.some((x) => !(x >= 0))) return "Enter the prizes as numbers, first place first.";
    if (nums(othersText.value).some((x) => !(x > 0))) return "Other stacks must be numbers above zero.";
  }
  return "";
});
const signed = (x: number) => `${x >= 0 ? "+" : "-"}${Math.abs(x).toFixed(2)}`;
const edgeText = (x: number) => (res.value?.playing_for === "chips" ? `${Math.abs(x).toFixed(2)} big blinds` : `${Math.abs(x).toFixed(2)} in prize money`);
const gainText = computed(() => {
  const r = res.value; if (!r) return "";
  const g = Math.max(...r.max_gain);
  return r.playing_for === "chips" ? `${g.toFixed(3)} big blinds a hand` : `${g.toFixed(3)} in prize money a hand`;
});

async function run() {
  busy.value = true; error.value = ""; picked.value = null;
  try {
    const a = anteKind.value === "none" ? 0 : (ante.value || 0) / bb.value;
    res.value = await pushfold3Solve({
      stacks: [stackBb.value[0], stackBb.value[1], stackBb.value[2]], small_blind: sb.value / bb.value,
      ante: anteKind.value === "each" ? a : 0, bb_ante: anteKind.value === "bb" ? a : 0,
      payouts: playFor.value === "prizes" ? nums(payoutText.value) : [], others: playFor.value === "prizes" ? nums(othersText.value).map((x) => x / bb.value) : [],
    });
  } catch (e) { error.value = String((e as Error).message ?? e); }
  finally { busy.value = false; }
}
</script>

<style scoped>
.field { @apply flex flex-col text-sm gap-1 font-bold; }
.input { @apply rounded-md border-rule text-sm font-normal focus:border-felt focus:ring-felt; }
.hint { @apply text-sm text-muted; }
.panel { @apply bg-white rounded-xl border border-rule p-4; }
.btn-go { @apply px-5 py-2 rounded-lg bg-felt text-white font-bold hover:opacity-90 disabled:opacity-50; }
button:focus-visible, input:focus-visible, select:focus-visible, textarea:focus-visible, summary:focus-visible { @apply outline-none ring-2 ring-felt ring-offset-1; }
</style>
