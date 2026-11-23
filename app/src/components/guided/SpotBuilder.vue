<template>
  <div class="h-full overflow-y-auto bg-paper text-ink font-body">
    <div class="max-w-[1560px] mx-auto px-6 py-5">
      <div class="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h1 class="font-display font-extrabold text-[1.9rem] leading-tight tracking-tight">Study a spot</h1>
          <p class="text-muted mt-0.5 max-w-[80ch]">
            Describe the table and what each player did before the flop. The solver that fits is picked for you: two players, three players, or the closest it can get.
          </p>
        </div>
        <div class="flex gap-1.5">
          <button v-for="q in quickStarts" :key="q.id" class="pill" :title="q.help" @click="applyQuick(q.id)">{{ q.label }}</button>
          <button class="pill" :class="hhOpen ? 'pill-on' : ''" title="Fill everything in from a hand you played" @click="hhOpen = !hhOpen">Paste a hand history</button>
        </div>
      </div>
      <p v-if="fromPreflop" class="mt-3 rounded-lg border border-mine bg-mine-light px-3 py-2 text-sm">Loaded from the heads-up preflop solver: both ranges in step 3 are its ranges for this line. Pick a flop in step 4 and solve. <button class="underline font-bold" @click="fromPreflop = ''">Dismiss</button></p>
      <section v-if="hhOpen" class="panel mt-4">
        <h2 class="font-bold">Load a hand you played</h2>
        <p class="hint mt-1">Paste the hand's text as the site saved it. PokerStars, GGPoker, Winamax, 888poker, partypoker, Americas Cardroom, Ignition and Bovada, iPoker sites and PokerNow logs are read. The table, each player's preflop action, the stacks, the flop and your cards are filled in below for you to check.</p>
        <textarea v-model="hhText" rows="7" class="input w-full mt-2 font-mono text-xs" placeholder="Paste one hand here, from its first line to its last" aria-label="Hand history text"></textarea>
        <div class="mt-2 flex items-center gap-3">
          <button class="btn-quiet" :disabled="!hhText.trim()" @click="loadHistory">Load this hand</button>
          <span v-if="hhDone" class="text-sm font-bold text-felt">Loaded{{ hhSite ? ` (${hhSite} hand)` : "" }}. Check steps 1 to 4 below{{ heroHand ? `; your hand ${heroHand} will be tracked on the result` : "" }}.</span>
        </div>
        <p v-if="hhError" class="mt-2 text-sm font-bold text-danger">{{ hhError }}</p>
        <ul v-if="hhNotes.length" class="mt-2 space-y-1 text-sm">
          <li v-for="(n, i) in hhNotes" :key="i" class="flex gap-2"><span class="text-felt font-bold">•</span><span>{{ n }}</span></li>
        </ul>
      </section>

      <div class="grid gap-5 mt-4 xl:grid-cols-[minmax(0,1fr)_370px] items-start">
        <div class="space-y-5 min-w-0">
          <!-- 1 table -->
          <section class="panel">
            <h2 class="step-title"><span class="step-num">1</span>The table</h2>
            <div class="flex flex-wrap items-end gap-x-6 gap-y-3 mt-3">
              <div>
                <div class="lbl">Game</div>
                <div class="flex gap-1.5">
                  <button class="pill" :class="table.game === 'cash' ? 'pill-on' : ''" @click="setGame('cash')">Cash game</button>
                  <button class="pill" :class="table.game === 'tourney' ? 'pill-on' : ''" @click="setGame('tourney')">Tournament</button>
                </div>
              </div>
              <div>
                <div class="lbl">Players at the table</div>
                <div class="flex flex-wrap gap-1.5">
                  <button v-for="s in TABLE_SIZES" :key="s.n" class="pill" :class="table.size === s.n ? 'pill-on' : ''" @click="setSize(s.n)">{{ s.label }}</button>
                </div>
              </div>
              <template v-if="table.game === 'cash'">
                <label class="field"><span>Blinds</span>
                  <select v-model="blindId" class="input w-32" @change="setBlinds">
                    <option v-for="b in BLINDS" :key="b.id" :value="b.id">{{ b.label }}</option>
                    <option value="custom">Other…</option>
                  </select>
                </label>
                <template v-if="blindId === 'custom'">
                  <label class="field"><span>Small blind ($)</span><input v-model.number="table.sb" type="number" min="0" step="any" class="input w-24" /></label>
                  <label class="field"><span>Big blind ($)</span><input v-model.number="table.bb" type="number" min="0" step="any" class="input w-24" /></label>
                </template>
              </template>
              <label v-else class="field"><span>Antes</span>
                <select v-model="table.ante" class="input w-56">
                  <option value="bb">Big-blind ante (1 bb)</option>
                  <option value="each">Each player (0.125 bb)</option>
                  <option value="none">None</option>
                </select>
              </label>
              <label class="field"><span>Usual stack ({{ unit }})</span><input v-model.number="table.startStack" type="number" min="1" step="any" class="input w-28" @change="setAllStacks" /></label>
              <div v-if="table.game === 'tourney'" class="flex flex-wrap gap-1.5 pb-0.5">
                <button v-for="n in [15, 20, 25, 30, 40, 60, 100]" :key="n" class="pill !py-1" :class="table.startStack === n ? 'pill-on' : ''" @click="table.startStack = n; setAllStacks()">{{ n }} bb</button>
              </div>
              <div v-else class="text-sm text-muted pb-2">{{ Math.round(table.startStack / table.bb) }} big blinds deep</div>
            </div>
          </section>

          <!-- 2 preflop action -->
          <section class="panel">
            <div class="flex flex-wrap items-baseline justify-between gap-2">
              <h2 class="step-title"><span class="step-num">2</span>Before the flop, seat by seat</h2>
              <span class="text-sm text-muted">Seats are in the order they act. Pick your own seat with “This is me”.</span>
            </div>
            <div class="grid gap-2.5 mt-3" style="grid-template-columns: repeat(auto-fill, minmax(178px, 1fr))">
              <div v-for="st in seats" :key="st.seat" class="seat" :class="[inHand(st.seat) ? 'seat-in' : 'seat-out', hero === st.seat ? 'seat-me' : '']">
                <div class="flex items-center justify-between gap-2">
                  <div class="min-w-0">
                    <div class="font-display font-extrabold leading-tight">{{ st.seat }}</div>
                    <div class="text-[11px] text-muted truncate">{{ SEAT_NAME[st.seat] }}</div>
                  </div>
                  <button class="me-btn" :class="hero === st.seat ? 'me-on' : ''" :aria-pressed="hero === st.seat" @click="hero = st.seat">{{ hero === st.seat ? "You" : "This is me" }}</button>
                </div>
                <select v-model="st.act" class="input w-full mt-2 !py-1.5 font-bold" :aria-label="`${st.seat} action`" @change="onAct(st)">
                  <option v-for="a in pre.options[st.seat]?.acts ?? ['fold']" :key="a" :value="a">{{ actLabel(st.seat, a) }}</option>
                </select>
                <label v-if="st.act === 'raise'" class="flex items-center gap-1.5 text-xs mt-1.5">
                  <span class="text-muted">to</span>
                  <input v-model.number="st.raiseTo" type="number" min="0" step="any" class="input w-20 !py-1" :aria-label="`${st.seat} raise size`" /><span class="text-muted">{{ unit }}</span>
                </label>
                <select v-if="pre.options[st.seat]?.needsReply" v-model="st.reply" class="input w-full mt-1.5 !py-1 text-xs" :aria-label="`${st.seat} answer to the raise`">
                  <option value="call">then calls the raise</option>
                  <option value="fold">then folds to the raise</option>
                </select>
                <template v-if="st.act !== 'fold'">
                  <label class="flex items-center gap-1.5 text-xs mt-1.5">
                    <span class="text-muted w-9">Stack</span>
                    <input v-model.number="st.stack" type="number" min="0" step="any" class="input w-20 !py-1" :aria-label="`${st.seat} stack`" /><span class="text-muted">{{ unit }}</span>
                  </label>
                  <select v-if="hero !== st.seat" v-model="st.type" class="input w-full mt-1.5 !py-1 text-xs" :title="typeHelp(st.type)" :aria-label="`${st.seat} player type`">
                    <option v-for="t in PLAYER_TYPES" :key="t.id" :value="t.id">{{ t.label }}</option>
                  </select>
                </template>
              </div>
            </div>
            <p class="hint mt-3">{{ pre.story }} Player types change the starting ranges below. A player who acted before a later raise gets a “then calls / then folds” choice; a further re-raise by that player is not covered.</p>
          </section>

          <!-- 3 who sees the flop -->
          <section class="panel">
            <h2 class="step-title"><span class="step-num">3</span>Who sees the flop</h2>
            <div class="route mt-3" :class="route.kind === 'many' ? 'route-warn' : route.kind === 'none' ? 'route-none' : 'route-ok'">
              <div class="font-bold">{{ route.title }}</div>
              <p v-for="(t, i) in route.text" :key="i" class="text-sm mt-1">{{ t }}</p>
              <div v-if="route.kind === 'many'" class="mt-3">
                <div class="text-sm font-bold">Opponents to keep (pick two for a three-way solve, or one for an exact two-player solve)</div>
                <div class="grid gap-2 mt-2 md:grid-cols-2">
                  <label v-for="p in opponents" :key="p.seat" class="keep" :class="keep.includes(p.seat) ? 'keep-on' : ''">
                    <input type="checkbox" class="mt-0.5 rounded text-felt focus:ring-felt" :checked="keep.includes(p.seat)" :disabled="!keep.includes(p.seat) && keep.length >= 2" @change="toggleKeep(p.seat)" />
                    <span>
                      <b>{{ p.name }}</b> <span class="text-muted">· {{ p.role }}</span>
                      <span v-if="route.keep.includes(p.seat)" class="ml-1 text-xs font-bold text-felt">suggested</span>
                      <span class="block text-xs text-muted">{{ p.why ? cap(p.why) + "." : "Average influence on the hand." }}</span>
                    </span>
                  </label>
                </div>
                <p v-if="spot.left.length" class="text-sm mt-2">
                  Left out: <b>{{ spot.left.map((x) => x.name).join(", ") }}</b>. Their {{ amt(leftMoney) }} stays in the pot.
                </p>
              </div>
            </div>

            <div class="grid gap-3 mt-4" :class="spot.players.length >= 3 ? 'lg:grid-cols-3' : 'lg:grid-cols-2'">
              <div v-for="p in spot.players" :key="p.seat" class="rounded-lg border border-rule p-3" :class="p.hero ? 'bg-felt-light border-felt' : 'bg-surface'">
                <div class="flex items-start justify-between gap-2">
                  <div class="min-w-0">
                    <div class="font-bold truncate">{{ p.name }}</div>
                    <div class="text-xs text-muted">{{ cap(p.role) }} · {{ amt(p.behind) }} behind</div>
                  </div>
                  <div class="text-right shrink-0">
                    <div class="font-display font-extrabold tabular-nums">{{ pctOf(p.seat).toFixed(1) }}%</div>
                    <div class="text-[11px] text-muted">of hands</div>
                  </div>
                </div>
                <div class="flex gap-3 mt-2">
                  <div class="grid gap-px shrink-0 w-[104px] h-[104px]" style="grid-template-columns: repeat(13, 1fr)" aria-hidden="true">
                    <div v-for="(w, i) in miniGrid(p.seat)" :key="i" :style="{ background: w > 0 ? `rgb(var(--felt) / ${0.3 + 0.7 * w})` : 'rgb(var(--rule) / 0.55)' }"></div>
                  </div>
                  <div class="min-w-0 flex-1">
                    <textarea v-model="edits[p.seat]" rows="4" class="input w-full text-[11px] leading-snug font-body" :aria-label="`${p.name} range`"></textarea>
                  </div>
                </div>
                <RangeSlider v-model="edits[p.seat]" :label="p.name" />
                <div class="mt-1.5">
                  <select class="input w-full !py-1 text-xs" :aria-label="`Load a saved range for ${p.name}`" @change="loadPreset(p.seat, $event)">
                    <option value="">Load a built-in range…</option>
                    <option v-for="(r, k) in RANGES" :key="k" :value="k">{{ r.label }}</option>
                  </select>
                </div>
                <button v-if="edits[p.seat] !== p.range" class="text-xs text-felt underline font-bold mt-1" @click="edits[p.seat] = p.range">Back to the suggested range</button>
              </div>
            </div>
            <p v-if="spot.players.length" class="hint mt-2">
              Ranges are starting points worked out from each seat, action and player type. They are not solved preflop ranges: adjust them with the sliders or by typing.
            </p>
          </section>

          <!-- 4 board -->
          <section class="panel">
            <div class="flex flex-wrap items-baseline justify-between gap-2">
              <h2 class="step-title"><span class="step-num">4</span>The board</h2>
              <div class="flex flex-wrap gap-1.5">
                <button v-for="t in TEXTURES" :key="t.id" class="pill !py-1" @click="randomBoard(t.id)">{{ t.label }}</button>
                <button class="pill !py-1" @click="board = []">Clear</button>
              </div>
            </div>
            <div class="grid gap-[3px] mt-3 max-w-[760px]" style="grid-template-columns: repeat(13, minmax(0, 1fr))" role="group" aria-label="Board cards">
              <template v-for="s in [3, 2, 1, 0]" :key="s">
                <button
                  v-for="r in RANKS_DESC" :key="r * 4 + s" class="deck-btn"
                  :class="[SUIT_COLOR[SUIT_CHARS[s]], board.includes(r * 4 + s) ? 'deck-on' : '']"
                  :disabled="!board.includes(r * 4 + s) && board.length >= maxBoard"
                  @click="toggleCard(r * 4 + s)"
                >{{ RANK_CHARS[r] }}{{ SUIT_SYMBOL[SUIT_CHARS[s]] }}</button>
              </template>
            </div>
            <p class="hint mt-2">{{ boardHint }}</p>
            <details v-if="spot.players.length === 2" class="mt-3 border-t border-rule pt-3" :open="reportOpen" @toggle="reportOpen = ($event.target as HTMLDetailsElement).open">
              <summary class="cursor-pointer font-bold text-sm">Not sure which flop? Compare many flops at once</summary>
              <FlopReport v-if="reportOpen" class="mt-3" :hero="reportNames.hero" :oop="reportNames.oop" :ip="reportNames.ip" :disabled="!!reportProblem || !!solving" :solve="reportSolve" :cancel="studyCancel" @pick="board = $event" @busy="reporting = $event" />
              <p v-if="reportOpen && reportProblem" class="mt-2 text-sm font-bold text-danger">{{ reportProblem }}</p>
            </details>
          </section>

          <!-- 5 options -->
          <section class="panel">
            <h2 class="step-title"><span class="step-num">5</span>Extras <span class="font-body font-normal text-muted text-base">(optional)</span></h2>
            <div class="flex flex-wrap items-end gap-5 mt-3">
              <label class="field"><span>Exploit a habit</span>
                <select v-model="exploitSeat" class="input w-72">
                  <option value="">No: just the balanced strategy</option>
                  <option v-for="p in opponentsInSpot" :key="p.seat" :value="p.seat">{{ p.name }}: {{ profileLabel(p.type) }}</option>
                </select>
              </label>
              <p class="hint max-w-[60ch] pb-1">
                Solves the hand twice: balanced play first, then your best counter when that opponent plays their type's habits on every street. Pick a player type other than “Solid regular” in step 2 to choose the habit.
              </p>
            </div>
            <template v-if="true">
              <div v-if="route.kind === 'two' || spot.players.length === 2" class="flex flex-wrap items-end gap-4 mt-4">
                <label class="field"><span>Raiser's flop bets</span><input v-model="sizes.pfrFlop" class="input w-32" /></label>
                <label class="field"><span>Caller's flop bets</span><input v-model="sizes.callerFlop" class="input w-32" /></label>
                <label class="field"><span>Turn bets</span><input v-model="sizes.turn" class="input w-28" /></label>
                <label class="field"><span>River bets</span><input v-model="sizes.river" class="input w-28" /></label>
                <label class="field"><span>Raises</span><input v-model="sizes.raise" class="input w-24" /></label>
                <label class="field"><span>Accuracy (% of pot)</span><input v-model.number="accuracyPct" type="number" min="0.05" max="5" step="0.05" class="input w-24" /></label>
                <p class="hint basis-full">Bet sizes are shares of the pot, separated by commas (“33%, 75%”). Raises are multiples of the bet (“3x”). More sizes mean a slower solve and more memory.</p>
                <label class="flex items-start gap-2 basis-full cursor-pointer">
                  <input v-model="autoSizes" type="checkbox" class="mt-1" />
                  <span><b>Pick the bet sizes for me.</b> <span class="hint">A rough first solve tries 33, 50, 75, 100 and 150% of the pot on the {{ board.length === 3 ? "flop" : board.length === 4 ? "turn" : "river" }} and keeps the two each player uses most; the real solve then uses those instead of the sizes typed above for that street. Later streets keep the sizes above. Adds time, and the first solve needs more memory than the figure shown.</span></span>
                </label>
              </div>
              <div v-else-if="spot.players.length === 3" class="flex flex-wrap items-end gap-4 mt-4">
                <label class="field"><span>Bet sizes</span>
                  <select v-model="sizing3" class="input w-80" @change="touched3 = true">
                    <option value="std">Standard: one size early, river 33% or 75%</option>
                    <option value="simple">Simple: one size, 66% (fastest; best for flops)</option>
                    <option value="big">Detailed: two sizes early, three river sizes</option>
                  </select>
                </label>
                <label class="field"><span>Later cards</span>
                  <select v-model="later3" class="input w-80" @change="touched3 = true">
                    <option value="fast">Balanced sample (fastest)</option>
                    <option value="better">Larger sample</option>
                    <option value="exact">Every card (exact; flops need a lot of memory)</option>
                  </select>
                </label>
                <template v-if="table.game === 'cash'">
                  <label class="field"><span>Rake (% of pot)</span><input v-model.number="rakePct" type="number" min="0" max="20" step="0.5" class="input w-24" /></label>
                  <label class="field"><span>Rake cap ($)</span><input v-model.number="rakeCap" type="number" min="0" class="input w-24" /></label>
                </template>
              </div>
            </template>
          </section>
        </div>

        <!-- summary -->
        <aside class="panel xl:sticky xl:top-4">
          <div class="flex gap-2 flex-wrap">
            <div v-for="c in boardCards" :key="c" class="w-11 h-14 rounded-md border border-rule bg-surface flex flex-col items-center justify-center font-display font-extrabold" :class="SUIT_COLOR[c[1]]">
              <span>{{ c[0] === "T" ? "10" : c[0] }}</span><span class="text-lg leading-none">{{ SUIT_SYMBOL[c[1]] }}</span>
            </div>
            <div v-if="!boardCards.length" class="text-sm text-muted h-14 flex items-center">No board yet</div>
          </div>
          <p class="text-sm mt-3">{{ pre.story }}</p>
          <dl class="grid grid-cols-3 gap-2 mt-3">
            <div class="stat"><dt>Pot</dt><dd>{{ amt(pre.pot) }}</dd></div>
            <div class="stat"><dt>Players</dt><dd>{{ spot.players.length }}<span v-if="spot.left.length" class="text-muted text-xs"> of {{ pre.players.length }}</span></dd></div>
            <div class="stat"><dt>Stack-to-pot</dt><dd>{{ sprText }}</dd></div>
          </dl>
          <ol class="mt-3 space-y-1 text-sm">
            <li v-for="(p, i) in spot.players" :key="p.seat" class="flex justify-between gap-2">
              <span class="truncate"><span class="text-muted">{{ i + 1 }}.</span> <b>{{ p.name }}</b></span>
              <span class="text-muted whitespace-nowrap">{{ amt(p.behind) }} · {{ pctOf(p.seat).toFixed(0) }}%</span>
            </li>
          </ol>
          <p v-if="spot.players.length" class="hint mt-1">Listed in the order they act after the flop.</p>
          <div class="mt-3 text-sm font-bold">{{ engineText }}</div>
          <p v-if="stackNote" class="hint mt-1">{{ stackNote }}</p>
          <div v-if="table.game === 'tourney' && (spot.players.length === 2 || spot.players.length === 3)" class="mt-3 rounded-xl border border-rule p-3 text-sm">
            <label class="flex items-center gap-2 font-bold"><input v-model="icmOn" type="checkbox" class="accent-current" /> Play for prize money, not chips</label>
            <template v-if="icmOn">
              <label class="field mt-2"><span>Prizes, first place first</span><input v-model="icmPayouts" class="input w-full" placeholder="50, 30, 20" aria-label="Prizes, first place first" /></label>
              <label class="field mt-2"><span>Stacks at other tables (bb), if any</span><input v-model="icmExtra" class="input w-full" placeholder="e.g. 35, 22" aria-label="Stacks at other tables in big blinds" /></label>
              <p class="hint mt-2">
                Sitting out of this hand: {{ icmOthers.length ? icmOthers.map((x) => `${x} bb`).join(", ") : "nobody" }}.
                Each result of the hand is valued by what the stacks it leaves are worth in prizes (the Independent Chip Model), so a player who can be knocked out plays tighter.
                Stacks at this table come from step 1.
              </p>
              <p v-if="icmProblem" class="mt-1 font-bold text-danger">{{ icmProblem }}</p>
            </template>
          </div>
          <p v-if="!problem && estimate.text && !solving" class="hint mt-2">{{ estimate.text }}</p>
          <div v-if="solving" class="mt-4" role="status">
            <div class="flex justify-between text-sm font-bold">
              <span>{{ phaseText }}</span>
              <button class="text-felt underline" @click="stopEarly">Stop early</button>
            </div>
            <div class="h-2 bg-paper rounded mt-1.5 overflow-hidden"><div class="h-full bg-felt transition-all" :style="{ width: progressPct + '%' }"></div></div>
            <p class="hint mt-1.5">Step {{ progress[1] }} of up to {{ progress[2] }}, {{ elapsedText }} so far. {{ progress[3] >= 100 ? "Measuring accuracy…" : `Within ${progress[3].toFixed(2)}% of the pot so far.` }}</p>
            <p class="hint mt-1">The result opens by itself when this finishes. You can keep reading this page meanwhile; changes made now apply to the next solve.</p>
          </div>
          <button v-else class="btn-solve mt-4" :disabled="!!problem || !!estimate.problem || reporting" @click="go">Solve this spot</button>
          <p v-if="problem || estimate.problem || runError" class="mt-2 text-sm font-bold text-danger">{{ problem || estimate.problem || runError }}</p>
        </aside>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, reactive, ref, watch } from "vue";
