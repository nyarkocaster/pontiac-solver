<template>
  <div>
    <div v-if="!bets.length" class="text-sm text-muted">No bet or raise to study here.</div>
    <template v-else>
      <div class="flex flex-wrap items-center gap-2 text-sm">
        <span>When {{ actor }} chooses</span>
        <select v-model.number="pick" class="rounded-lg border border-rule px-2 py-1 text-sm font-bold">
          <option v-for="b in bets" :key="b.index" :value="b.index">{{ b.label }}</option>
        </select>
        <button class="btn-quiet" :disabled="busy" @click="run">{{ busy ? "Working…" : "Show blockers" }}</button>
      </div>
      <p v-if="error" class="mt-2 text-sm font-bold text-danger">{{ error }}</p>
      <template v-if="result">
        <p class="text-sm mt-3">{{ opp }} folds <b>{{ pc(result.base) }}</b> of the time overall. Holding one of these cards changes that:</p>
        <div class="grid grid-cols-2 gap-x-6 mt-2 text-sm">
          <div>
            <div class="text-xs text-muted pb-1">Good cards to bluff with (more folds)</div>
            <div v-for="c in result.up" :key="c.card" class="row"><span class="font-bold" :class="SUIT_COLOR[c.card[1]]">{{ c.card[0] }}{{ SUIT_SYMBOL[c.card[1]] }}</span><span class="tabular-nums">{{ pc(c.fold) }} <span class="text-muted text-xs">({{ diff(c.fold - result.base) }})</span></span></div>
          </div>
          <div>
            <div class="text-xs text-muted pb-1">Bad cards to bluff with (fewer folds)</div>
            <div v-for="c in result.down" :key="c.card" class="row"><span class="font-bold" :class="SUIT_COLOR[c.card[1]]">{{ c.card[0] }}{{ SUIT_SYMBOL[c.card[1]] }}</span><span class="tabular-nums">{{ pc(c.fold) }} <span class="text-muted text-xs">({{ diff(c.fold - result.base) }})</span></span></div>
          </div>
        </div>
        <p v-if="result.hands.length" class="text-sm mt-3">In {{ actor }}'s own range, the hands that get the most folds: <b>{{ result.hands.map((h) => `${show(h.hand)} (${pc(h.fold)})`).join(", ") }}</b>.</p>
        <p class="hint mt-1.5">A blocker is a card in your hand that the other player then cannot hold. Holding a card from the hands they would call with means they fold more often.</p>
      </template>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { RANK_CHARS, SUIT_CHARS, SUIT_SYMBOL, SUIT_COLOR } from "../../guided/data";

interface Combo { hand: string; weight: number; freqs: number[] }
const props = defineProps<{ actor: string; opp: string; board: string[]; actions: string[]; labels: string[]; freqs: number[]; combos: Combo[]; resetKey: string;
  /** the other player's hands and replies after the chosen action */
  load: (action: number) => Promise<{ actions: string[]; combos: Combo[] }> }>();
const bets = computed(() => props.actions.map((a, index) => ({ a, index, label: props.labels[index] })).filter((b) => /^(Bet|Raise|All-in)/.test(b.a)));
const pick = ref(-1); const busy = ref(false); const error = ref("");
interface Result { base: number; up: { card: string; fold: number }[]; down: { card: string; fold: number }[]; hands: { hand: string; fold: number }[] }
const result = ref<Result | null>(null);
watch(() => props.resetKey, () => { result.value = null; error.value = ""; pick.value = [...bets.value].sort((a, b) => props.freqs[b.index] - props.freqs[a.index])[0]?.index ?? -1; }, { immediate: true });
watch(pick, () => (result.value = null));
const pc = (v: number) => (v * 100).toFixed(1) + "%";
const diff = (v: number) => (v >= 0 ? "+" : "−") + Math.abs(v * 100).toFixed(1);
const show = (h: string) => h[0] + SUIT_SYMBOL[h[1]] + h[2] + SUIT_SYMBOL[h[3]];
async function run() {
  busy.value = true; error.value = "";
  try {
    const v = await props.load(pick.value);
    const f = v.actions.findIndex((a) => a.startsWith("Fold"));
    if (!v.combos.length) throw new Error("That action is never taken here, so there is nothing to measure. Pick another.");
    if (f < 0) throw new Error("The other player has no fold to make after that action.");
    const foldIf = (dead: string[]) => { let w = 0, fo = 0; for (const c of v.combos) { const a = c.hand.slice(0, 2), b = c.hand.slice(2, 4); if (dead.includes(a) || dead.includes(b)) continue; w += c.weight; fo += c.weight * c.freqs[f]; } return w > 0 ? fo / w : 0; };
    const base = foldIf([]);
    const cards: { card: string; fold: number }[] = [];
    for (const r of RANK_CHARS) for (const s of SUIT_CHARS) { const card = r + s; if (!props.board.includes(card)) cards.push({ card, fold: foldIf([card]) }); }
    cards.sort((a, b) => b.fold - a.fold);
    const hands = props.combos.filter((c) => c.weight > 0.001).map((c) => ({ hand: c.hand, fold: foldIf([c.hand.slice(0, 2), c.hand.slice(2, 4)]) })).sort((a, b) => b.fold - a.fold).slice(0, 5);
    result.value = { base, up: cards.slice(0, 5), down: cards.slice(-5).reverse(), hands };
  } catch (e) { error.value = String((e as Error).message ?? e); }
  finally { busy.value = false; }
}
</script>

<style scoped>
.row { @apply flex justify-between border-t border-rule py-1; }
.hint { @apply text-xs text-muted; }
.btn-quiet { @apply px-3 py-1 rounded-lg border border-rule bg-surface text-sm font-bold hover:border-felt disabled:opacity-50; }
</style>
