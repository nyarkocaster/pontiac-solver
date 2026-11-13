<template>
  <div class="h-full overflow-y-auto bg-paper text-ink font-body">
    <div class="max-w-[1180px] mx-auto px-6 py-6">
      <!-- ================= SETUP ================= -->
      <div v-if="step === 'setup'" class="grid grid-cols-12 gap-8">
        <div class="col-span-7">
          <h1 class="font-display font-extrabold text-[2rem] leading-tight tracking-tight">
            Three-way pots <span class="align-middle text-xs font-body font-bold bg-mine-light border border-mine rounded px-1.5 py-0.5">experimental</span>
          </h1>
          <p class="text-muted mt-1 max-w-[62ch]">
            Three players see a flop, turn or river. The solver finds a strategy for all three where nobody can do
            better by changing their own play, then, if you choose, how to adjust when one opponent has a habit.
          </p>

          <section class="mt-7">
            <h2 class="step-title"><span class="step-num">1</span>Who is in the hand?</h2>
            <div class="grid grid-cols-1 gap-2.5 mt-3">
              <button v-for="s in SCENARIOS3" :key="s.id" class="choice" :class="scenarioId === s.id ? 'choice-on' : ''" :aria-pressed="scenarioId === s.id" @click="pickScenario(s.id)">
                <div class="font-bold">{{ s.label }}</div>
                <div class="text-sm text-muted mt-0.5">{{ s.help }}</div>
              </button>
            </div>
            <div class="mt-4 space-y-3">
              <div v-for="(p, i) in players" :key="i" class="panel !p-3">
                <div class="flex flex-wrap items-end gap-3">
                  <label class="field">
                    <span>{{ ["Acts first", "Acts second", "Acts last"][i] }}</span>
                    <input v-model="p.name" class="input w-44" maxlength="24" />
                  </label>
                  <label class="field">
                    <span>Starting range</span>
                    <select class="input w-64" :value="p.preset" @change="setPreset(i, ($event.target as HTMLSelectElement).value)">
                      <option value="">Custom (edit below)</option>
                      <option v-for="(r, k) in RANGES" :key="k" :value="k">{{ r.label }}</option>
                    </select>
                  </label>
                  <label class="flex items-center gap-1.5 text-sm font-bold pb-2">
                    <input type="radio" name="hero3" :checked="hero === i" class="text-felt focus:ring-felt" @change="hero = i" /> This is me
                  </label>
                  <span class="ml-auto text-sm text-muted pb-2">{{ comboText(i) }}</span>
                </div>
                <textarea v-model="p.range" rows="2" class="input font-body text-xs w-full mt-2" @input="p.preset = ''"></textarea>
              </div>
            </div>
            <p class="hint mt-2 max-w-[70ch]">
              Ranges are what each player could hold when your spot starts. The built-in ones are pre-flop ranges, so
              they fit a flop spot as they are, and the solver narrows them street by street. If you start on the turn
              or river after earlier betting, trim out the hands that would have folded and raise the pot below.
            </p>
          </section>

          <section class="mt-8">
            <h2 class="step-title"><span class="step-num">2</span>Pick the board: three cards for a flop, four for a turn, five for a river</h2>
            <div class="flex flex-wrap gap-2 mt-3 items-center">
              <button class="pill" @click="randomBoard(5)">Random river</button>
              <button class="pill" :disabled="!board.length" @click="board = []">Clear</button>
              <button class="pill" @click="randomBoard(4)">Random turn</button>
              <button class="pill" @click="randomBoard(3)">Random flop</button>
              <span class="text-sm text-muted">{{ board.length }} chosen{{ board.length === 3 ? " (flop spot)" : board.length === 4 ? " (turn spot)" : board.length === 5 ? " (river spot)" : "" }}</span>
            </div>
            <div class="mt-3 inline-grid gap-1" style="grid-template-columns: repeat(13, 2.25rem)" role="group" aria-label="Choose board cards">
              <template v-for="s in [3, 2, 1, 0]" :key="s">
                <button
                  v-for="r in RANKS_DESC"
                  :key="r * 4 + s"
                  class="deck-btn"
                  :class="[SUIT_COLOR[SUIT_CHARS[s]], board.includes(r * 4 + s) ? 'deck-on' : '']"
                  :disabled="!board.includes(r * 4 + s) && board.length >= 5"
                  :aria-pressed="board.includes(r * 4 + s)"
                  @click="toggleCard(r * 4 + s)"
                >
                  {{ RANK_CHARS[r] }}{{ SUIT_SYMBOL[SUIT_CHARS[s]] }}
                </button>
              </template>
            </div>
          </section>

          <section class="mt-8">
            <h2 class="step-title"><span class="step-num">3</span>Money and bet sizes</h2>
            <div class="flex flex-wrap gap-5 mt-3 items-end">
              <label class="field"><span>Pot now ($)</span><input v-model.number="pot" type="number" min="1" class="input w-28" /></label>
              <label class="field"><span>Stack behind, each ($)</span><input v-model.number="stack" type="number" min="1" class="input w-28" /></label>
              <label class="field">
                <span>Bet sizes</span>
                <select v-model="sizingId" class="input w-72">
                  <option v-for="s in SIZINGS3" :key="s.id" :value="s.id">{{ s.label }}</option>
                </select>
              </label>
            </div>
            <div v-if="board.length === 3 || board.length === 4" class="mt-4">
              <label class="field">
                <span>Later cards to solve</span>
                <select v-model="detailId" class="input w-[34rem] max-w-full">
                  <option v-for="d in detailOptions" :key="d.id" :value="d.id">{{ d.label }}</option>
                </select>
              </label>
              <p class="hint mt-2 max-w-[70ch]">
                Solving every possible turn and river card is slow and memory-hungry. A balanced sample keeps every card
                that matters for flushes and one or two cards of each rank for the rest. In tests it changed hand values
                by well under 1%.
              </p>
            </div>
            <p class="hint mt-2 max-w-[70ch]">
              Everyone is treated as having the same stack behind, so enter the shortest of the three. Different stack
              sizes and side pots are not supported yet.
            </p>
          </section>

          <section class="mt-8">
            <h2 class="step-title"><span class="step-num">4</span>Does an opponent have a habit? <span class="font-body font-normal text-sm text-muted">(optional)</span></h2>
            <div class="flex flex-wrap gap-2 mt-3">
              <button class="pill" :class="habitSeat === -1 ? 'pill-on' : ''" @click="habitSeat = -1">No habit</button>
              <template v-for="(p, i) in players" :key="i">
                <button v-if="i !== hero" class="pill" :class="habitSeat === i ? 'pill-on' : ''" @click="habitSeat = i">{{ p.name }}</button>
              </template>
            </div>
            <div v-if="habitSeat >= 0 && habitSeat !== hero" class="grid grid-cols-2 gap-2.5 mt-3">
              <button v-for="p in PROFILES" :key="p.id" class="choice" :class="profileId === p.id ? 'choice-on' : ''" @click="profileId = p.id">
                <div class="font-bold">{{ p.label }}</div>
                <div class="text-sm text-muted mt-0.5">{{ p.help }}</div>
              </button>
            </div>
            <p v-if="habitSeat >= 0" class="hint mt-2 max-w-[70ch]">
              That player keeps their normal game plus this habit. The other two players, including you, then re-solve
              against it.
            </p>
          </section>
        </div>

        <!-- summary / solve -->
        <aside class="col-span-5">
          <div class="panel sticky top-4">
            <h2 class="font-display font-extrabold text-lg">Your spot</h2>
            <div class="flex gap-1.5 mt-3">
              <PlayingCard v-for="k in 5" :key="k" :card="board[k - 1] !== undefined ? cardStr(board[k - 1]) : ''" size="md" />
            </div>
            <dl class="grid grid-cols-3 gap-2 mt-4 text-center">
              <div class="stat"><dt>Pot</dt><dd>${{ pot }}</dd></div>
              <div class="stat"><dt>Stacks</dt><dd>${{ stack }}</dd></div>
              <div class="stat"><dt>Stack-to-pot</dt><dd>{{ (stack / Math.max(pot, 1)).toFixed(1) }}</dd></div>
            </dl>
            <ol class="mt-4 space-y-1 text-sm">
              <li v-for="(p, i) in players" :key="i" class="flex justify-between">
                <span><b>{{ i + 1 }}.</b> {{ p.name }}<span v-if="i === hero" class="ml-1 text-xs font-bold bg-mine-light border border-mine rounded px-1">you</span></span>
                <span class="text-muted">{{ comboText(i) }}</span>
              </li>
            </ol>
            <p v-if="estimate" class="hint mt-3">
              {{ estimate.start === 3 ? "Flop, turn and river" : estimate.start === 4 ? "Turn and river" : "River only" }}: about {{ estimate.nodes.toLocaleString() }} decision points, {{ Math.round(estimate.memory_mb) }} MB of memory.
              Rough solve time on this computer: <b class="text-ink">{{ timeText }}</b>.
              <span v-if="estSecs > 600"> Pick simpler bet sizes to make it faster; you can also stop early and keep the result so far.</span>
            </p>
            <p v-if="memoryProblem" class="mt-3 text-sm font-bold text-[#B42318]">{{ memoryProblem }}</p>
            <p v-if="problem" class="mt-3 text-sm font-bold text-[#B42318]">{{ problem }}</p>
            <button class="btn-solve mt-4" :disabled="!!problem || !!memoryProblem || solving" @click="solve">Solve this spot</button>
            <div v-if="solving" class="mt-4">
              <div class="flex justify-between text-sm font-bold">
                <span>{{ phaseText }}</span>
                <button class="text-felt underline" @click="api.mwCancel()">Stop early</button>
              </div>
              <div class="h-2 bg-paper rounded mt-1.5 overflow-hidden"><div class="h-full bg-felt transition-all" :style="{ width: progressPct + '%' }"></div></div>
              <p class="hint mt-1.5">Step {{ progress[1] }} of up to {{ progress[2] }}, {{ elapsedText }} so far. Best single-player improvement left: {{ progress[3] >= 100 ? "..." : progress[3].toFixed(2) + "% of the pot" }}</p>
            </div>
            <p v-if="error" class="mt-3 text-sm font-bold text-[#B42318]">{{ error }}</p>
          </div>
        </aside>
      </div>

      <!-- ================= RESULTS ================= -->
      <div v-if="step === 'results' && node && summary">
        <div class="flex items-center justify-between gap-4">
          <button class="btn-quiet" @click="step = 'setup'">New spot</button>
          <div v-if="summary.locked" class="seg" role="group" aria-label="Which solution to show">
            <button :class="which === 'base' ? 'seg-on' : ''" @click="setWhich('base')">Normal strategy</button>
            <button :class="which === 'locked' ? 'seg-on' : ''" @click="setWhich('locked')">Vs {{ players[habitSolved].name }}'s habit</button>
          </div>
        </div>

        <div class="panel mt-4 flex flex-wrap items-center gap-6">
          <div class="flex gap-1.5">
            <PlayingCard v-for="c in node.board" :key="c" :card="c" size="md" />
            <PlayingCard v-for="k in 5 - node.board.length" :key="'e' + k" card="" size="md" />
          </div>
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
              <div class="text-xs text-muted tabular-nums">{{ node.range_combos[i].toFixed(0) }} hands left<span v-if="node.invested[i] > 0"> · {{ money(node.invested[i]) }} in</span></div>
            </div>
          </div>
        </div>

        <!-- line so far -->
        <div class="flex flex-wrap items-center gap-1.5 mt-3 text-sm">
          <button class="crumb" @click="goto(0)">{{ ["Flop", "Turn", "River"][startLen - 3] }} starts</button>
          <template v-for="(st, k) in line" :key="k">
            <span class="text-muted">›</span>
            <button class="crumb" @click="goto(k + 1)"><template v-if="st.player >= 0"><b>{{ players[st.player].name }}</b> {{ st.label.toLowerCase() }}</template><b v-else>{{ st.label }}</b></button>
          </template>
          <button v-if="history.length" class="ml-2 text-felt underline font-bold" @click="goto(history.length - 1)">Back one step</button>
        </div>

        <div class="grid grid-cols-12 gap-5 mt-4">
          <div class="col-span-5 space-y-4">
            <div v-if="node.kind === 'player'" class="panel">
              <h2 class="font-display font-extrabold text-lg">
                {{ players[node.player].name }} to act
                <span v-if="node.locked" class="ml-1 align-middle text-xs font-body font-bold bg-mine-light border border-mine rounded px-1.5 py-0.5">habit applied</span>
              </h2>
              <p class="hint">Click an action to follow that line.</p>
              <div class="mt-3 space-y-2">
                <button v-for="(a, i) in node.actions" :key="i" class="action-row" @click="play(i)">
                  <span class="w-3 h-8 rounded" :style="{ background: colors[i] }"></span>
                  <span class="font-bold w-28 text-left">{{ actionText(a) }}</span>
                  <span class="flex-1 h-2 bg-paper rounded overflow-hidden"><span class="block h-full" :style="{ width: node.action_freqs[i] * 100 + '%', background: colors[i] }"></span></span>
                  <span class="w-12 text-right font-bold tabular-nums">{{ pct1(node.action_freqs[i]) }}</span>
                  <span v-if="other && other.kind === 'player'" class="w-16 text-right text-xs text-muted tabular-nums" :title="which === 'base' ? 'Against the habit' : 'Normal strategy'">
                    {{ which === "base" ? "habit" : "normal" }} {{ pct1(other.action_freqs[i] ?? 0) }}
                  </span>
                </button>
              </div>
              <ul class="mt-4 space-y-1.5 text-sm">
                <li v-for="(n, i) in notes" :key="i" class="flex gap-2"><span class="text-felt font-bold">•</span><span>{{ n }}</span></li>
              </ul>
            </div>
            <div v-else-if="node.kind === 'chance'" class="panel">
              <h2 class="font-display font-extrabold text-lg">Deal the {{ node.street === "flop" ? "turn" : "river" }}</h2>
              <p class="hint">The {{ node.street }} betting is over. Pick the next card to see how everyone plays it.</p>
              <div class="mt-3 grid gap-[3px]" style="grid-template-columns: repeat(13, minmax(0, 1fr))" role="group" aria-label="Choose the river card">
                <template v-for="s in [3, 2, 1, 0]" :key="s">
                  <button v-for="r in RANKS_DESC" :key="r * 4 + s" class="deck-btn !h-8 !text-[11px] !px-0" :class="SUIT_COLOR[SUIT_CHARS[s]]" :disabled="!dealable.has(r * 4 + s)" @click="deal(r * 4 + s)">
                    {{ RANK_CHARS[r] }}{{ SUIT_SYMBOL[SUIT_CHARS[s]] }}
                  </button>
                </template>
              </div>
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
                This solve covered {{ node.cards.length }} of the {{ node.cards_possible }} cards that could come. The greyed-out cards were not
                solved; pick the same rank in another suit, which plays almost the same unless it changes a flush.
              </p>
            </div>
            <div class="panel text-sm">
              <h3 class="font-bold">How good is this solution?</h3>
              <p class="mt-1">
                Best gain any single player could get by changing their own strategy:
                <b>{{ worstGain.toFixed(2) }}% of the pot</b> ({{ worstGain <= 0.15 ? "very close" : worstGain <= 0.5 ? "close enough to study" : "rough, solve longer" }}).
                Solved in {{ (which === "base" ? summary.base_secs : summary.locked_secs).toFixed(0) }}s.
              </p>
              <p class="hint mt-1.5">
                With three players there can be more than one sound solution, and two opponents' choices can help or hurt
                the third. Treat exact percentages as a guide, and look for the patterns.
              </p>
              <p v-if="summary.locked" class="mt-2">
                Average result with normal play: {{ evLine(summary.base_ev) }}.<br />
                Against the habit: {{ evLine(summary.locked_ev) }}.
              </p>
            </div>
          </div>

          <div v-if="node.kind === 'player'" class="col-span-7">
            <div class="panel">
              <div class="flex items-baseline justify-between">
                <h3 class="font-bold">{{ players[node.player].name }}'s hands</h3>
                <div class="flex flex-wrap gap-3 text-xs">
                  <span v-for="(a, i) in node.actions" :key="i" class="inline-flex items-center gap-1"><span class="w-3 h-3 rounded-sm" :style="{ background: colors[i] }"></span>{{ actionText(a) }}</span>
                </div>
              </div>
              <HandGrid class="mt-3" :grid="node.grid" :colors="colors" :selected="selected" @select="selected = selected === $event ? null : $event" />
              <p class="hint mt-2">The fill height shows how much of each hand is still here. A sliver means that hand rarely reaches this point, so it barely moves the totals on the left.</p>
              <p v-if="selected === null" class="hint mt-1">Click a square to see every exact hand in it.</p>
              <div v-else class="mt-3">
                <div v-if="!cellCombos.length" class="hint">No hands from this square get here.</div>
                <table v-else class="w-full text-sm">
                  <thead><tr class="text-left text-xs text-muted"><th class="font-normal">Hand</th><th class="font-normal pl-2">What it does</th><th class="font-normal text-right" title="How often this exact hand is still here, after starting ranges and earlier actions">Gets here</th><th class="font-normal text-right">Wins</th><th class="font-normal text-right">EV</th></tr></thead>
                  <tbody>
                    <tr v-for="c in cellCombos" :key="c.hand" class="border-t border-rule">
                      <td class="py-1 font-display font-bold whitespace-nowrap">
                        <span :class="SUIT_COLOR[c.hand[1]]">{{ c.hand[0] }}{{ SUIT_SYMBOL[c.hand[1]] }}</span><span :class="SUIT_COLOR[c.hand[3]]">{{ c.hand[2] }}{{ SUIT_SYMBOL[c.hand[3]] }}</span>
                      </td>
                      <td class="pl-2">{{ mixText(c.freqs) }}</td>
                      <td class="text-right tabular-nums">{{ pct1(Math.min(1, c.weight)) }}</td>
                      <td class="text-right tabular-nums">{{ pct(c.equity) }}</td>
                      <td class="text-right tabular-nums">{{ node.has_ev ? money1(c.ev) : "…" }}</td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import * as api from "../../guided/api3";