import RangeSlider from "./RangeSlider.vue";
import FlopReport, { type FlopRow } from "./FlopReport.vue";
import { parseHandHistory, type LaterStep } from "../../guided/handHistory";
import type { ReviewStep } from "../../guided/api";
import { useStore } from "../../store";
import { RANGES, RANK_CHARS, SUIT_CHARS, SUIT_COLOR, SUIT_SYMBOL, cardStr, PROFILES, type ScenarioResult } from "../../guided/data";
import { parseRange, rangePercent } from "../../guided/rangeTools";
import { studyCancel, studyProgress } from "../../guided/api";
import { mwCancel, mwProgress } from "../../guided/api3";
import { PLAYER_TYPES, SEAT_NAME, TABLE_SIZES, buildPreflop, fmtAmt, routeFor, seatsFor, spotFor, type Act, type SeatInput, type TableSetup, type TypeId } from "../../guided/preflop";

type Outcome = { ok: boolean; error: string };
type Preview = { text: string; problem: string };
const props = defineProps<{
  run2: (p: Solve2) => Promise<Outcome>; run3: (p: Solve3) => Promise<Outcome>;
  preview2: (p: Solve2) => Promise<Preview>; preview3: (p: Solve3) => Promise<Preview>;
  quickFlop: (p: Solve2, flop: number[]) => Promise<FlopRow>;
}>();
export interface Solve2 { autoSizes?: boolean; res: ScenarioResult; format: "live" | "hu"; board: number[]; profile: string | null; heroHand?: string; overrides: Record<string, unknown>;
  /** from a pasted hand history: what happened after the flop, for the hand review (amounts in the solve's chips) */
  line?: ReviewStep[] }
