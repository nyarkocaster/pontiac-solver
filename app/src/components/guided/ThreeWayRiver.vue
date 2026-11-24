<template>
  <div class="h-full overflow-y-auto bg-paper text-ink font-body">
    <div class="mx-auto px-6 py-6" :class="step === 'results' ? 'max-w-[1680px]' : 'max-w-[1180px]'">
      <!-- nothing solved yet -->
      <div v-if="step === 'setup'" class="max-w-xl mx-auto mt-24 text-center">
        <h1 class="font-display font-extrabold text-2xl">No result yet</h1>
        <p class="text-muted mt-2">This page shows a solved spot: what each player does, hand by hand. Set a spot up first and it opens here when the solve finishes.</p>
        <button class="btn-solve mt-5 !w-auto px-6" @click="emit('new')">Study a spot</button>
      </div>

      <!-- ================= RESULTS ================= -->
      <div v-if="step === 'results' && node && summary">
        <div class="flex items-center justify-between gap-4">
          <button class="btn-quiet" @click="newSpot">New spot</button>
          <button class="btn-quiet ml-2" :disabled="saveState === 'busy'" title="Keeps this solve, so it opens again without solving." @click="saveSpot">{{ saveState === "done" ? "Saved to library ✓" : "Save to library" }}</button>
          <span v-if="saveError" class="ml-2 text-xs font-bold text-danger">{{ saveError }}</span>
          <div v-if="summary.locked" class="seg" role="group" aria-label="Which solution to show">
            <button :class="which === 'base' ? 'seg-on' : ''" @click="setWhich('base')">Normal strategy</button>
            <button :class="which === 'locked' ? 'seg-on' : ''" @click="setWhich('locked')">{{ lockedLabel }}</button>
          </div>
        </div>

        <div v-if="summary.icm" class="rounded-xl border border-felt bg-felt-light p-3 text-sm mt-4">
          <b>Solved for prize money.</b>
          After this spot {{ players[heroSolved].name === "You" || players[heroSolved].name.startsWith("You ") ? "you hold" : players[heroSolved].name + " holds" }}
          <b>{{ (which === "locked" && summary.locked ? summary.icm.locked_share : summary.icm.base_share)[heroSolved].toFixed(2) }}%</b> of the prize pool on average,
          against {{ summary.icm.split[heroSolved].toFixed(2) }}% if the pot were shared out now.
          <span class="text-muted">Others: {{ [0, 1, 2].filter((i) => i !== heroSolved).map((i) => `${players[i].name} ${(which === "locked" && summary!.locked ? summary!.icm!.locked_share : summary!.icm!.base_share)[i].toFixed(2)}%`).join(", ") }}.</span>
          Amounts won and lost on this screen are prize money in chip-sized units, so the three shares of the pot add up to less than the pot: players outside the hand gain when these three clash, and a player who folds still gains or loses by how the others finish.
        </div>

        <div class="panel mt-4 flex flex-wrap items-center gap-6">
          <div>
            <div class="text-xs text-muted">Pot now</div>
            <div class="font-display font-extrabold text-2xl tabular-nums">{{ money(node.pot) }}</div>
          </div>
          <div class="flex-1 grid grid-cols-3 gap-2">
            <div v-for="(p, i) in players" :key="i" class="rounded-lg border px-3 py-2" :class="node.kind === 'player' && node.player === i ? 'border-felt bg-felt-light' : 'border-rule'">
              <div class="flex items-center justify-between gap-2">
                <span class="font-bold truncate">{{ p.name }}</span>
                <span v-if="i === heroSolved" class="text-xs font-bold bg-mine-light border border-mine rounded px-1">you</span>
              </div>
              <div v-if="node.folded[i]" class="text-sm text-muted">Folded</div>
              <div v-else class="text-sm tabular-nums">
                <span title="Average amount of the pot this player ends up with">EV {{ node.has_ev ? money1(node.ev[i]) : "…" }}</span>
                <span class="text-muted"> · wins {{ pct(node.equity[i]) }} at showdown</span>
              </div>
              <div class="text-xs text-muted tabular-nums">{{ node.range_combos[i].toFixed(0) }} hands left<span v-if="node.invested[i] > 0"> · {{ money(node.invested[i]) }} in</span><span v-if="!node.folded[i]"> · {{ node.stacks[i] > 0.5 ? money(node.stacks[i]) + " behind" : "all-in" }}</span></div>
            </div>
          </div>
        </div>

        <!-- line so far -->
        <ActionBar class="mt-3" :start="['Flop', 'Turn', 'River'][startLen - 3]" :board="node.board" :steps="steps" :now="nowText" :pot="money(node.pot)" @goto="goto">
          <span v-if="node.exact" class="self-center ml-2 text-xs font-bold bg-felt-light border border-felt rounded px-1.5 py-0.5 whitespace-nowrap" title="This turn card was solved on its own with every river card, starting from the ranges the flop solve sends here">exact turn solve</span>
        </ActionBar>

        <div class="grid grid-cols-12 gap-5 mt-4">
          <div class="col-span-5 min-[1500px]:col-span-7 space-y-4 min-[1500px]:space-y-0 min-[1500px]:columns-2 min-[1500px]:gap-4 [&>*]:break-inside-avoid min-[1500px]:[&>*]:mb-4">
            <div v-if="node.kind === 'player'" class="panel">
              <h2 class="font-display font-extrabold text-lg">
                {{ players[node.player].name }} to act
                <span v-if="node.locked" class="ml-1 align-middle text-xs font-body font-bold bg-mine-light border border-mine rounded px-1.5 py-0.5">{{ lockedHere ? "locked" : "habit applied" }}</span>
              </h2>
              <p class="hint">Click an action to follow that line.</p>
              <div class="mt-3 space-y-2">
                <button v-for="(a, i) in node.actions" :key="i" class="action-row" @click="play(i)">
                  <span class="w-3 h-8 rounded" :style="{ background: colors[i] }"></span>
                  <span class="font-bold w-28 text-left">{{ actionText(a) }}</span>
                  <span class="flex-1 h-2 bg-paper rounded overflow-hidden"><span class="block h-full" :style="{ width: node.action_freqs[i] * 100 + '%', background: colors[i] }"></span></span>
                  <span class="w-12 text-right font-bold tabular-nums">{{ pct1(node.action_freqs[i]) }}</span>
                  <span v-if="other && other.kind === 'player'" class="w-16 text-right text-xs text-muted tabular-nums" :title="which === 'base' ? (manualLocks.length ? 'With the locks' : 'Against the habit') : 'Normal strategy'">
                    {{ which === "base" ? (manualLocks.length ? "locked" : "habit") : "normal" }} {{ pct1(other.action_freqs[i] ?? 0) }}
                  </span>
                </button>
              </div>
              <ul class="mt-4 space-y-1.5 text-sm">
                <li v-for="(n, i) in notes" :key="i" class="flex gap-2"><span class="text-felt font-bold">•</span><span>{{ n }}</span></li>
              </ul>
              <details class="mt-3 rounded-lg border border-rule px-3 py-2" :open="lockOpen" @toggle="lockOpen = ($event.target as HTMLDetailsElement).open">
                <summary class="cursor-pointer text-sm font-bold">Change how {{ players[node.player].name }} plays here (node lock){{ manualLocks.length ? ` · ${manualLocks.length} set` : "" }}</summary>
                <div v-if="lockOpen" class="mt-2 text-sm">
                  <p class="hint">Fix what this player does at this exact point, then solve again to see how the other two should adjust. Use it for reads like "this player never raises here".</p>
                  <p v-if="node.exact" class="hint mt-1 font-bold">This point is inside an exact turn solve. Locks are set in the main solve: go back to the flop to add one.</p>
                  <template v-else>
                    <div class="seg mt-2" role="group" aria-label="Which hands the lock covers">
                      <button :class="lockScope === 'all' ? 'seg-on' : ''" @click="lockScope = 'all'">Every hand</button>
                      <button :class="lockScope === 'cell' ? 'seg-on' : ''" :disabled="selected === null" :title="selected === null ? 'Click a square in the chart first' : ''" @click="lockScope = 'cell'">Only {{ selected !== null ? cellName(selected) : "the square I click" }}</button>
                    </div>
                    <div class="mt-2 space-y-1">
                      <label v-for="(a, i) in node.actions" :key="i" class="flex items-center gap-2">
                        <span class="w-3 h-3 rounded-sm shrink-0" :style="{ background: colors[i] }"></span>
                        <span class="w-40">{{ actionText(a) }}</span>
                        <input v-model.number="lockPct[i]" type="number" min="0" max="100" step="5" class="input !w-20 !py-1 text-right" :aria-label="actionText(a) + ' percent'" /><span>%</span>
                      </label>
                    </div>
                    <div class="mt-2 flex items-center gap-3">
                      <button class="btn-quiet" :disabled="lockTotal !== 100" @click="addLock">Add this lock</button>
                      <span class="text-xs" :class="lockTotal === 100 ? 'text-muted' : 'text-danger font-bold'">Total {{ lockTotal }}%{{ lockTotal === 100 ? "" : " (must be 100)" }}</span>
                    </div>
                  </template>
                  <ul v-if="manualLocks.length" class="mt-3 space-y-1">
                    <li v-for="(l, i) in manualLocks" :key="i" class="flex items-start justify-between gap-2 border-t border-rule pt-1">
                      <span>{{ l.label }}</span>
                      <button class="text-xs text-danger underline shrink-0" :disabled="relocking" @click="removeLock(i)">Remove</button>
                    </li>
                  </ul>
                  <div v-if="manualLocks.length || lockStale" class="mt-3">
                    <button class="btn-solve !w-auto px-5 !py-2" :disabled="relocking || !manualLocks.length" @click="relockNow">{{ relocking ? `Solving again… step ${progress[1]} of up to ${progress[2]}` : `Solve again with ${manualLocks.length} lock${manualLocks.length === 1 ? "" : "s"}` }}</button>
                    <button v-if="relocking" class="ml-2 text-xs underline" @click="api.mwCancel()">Stop</button>
                    <p v-if="lockStale && !relocking" class="hint mt-1">The locks changed since the last solve. Solve again to see their effect.</p>
                    <p v-if="lockError" class="text-sm font-bold text-danger mt-1">{{ lockError }}</p>
                  </div>
                </div>
              </details>
            </div>
            <div v-else-if="node.kind === 'chance'" class="panel">
              <h2 class="font-display font-extrabold text-lg">Deal the {{ node.street === "flop" ? "turn" : "river" }}</h2>
              <p class="hint">The {{ node.street }} betting is over. Pick the next card to see how everyone plays it.</p>
              <div class="mt-3 grid gap-[3px]" style="grid-template-columns: repeat(13, minmax(0, 1fr))" role="group" :aria-label="node.street === 'flop' ? 'Choose the turn card' : 'Choose the river card'">
                <template v-for="s in [3, 2, 1, 0]" :key="s">
                  <button
                    v-for="r in RANKS_DESC"
                    :key="r * 4 + s"
                    class="deck-btn !h-8 !text-[11px] !px-0"
                    :class="[SUIT_COLOR[SUIT_CHARS[s]], canExact && !dealable.has(r * 4 + s) && !onBoard.has(r * 4 + s) ? 'deck-extra' : '', exactCard === r * 4 + s ? 'deck-done' : '']"
                    :disabled="resolving || onBoard.has(r * 4 + s) || (!dealable.has(r * 4 + s) && !canExact)"
                    :title="cardTitle(r * 4 + s)"
                    @click="deal(r * 4 + s)"
                  >
                    {{ RANK_CHARS[r] }}{{ SUIT_SYMBOL[SUIT_CHARS[s]] }}
                  </button>
                </template>
              </div>
              <template v-if="!resolving && node.cards.length > 1">
                <h3 class="font-bold mt-4">Which cards matter</h3>
                <CardReport class="mt-2" :cards="node.cards" :names="players.map((p) => p.name)" :hero="heroSolved" :street="node.street === 'flop' ? 'turn' : 'river'" :reset-key="which + ':' + history.join(',')" :load="loadCard" @pick="deal($event)" />
              </template>
              <div v-if="canExact" class="mt-3 text-sm">
                <label class="flex items-start gap-2 font-bold">
                  <input v-model="exactMode" type="checkbox" class="mt-0.5 rounded text-felt focus:ring-felt" :disabled="resolving" />
                  <span>Solve the card I pick exactly <span class="font-normal text-muted">(every river card, starting from the ranges that reach this point; usually a minute or more)</span></span>
                </label>
                <p v-if="node.cards.length < node.cards_possible" class="hint mt-1.5">
                  Cards with a dashed edge were not in the flop solve's sample. Picking one always solves it exactly.
                </p>
              </div>
              <div v-if="resolving" class="mt-3">
                <div class="flex justify-between text-sm font-bold">
                  <span>Solving this turn card exactly…</span>
                  <button class="text-felt underline" @click="api.mwCancel()">Stop early</button>
                </div>
                <div class="h-2 bg-paper rounded mt-1.5 overflow-hidden"><div class="h-full bg-felt transition-all" :style="{ width: progressPct + '%' }"></div></div>
                <p class="hint mt-1.5">Step {{ progress[1] }} of up to {{ progress[2] }}, {{ elapsedText }} so far. Best single-player improvement left: {{ progress[3] >= 100 ? "..." : progress[3].toFixed(2) + "% of the pot" }}</p>
              </div>
              <p v-if="resolveError" class="mt-2 text-sm font-bold text-danger">{{ resolveError }}</p>
            </div>
            <div v-else class="panel">
              <h2 class="font-display font-extrabold text-lg">{{ node.result === "fold" ? `${players[node.winner].name} wins the pot` : "Showdown" }}</h2>
              <p class="mt-1 text-sm">
                {{ node.result === "fold" ? "Everyone else folded." : `${stillIn} show their hands. The EV figures above are each player's average share of the ${money(node.pot)} pot.` }}
              </p>
            </div>

            <div v-if="node.kind === 'player'" class="panel">
              <h3 class="font-bold">By hand strength</h3>
              <table class="w-full text-sm mt-2">
                <thead><tr class="text-left text-xs text-muted"><th class="font-normal">Group</th><th class="font-normal text-right">Share</th><th class="font-normal pl-3">What they do</th></tr></thead>
                <tbody>
                  <tr v-for="b in node.buckets" :key="b.name" class="border-t border-rule">
                    <td class="py-1.5">{{ b.name }}</td>
                    <td class="text-right tabular-nums">{{ pct(b.share) }}</td>
                    <td class="pl-3">
                      <div class="flex h-3 rounded overflow-hidden bg-paper" :title="mixText(b.freqs)">
                        <div v-for="(f, a) in b.freqs" :key="a" :style="{ width: f * 100 + '%', background: colors[a] }"></div>
                      </div>
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>

            <div v-if="node.kind === 'chance'" class="panel text-sm">
              <h3 class="font-bold">Ranges going to the {{ node.street === "flop" ? "turn" : "river" }}</h3>
              <p class="mt-1">The solver has already removed the hands that folded or took a different line. Each player's remaining hand count is shown above.</p>
              <p v-if="node.cards.length < node.cards_possible" class="mt-2">
                This solve covered {{ node.cards.length }} of the {{ node.cards_possible }} cards that could come.
                <template v-if="canExact">Any other card can be solved on the spot.</template>
                <template v-else>The greyed-out cards were not solved; pick the same rank in another suit, which plays almost the same unless it changes a flush.</template>
              </p>
            </div>
            <div v-if="node.curves && node.curves.some((c) => c.length)" class="panel">
              <h3 class="font-bold">Who has the stronger range</h3>
              <EquityGraph class="mt-2" :curves="node.curves" :names="players.map((p) => p.name)" />
            </div>
            <div v-if="node.kind === 'player'" class="panel">
              <h3 class="font-bold">{{ players[node.player].name }}'s play by type of hand</h3>
              <HandTypes class="mt-2" :combos="node.combos" :board="node.board" :actions="node.actions.map((x) => actionText(x))" :colors="colors" />
            </div>
            <div class="panel text-sm">
              <h3 class="font-bold">How good is this solution?</h3>
              <p class="mt-1">
                Best gain any single player could get by changing their own strategy:
                <b>{{ worstGain.toFixed(2) }}% of the pot</b> ({{ worstGain <= 0.15 ? "very close" : worstGain <= 0.5 ? "close enough to study" : "rough, solve longer" }}).
                Solved in {{ (which === "base" ? summary.base_secs : summary.locked_secs).toFixed(0) }}s.
              </p>
              <p v-if="node.exact && exactSummary" class="mt-1.5">
                This turn card was then solved exactly: <b>{{ Math.max(0, ...(exactSummary.locked ? exactSummary.locked_gain_pct : exactSummary.base_gain_pct)).toFixed(2) }}% of the pot</b> left, in {{ (exactSummary.base_secs + exactSummary.locked_secs).toFixed(0) }}s.
              </p>
              <p class="hint mt-1.5">
                With three players there can be more than one sound solution, and two opponents' choices can help or hurt
                the third. Treat exact percentages as a guide, and look for the patterns.
              </p>
              <p v-if="summary.locked" class="mt-2">
                Average result with normal play: {{ evLine(summary.base_ev) }}.<br />
                {{ manualLocks.length ? "With the locks" : "Against the habit" }}: {{ evLine(summary.locked_ev) }}.
              </p>
            </div>
          </div>

          <div v-if="node.kind === 'player'" class="col-span-7 min-[1500px]:col-span-5">
            <div class="panel">
              <div class="flex items-baseline justify-between">
                <h3 class="font-bold">{{ players[node.player].name }}'s hands</h3>
                <div class="flex flex-wrap gap-3 text-xs">
                  <span v-for="(a, i) in node.actions" :key="i" class="inline-flex items-center gap-1"><span class="w-3 h-3 rounded-sm" :style="{ background: colors[i] }"></span>{{ actionText(a) }}</span>
                </div>
              </div>
              <ChartMode v-model="chartMode" class="mt-2" />
              <HandGrid class="mt-3" :grid="node.grid" :colors="colors" :values="node.has_ev" :selected="selected" :marks="suitMarks" :mode="node.has_ev ? chartMode : chartMode === 'ev' ? 'strategy' : chartMode" :fmt-ev="money" @select="selected = selected === $event ? null : $event" />
              <p class="hint mt-2">The fill height shows how much of each hand is still here. A sliver means that hand rarely reaches this point, so it barely moves the totals on the left.</p>
              <p v-if="selected === null" class="hint mt-1">Click a square to see every exact hand in it. A dot in the corner marks squares where the suits play differently.</p>
              <div v-else class="mt-3">
                <SuitBreakdown :combos="cellCombos" :actions="node.actions.map((x) => actionText(x))" :colors="colors" :fmt-ev="money1" :show-ev="node.has_ev" />
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, reactive, ref, watch } from "vue";
import * as api from "../../guided/api3";
import { studyRangeGrid } from "../../guided/api";
import * as api2 from "../../guided/api";
import { memory, refreshMemory, releaseSolve, startMemoryWatch, gb } from "../../guided/memory";
import { RANGES, PROFILES, LEAKS, RANK_CHARS, SUIT_CHARS, SUIT_SYMBOL, SUIT_COLOR, cardStr } from "../../guided/data";
import { actionColors } from "../../guided/coach";
import HandGrid from "./HandGrid.vue";
import RangeSlider from "./RangeSlider.vue";
import ActionBar, { type Step } from "./ActionBar.vue";
import SuitBreakdown from "./SuitBreakdown.vue";
import EquityGraph from "./EquityGraph.vue";
import ChartMode from "./ChartMode.vue";
import HandTypes from "./HandTypes.vue";
import CardReport, { type CardRow } from "./CardReport.vue";
import PlayingCard from "./PlayingCard.vue";