import { studyRangeGrid, systemMemory } from "../../guided/api";
import { RANGES, PROFILES, LEAKS, RANK_CHARS, SUIT_CHARS, SUIT_SYMBOL, SUIT_COLOR, cardStr } from "../../guided/data";
import { actionColors } from "../../guided/coach";
import HandGrid from "./HandGrid.vue";
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
const scenarioId = ref("open_two");
const players = reactive<Seat3[]>([{ name: "", preset: "", range: "" }, { name: "", preset: "", range: "" }, { name: "", preset: "", range: "" }]);
const hero = ref(1);
const board = ref<number[]>([]);
const pot = ref(46);
const stack = ref(285);
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
  s.seats.forEach(([name, preset], i) => { players[i].name = name; players[i].preset = preset; players[i].range = RANGES[preset].range; });
  hero.value = s.hero; pot.value = s.pot; stack.value = s.stack;
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
    pot: pot.value, stack: stack.value, flop_bet_sizes: s.flop, flop_max_raises: s.turnMax, turn_bet_sizes: s.turn, turn_max_raises: s.turnMax,
    bet_sizes: s.bet, raise_sizes: s.raise, max_raises: s.max, allin_spr: 1.0, threads: 0,
    turn_cards: detail.value.turn, river_cards: board.value.length === 5 ? 0 : detail.value.river,
    accuracy_pct: [0.5, 0.3, 0.1][board.value.length - 3] ?? 0.1, max_iter: [400, 300, 600][board.value.length - 3] ?? 600, leaks: leaks.value,
  };
});
const problem = computed(() => {
  if (board.value.length < 3) return `Choose ${3 - board.value.length} more board card${board.value.length === 2 ? "" : "s"} (3 for a flop, 4 for a turn, 5 for a river).`;
  if (!(pot.value > 0) || !(stack.value > 0)) return "Enter a pot and stack above zero.";
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
const freeMb = ref(0);
const memoryProblem = computed(() => {
  const e = estimate.value; if (!e || !freeMb.value) return "";
  if (e.memory_mb < 0.8 * freeMb.value) return "";
  return `This needs about ${(e.memory_mb / 1024).toFixed(1)} GB of memory and about ${(freeMb.value / 1024).toFixed(1)} GB is free. Choose simpler bet sizes or a smaller sample, or close other programs.`;
});
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
async function solve() {
  solving.value = true; error.value = ""; progress.value = [1, 0, spec.value.max_iter, 100];
  const began = Date.now(); elapsed.value = 0;
  const timer = window.setInterval(async () => { elapsed.value = Math.round((Date.now() - began) / 1000); try { progress.value = await api.mwProgress(); } catch { /* ignore */ } }, 400);
  try {
    const s = await api.mwSolve(spec.value);
    if (s.error) { error.value = s.error; return; }
    summary.value = s;
    startLen.value = board.value.length;
    heroSolved.value = hero.value; habitSolved.value = Math.max(habitSeat.value, 0);
    which.value = s.locked ? "locked" : "base";
    history.value = []; line.value = []; selected.value = null;
    await load();
    step.value = "results";
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
async function load() {
  const token = ++loadToken; const w = which.value; const h = [...history.value];
  // strategies first (fast), then the EVs, which need a walk of everything below this point
  node.value = await api.mwNode(w, h, false);
  other.value = summary.value?.locked ? await api.mwNode(w === "base" ? "locked" : "base", h, false) : null;
  api.mwNode(w, h, true).then((full) => { if (token === loadToken) node.value = full; }).catch(() => undefined);
}
async function play(i: number) {
  if (!node.value || node.value.kind !== "player") return;
  line.value.push({ player: node.value.player, label: actionText(node.value.actions[i]) });
  history.value.push(i);
  await load();
}
const dealable = computed(() => new Set((node.value?.cards ?? []).map((c) => c[1])));
async function deal(card: number) {
  if (!node.value || node.value.kind !== "chance") return;
  const c = cardStr(card);
  line.value.push({ player: -1, label: `${node.value.street === "flop" ? "Turn" : "River"} ${c[0] === "T" ? "10" : c[0]}${SUIT_SYMBOL[c[1]]}` });
  history.value.push(card);
  await load();
}
async function goto(n: number) { history.value = history.value.slice(0, n); line.value = line.value.slice(0, n); await load(); }
async function setWhich(w: "base" | "locked") { which.value = w; await load(); }

const colors = computed(() => actionColors(node.value?.actions ?? []));
const money = (x: number) => `${x < -0.5 ? "-" : ""}$${Math.abs(Math.round(x))}`;
const money1 = (x: number) => `${x < -0.05 ? "-" : ""}$${Math.abs(x).toFixed(1)}`;
const pct = (x: number) => `${Math.round(x * 100)}%`;
const pct1 = (x: number) => `${(x * 100).toFixed(x > 0 && x < 0.1 ? 1 : 0)}%`;
const actionText = (a: string) => a.replace(/^(Bet|Raise to|All-in) (\d+)/, (_m, v, n) => `${v === "Bet" ? "Bets" : v === "Raise to" ? "Raises to" : "All-in"} $${n}`)
  .replace(/^Fold$/, "Folds").replace(/^Check$/, "Checks").replace(/^Call$/, "Calls");
const mixText = (f: number[]) => {
  const acts = node.value?.actions ?? [];
  const parts = f.map((x, i) => [x, i]).filter(([x]) => x >= 0.005).sort((a, b) => b[0] - a[0]).map(([x, i]) => `${actionText(acts[i]).toLowerCase()} ${pct(x)}`);
  return parts.join(", ");
};
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
    if (Math.abs(big) >= 0.03) out.push(`Biggest change ${which.value === "locked" ? "against the habit" : "compared with the habit solve"}: ${actionText(v.actions[bi]).toLowerCase()} ${big > 0 ? "more" : "less"} often (${pct1(Math.abs(big))} ${big > 0 ? "up" : "down"}).`);
  }
  return out;
});

onMounted(async () => {
  pickScenario("open_two");
  try { const m = await systemMemory(); freeMb.value = m[0] / 1048576; } catch { freeMb.value = 0; }
});
</script>

<style scoped>
.step-title { @apply font-display font-extrabold text-lg flex items-center gap-2.5; }
.step-num { @apply inline-flex items-center justify-center w-7 h-7 rounded-full bg-ink text-white text-sm font-body font-bold; }
.choice { @apply w-full text-left bg-white border border-rule rounded-lg px-3.5 py-3 hover:border-felt transition-colors; }
.choice-on { @apply border-felt ring-1 ring-felt bg-felt-light; }
.pill { @apply px-3 py-1.5 rounded-full border border-rule bg-white text-sm hover:border-felt transition-colors disabled:opacity-40; }
.pill-on { @apply border-felt bg-felt text-white hover:border-felt; }
.field { @apply flex flex-col text-sm gap-1 font-bold; }
.input { @apply rounded-md border-rule text-sm font-normal focus:border-felt focus:ring-felt; }
.hint { @apply text-sm text-muted; }
.deck-btn { @apply h-9 rounded-md border border-rule bg-white text-sm font-bold font-display hover:border-felt disabled:opacity-25 disabled:hover:border-rule; }
.deck-on { @apply !bg-ink !text-white border-ink; }
.stat { @apply rounded-lg bg-paper py-2; }
.stat dt { @apply text-xs text-muted; }
.stat dd { @apply font-display font-extrabold text-lg tabular-nums; }
.btn-solve { @apply w-full py-3 rounded-lg bg-felt text-white text-lg font-bold hover:bg-felt-dark disabled:opacity-40 disabled:hover:bg-felt; }
.btn-quiet { @apply px-3 py-1.5 rounded-md border border-rule bg-white text-sm font-bold hover:border-felt disabled:opacity-40; }
.crumb { @apply px-2.5 py-1 rounded-md bg-white border border-rule hover:border-felt; }
.panel { @apply bg-white rounded-xl border border-rule p-4; }
.action-row { @apply w-full flex items-center gap-3 rounded-lg border border-rule px-3 py-2 hover:border-felt hover:bg-felt-light/40 transition-colors; }
.seg { @apply inline-flex rounded-lg border border-ink overflow-hidden; }
.seg button { @apply px-3 py-1.5 text-sm font-bold bg-white; }
.seg .seg-on { @apply bg-ink text-white; }
button:focus-visible, input:focus-visible, select:focus-visible, textarea:focus-visible { @apply outline-none ring-2 ring-felt ring-offset-1; }
</style>