export interface Solve3 {
  format: "cash" | "tourney"; names: string[]; ranges: string[]; hero: number; pot: number; stacks: number[]; board: number[];
  rakePct: number; rakeCap: number; habitSeat: number; profile: string | null; sizingId: string; laterId: string;
  /** play for prize money: prizes (first place first) and the stacks, in big blinds, of players not in the hand */
  icm?: { payouts: number[]; others: number[] };
}
const emit = defineEmits<{ (e: "done", kind: "guided" | "threeway"): void }>();

const BLINDS = [
  { id: "1/2", label: "$1 / $2", sb: 1, bb: 2 }, { id: "1/3", label: "$1 / $3", sb: 1, bb: 3 }, { id: "2/5", label: "$2 / $5", sb: 2, bb: 5 }, { id: "5/10", label: "$5 / $10", sb: 5, bb: 10 },
];
const RANKS_DESC = [12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0];
const table = reactive<TableSetup>({ game: "cash", size: 9, sb: 1, bb: 3, ante: "none", startStack: 300 });
const blindId = ref("1/3");
const unit = computed(() => (table.game === "tourney" ? "bb" : "$"));
const amt = (x: number) => fmtAmt(table, x);
const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

const seats = reactive<SeatInput[]>([]);
const hero = ref("BTN");
const openTo = () => (table.game === "tourney" ? (table.size === 2 ? 2 : 2.2) : table.bb * (table.bb <= 3 ? 5 : 3));
function resetSeats(quick = "open_call") {
  seats.splice(0, seats.length, ...seatsFor(table.size).map((s) => ({ seat: s, act: "fold" as Act, raiseTo: 0, reply: "call" as const, stack: table.startStack, type: "solid" as TypeId })));
  applyQuick(quick);
}
const seat = (s: string) => seats.find((x) => x.seat === s);
const quickStarts = [
  { id: "open_call", label: "Raise and one call", help: "A raise and one caller: the most common pot" },
  { id: "open_two", label: "Raise, two callers", help: "A raise, a call in position and the big blind comes along" },
  { id: "threebet", label: "3-bet pot", help: "A raise, a re-raise, and the first raiser calls" },
  { id: "limped", label: "Limped pot", help: "Two limpers and the blinds, no raise" },
];
function applyQuick(id: string) {
  seats.forEach((s) => { s.act = "fold"; s.raiseTo = 0; s.reply = "call"; });
  const o = seatsFor(table.size); const open = openTo();
  const set = (s: string, act: Act, to = 0) => { const x = seat(s); if (x) { x.act = act; x.raiseTo = to; } };
  if (table.size === 2) {
    if (id === "limped") { set("BTN", "limp"); set("BB", "limp"); }
    else if (id === "threebet") { set("BTN", "raise", open); set("BB", "raise", open * 4); }
    else { set("BTN", "raise", open); set("BB", "call"); }
    hero.value = "BB"; return;
  }
  const opener = o.includes("LJ") ? "LJ" : "BTN";
  if (id === "open_call") { set(opener, "raise", open); set(opener === "BTN" ? "BB" : "BTN", "call"); hero.value = opener === "BTN" ? "BB" : "BTN"; }
  else if (id === "open_two") { set(opener === "BTN" ? "BTN" : "CO", "raise", open); if (opener !== "BTN") set("BTN", "call"); else set("SB", "call"); set("BB", "call"); hero.value = opener === "BTN" ? "BTN" : "CO"; }
  else if (id === "threebet") { set(opener === "BTN" ? "BTN" : "CO", "raise", open); set(opener === "BTN" ? "BB" : "BTN", "raise", open * 3.2); hero.value = opener === "BTN" ? "BTN" : "CO"; }
  else { if (o.includes("LJ")) { set("LJ", "limp"); set("CO", "limp"); } set("BTN", "limp"); set("SB", "limp"); set("BB", "limp"); hero.value = "BTN"; }
}
function setGame(g: "cash" | "tourney") {
  table.game = g;
  if (g === "tourney") { table.sb = 0.5; table.bb = 1; table.ante = "bb"; table.startStack = 25; }
  else { setBlinds(); table.ante = "none"; }
  resetSeats();
}
function setSize(n: number) { table.size = n; resetSeats(); }
function setBlinds() {
  const b = BLINDS.find((x) => x.id === blindId.value);
  if (b) { table.sb = b.sb; table.bb = b.bb; }
  table.startStack = table.bb * 100; resetSeats();
}
function setAllStacks() { seats.forEach((s) => { s.stack = table.startStack; }); }
function onAct(st: SeatInput) { if (st.act === "raise" && !(st.raiseTo > (pre.value.options[st.seat]?.facing ?? 0))) st.raiseTo = pre.value.raises > 1 || (pre.value.options[st.seat]?.facing ?? 0) > table.bb ? Math.round((pre.value.options[st.seat]?.facing ?? table.bb) * 3.2 * 10) / 10 : openTo(); }
resetSeats();