interface Seat3 { name: string; preset: string; range: string }
const SCENARIOS3: { id: string; label: string; help: string; hero: number; pot: number; stack: number; seats: [string, string][] }[] = [
  { id: "open_two", label: "I open, the button and big blind call", help: "You raise from the cutoff and get two callers. You are in the middle after the flop.", hero: 1, pot: 46, stack: 285,
    seats: [["Big blind", "rec_bbdefend"], ["You (Cutoff)", "hero_open_CO"], ["Button", "rec_coldcall"]] },
  { id: "call_btn", label: "Someone opens, I call on the button, big blind calls", help: "You have position on both opponents.", hero: 2, pot: 46, stack: 285,
    seats: [["Big blind", "rec_bbdefend"], ["Opener", "rec_open"], ["You (Button)", "hero_btnflat_MP"]] },
  { id: "bb_two", label: "I defend my big blind against an opener and a caller", help: "You act first on every street.", hero: 0, pot: 46, stack: 285,
    seats: [["You (Big blind)", "hero_bbflat_MP"], ["Opener", "rec_open"], ["Caller", "rec_coldcall"]] },
];
const SIZINGS3 = [
  { id: "std", label: "Standard: river bets 33% or 75% of the pot", flop: "66%", turn: "66%", bet: "33%, 75%", raise: "60%", turnMax: 2, max: 3 },
  { id: "simple", label: "Simple: one bet size, 66% (fastest)", flop: "66%", turn: "66%", bet: "66%", raise: "60%", turnMax: 2, max: 2 },
  { id: "big", label: "Detailed: two sizes early, three river sizes", flop: "33%, 75%", turn: "33%, 75%", bet: "33%, 75%, 125%", raise: "60%", turnMax: 2, max: 3 },
];
const RANKS_DESC = [12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0];