const pre = computed(() => buildPreflop(table, seats, hero.value));
const inHand = (s: string) => pre.value.players.some((p) => p.seat === s);
const actLabel = (s: string, a: Act) => {
  const o = pre.value.options[s];
  if (a === "fold") return "Folds";
  if (a === "limp") return s === "BB" ? "Checks" : s === "SB" ? "Completes" : "Limps";
  if (a === "call") return `Calls ${amt(o?.facing ?? 0)}`;
  return (o?.facing ?? 0) > (table.game === "tourney" ? 1 : table.bb) ? "Re-raises" : "Raises";
};
const typeHelp = (t: TypeId) => PLAYER_TYPES.find((x) => x.id === t)?.help ?? "";
const profileLabel = (t: TypeId) => { const p = PLAYER_TYPES.find((x) => x.id === t)?.profile; return p ? PROFILES.find((x) => x.id === p)?.label ?? "habits" : "no habit set (solid regular)"; };

const route = computed(() => routeFor(pre.value));
const keep = ref<string[]>([]);
watch(() => route.value.keep.join(","), () => { keep.value = [...route.value.keep]; }, { immediate: true });
const toggleKeep = (s: string) => { keep.value = keep.value.includes(s) ? keep.value.filter((x) => x !== s) : [...keep.value, s]; };
const opponents = computed(() => pre.value.players.filter((p) => !p.hero));
const spot = computed(() => spotFor(pre.value, keep.value));
const opponentsInSpot = computed(() => spot.value.players.filter((p) => !p.hero));
const leftMoney = computed(() => spot.value.left.reduce((a, p) => a + p.put, 0));

// range text per seat: starts as the suggestion and follows it until the user edits
const edits = reactive<Record<string, string>>({});
const lastSuggested: Record<string, string> = {};
watch(() => pre.value.players.map((p) => p.seat + "=" + p.range).join("|"), () => {
  for (const p of pre.value.players) { if (edits[p.seat] === undefined || edits[p.seat] === lastSuggested[p.seat]) edits[p.seat] = p.range; lastSuggested[p.seat] = p.range; }
}, { immediate: true });
const pctOf = (s: string) => rangePercent(parseRange(edits[s] ?? ""));
const R = "AKQJT98765432";
const miniGrid = (s: string) => {
  const w = parseRange(edits[s] ?? "").weights; const out: number[] = [];
  for (let r = 0; r < 13; r++) for (let c = 0; c < 13; c++) out.push(w.get(r === c ? R[r] + R[c] : c > r ? R[r] + R[c] + "s" : R[c] + R[r] + "o") ?? 0);
  return out;
};
const loadPreset = (s: string, ev: Event) => { const k = (ev.target as HTMLSelectElement).value; if (k && RANGES[k]) edits[s] = RANGES[k].range; (ev.target as HTMLSelectElement).value = ""; };

// board
const board = ref<number[]>([47, 22, 1]);
const maxBoard = computed(() => (spot.value.players.length === 3 ? 5 : 3));
watch(maxBoard, (m) => { if (board.value.length > m) board.value = board.value.slice(0, m); });
const toggleCard = (c: number) => { const i = board.value.indexOf(c); if (i >= 0) board.value.splice(i, 1); else if (board.value.length < maxBoard.value) board.value.push(c); };
const boardCards = computed(() => board.value.map(cardStr));
const boardHint = computed(() => (spot.value.players.length === 3 ? "Three players: pick 3 cards for a flop, 4 to start on the turn, or 5 for a river spot." : "Two players: pick the three flop cards. Turn and river cards are all solved, and you choose them while reading the result."));
const TEXTURES = [{ id: "any", label: "Random flop" }, { id: "dry", label: "Dry, high card" }, { id: "wet", label: "Wet and connected" }, { id: "paired", label: "Paired" }, { id: "mono", label: "One suit" }];
function randomBoard(kind: string) {
  const rnd = (n: number) => Math.floor(Math.random() * n);
  for (let tries = 0; tries < 400; tries++) {
    const cs = new Set<number>(); while (cs.size < 3) cs.add(rnd(52));
    const c = [...cs]; const ranks = c.map((x) => x >> 2).sort((a, b) => b - a); const suits = new Set(c.map((x) => x & 3)).size;
    const paired = new Set(ranks).size < 3; const span = ranks[0] - ranks[2];
    const ok = kind === "any" || (kind === "dry" && !paired && suits === 3 && ranks[0] >= 10 && span >= 5 && ranks[1] - ranks[2] >= 2)
      || (kind === "wet" && !paired && suits === 2 && span <= 4) || (kind === "paired" && paired && new Set(ranks).size === 2) || (kind === "mono" && suits === 1);
    if (ok) { board.value = c.sort((a, b) => b - a); return; }
  }
}