const step = ref<"setup" | "results">("setup");
const emit = defineEmits<{ (e: "new"): void }>();
const fromBuilder = ref(false);
const newSpot = () => { step.value = "setup"; if (fromBuilder.value) emit("new"); };
const scenarioId = ref("open_two");
const players = reactive<Seat3[]>([{ name: "", preset: "", range: "" }, { name: "", preset: "", range: "" }, { name: "", preset: "", range: "" }]);
const hero = ref(1);
const board = ref<number[]>([]);
const pot = ref(46);
const stacks = reactive<number[]>([285, 285, 285]);
const rakePct = ref(0);
const rakeCap = ref(0);
const sameStacks = computed(() => stacks[0] === stacks[1] && stacks[1] === stacks[2]);
const stackText = computed(() => (sameStacks.value ? amt(stacks[0]) : `${amt(Math.min(...stacks))} to ${amt(Math.max(...stacks))}`));
// cash games are typed in dollars; tournaments in big blinds (sent to the engine as hundredths of a big blind)
const format = ref<"cash" | "tourney">("cash");
const depth = ref(25);
const openTo = ref(2.2);
const anteKind = ref<"bb" | "each" | "none">("bb");
const unit = computed(() => (format.value === "tourney" ? "bb" : "$"));
const scale = computed(() => (format.value === "tourney" ? 100 : 1));
const solvedScale = ref(1);
const amt = (x: number) => (format.value === "tourney" ? `${Math.round(x * 100) / 100} bb` : `$${x}`);
function tourneyMoney() {
  const dead = anteKind.value === "bb" ? 1 : anteKind.value === "each" ? 9 * 0.125 : 0;
  const each = anteKind.value === "each" ? 0.125 : 0;
  const r2 = (x: number) => Math.round(x * 100) / 100;
  pot.value = r2(3 * openTo.value + 0.5 + dead);
  // the big blind pays the big-blind ante on top of the call
  const bbSeat = 0;
  for (let i = 0; i < 3; i++) stacks[i] = r2(Math.max(0.5, depth.value - openTo.value - each - (i === bbSeat && anteKind.value === "bb" ? 1 : 0)));
  rakePct.value = 0; rakeCap.value = 0;
}
function setFormat(f: "cash" | "tourney") { format.value = f; pickScenario(scenarioId.value); }
watch([depth, openTo, anteKind], () => { if (format.value === "tourney") pickScenario(scenarioId.value); });
const T_SEATS: Record<string, string[]> = { open_two: ["bb_vs_late", "open_CO", "btn_flat"], call_btn: ["bb_vs_late", "open_CO", "btn_flat"], bb_two: ["bb_vs_early", "open_MP", "btn_flat"] };