// extras
const exploitSeat = ref("");
watch(() => opponentsInSpot.value.map((p) => p.seat).join(","), () => { if (!opponentsInSpot.value.some((p) => p.seat === exploitSeat.value)) exploitSeat.value = ""; });
const sizes = reactive({ pfrFlop: "33%, 66%", callerFlop: "33%", turn: "75%", river: "75%", raise: "3x" });
const accuracyPct = ref(0.5);
const autoSizes = ref(false);
const sizing3 = ref("simple"); const later3 = ref("fast");
// flops default to the light tree and a card sample; later streets to fuller sizes and every card. A manual choice sticks.
const touched3 = ref(false);
watch(() => board.value.length, (n) => { if (touched3.value) return; sizing3.value = n <= 3 ? "simple" : "std"; later3.value = n <= 3 ? "fast" : "exact"; }, { immediate: true });
const rakePct = ref(0); const rakeCap = ref(0);

// tournament prize money (two-player pots): prizes, plus every stack not in the hand
const icmOn = ref(false); const icmPayouts = ref("50, 30, 20"); const icmExtra = ref("");
const numList = (t: string) => t.split(/[,\s]+/).map((x) => Number(x)).filter((x) => Number.isFinite(x) && x > 0);
const icmOthers = computed(() => [...pre.value.out.map((x) => x.behind), ...spot.value.left.map((x) => x.behind), ...numList(icmExtra.value)].filter((x) => x > 0));
const icmActive = computed(() => icmOn.value && table.game === "tourney" && (spot.value.players.length === 2 || spot.value.players.length === 3));
const icmProblem = computed(() => {
  if (!icmActive.value) return "";
  if (!numList(icmPayouts.value).length) return "Enter at least one prize.";
  if (icmOthers.value.length > 7) return "Prize money works with up to 7 players outside the hand. Combine or drop the smallest stacks.";
  if (!icmOthers.value.length) return "With nobody else left in the tournament, prize money and chips are the same game.";
  return "";
});

const sprText = computed(() => {
  const b = spot.value.players.map((p) => p.behind); if (!b.length || !(pre.value.pot > 0)) return "–";
  const lo = Math.min(...b) / pre.value.pot, hi = Math.max(...b) / pre.value.pot;
  return Math.abs(hi - lo) < 0.05 ? lo.toFixed(1) : `${lo.toFixed(1)}–${hi.toFixed(1)}`;
});
const engineText = computed(() => (spot.value.players.length === 2 ? "Two-player solver, every card exact" : spot.value.players.length === 3 ? "Three-player solver" : "Pick who to keep in step 3"));
const stackNote = computed(() => {
  const ps = spot.value.players;
  if (ps.length === 2 && Math.abs(ps[0].behind - ps[1].behind) > 1e-9) return `Two-player solves use the shorter stack for both players: ${amt(Math.min(ps[0].behind, ps[1].behind))}. The extra chips could not be bet anyway.`;
  if (ps.length === 3 && new Set(ps.map((p) => p.behind)).size > 1) return "Stacks differ, so the solve includes side pots.";
  return "";
});
const problem = computed(() => {
  if (pre.value.errors.length) return pre.value.errors[0];
  const ps = spot.value.players;
  if (route.value.kind === "none") return route.value.text[0];
  if (ps.length > 3) return "Choose at most two opponents to keep in step 3.";
  if (ps.length < 2) return "Keep at least one opponent in step 3.";
  if (icmActive.value && icmProblem.value && icmOthers.value.length) return icmProblem.value;
  if (ps.some((p) => !(pctOf(p.seat) > 0))) return "Every player needs at least one hand in their range.";
  if (ps.length === 2 && board.value.length !== 3) return "Pick the three flop cards in step 4.";
  if (ps.length === 3 && board.value.length < 3) return "Pick at least three board cards in step 4.";
  return "";
});