const sizingId = ref("std");
const DETAILS = [
  { id: "fast", label: "Balanced sample: one card of each rank (fastest)", turn: 1, river: 1 },
  { id: "better", label: "Larger sample: two turn cards of each rank", turn: 2, river: 1 },
  { id: "allturn", label: "Every turn card, sampled river cards (near exact, slow)", turn: 0, river: 1 },
  { id: "exact", label: "Every turn and river card (exact, very slow)", turn: 0, river: 0 },
];
const detailId = ref("fast");
const detailOptions = computed(() => (board.value.length === 3 ? DETAILS : [
  { id: "exact", label: "Every river card (exact)", turn: 0, river: 0 },
  { id: "fast", label: "Balanced sample of river cards (2-3 times faster)", turn: 0, river: 1 },
]));
watch(() => board.value.length, (n, old) => {
  if (n === 3 && old !== 3) { detailId.value = "fast"; sizingId.value = "simple"; }
  if (n === 4 && old !== 4) detailId.value = "exact";
  if (n === 5 && old === 3) sizingId.value = "std";
});
const habitSeat = ref(-1);
const profileId = ref("station");
const combos = ref<number[]>([0, 0, 0]);
const estimate = ref<api.Estimate3 | null>(null);
const estimateError = ref("");

function pickScenario(id: string) {
  const s = SCENARIOS3.find((x) => x.id === id)!;
  scenarioId.value = id;
  const tier = depth.value < 23 ? "short" : "deep";
  s.seats.forEach(([name, cash], i) => {
    const preset = format.value === "tourney" ? `t_${tier}_${T_SEATS[id][i]}` : cash;
    players[i].name = name; players[i].preset = preset; players[i].range = RANGES[preset].range;
  });
  hero.value = s.hero;
  if (format.value === "tourney") tourneyMoney(); else { pot.value = s.pot; stacks.splice(0, 3, s.stack, s.stack, s.stack); }
  if (habitSeat.value === s.hero) habitSeat.value = -1;
}
function setPreset(i: number, k: string) { players[i].preset = k; if (k) players[i].range = RANGES[k].range; }
function toggleCard(c: number) {
  const k = board.value.indexOf(c);
  if (k >= 0) board.value.splice(k, 1); else if (board.value.length < 5) board.value.push(c);
}
function randomBoard(n: number) {
  const deck = [...Array(52).keys()]; const b: number[] = [];
  while (b.length < n) b.push(deck.splice(Math.floor(Math.random() * deck.length), 1)[0]);
  board.value = b;
}
watch(hero, (h) => { if (habitSeat.value === h) habitSeat.value = -1; });

const comboText = (i: number) => (combos.value[i] ? `${combos.value[i].toFixed(0)} hands` : "");
async function countCombos() {
  for (let i = 0; i < 3; i++) {
    try {
      const w = await studyRangeGrid(players[i].range);
      combos.value[i] = w.reduce((s, x, k) => s + x * (Math.floor(k / 13) === k % 13 ? 6 : Math.floor(k / 13) < k % 13 ? 4 : 12), 0);
    } catch { combos.value[i] = 0; }
  }
}