function payload(): { two?: Solve2; three?: Solve3 } {
  const ps = spot.value.players; const tourney = table.game === "tourney";
  const hab = ps.find((p) => p.seat === exploitSeat.value);
  const profile = hab ? PLAYER_TYPES.find((t) => t.id === hab.type)?.profile ?? "austin" : null;
  const left = spot.value.left.length ? ` (${spot.value.left.map((p) => p.name).join(" and ")} left out; their chips stay in the pot.)` : "";
  if (ps.length === 2) {
    const k = tourney ? 10 : 1; // tournament amounts go to the two-player screen in tenths of a big blind
    const [oop, ip] = ps;
    return { two: {
      res: {
        heroIsOop: oop.hero, oopRange: edits[oop.seat], ipRange: edits[ip.seat], pot: Math.max(1, Math.round(pre.value.pot * k)),
        stack: Math.max(1, Math.round(Math.min(oop.behind, ip.behind) * k)), pfrIsOop: oop.raisedLast,
        description: pre.value.story + left, oopName: oop.name, ipName: ip.name,
      },
      format: tourney ? "hu" : "live", board: [...board.value], profile, heroHand: heroHand.value, line: reviewLine(oop.seat, ip.seat, k), autoSizes: autoSizes.value,
      overrides: { oop_flop_bet: oop.raisedLast ? sizes.pfrFlop : sizes.callerFlop, ip_flop_bet: oop.raisedLast ? sizes.callerFlop : sizes.pfrFlop, turn_bet: sizes.turn, river_bet: sizes.river, raise: sizes.raise, accuracy_pct: accuracyPct.value,
        ...(icmActive.value && icmOthers.value.length && numList(icmPayouts.value).length ? { icm: { payouts: numList(icmPayouts.value), oop_stack: Math.round(oop.behind * k), ip_stack: Math.round(ip.behind * k), others: icmOthers.value.map((x) => Math.round(x * k)) } } : {}) },
    } };
  }
  return { three: {
    format: table.game, names: ps.map((p) => p.name), ranges: ps.map((p) => edits[p.seat]), hero: ps.findIndex((p) => p.hero), pot: pre.value.pot, stacks: ps.map((p) => p.behind),
    board: [...board.value], rakePct: tourney ? 0 : rakePct.value, rakeCap: tourney ? 0 : rakeCap.value, habitSeat: hab ? ps.indexOf(hab) : -1, profile, sizingId: sizing3.value, laterId: later3.value,
    ...(icmActive.value && icmOthers.value.length && numList(icmPayouts.value).length ? { icm: { payouts: numList(icmPayouts.value), others: [...icmOthers.value] } } : {}),
  } };
}

// size and time of the spot as it stands, refreshed a moment after the last change
// flop report: the same two-player spot on a spread of flops
const reportOpen = ref(false); const reporting = ref(false);
const reportNames = computed(() => { const ps = spot.value.players; return { oop: ps[0]?.name ?? "", ip: ps[1]?.name ?? "", hero: ps.find((p) => p.hero)?.name ?? ps[0]?.name ?? "" }; });
const reportProblem = computed(() => (pre.value.errors.length ? pre.value.errors[0] : spot.value.players.some((p) => !edits[p.seat]?.trim()) ? "Give both players a range in step 3 first." : ""));
const reportSolve = (flop: number[]) => { const two = payload().two; if (!two) throw new Error("The flop report needs exactly two players."); return props.quickFlop({ ...two, profile: null }, flop); };
// a line from the heads-up preflop solver: set the table up and use its ranges as they are
const store = useStore();
const fromPreflop = ref("");
watch(() => store.preflopLine, async (l) => {
  if (!l) return;
  store.preflopLine = null;
  Object.assign(table, { game: "tourney", size: 2, sb: 0.5, bb: 1, ante: l.ante > 0 ? "each" : "none", startStack: l.stack });
  const mk = (seat: string): SeatInput => ({ seat, act: "fold", raiseTo: 0, reply: "call", stack: l.stack, type: "solid" });
  const btn = mk("BTN"), bb = mk("BB"); let raised = false;
  for (const step of l.path.split(" > ")) {
    const [who, what, size] = step.split(" "); const x = who === "SB" ? btn : bb;
    if (what === "limp") x.act = "limp"; else if (what === "check") x.act = "limp";
    else if (what === "raise") { x.act = "raise"; x.raiseTo = Number(size); raised = true; }
    else if (what === "call") { if (x.act === "fold") x.act = raised ? "call" : "limp"; else x.reply = "call"; }
  }
  seats.splice(0, seats.length, btn, bb); hero.value = "BB";
  await nextTick(); await nextTick();
  edits.BTN = l.button; edits.BB = l.bigBlind; fromPreflop.value = l.path; hhOpen.value = false;
}, { immediate: true });
// a pasted hand history fills the whole setup in
const hhOpen = ref(false); const hhText = ref(""); const hhError = ref(""); const hhNotes = ref<string[]>([]); const hhDone = ref(false);
const heroHand = ref("");
const hhSite = ref("");
const hhLater = ref<LaterStep[]>([]); const hhFlop = ref("");
/** The pasted hand's play after the flop, for the two players in the solve; nothing if anyone else acted or the flop was changed. */
function reviewLine(oopSeat: string, ipSeat: string, k: number): ReviewStep[] | undefined {
  if (!hhDone.value || !heroHand.value || !hhLater.value.length || board.value.slice(0, 3).join() !== hhFlop.value) return undefined;
  const out: ReviewStep[] = [];
  for (const st of hhLater.value) {
    if (st.kind === "card") { out.push({ kind: "card", card: st.card }); continue; }
    if (st.seat !== oopSeat && st.seat !== ipSeat) return undefined;
    out.push({ kind: "action", who: st.seat === oopSeat ? 0 : 1, verb: st.verb, to: Math.round(st.to * k * 100) / 100 });
  }
  return out;
}
async function loadHistory() {
  hhError.value = ""; hhNotes.value = []; hhDone.value = false;
  try {
    const h = parseHandHistory(hhText.value);
    Object.assign(table, h.table);
    blindId.value = table.game === "cash" ? BLINDS.find((b) => b.sb === h.table.sb && b.bb === h.table.bb)?.id ?? "custom" : blindId.value;
    seats.splice(0, seats.length, ...h.seats);
    hero.value = h.hero;
    await nextTick();
    board.value = [...h.board]; heroHand.value = h.heroHand; hhNotes.value = h.notes; hhSite.value = h.site; hhDone.value = true;
    hhLater.value = h.later; hhFlop.value = h.board.join();
  } catch (e) { hhError.value = String((e as Error).message ?? e); }
}
watch(() => [hero.value, seats.map((x) => x.act).join()], () => { if (!hhDone.value) heroHand.value = ""; });
const estimate = reactive<Preview>({ text: "", problem: "" });
let estTimer: number | undefined; let estToken = 0;
watch(() => (problem.value ? "" : JSON.stringify(payload())), (key) => {
  window.clearTimeout(estTimer); const token = ++estToken;
  if (!key) { estimate.text = ""; estimate.problem = ""; return; }
  estTimer = window.setTimeout(async () => {
    const p = payload();
    const r = p.two ? await props.preview2(p.two) : await props.preview3(p.three!);
    if (token === estToken) { estimate.text = r.text; estimate.problem = r.problem; }
  }, 500);
}, { immediate: true });