/** Set when the spot on screen is played for prize money (only from Study a spot, tournaments). */
const icm = ref<{ payouts: number[]; others: number[] } | null>(null);
const leaks = computed<api.Leak3[]>(() => {
  if (habitSeat.value < 0 || habitSeat.value === hero.value) return [];
  const prof = PROFILES.find((p) => p.id === profileId.value);
  if (!prof) return [];
  return Object.entries(prof.leaks).flatMap(([id, strength]) => {
    const def = LEAKS.find((l) => l.id === id);
    return def ? [{ player: habitSeat.value, street: def.street, effect: def.effect, strength }] : [];
  });
});
const detail = computed(() => detailOptions.value.find((d) => d.id === detailId.value) ?? detailOptions.value[0]);
const spec = computed<api.Spec3>(() => {
  const s = SIZINGS3.find((x) => x.id === sizingId.value)!;
  return {
    ranges: [players[0].range, players[1].range, players[2].range], board: board.value.map(cardStr).join(""),
    pot: pot.value * scale.value, stack: Math.max(...stacks) * scale.value, stacks: [stacks[0] * scale.value, stacks[1] * scale.value, stacks[2] * scale.value], rake_rate: Math.max(0, rakePct.value || 0) / 100, rake_cap: Math.max(0, rakeCap.value || 0) * scale.value, flop_bet_sizes: s.flop, flop_max_raises: s.turnMax, turn_bet_sizes: s.turn, turn_max_raises: s.turnMax,
    bet_sizes: s.bet, raise_sizes: s.raise, max_raises: s.max, allin_spr: 1.0, threads: 0,
    turn_cards: detail.value.turn, river_cards: board.value.length === 5 ? 0 : detail.value.river,
    accuracy_pct: [0.5, 0.3, 0.1][board.value.length - 3] ?? 0.1, max_iter: [400, 300, 600][board.value.length - 3] ?? 600, leaks: leaks.value,
    ...(icm.value ? { icm_payouts: icm.value.payouts, icm_others: icm.value.others.map((x) => x * scale.value) } : {}),
  };
});
const problem = computed(() => {
  if (board.value.length < 3) return `Choose ${3 - board.value.length} more board card${board.value.length === 2 ? "" : "s"} (3 for a flop, 4 for a turn, 5 for a river).`;
  if (!(pot.value > 0) || stacks.some((x) => !(x > 0))) return "Enter a pot and three stacks above zero.";
  if (players.some((p) => !p.name.trim())) return "Give every player a name.";
  return estimateError.value;
});
let estTimer: number | undefined;
watch([spec], () => {
  window.clearTimeout(estTimer);
  estTimer = window.setTimeout(async () => {
    countCombos();
    if (board.value.length < 3) { estimate.value = null; estimateError.value = ""; return; }
    try { estimate.value = await api.mwEstimate(spec.value); estimateError.value = ""; }
    catch (e) { estimate.value = null; estimateError.value = String((e as Error).message ?? e); }
  }, 250);
}, { deep: true });

const estSecs = computed(() => {
  const e = estimate.value; if (!e) return 0;
  return e.secs_per_iter * (e.start === 3 ? 250 : e.start === 4 ? 130 : 200) * 1.2 * (leaks.value.length ? 1.6 : 1);
});
// this tab's own stored solve is dropped when the next one starts, so it counts as free
const freeMb = computed(() => (memory.availMb ? memory.availMb + memory.heldThreeMb : 0));
const memoryProblem = computed(() => {
  const e = estimate.value; if (!e || !freeMb.value) return "";
  if (e.memory_mb < 0.8 * freeMb.value) return "";
  return `This needs about ${gb(e.memory_mb)} of memory and about ${gb(freeMb.value)} of your ${gb(memory.totalMb)} is free right now. Choose simpler bet sizes or a smaller sample, or close other programs; the figure updates by itself.`;
});
watch(() => memory.released.threeway, () => { step.value = "setup"; node.value = null; summary.value = null; });
const duration = (s: number) => (s < 45 ? "under a minute" : s < 5400 ? `about ${Math.max(1, Math.round(s / 60))} minute${Math.round(s / 60) > 1 ? "s" : ""}` : `about ${(s / 3600).toFixed(1)} hours`);
const timeText = computed(() => duration(estSecs.value));

// ---------- solving ----------
const solving = ref(false);
const error = ref("");
const progress = ref<[number, number, number, number]>([0, 0, 0, 100]);
const summary = ref<api.Summary3 | null>(null);
const startLen = ref(5);
const elapsed = ref(0);
const elapsedText = computed(() => (elapsed.value < 60 ? `${elapsed.value}s` : `${Math.floor(elapsed.value / 60)}m ${elapsed.value % 60}s`));
const heroSolved = ref(0);
const habitSolved = ref(0);
const phaseText = computed(() => (progress.value[0] === 2 ? "Solving against the habit…" : "Solving normal play…"));
const progressPct = computed(() => {
  const g = Math.max(progress.value[3], 0.1);
  const byAccuracy = Math.min(1, Math.max(0, 1 - Math.log10(g / 0.1) / 3));
  const byIter = progress.value[2] ? progress.value[1] / progress.value[2] : 0;
  return Math.round(100 * Math.max(byAccuracy, byIter));
});
type Custom3 = { format: "cash" | "tourney"; names: string[]; ranges: string[]; hero: number; pot: number; stacks: number[]; board: number[]; rakePct: number; rakeCap: number; habitSeat: number; profile: string | null; sizingId: string; laterId: string; icm?: { payouts: number[]; others: number[] } };
/** Size and time of a spot from the spot builder, without touching what is on this screen. */
async function previewCustom(p: Custom3): Promise<{ text: string; problem: string }> {
  const sz = SIZINGS3.find((x) => x.id === p.sizingId) ?? SIZINGS3[0];
  const k = p.format === "tourney" ? 100 : 1;
  const opts = p.board.length === 3 ? DETAILS : [{ id: "exact", turn: 0, river: 0 }, { id: "fast", turn: 0, river: 1 }];
  const d = opts.find((x) => x.id === p.laterId) ?? opts[0];
  const sp: api.Spec3 = {
    ...spec.value, ranges: [p.ranges[0], p.ranges[1], p.ranges[2]], board: p.board.map(cardStr).join(""),
    pot: p.pot * k, stack: Math.max(...p.stacks) * k, stacks: [p.stacks[0] * k, p.stacks[1] * k, p.stacks[2] * k], rake_rate: Math.max(0, p.rakePct || 0) / 100, rake_cap: Math.max(0, p.rakeCap || 0) * k,
    flop_bet_sizes: sz.flop, flop_max_raises: sz.turnMax, turn_bet_sizes: sz.turn, turn_max_raises: sz.turnMax, bet_sizes: sz.bet, raise_sizes: sz.raise, max_raises: sz.max,
    turn_cards: d.turn, river_cards: p.board.length === 5 ? 0 : d.river, leaks: [],
    accuracy_pct: [0.5, 0.3, 0.1][p.board.length - 3] ?? 0.1, max_iter: [400, 300, 600][p.board.length - 3] ?? 600,
    icm_payouts: p.icm?.payouts ?? [], icm_others: (p.icm?.others ?? []).map((x) => x * k),
  };
  try {
    const e = await api.mwEstimate(sp);
    await refreshMemory();
    const secs = e.secs_per_iter * (e.start === 3 ? 250 : e.start === 4 ? 130 : 200) * 1.2 * (p.habitSeat >= 0 ? 1.6 : 1) * (p.icm ? 1.6 : 1);
    const free = memory.availMb ? memory.availMb + memory.heldThreeMb : 0;
    const problem = free && e.memory_mb >= 0.8 * free ? `This needs about ${gb(e.memory_mb)} of memory and about ${gb(free)} of your ${gb(memory.totalMb)} is free right now. Choose simpler bet sizes or a smaller card sample, or close other programs.` : "";
    const size = e.memory_mb >= 1024 ? gb(e.memory_mb) : `${Math.max(1, Math.round(e.memory_mb))} MB`;
    return { text: `${e.nodes.toLocaleString()} decision points, about ${size} of memory. Rough solve time on this computer: ${duration(secs)}.`, problem };
  } catch (e) { return { text: "", problem: String((e as Error).message ?? e) }; }
}
/** Takes a spot from the spot builder and solves it. Resolves when the solve is over. */
const chartMode = ref<"strategy" | "ev" | "equity">("strategy");
let lastPayload: Custom3 | null = null;
const saveState = ref<"" | "busy" | "done">(""); const saveError = ref("");
async function saveSpot() {
  if (!lastPayload) { saveError.value = "Only spots set up in Study a spot can be saved."; return; }
  saveState.value = "busy"; saveError.value = "";
  const cards = lastPayload.board.map((c) => RANK_CHARS[c >> 2] + SUIT_SYMBOL[SUIT_CHARS[c & 3]]).join(" ");
  try { await api2.librarySaveThree(`${lastPayload.names.join(", ")} · ${cards}`, { payload: lastPayload, locks: lockStale.value ? [] : manualLocks.value }); saveState.value = "done"; }
  catch (e) { saveError.value = String((e as Error).message ?? e); saveState.value = ""; }
}
/** Puts a spot from the spot builder on this screen, ready to solve or to show a stored solve. */
async function applyCustom(p: Custom3) {
  lastPayload = p; saveState.value = "";
  fromBuilder.value = true; format.value = p.format;
  p.names.forEach((n, i) => { players[i].name = n; players[i].preset = ""; players[i].range = p.ranges[i]; });
  hero.value = p.hero; pot.value = p.pot; stacks.splice(0, 3, p.stacks[0], p.stacks[1], p.stacks[2]);
  board.value = [...p.board]; rakePct.value = p.rakePct; rakeCap.value = p.rakeCap;
  habitSeat.value = p.habitSeat; icm.value = p.icm && p.format === "tourney" ? { payouts: [...p.icm.payouts], others: [...p.icm.others] } : null;
  if (p.profile && PROFILES.some((x) => x.id === p.profile)) profileId.value = p.profile;
  await nextTick();
  sizingId.value = p.sizingId;
  if (detailOptions.value.some((x) => x.id === p.laterId)) detailId.value = p.laterId;
  step.value = "setup"; error.value = "";
  await nextTick();
}
async function loadCustom(p: Custom3): Promise<{ ok: boolean; error: string }> {
  await applyCustom(p);
  try { estimate.value = await api.mwEstimate(spec.value); } catch { /* the solve reports it */ }
  if (memoryProblem.value) return { ok: false, error: memoryProblem.value };
  await solve();
  return { ok: (step.value as string) === "results" && !error.value, error: error.value };
}
/** Shows a three-player solve kept in the library, without solving again. */
async function openSaved(e: api2.LibraryEntry): Promise<{ ok: boolean; error: string }> {
  const p = e.meta.payload as Custom3 | undefined;
  if (!p) return { ok: false, error: "That entry is missing its spot." };
  try {
    await applyCustom(p);
    const s = await api2.libraryOpenThree<api.Summary3>(e.id);
    solvedScale.value = scale.value;
    await showSolved(s, (e.meta.locks as api.ManualLock3[] | undefined) ?? []);
    saveState.value = "done";
    return { ok: true, error: "" };
  } catch (x) { step.value = "setup"; return { ok: false, error: String((x as Error).message ?? x) }; }
}
/** Switches the screen to a solve the engine now holds. */
async function showSolved(s: api.Summary3, locks: api.ManualLock3[] = []) {
  manualLocks.value = locks; lockStale.value = false; lockError.value = ""; hadHabit.value = leaks.value.length > 0;
  summary.value = s;
  startLen.value = board.value.length;
  heroSolved.value = hero.value; habitSolved.value = Math.max(habitSeat.value, 0);
  which.value = s.locked ? "locked" : "base";
  history.value = []; line.value = []; selected.value = null; exactAt.value = null; exactSummary.value = null; resolveError.value = "";
  await load();
  step.value = "results";
}
defineExpose({ loadCustom, previewCustom, openSaved });
async function solve() {
  await refreshMemory();
  if (memoryProblem.value) return;
  solving.value = true; error.value = ""; progress.value = [1, 0, spec.value.max_iter, 100];
  const began = Date.now(); elapsed.value = 0;
  const timer = window.setInterval(async () => { elapsed.value = Math.round((Date.now() - began) / 1000); try { progress.value = await api.mwProgress(); } catch { /* ignore */ } }, 400);
  try {
    const usedScale = scale.value;
    const s = await api.mwSolve(spec.value);
    solvedScale.value = usedScale;
    if (s.error) { error.value = s.error; return; }
    await showSolved(s);
  } catch (e) { error.value = String((e as Error).message ?? e); }
  finally { window.clearInterval(timer); solving.value = false; }
}