// solving stays on this page; the result opens when it is ready
const solving = ref<"" | "two" | "three">("");
const runError = ref("");
const progress = ref<[number, number, number, number]>([0, 0, 1, 100]);
const elapsed = ref(0);
const withHabit = ref(false);
const progressPct = computed(() => Math.min(100, Math.round((progress.value[1] / Math.max(1, progress.value[2])) * 100)));
const elapsedText = computed(() => (elapsed.value < 60 ? `${elapsed.value}s` : `${Math.floor(elapsed.value / 60)}m ${elapsed.value % 60}s`));
const phaseText = computed(() => (!withHabit.value ? "Solving…" : progress.value[0] >= 2 ? "Step 2 of 2: your best counter to the habit" : "Step 1 of 2: balanced play"));
const stopEarly = () => (solving.value === "two" ? studyCancel() : mwCancel());
async function go() {
  if (solving.value || problem.value) return;
  const p = payload(); const kind = p.two ? "two" : "three";
  solving.value = kind; runError.value = ""; progress.value = [0, 0, 1, 100]; withHabit.value = !!(p.two?.profile || (p.three && p.three.habitSeat >= 0));
  const began = Date.now(); elapsed.value = 0;
  const timer = window.setInterval(async () => {
    elapsed.value = Math.round((Date.now() - began) / 1000);
    try { const g = kind === "two" ? await studyProgress() : await mwProgress(); progress.value = [g[0], g[1], g[2] || 1, g[3] > 1e5 || g[1] === 0 ? 100 : g[3]]; } catch { /* keep the last reading */ }
  }, 400);
  try {
    const r = p.two ? await props.run2(p.two) : await props.run3(p.three!);
    if (r.ok) emit("done", kind === "two" ? "guided" : "threeway"); else runError.value = r.error || "The solve was stopped before it produced a result.";
  } catch (e) { runError.value = String((e as Error).message ?? e); }
  finally { window.clearInterval(timer); solving.value = ""; }
}
</script>

<style scoped>
.btn-quiet { @apply px-3 py-1.5 rounded-lg border border-rule bg-surface text-sm font-bold hover:border-felt disabled:opacity-50; }
.pill { @apply px-3 py-1.5 rounded-full border border-rule bg-surface text-sm hover:border-felt transition-colors whitespace-nowrap; }
.pill-on { @apply border-felt bg-felt text-white hover:border-felt; }
.field { @apply flex flex-col text-sm gap-1 font-bold; }
.lbl { @apply text-sm font-bold mb-1; }
.input { @apply rounded-md border-rule text-sm font-normal focus:border-felt focus:ring-felt; }
.hint { @apply text-sm text-muted; }
.panel { @apply bg-surface rounded-xl border border-rule p-4; }
.step-title { @apply font-display font-extrabold text-lg flex items-center gap-2; }
.step-num { @apply inline-flex items-center justify-center w-7 h-7 rounded-full bg-ink text-white text-sm; }
.seat { @apply rounded-lg border p-2.5 transition-colors; }
.seat-in { @apply bg-surface border-ink/40; }
.seat-out { @apply bg-paper border-rule; }
.seat-out select:first-of-type { @apply text-muted font-normal; }
.seat-me { @apply border-felt bg-felt-light ring-1 ring-felt; }
.me-btn { @apply shrink-0 px-2 py-0.5 rounded-full border border-rule bg-surface text-[11px] font-bold hover:border-felt whitespace-nowrap; }
.me-on { @apply bg-felt border-felt text-white; }
.route { @apply rounded-lg border p-3; }
.route-ok { @apply border-felt bg-felt-light; }
.route-warn { @apply border-mine bg-mine-light; }
.route-none { @apply border-rule bg-paper; }
.keep { @apply flex items-start gap-2 rounded-lg border border-rule bg-surface p-2 text-sm cursor-pointer; }
.keep-on { @apply border-felt; }
.deck-btn { @apply h-9 rounded border border-rule bg-surface text-sm font-bold font-display hover:border-felt disabled:opacity-30 disabled:hover:border-rule; }
.deck-on { @apply !bg-ink !text-white border-ink; }
.stat { @apply rounded-lg bg-paper px-2 py-1.5 text-center; }
.stat dt { @apply text-[11px] text-muted; }
.stat dd { @apply font-display font-extrabold tabular-nums; }
.btn-solve { @apply w-full py-3 rounded-lg bg-felt text-white text-lg font-bold hover:opacity-90 disabled:opacity-40; }
button:focus-visible, input:focus-visible, select:focus-visible, textarea:focus-visible { @apply outline-none ring-2 ring-felt ring-offset-1; }
</style>