// ---------- results ----------
const which = ref<"base" | "locked">("base");
const history = ref<number[]>([]);
const line = ref<{ player: number; label: string }[]>([]);
const node = ref<api.NodeView3 | null>(null);
const other = ref<api.NodeView3 | null>(null);
const selected = ref<number | null>(null);
let loadToken = 0;
let loading = false; // clicks are ignored until the screen shows the point they would act on
async function load() {
  const token = ++loadToken; const w = which.value; const h = [...history.value];
  // strategies first (fast), then the EVs, which need a walk of everything below this point
  loading = true;
  try { node.value = await api.mwNode(w, h, false); } finally { loading = false; }
  other.value = summary.value?.locked ? await api.mwNode(w === "base" ? "locked" : "base", h, false).catch(() => null) : null;
  api.mwNode(w, h, true).then((full) => { if (token === loadToken) node.value = full; }).catch(() => undefined);
}
async function play(i: number) {
  if (!node.value || node.value.kind !== "player" || loading) return;
  line.value.push({ player: node.value.player, label: actionText(node.value.actions[i]) });
  history.value.push(i);
  await load();
}
const dealable = computed(() => new Set((node.value?.cards ?? []).map((c) => c[1])));
const onBoard = computed(() => new Set((node.value?.board ?? []).map((c) => RANK_CHARS.indexOf(c[0]) * 4 + SUIT_CHARS.indexOf(c[1]))));
// exact turn solving: offered where the flop betting ends
const canExact = computed(() => node.value?.kind === "chance" && node.value.street === "flop" && !node.value.exact);
const exactMode = ref(false);
const resolving = ref(false);
const resolveError = ref("");
const exactSummary = ref<api.Summary3 | null>(null);
const exactAt = ref<{ which: string; prefix: number[] } | null>(null);
const exactCard = computed(() => {
  const e = exactAt.value; const h = history.value;
  return e && e.which === which.value && e.prefix.length === h.length + 1 && h.every((x, i) => x === e.prefix[i]) ? e.prefix[h.length] : -1;
});
const cardTitle = (card: number) => {
  if (!canExact.value || onBoard.value.has(card)) return "";
  if (exactCard.value === card) return "Already solved exactly";
  return dealable.value.has(card) ? "In the flop solve's sample" : "Not in the sample: picking it solves it exactly";
};
async function deal(card: number) {
  const v = node.value;
  if (!v || v.kind !== "chance" || resolving.value || loading) return;
  resolveError.value = "";
  if (canExact.value && exactCard.value !== card && (exactMode.value || !dealable.value.has(card))) {
    resolving.value = true; progress.value = [1, 0, 300, 100];
    const began = Date.now(); elapsed.value = 0;
    const timer = window.setInterval(async () => { elapsed.value = Math.round((Date.now() - began) / 1000); try { progress.value = await api.mwProgress(); } catch { /* ignore */ } }, 400);
    try {
      const s = await api.mwResolve(which.value, [...history.value], card);
      if (s.error) { resolveError.value = s.error; exactAt.value = null; return; }
      exactSummary.value = s; exactAt.value = { which: which.value, prefix: [...history.value, card] };
    } catch (e) { resolveError.value = String((e as Error).message ?? e); return; }
    finally { window.clearInterval(timer); resolving.value = false; refreshMemory(); }
  }
  const c = cardStr(card);
  line.value.push({ player: -1, label: `${v.street === "flop" ? "Turn" : "River"} ${c[0] === "T" ? "10" : c[0]}${SUIT_SYMBOL[c[1]]}` });
  history.value.push(card);
  await load();
}
/** what the next player does, and who is ahead, if this card comes (from the solve already done) */
async function loadCard(id: number): Promise<CardRow> {
  const v = await api.mwNode(which.value, [...history.value, id], false);
  const bet = v.actions.reduce((a, name, i) => a + (/^(Bet|Raise|All-in)/.test(name) ? v.action_freqs[i] : 0), 0);
  return { actor: v.kind === "player" ? players[v.player].name : "the next player", bet, equity: v.equity.map((e, i) => (v.folded[i] ? undefined : e)) as number[] };
}
// ---------- locks made by hand ----------
const RANKS_HI = "AKQJT98765432";
const cellName = (i: number) => { const r = Math.floor(i / 13), c = i % 13; return r === c ? RANKS_HI[r] + RANKS_HI[c] : c > r ? RANKS_HI[r] + RANKS_HI[c] + "s" : RANKS_HI[c] + RANKS_HI[r] + "o"; };
const manualLocks = ref<api.ManualLock3[]>([]);
const lockOpen = ref(false); const lockScope = ref<"all" | "cell">("all"); const lockPct = ref<number[]>([]);
const lockStale = ref(false); const relocking = ref(false); const lockError = ref("");
const lockTotal = computed(() => lockPct.value.reduce((a, v) => a + (Number(v) || 0), 0));
const hadHabit = ref(false);
const lockedLabel = computed(() => (manualLocks.value.length && !lockStale.value ? (hadHabit.value ? "Vs the habit and your locks" : "With your locks") : `Vs ${players[habitSolved.value].name}'s habit`));
const lockedHere = computed(() => manualLocks.value.some((l) => l.history.join() === history.value.join()));
function fillLock() {
  const v = node.value; if (!v || v.kind !== "player") return;
  if (lockScope.value === "cell" && selected.value === null) lockScope.value = "all";
  const src = lockScope.value === "cell" && selected.value !== null && v.grid[selected.value].weight > 0 ? v.grid[selected.value].freqs : v.action_freqs;
  const out = src.map((f) => Math.round(f * 100));
  const off = 100 - out.reduce((a, x) => a + x, 0);
  if (out.length) out[out.indexOf(Math.max(...out))] += off;
  lockPct.value = out;
}
watch([() => node.value?.actions.join("|"), () => history.value.join(), selected, lockScope, lockOpen, which, summary], fillLock);
function addLock() {
  const v = node.value!; const cell = lockScope.value === "cell" ? selected.value : null;
  const what = lockPct.value.map((p, i) => (p > 0 ? `${actionText(v.actions[i]).toLowerCase()} ${p}%` : "")).filter(Boolean).join(", ");
  const where = line.value.length ? line.value.map((l) => (l.player < 0 ? l.label : `${players[l.player].name} ${l.label.toLowerCase()}`)).join(" › ") : "First decision";
  manualLocks.value = [...manualLocks.value.filter((l) => !(l.history.join() === history.value.join() && l.cells.join() === (cell === null ? "" : String(cell)))),
    { history: [...history.value], cells: cell === null ? [] : [cell], freqs: lockPct.value.map((p) => (Number(p) || 0) / 100), label: `${where}: ${players[v.player].name} with ${cell === null ? "every hand" : cellName(cell)}: ${what}` }];
  lockStale.value = true; saveState.value = "";
}
const removeLock = (i: number) => { manualLocks.value = manualLocks.value.filter((_l, j) => j !== i); lockStale.value = true; };
async function relockNow() {
  relocking.value = true; lockError.value = ""; progress.value = [2, 0, spec.value.max_iter, 100];
  const timer = window.setInterval(async () => { try { progress.value = await api.mwProgress(); } catch { /* ignore */ } }, 400);
  try {
    const s = await api.mwRelock(manualLocks.value);
    if (s.error) { lockError.value = s.error === "Solve cancelled" ? "Stopped. The normal solve is still here." : s.error; summary.value = { ...s, error: null }; which.value = "base"; }
    else { summary.value = s; which.value = "locked"; lockStale.value = false; saveState.value = ""; exactAt.value = exactAt.value?.which === "locked" ? null : exactAt.value; }
    await load();
  } catch (e) { lockError.value = String((e as Error).message ?? e); }
  finally { window.clearInterval(timer); relocking.value = false; }
}

async function goto(n: number) { history.value = history.value.slice(0, n); line.value = line.value.slice(0, n); await load(); }
async function setWhich(w: "base" | "locked") {
  which.value = w;
  try { await load(); }
  catch {
    // the other solution has no exact solve for this turn card: step back to where the flop betting ends
    const n = exactAt.value ? exactAt.value.prefix.length - 1 : 0;
    history.value = history.value.slice(0, n); line.value = line.value.slice(0, n);
    await load();
  }
}

const colors = computed(() => actionColors(node.value?.actions ?? []));
const money = (x: number) => (solvedScale.value > 1 ? `${x < -0.5 ? "-" : ""}${(Math.abs(x) / solvedScale.value).toFixed(1)} bb` : `${x < -0.5 ? "-" : ""}$${Math.abs(Math.round(x))}`);
const money1 = (x: number) => (solvedScale.value > 1 ? `${x < -0.5 ? "-" : ""}${(Math.abs(x) / solvedScale.value).toFixed(2)} bb` : `${x < -0.05 ? "-" : ""}$${Math.abs(x).toFixed(1)}`);
const pct = (x: number) => `${Math.round(x * 100)}%`;
const pct1 = (x: number) => `${(x * 100).toFixed(x > 0 && x < 0.1 ? 1 : 0)}%`;
const actionText = (a: string) => a.replace(/^(Bet|Raise to|All-in) (\d+)/, (_m, v, n) => `${v === "Bet" ? "Bets" : v === "Raise to" ? "Raises to" : "All-in"} ${money(Number(n))}`)
  .replace(/^Fold$/, "Folds").replace(/^Check$/, "Checks").replace(/^Call$/, "Calls");
const mixText = (f: number[]) => {
  const acts = node.value?.actions ?? [];
  const parts = f.map((x, i) => [x, i]).filter(([x]) => x >= 0.005).sort((a, b) => b[0] - a[0]).map(([x, i]) => `${actionText(acts[i]).toLowerCase()} ${pct(x)}`);
  return parts.join(", ");
};
const steps = computed<Step[]>(() => line.value.map((st, k) => {
  if (st.player >= 0) return { who: players[st.player].name, text: st.label };
  const [street] = st.label.split(" ");
  return { who: street, text: st.label, card: cardStr(history.value[k]) };
}));
const nowText = computed(() => {
  const v = node.value; if (!v) return "";
  return v.kind === "player" ? `${players[v.player].name} to act` : v.kind === "chance" ? `Deal the ${v.street === "flop" ? "turn" : "river"}` : "Hand over";
});
// squares where the exact hands do not all play alike (20 points or more apart on some action)
const suitMarks = computed(() => {
  const lo = new Map<number, number[]>(); const hi = new Map<number, number[]>();
  for (const c of node.value?.combos ?? []) {
    if (c.weight < 0.02) continue;
    const a = lo.get(c.cell), b = hi.get(c.cell);
    if (!a || !b) { lo.set(c.cell, [...c.freqs]); hi.set(c.cell, [...c.freqs]); continue; }
    c.freqs.forEach((f, i) => { a[i] = Math.min(a[i], f); b[i] = Math.max(b[i], f); });
  }
  const out = new Set<number>();
  lo.forEach((a, cell) => { const b = hi.get(cell)!; if (a.some((x, i) => b[i] - x >= 0.2)) out.add(cell); });
  return out;
});
const cellCombos = computed(() => (node.value?.combos ?? []).filter((c) => c.cell === selected.value).sort((a, b) => b.ev - a.ev));
const stillIn = computed(() => players.filter((_p, i) => !node.value?.folded[i]).map((p) => p.name).join(" and "));
const worstGain = computed(() => {
  const g = which.value === "base" ? summary.value?.base_gain_pct : summary.value?.locked_gain_pct;
  return Math.max(0, ...(g ?? [0]));
});
const evLine = (ev: number[]) => players.map((p, i) => `${p.name} ${money1(ev[i])}`).join(", ");
const notes = computed(() => {
  const v = node.value; if (!v || v.kind !== "player") return [];
  const out: string[] = []; const name = players[v.player].name;
  const aggr = v.actions.map((a, i) => (/^(Bet|Raise|All-in)/.test(a) ? v.action_freqs[i] : 0)).reduce((s, x) => s + x, 0);
  const facing = v.actions[0] === "Fold";
  if (facing) {
    const callAmt = Math.max(...v.invested) - v.invested[v.player];
    out.push(`${name} must call ${money(callAmt)} to win ${money(v.pot)}, and folds ${pct(v.action_freqs[0])} of the time.`);
    const behind = [0, 1, 2].filter((i) => i !== v.player && !v.folded[i] && v.invested[i] < Math.max(...v.invested)).length;
    if (behind) out.push("A player still to act behind makes calling riskier, so expect tighter calls than heads-up.");
  } else {
    out.push(aggr < 0.15 ? `${name} checks almost everything here (${pct(1 - aggr)}).` : `${name} bets ${pct(aggr)} of the time and checks the rest.`);
    const weak = v.buckets[2]; const strong = v.buckets[0];
    if (weak && strong && aggr >= 0.05) out.push(`Strong hands bet ${pct(1 - strong.freqs[0])}; weak hands bluff ${pct(1 - weak.freqs[0])}. Against two players, bluffs must get through both.`);
  }
  if (other.value && other.value.kind === "player") {
    let big = 0; let bi = 0;
    v.action_freqs.forEach((f, i) => { const d = f - (other.value!.action_freqs[i] ?? 0); if (Math.abs(d) > Math.abs(big)) { big = d; bi = i; } });
    if (Math.abs(big) >= 0.03) out.push(`Biggest change ${which.value === "locked" ? (manualLocks.value.length ? "with the locks" : "against the habit") : manualLocks.value.length ? "compared with the locked solve" : "compared with the habit solve"}: ${actionText(v.actions[bi]).toLowerCase()} ${big > 0 ? "more" : "less"} often (${pct1(Math.abs(big))} ${big > 0 ? "up" : "down"}).`);
  }
  return out;
});

onMounted(() => {
  pickScenario("open_two");
  startMemoryWatch();
});
</script>

<style scoped>
.step-title { @apply font-display font-extrabold text-lg flex items-center gap-2.5; }
.step-num { @apply inline-flex items-center justify-center w-7 h-7 rounded-full bg-ink text-white text-sm font-body font-bold; }
.choice { @apply w-full text-left bg-surface border border-rule rounded-lg px-3.5 py-3 hover:border-felt transition-colors; }
.choice-on { @apply border-felt ring-1 ring-felt bg-felt-light; }
.pill { @apply px-3 py-1.5 rounded-full border border-rule bg-surface text-sm hover:border-felt transition-colors disabled:opacity-40; }
.pill-on { @apply border-felt bg-felt text-white hover:border-felt; }
.field { @apply flex flex-col text-sm gap-1 font-bold; }
.input { @apply rounded-md border-rule text-sm font-normal focus:border-felt focus:ring-felt; }
.hint { @apply text-sm text-muted; }
.deck-btn { @apply h-9 rounded-md border border-rule bg-surface text-sm font-bold font-display hover:border-felt disabled:opacity-25 disabled:hover:border-rule; }
.deck-on { @apply !bg-ink !text-white border-ink; }
.deck-extra { @apply border-dashed border-muted bg-paper; }
.deck-done { @apply !border-felt ring-1 ring-felt bg-felt-light; }
.stat { @apply rounded-lg bg-paper py-2; }
.stat dt { @apply text-xs text-muted; }
.stat dd { @apply font-display font-extrabold text-lg tabular-nums; }
.btn-solve { @apply w-full py-3 rounded-lg bg-felt text-white text-lg font-bold hover:bg-felt-dark disabled:opacity-40 disabled:hover:bg-felt; }
.btn-quiet { @apply px-3 py-1.5 rounded-md border border-rule bg-surface text-sm font-bold hover:border-felt disabled:opacity-40; }
.crumb { @apply px-2.5 py-1 rounded-md bg-surface border border-rule hover:border-felt; }
.panel { @apply bg-surface rounded-xl border border-rule p-4; }
.action-row { @apply w-full flex items-center gap-3 rounded-lg border border-rule px-3 py-2 hover:border-felt hover:bg-felt-light/40 transition-colors; }
.seg { @apply inline-flex rounded-lg border border-ink overflow-hidden; }
.seg button { @apply px-3 py-1.5 text-sm font-bold bg-surface; }
.seg .seg-on { @apply bg-felt text-white; }
button:focus-visible, input:focus-visible, select:focus-visible, textarea:focus-visible { @apply outline-none ring-2 ring-felt ring-offset-1; }
</style>
