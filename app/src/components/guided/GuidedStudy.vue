<template>
  <div class="h-full overflow-y-auto bg-paper text-ink font-body">
    <div class="mx-auto px-6 py-6" :class="step === 'results' ? 'max-w-[1680px]' : 'max-w-[1180px]'">
      <!-- top row (results) -->
      <div v-if="step === 'results'" class="flex items-center justify-between gap-4">
        <button class="btn-quiet" @click="newSpot">New spot</button>
        <button class="btn-quiet" @click="notebookOpen = true">
          Notebook<span v-if="notebook.length" class="ml-1.5 inline-flex min-w-[1.25rem] justify-center rounded-full bg-felt text-white text-xs px-1.5">{{ notebook.length }}</span>
        </button>
      </div>

      <!-- nothing solved yet -->
      <div v-if="step === 'setup'" class="max-w-xl mx-auto mt-24 text-center">
        <h1 class="font-display font-extrabold text-2xl">No result yet</h1>
        <p class="text-muted mt-2">This page shows a solved spot: what each player does, hand by hand. Set a spot up first and it opens here when the solve finishes.</p>
        <button class="btn-solve mt-5 !w-auto px-6" @click="emit('new')">Study a spot</button>
      </div>
      <!-- ================= SOLVING ================= -->
      <div v-if="step === 'solving'" class="max-w-xl mx-auto mt-20">
        <div class="flex gap-2 justify-center">
          <PlayingCard v-for="i in 3" :key="i" :card="boardCards[i - 1] ?? ''" size="lg" />
        </div>
        <h1 class="font-display font-extrabold text-2xl text-center mt-6">Solving</h1>
        <p class="text-muted text-center mt-1">{{ phaseText }}</p>
        <div class="h-2.5 bg-rule rounded-full mt-6 overflow-hidden" role="progressbar" :aria-valuenow="progressPct" aria-valuemin="0" aria-valuemax="100">
          <div class="h-full bg-felt transition-[width] duration-500" :style="{ width: progressPct + '%' }"></div>
        </div>
        <div class="flex justify-between text-sm text-muted mt-2">
          <span>{{ elapsedText }} elapsed</span>
          <span>{{ etaText }}</span>
        </div>
        <p class="hint mt-5 text-center max-w-[52ch] mx-auto">
          The solver plays the spot against itself over and over, and stops when neither player could gain more than
          {{ accuracy.pct }}% of the pot by changing strategy.<template v-if="expl < 100"> It's at {{ expl.toFixed(2) }}% now.</template>
        </p>
        <div class="text-center mt-6"><button class="btn-quiet" @click="cancel">Cancel</button></div>
      </div>

      <!-- ================= RESULTS ================= -->
      <div v-if="step === 'results' && node" class="mt-3">
        <div class="flex items-end justify-between gap-6">
          <div>
            <h1 class="font-display font-extrabold text-2xl leading-tight">{{ built.description }}</h1>
            <p v-if="fromFile" class="text-sm text-muted mt-1">
              From a library file.<template v-if="summary?.base_expl_pct"> Its flop was solved to within {{ summary.base_expl_pct.toFixed(2) }}% of the pot.</template>
            </p>
            <p v-else class="text-sm text-muted mt-1">
              Accurate to within {{ summary?.base_expl_pct.toFixed(2) }}% of the pot. Solved in {{ secs(summary?.base_secs ?? 0) }}<template v-if="summary?.locked">, exploit solve {{ secs(summary.locked_secs) }}</template>.
            </p>
            <p v-if="pickedText" class="text-sm text-muted mt-1 max-w-[110ch]">{{ pickedText }}</p>
          </div>
          <div class="shrink-0 text-right">
            <div v-if="saveOpen" class="flex items-center gap-2">
              <input v-model="saveName" class="input !w-72" aria-label="Name for this solve" @keyup.enter="saveNow" />
              <button class="btn-quiet" :disabled="saveBusy || !saveName.trim()" title="Keeps the whole solve: every turn and river as solved. A large file." @click="saveNow">{{ saveBusy ? "Saving…" : "Save whole" }}</button>
              <button class="btn-quiet" :disabled="saveBusy || !saveName.trim()" title="Keeps the flop only, in a file small enough to send to someone. Turn and river are solved again in a few seconds when a card is picked." @click="shareNow">Save small, to share</button>
              <button class="text-xs text-muted underline" @click="saveOpen = false">Cancel</button>
            </div>
            <span v-else-if="fromFile" class="text-xs text-muted">Library file: the flop is stored, each turn is solved when you pick its card.</span>
            <button v-else class="btn-quiet" @click="openSave">{{ savedId ? "Saved to library ✓" : "Save to library" }}</button>
            <p v-if="saveError" class="text-xs font-bold text-danger mt-1">{{ saveError }}</p>
          </div>
        </div>

        <!-- first-run guide -->
        <div v-if="!introDismissed" class="mt-4 bg-surface border border-rule rounded-xl p-4 flex gap-4">
          <div class="text-sm leading-relaxed max-w-[90ch]">
            <b>How to read this.</b> Each decision shows what the player does and how often. Click an action to follow
            that line; click a turn or river card when one is dealt. The chart colours every starting hand by what it
            does here — split colours mean the solver mixes. Type your own hand under <b>Your hand</b> to see exactly what
            to do with it at each step.
          </div>
          <button class="btn-quiet self-start shrink-0" @click="dismissIntro">Got it</button>
        </div>

        <div v-if="summary?.icm" class="rounded-xl border border-felt bg-felt-light p-3 text-sm mb-3">
          <b>Solved for prize money.</b>
          After this spot you hold <b>{{ summary.icm.base_share[heroIdx].toFixed(2) }}%</b> of the prize pool on average<template v-if="summary.locked"> (<b>{{ summary.icm.locked_share[heroIdx].toFixed(2) }}%</b> {{ manualLocks.length ? "with the locks in place" : "against this player" }})</template>,
          against {{ summary.icm.split[heroIdx].toFixed(2) }}% if the pot were split and {{ summary.icm.give_up[heroIdx].toFixed(2) }}% if you gave it up now.
          Amounts won and lost on this screen are prize money in chip-sized units, so the two players' shares of the pot add up to less than the pot: the players sitting out gain when you two clash.
        </div>

        <!-- exploit bar -->
        <div v-if="summary?.locked" class="mt-4 rounded-xl border border-mine bg-mine-light p-4">
          <div class="flex flex-wrap items-center gap-3">
            <div class="seg" role="group" aria-label="Which strategy to show">
              <button :class="view === 'base' ? 'seg-on' : ''" @click="setView('base')">Normal strategy</button>
              <button :class="view === 'locked' ? 'seg-on' : ''" @click="setView('locked')">{{ lockedLabel }}</button>
            </div>
            <label class="text-sm flex items-center gap-1.5">
              <input v-model="compareOn" type="checkbox" class="rounded text-felt" @change="load" />
              Show what changes at each step
            </label>
            <span v-if="busy" class="text-sm text-felt font-bold" role="status">Loading…</span>
          </div>
          <p class="text-sm mt-2.5">
            You win on average <b>{{ fmt(summary.base_ev[heroIdx]) }}</b> of this {{ fmt(built.pot) }} pot normally, and
            <b>{{ fmt(summary.locked_ev[heroIdx]) }}</b> {{ manualLocks.length ? "with the locks in place" : "against this player" }} —
            <b :class="evGain >= 0 ? 'text-felt' : 'text-danger'">{{ evGain >= 0 ? '+' : '' }}{{ fmt(evGain) }}</b> a hand,
            {{ manualLocks.length ? "if they really play the way you locked." : "if the habit is as strong as you set it." }} Real players are less consistent, so trust the direction of each
            change more than its size.
          </p>
        </div>

        <!-- a pasted hand, decision by decision -->
        <div v-if="review || reviewError" class="mt-4 bg-surface border border-rule rounded-xl p-4">
          <h2 class="font-display font-extrabold text-lg">Your hand, decision by decision</h2>
          <p v-if="reviewError" class="text-sm font-bold text-danger mt-1">{{ reviewError }}</p>
          <template v-if="review">
            <p class="hint mt-1">
              Each of your decisions after the flop, priced for {{ lastHeroHand }} against the normal solve. "Gave up" is how much less the action you took is worth than the best one, on average.
              <template v-if="reviewRows.length"> In total: <b :class="review.total_loss > 0.02 * built.pot ? 'text-danger' : ''">{{ fmt(Math.round(review.total_loss * 10) / 10) }}</b>.</template>
            </p>
            <div class="overflow-x-auto">
              <table v-if="reviewRows.length" class="w-full text-sm mt-3">
                <thead><tr class="text-left text-xs text-muted border-b border-rule"><th class="py-1 pr-3 font-normal">Where</th><th class="py-1 pr-3 font-normal">You</th><th class="py-1 pr-3 font-normal">The solve with this hand</th><th class="py-1 pr-3 font-normal text-right">Gave up</th><th class="py-1 font-normal">Reading</th><th></th></tr></thead>
                <tbody>
                  <tr v-for="x in reviewRows" :key="x.i" class="border-b border-rule align-top">
                    <td class="py-1.5 pr-3 whitespace-nowrap"><span class="capitalize font-bold">{{ x.r.street }}</span> <span class="text-muted">· pot {{ fmt(x.r.pot) }}</span></td>
                    <td class="py-1.5 pr-3">
                      <b>{{ reviewAction(x.r, x.r.chosen) }}</b>
                      <div v-if="x.r.played_as" class="text-xs text-muted">you made it {{ fmt(Number(x.r.played_as.split(" to ")[1])) }}; this is the closest size in the solve</div>
                    </td>
                    <td class="py-1.5 pr-3">
                      <span v-for="(f, a) in x.r.freqs" :key="a" class="inline-block mr-3 whitespace-nowrap" :class="a === x.r.best ? 'font-bold' : ''">{{ reviewAction(x.r, a) }} {{ pct(f) }} <span class="text-muted font-normal">({{ fmt(Math.round(x.r.evs[a] * 10) / 10) }})</span></span>
                    </td>
                    <td class="py-1.5 pr-3 text-right tabular-nums font-bold" :class="x.r.loss >= 0.05 * x.r.pot ? 'text-danger' : ''">{{ x.r.loss <= 0.005 * x.r.pot ? "–" : fmt(Math.round(x.r.loss * 10) / 10) }}</td>
                    <td class="py-1.5 pr-3">{{ reviewVerdict(x.r) }}</td>
                    <td class="py-1.5 text-right"><button class="text-xs text-felt underline font-bold whitespace-nowrap" @click="showReviewRow(x.r)">Show</button></td>
                  </tr>
                </tbody>
              </table>
            </div>
            <p v-for="(n, i) in reviewSizeNotes" :key="'n' + i" class="hint mt-2">{{ n }}</p>
            <p v-if="!reviewRows.length && !review.stopped" class="text-sm mt-2">You had no decisions after the flop in this hand.</p>
            <p v-if="review.stopped" class="text-sm mt-2"><b>The review stops {{ reviewRows.length ? "there" : "before your first decision" }}.</b> {{ review.stopped }}</p>
            <p class="hint mt-2">Figures in brackets are what each action is worth with this exact hand. A mixed strategy means the actions are worth about the same, so "fine" covers anything the solve does a fair share of the time.</p>
          </template>
        </div>

        <!-- line -->
        <ActionBar class="mt-4" start="Flop" :board="displayBoard" :steps="steps" :now="nowText" :pot="node ? fmt(node.pot) : ''" @goto="goTo" />

        <!-- the spot as a table -->
        <div class="mt-3">
          <button class="text-xs text-muted underline" :aria-expanded="tableOn" @click="toggleTable">{{ tableOn ? "Hide the table" : "Show the table" }}</button>
          <TableView v-if="tableOn" class="mt-1" :board="node.board" :pot="node.pot" :bets="node.bets" :stack="node.stack" :acting="node.kind === 'player' ? node.player : -1" :names="[built.oopName, built.ipName]" :hero="heroIdx" :hero-hand="lastHeroHand" :ev="node.ev" :equity="node.equity" :fmt="fmt" />
        </div>

        <div class="mt-4 grid grid-cols-12 gap-5">
          <!-- decision -->
          <div class="col-span-7 min-[1500px]:col-span-5">
            <div class="bg-surface rounded-xl border border-rule p-5">
              <template v-if="node.kind === 'player'">
                <div class="flex items-baseline justify-between gap-3">
                  <h2 class="font-display font-extrabold text-xl">
                    {{ actorName }}
                    <span v-if="node.locked && view === 'locked'" class="ml-1 align-middle text-xs font-body font-bold bg-mine text-[#1B1417] px-2 py-0.5 rounded">{{ manualLocks.length ? "locked" : "habit applied" }}</span>
                  </h2>
                  <span class="text-sm text-muted">Pot {{ fmt(node.pot) }}</span>
                </div>
                <p class="text-sm mt-1" :class="lineProb < 0.03 ? 'text-danger' : 'text-muted'">
                  <template v-if="history.length">Players take this line {{ probText(lineProb) }} of the time<template v-if="node.board.length > 3"> when {{ node.board.length === 4 ? 'this turn card comes' : 'these turn and river cards come' }}</template>.</template>
                  <template v-else>Click an action to see what happens next.</template>
                  <template v-if="lineProb < 0.03"> That's rare, so the solver spent little effort here — treat these numbers as rough.</template>
                </p>
                <div class="mt-4 space-y-1.5">
                  <button
                    v-for="(a, i) in node.actions"
                    :key="i"
                    class="action-row"
                    @click="play(i, actionLabel(i))"
                  >
                    <span class="w-3.5 h-3.5 rounded-sm shrink-0" :style="{ background: colors[i] }"></span>
                    <span class="w-44 font-bold text-left">{{ actionLabel(i) }}</span>
                    <span class="flex-1 h-2.5 bg-paper rounded-full overflow-hidden">
                      <span class="block h-full rounded-full" :style="{ width: node.action_freqs[i] * 100 + '%', background: colors[i] }"></span>
                    </span>
                    <span class="w-12 text-right tabular-nums font-bold">{{ pct(node.action_freqs[i]) }}</span>
                    <span v-if="compareNode" class="w-24 text-right text-xs tabular-nums text-muted">
                      {{ view === 'locked' ? 'normally' : manualLocks.length ? 'locked' : 'vs habit' }} {{ pct(compareNode.action_freqs[i] ?? 0) }}
                    </span>
                  </button>
                </div>

                <p v-if="turnNote" class="hint mt-3">
                  The {{ turnNote.card[0] === "T" ? "10" : turnNote.card[0] }}{{ SUIT_SYMBOL[turnNote.card[1]] }} turn was solved on this computer in {{ turnNote.secs < 10 ? turnNote.secs.toFixed(1) : Math.round(turnNote.secs) }}s, to within {{ turnNote.expl_pct.toFixed(2) }}% of the pot.
                  The stored flop solve valued this card at {{ fmt(Math.round(turnNote.stored_ev[heroIdx] * 10) / 10) }} for {{ heroIdx === 0 ? built.oopName : built.ipName }}; this solve says {{ fmt(Math.round(turnNote.ev[heroIdx] * 10) / 10) }}.
                </p>
                <details v-if="!fromFile" class="mt-3 rounded-lg border border-rule px-3 py-2" :open="lockOpen" @toggle="lockOpen = ($event.target as HTMLDetailsElement).open">
                  <summary class="cursor-pointer text-sm font-bold">Change how {{ actorName }} plays here (node lock){{ manualLocks.length ? ` · ${manualLocks.length} set` : "" }}</summary>
                  <div v-if="lockOpen" class="mt-2 text-sm">
                    <p class="hint">Fix what this player does at this exact point, then solve again to see how everyone should adjust. Use it for reads like "this player never raises here".</p>
                    <div class="seg mt-2" role="group" aria-label="Which hands the lock covers">
                      <button :class="lockScope === 'all' ? 'seg-on' : ''" @click="lockScope = 'all'">Every hand</button>
                      <button :class="lockScope === 'cell' ? 'seg-on' : ''" :disabled="selectedCell === null" :title="selectedCell === null ? 'Click a square in the chart first' : ''" @click="lockScope = 'cell'">Only {{ selectedCell !== null ? cellName(selectedCell) : "the square I click" }}</button>
                    </div>
                    <div class="mt-2 space-y-1">
                      <label v-for="(a, i) in node.actions" :key="i" class="flex items-center gap-2">
                        <span class="w-3 h-3 rounded-sm shrink-0" :style="{ background: colors[i] }"></span>
                        <span class="w-40">{{ actionLabel(i) }}</span>
                        <input v-model.number="lockPct[i]" type="number" min="0" max="100" step="5" class="input !w-20 !py-1 text-right" :aria-label="actionLabel(i) + ' percent'" /><span>%</span>
                      </label>
                    </div>
                    <div class="mt-2 flex items-center gap-3">
                      <button class="btn-quiet" :disabled="lockTotal !== 100" @click="addLock">Add this lock</button>
                      <span class="text-xs" :class="lockTotal === 100 ? 'text-muted' : 'text-danger font-bold'">Total {{ lockTotal }}%{{ lockTotal === 100 ? "" : " (must be 100)" }}</span>
                    </div>
                    <ul v-if="manualLocks.length" class="mt-3 space-y-1">
                      <li v-for="(l, i) in manualLocks" :key="i" class="flex items-start justify-between gap-2 border-t border-rule pt-1">
                        <span>{{ l.label }}</span>
                        <button class="text-xs text-danger underline shrink-0" @click="removeLock(i)">Remove</button>
                      </li>
                    </ul>
                    <div v-if="manualLocks.length || lockStale" class="mt-3">
                      <button class="btn-solve !w-auto px-5 !py-2" :disabled="relocking || !manualLocks.length" @click="relockNow">{{ relocking ? `Solving again… ${iter} of ${maxIter}` : `Solve again with ${manualLocks.length} lock${manualLocks.length === 1 ? "" : "s"}` }}</button>
                      <button v-if="relocking" class="ml-2 text-xs underline" @click="cancel">Stop</button>
                      <p v-if="lockStale && !relocking" class="hint mt-1">The locks changed since the last solve. Solve again to see their effect.</p>
                      <p v-if="lockError" class="text-sm font-bold text-danger mt-1">{{ lockError }}</p>
                    </div>
                    <div v-if="lockRuns.length" class="mt-4 border-t border-rule pt-3">
                      <h4 class="font-bold">Lock sets solved so far</h4>
                      <p class="hint">Each row is one solve with a set of locks. The play shown is at the decision that was on screen when that solve finished. Only the newest solve can be browsed; “Use again” brings a set's locks back so it can be solved again.</p>
                      <div class="overflow-x-auto mt-2">
                        <table class="w-full text-xs tabular-nums">
                          <thead><tr class="text-left text-muted"><th class="pr-2 py-1 font-bold">Locks</th><th class="pr-2 py-1 font-bold text-right">Value for {{ heroIdx === 0 ? built.oopName : built.ipName }}</th><th class="pr-2 py-1 font-bold text-right">Change</th><th class="pr-2 py-1 font-bold">Play there</th><th></th></tr></thead>
                          <tbody>
                            <tr class="border-t border-rule align-top"><td class="pr-2 py-1">None (the normal solve)</td><td class="pr-2 py-1 text-right">{{ fmt(lockRuns[0].baseEv) }}</td><td class="pr-2 py-1 text-right">–</td><td class="pr-2 py-1 text-muted">see each row</td><td></td></tr>
                            <tr v-for="(r, i) in lockRuns" :key="i" class="border-t border-rule align-top" :class="i === lockRuns.length - 1 && !lockStale ? 'bg-mine-light' : ''">
                              <td class="pr-2 py-1"><div v-for="(l, j) in r.locks" :key="j">{{ l.label }}</div></td>
                              <td class="pr-2 py-1 text-right font-bold">{{ fmt(r.ev) }}</td>
                              <td class="pr-2 py-1 text-right">{{ r.ev - r.baseEv >= 0 ? "+" : "−" }}{{ fmt(Math.abs(r.ev - r.baseEv)) }}</td>
                              <td class="pr-2 py-1"><template v-if="r.acts.length"><div class="text-muted">{{ r.where }}, {{ r.who }}:</div><div v-for="(a, j) in r.acts" :key="j" class="whitespace-nowrap">{{ a.name }} {{ pct(a.f) }} <span class="text-muted">(normally {{ pct(a.base) }})</span></div></template></td>
                              <td class="py-1 text-right"><button class="underline text-felt font-bold whitespace-nowrap" :disabled="relocking" @click="useRun(i)">Use again</button></td>
                            </tr>
                          </tbody>
                        </table>
                      </div>
                    </div>
                  </div>
                </details>

                <div class="mt-5 flex items-center justify-between gap-2 flex-wrap">
                  <h3 class="text-sm font-bold">{{ actorName }}'s hands</h3>
                  <ChartMode v-model="chartMode" />
                  <ul class="flex flex-wrap gap-x-3 gap-y-1 text-xs text-muted" aria-label="Colour key">
                    <li v-for="(a, i) in node.actions" :key="i" class="flex items-center gap-1">
                      <span class="w-2.5 h-2.5 rounded-sm" :style="{ background: colors[i] }"></span>{{ shortAction(i) }}
                    </li>
                    <li class="flex items-center gap-1"><span class="w-2.5 h-2.5 rounded-sm bg-paper border border-rule"></span>not in range</li>
                  </ul>
                </div>
                <div class="mt-2">
                  <HandGrid :grid="node.grid" :colors="colors" :actions="node.actions.map((_x, a) => shortAction(a))" :selected="selectedCell" :mine="myCell" :marks="suitMarks" :mode="chartMode" :fmt-ev="fmt" :lit="typeLit ?? undefined" @select="selectCell" />
                  <p class="hint mt-1.5">The fill height shows how much of each hand is still in the range here. A sliver means that hand rarely reaches this point. A dot in the corner marks squares where the suits play differently; click one to see each suit.</p>
                </div>
                <details v-if="otherGrid" class="mt-3 rounded-lg border border-rule px-3 py-2" :open="otherOn" @toggle="setOther(($event.target as HTMLDetailsElement).open)">
                  <summary class="cursor-pointer font-bold text-sm">What {{ node.player === 0 ? built.ipName : built.oopName }} holds here</summary>
                  <div v-if="otherOn" class="mt-2 max-w-sm">
                    <HandGrid :grid="otherGrid" :colors="['#8792a8']" :selected="null" :mode="chartMode" :fmt-ev="fmt" />
                    <p class="hint mt-1.5">The other player's range at this point: {{ Math.round(otherCombos) }} hands, winning {{ Math.round(node.equity[1 - node.player] * 100) }}% against the range above. The fill height is how much of each hand is left; the chart buttons above switch both charts.</p>
                  </div>
                </details>
              </template>

              <template v-else-if="node.kind === 'chance'">
                <h2 class="font-display font-extrabold text-xl">Deal the {{ node.board.length === 3 ? 'turn' : 'river' }}</h2>
                <p class="text-sm text-muted mt-1">Pick the card that came. Pot {{ fmt(node.pot) }}.<template v-if="fromFile && node.board.length === 3"> This is a library file: the turn you pick is solved here first, usually in a few seconds.</template></p>
                <p v-if="turnBusy" class="text-sm font-bold text-felt mt-2" role="status">Solving the {{ turnBusy }} turn… {{ iter }} steps<template v-if="expl < 100">, within {{ expl.toFixed(2) }}% of the pot</template> <button class="ml-2 text-xs underline" @click="cancel">Stop</button></p>
                <p v-if="turnError" class="text-sm font-bold text-danger mt-2">{{ turnError }}</p>
                <div class="mt-4 inline-grid gap-1" style="grid-template-columns: repeat(13, 2.25rem)">
                  <template v-for="s in [3, 2, 1, 0]" :key="s">
                    <button
                      v-for="r in RANKS_DESC"
                      :key="r * 4 + s"
                      class="deck-btn"
                      :class="suitColor(r * 4 + s)"
                      :disabled="!dealable.has(r * 4 + s)"
                      @click="play(r * 4 + s, cardLabel(r * 4 + s))"
                    >
                      {{ cardLabel(r * 4 + s) }}
                    </button>
                  </template>
                </div>
                <h3 class="font-bold mt-5">Which cards matter</h3>
                <CardReport class="mt-2" :cards="node.cards" :names="[built.oopName, built.ipName]" :hero="heroIdx" :street="node.board.length === 3 ? 'turn' : 'river'" :reset-key="view + ':' + history.join(',')" :load="loadCard" @pick="play($event, cardLabel($event))" />
              </template>

              <template v-else>
                <h2 class="font-display font-extrabold text-xl">Hand over</h2>
                <p class="text-muted mt-1">{{ terminalText }}</p>
                <button class="btn-quiet mt-3" @click="goTo(crumbs.length - 1)">Back one step</button>
              </template>
            </div>
          </div>

          <!-- side panels -->
          <div class="col-span-5 min-[1500px]:col-span-7 space-y-4 min-[1500px]:space-y-0 min-[1500px]:columns-2 min-[1500px]:gap-4 [&>*]:break-inside-avoid min-[1500px]:[&>*]:mb-4">
            <section class="panel border-mine">
              <h3 class="font-bold">Your hand</h3>
              <div class="flex items-center gap-2 mt-2">
                <input
                  v-model="myHandInput"
                  class="input w-28 uppercase-first"
                  placeholder="e.g. AhKd"
                  aria-label="Your hand, like AhKd"
                  maxlength="4"
                  @keyup.enter="trackHand"
                />
                <button class="btn-quiet" @click="trackHand">Track</button>
                <div v-if="myHand" class="flex gap-1 ml-auto">
                  <PlayingCard :card="myHand.slice(0, 2)" size="sm" />
                  <PlayingCard :card="myHand.slice(2)" size="sm" />
                </div>
              </div>
              <p v-if="myHandError" class="text-sm text-danger mt-2">{{ myHandError }}</p>
              <div v-else-if="myHand && myView" class="mt-3 text-sm">
                <template v-if="myView.blocked">One of your cards is on the board. Track a different hand.</template>
                <template v-else-if="!myView.in_range">
                  You wouldn't have this hand here — your strategy folds it, or plays it differently, earlier in this line.
                </template>
                <template v-else>
                  <div v-if="myView.acting">
                    <div v-for="(f, a) in myView.freqs" :key="a" class="flex items-center gap-2 mt-1">
                      <span class="w-3 h-3 rounded-sm" :style="{ background: colors[a] }"></span>
                      <span class="flex-1">{{ actionLabel(a) }}</span>
                      <span class="tabular-nums font-bold">{{ pct(f) }}</span>
                      <span v-if="myCompare && myCompare.acting" class="w-20 text-right text-xs text-muted tabular-nums">{{ view === 'locked' ? 'normally' : manualLocks.length ? 'locked' : 'vs habit' }} {{ pct(myCompare.freqs[a] ?? 0) }}</span>
                    </div>
                  </div>
                  <div v-else class="text-muted">Waiting on the other player.</div>
                  <p class="mt-2 text-muted">
                    Wins {{ pct(myView.equity) }} at showdown against their range right now · average result {{ fmt(myView.ev) }}
                  </p>
                </template>
              </div>
              <p v-else class="hint mt-2">Type a hand, or click a square in the chart and choose "Track this hand".</p>
            </section>

            <section v-if="node.kind === 'player' && selectedCell !== null && node.grid[selectedCell]" class="panel">
              <div class="flex items-center justify-between">
                <h3 class="font-bold">{{ cellName(selectedCell) }}</h3>
                <button v-if="node.player === heroIdx" class="text-sm text-felt font-bold underline" @click="trackCell(selectedCell)">Track this hand</button>
              </div>
              <template v-if="node.grid[selectedCell].weight > 0">
                <p class="text-sm text-muted">Wins {{ pct(node.grid[selectedCell].equity) }} at showdown on average · average result {{ fmt(node.grid[selectedCell].ev) }}</p>
                <div v-for="(f, a) in node.grid[selectedCell].freqs" :key="a" class="flex items-center gap-2 text-sm mt-1">
                  <span class="w-3 h-3 rounded-sm" :style="{ background: colors[a] }"></span>
                  <span class="flex-1">{{ actionLabel(a) }}</span><span class="tabular-nums font-bold">{{ pct(f) }}</span>
                </div>
                <h4 class="font-bold text-sm mt-4">By suit</h4>
                <SuitBreakdown class="mt-1" :combos="cellCombos" :actions="node.actions.map((_x, a) => shortAction(a))" :colors="colors" :fmt-ev="fmt" />
              </template>
              <p v-else class="text-sm text-muted">Not in {{ actorName.toLowerCase() }}'s range at this point.</p>
            </section>

            <section v-if="node.kind === 'player'" class="panel">
              <h3 class="font-bold">Coach notes</h3>
              <ul class="mt-2 space-y-1.5 text-sm list-disc pl-5 marker:text-felt">
                <li v-for="(n, i) in notes" :key="i">{{ n }}</li>
              </ul>
              <template v-if="view === 'locked' && compareNotes.length">
                <h3 class="font-bold mt-4">{{ manualLocks.length ? "What changes with the locks" : "What changes against this player" }}</h3>
                <ul class="mt-2 space-y-1.5 text-sm list-disc pl-5 marker:text-mine">
                  <li v-for="(n, i) in compareNotes" :key="i">{{ n }}</li>
                </ul>
              </template>
            </section>

            <section v-if="node.curves && node.curves.length" class="panel">
              <h3 class="font-bold">Who has the stronger range</h3>
              <EquityGraph class="mt-2" :curves="node.curves" :names="[built.oopName, built.ipName]" />
            </section>

            <section v-if="node.kind === 'player'" class="panel">
              <h3 class="font-bold">{{ actorName }}'s play by type of hand</h3>
              <p class="text-xs text-muted mt-1">Click types to show only those hands in the chart and in the list of every hand. Ctrl-click (or Shift-click) a type to hide it. <button v-if="typeOn" class="underline text-felt font-bold" @click="typePick = {}">Show all hands</button></p>
              <HandTypes class="mt-2" :combos="node.combos" :board="node.board" :actions="node.actions.map((_x, a) => shortAction(a))" :colors="colors" :picked="typePick" @pick="typePick = $event" />
            </section>

            <section v-if="node.kind === 'player' && node.actions.some((a) => /^(Bet|Raise|All-in)/.test(a))" class="panel">
              <h3 class="font-bold">Blockers</h3>
              <Blockers class="mt-2" :actor="actorName" :opp="node.player === 0 ? built.ipName : built.oopName" :board="node.board" :actions="node.actions" :labels="node.actions.map((_x, a) => actionLabel(a))" :freqs="node.action_freqs" :combos="node.combos" :reset-key="view + ':' + history.join(',') + ':' + node.action_freqs.map((f) => f.toFixed(2)).join()" :load="loadReply" />
            </section>

            <section v-if="node.kind === 'player'" class="panel">
              <h3 class="font-bold">By hand strength</h3>
              <p class="text-xs text-muted">Strength = chance to win at showdown against the other player's range right now.</p>
              <div v-for="(b, bi) in node.buckets" :key="bi" class="mt-2.5">
                <div class="flex justify-between text-sm">
                  <span>{{ b.name }}</span><span class="text-muted">{{ pct(b.share) }} of hands</span>
                </div>
                <div class="flex h-2.5 rounded-full overflow-hidden mt-1 bg-paper">
                  <span v-for="(f, a) in b.freqs" :key="a" :style="{ width: f * 100 + '%', background: colors[a] }" :title="actionLabel(a) + ' ' + pct(f)"></span>
                </div>
              </div>
            </section>

            <section class="panel">
              <h3 class="font-bold">Takeaway</h3>
              <textarea v-model="takeaway" rows="4" class="input w-full mt-2 text-sm"></textarea>
              <div class="flex gap-2 mt-2">
                <button class="btn-primary" @click="saveTakeaway">{{ saved ? 'Saved to notebook' : 'Save to notebook' }}</button>
                <button class="btn-quiet" @click="copyText(takeaway)">{{ copied ? 'Copied' : 'Copy' }}</button>
              </div>
            </section>
          </div>
        </div>

        <!-- every exact hand, full width so all the columns fit -->
        <section v-if="node.kind === 'player' && node.combos.length" class="panel mt-4">
          <h3 class="font-bold">Every hand {{ actorName }} can hold</h3>
          <HandsTable class="mt-2" :combos="node.combos" :actions="node.actions.map((_x, a) => shortAction(a))" :colors="colors" :pot="node.pot" :selected="selectedCell" :fmt-ev="fmt" :only="typeHands" @select="selectCell" />
          <p v-if="typeOn" class="hint mt-1">{{ typeText }} <button class="underline text-felt font-bold" @click="typePick = {}">Show all hands</button></p>
        </section>
      </div>

      <div v-if="error" class="mt-6 bg-danger/10 border border-danger/40 text-danger rounded-lg p-3" role="alert">{{ error }}</div>
    </div>

    <!-- notebook drawer -->
    <div v-if="notebookOpen" class="fixed inset-0 z-40 flex justify-end bg-ink/30" @click.self="notebookOpen = false">
      <aside class="w-[440px] h-full bg-surface shadow-xl p-6 overflow-y-auto" aria-label="Notebook">
        <div class="flex items-center justify-between">
          <h2 class="font-display font-extrabold text-xl">Notebook</h2>
          <button class="btn-quiet" @click="notebookOpen = false">Close</button>
        </div>
        <p class="hint mt-1">Takeaways you've saved. They stay on this computer.</p>
        <div class="flex gap-2 mt-3">
          <button class="btn-quiet" :disabled="!notebook.length" @click="copyText(notebookText)">{{ copied ? 'Copied' : 'Copy all' }}</button>
          <button class="btn-quiet" :disabled="!notebook.length" @click="downloadNotebook">Download as text</button>
        </div>
        <p v-if="!notebook.length" class="mt-6 text-muted">Nothing saved yet. Solve a spot and press "Save to notebook".</p>
        <ul class="mt-4 space-y-3">
          <li v-for="(n, i) in notebook" :key="n.at" class="border border-rule rounded-lg p-3">
            <div class="text-xs text-muted">{{ new Date(n.at).toLocaleString() }}</div>
            <p class="text-sm mt-1 whitespace-pre-wrap">{{ n.text }}</p>
            <button class="text-xs text-danger underline mt-2" @click="removeNote(i)">Delete</button>
          </li>
        </ul>
      </aside>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, reactive, ref, watch, onMounted } from "vue";
import HandGrid from "./HandGrid.vue";
import RangeSlider from "./RangeSlider.vue";
import ActionBar, { type Step } from "./ActionBar.vue";
import SuitBreakdown from "./SuitBreakdown.vue";
import EquityGraph from "./EquityGraph.vue";
import ChartMode from "./ChartMode.vue";
import Blockers from "./Blockers.vue";
import HandTypes from "./HandTypes.vue";
import { classify } from "../../guided/handClass";
import HandsTable from "./HandsTable.vue";
import TableView from "./TableView.vue";
import CardReport, { type CardRow } from "./CardReport.vue";
import PlayingCard from "./PlayingCard.vue";
import RangePreview from "./RangePreview.vue";
import {
  SCENARIOS, OPPONENTS, SIZINGS, ACCURACY, LEAKS, STRENGTHS, PROFILES, TEXTURES, SEAT_LABEL,
  RANK_CHARS, SUIT_CHARS, SUIT_SYMBOL, SUIT_COLOR, randomFlop, type ScenarioResult,
} from "../../guided/data";
import * as api from "../../guided/api";
import { memory, releaseSolve, startMemoryWatch, gb } from "../../guided/memory";
import { actionColors, describeNode, compareNodes, pct } from "../../guided/coach";

const RANKS_DESC = [12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0];
const store = (k: string) => { try { return localStorage.getItem(k); } catch { return null; } };
const persist = (k: string, v: string) => { try { localStorage.setItem(k, v); } catch { /* ignore */ } };

// ---------- setup state ----------
const step = ref<"setup" | "solving" | "results">("setup");
const scenarioId = ref("open_bb");
const seat = ref("BTN");
const opponent = ref("rec");
const openSize = ref(15);
const stackIn = ref(200);
const board = ref<number[]>([47, 22, 1]); // Ks 7h 2d
const sizingId = ref("clp");
const accuracyId = ref("study");
const exploitOn = ref(false);
const profileId = ref("austin");
const leakOn = reactive<Record<string, boolean>>({});
const leakStrength = reactive<Record<string, number>>({});
LEAKS.forEach((l) => { leakOn[l.id] = false; leakStrength[l.id] = 0.6; });
const error = ref("");
const memMb = ref(0); // normal-mode estimate
const memLow = ref(0); // memory-saver estimate
const forceSaver = ref(false);
const freezeRest = ref(true);
const CONFIDENCE = [
  { value: 1.0, label: "Certain", help: "The maximum exploit, as if they always play this way. Can produce extreme strategies that backfire if the read is off." },
  { value: 0.75, label: "Fairly sure", help: "Recommended. They play this way most of the time, so you exploit hard but stay sensible." },
  { value: 0.5, label: "Just a hunch", help: "A gentle exploit that costs little if you're wrong." },
];
const confidence = ref(0.75);

const scenario = computed(() => SCENARIOS.find((s) => s.id === scenarioId.value)!);
const fmtKind = computed<"live" | "hu">(() => custom.value?.format ?? scenario.value.format);
const pickScenario = (id: string) => {
  scenarioId.value = id;
  const s = scenario.value;
  if (!s.seats.includes(seat.value)) seat.value = s.seats[s.seats.length - 1];
  if (s.format === "hu") { stackIn.value = 25; sizingId.value = "hu"; }
  else { if (stackIn.value < 40) stackIn.value = 200; if (sizingId.value === "hu") sizingId.value = "clp"; }
};
const seatLabel = (s: string) => SEAT_LABEL[s] ?? (s === "BB" ? "Big blind" : s);
/** chips per big blind in the spot on screen: 10 for spots set up here, 100 for library files made by the headless program */
const chipsPerBb = ref(10);
const fmt = (chips: number) =>
  fmtKind.value === "hu" ? `${(chips / chipsPerBb.value).toFixed(1)}bb` : `$${Number.isInteger(chips) ? chips : chips.toFixed(1)}`;
const secs = (s: number) => (s < 90 ? `${Math.round(s)}s` : `${Math.round(s / 60)} min`);

// a spot handed over by the spot builder replaces the built-in scenarios
const custom = ref<{ res: ScenarioResult; format: "live" | "hu" } | null>(null);
const override = ref<Record<string, unknown>>({});
const built = computed(() => custom.value?.res ?? scenario.value.build(seat.value, opponent.value, openSize.value, stackIn.value));
const emit = defineEmits<{ (e: "new"): void }>();
const newSpot = () => { step.value = "setup"; };
watch(step, (v) => { if (v === "setup" && custom.value) emit("new"); });
type Custom2 = { res: ScenarioResult; format: "live" | "hu"; board: number[]; profile: string | null; overrides: Record<string, unknown>; heroHand?: string; line?: api.ReviewStep[]; autoSizes?: boolean; /** chips per big blind for "hu" spots; 10 when missing (spots saved before 0.13.13) */ chips?: number };
/** "Pick the bet sizes for me": a rough first solve tries these sizes (percent of pot) and keeps each player's two most used */
const SIZES_TRIED = [33, 50, 75, 100, 150];
const autoSizes = ref(false);
const picking = ref(false);
const picked = ref<api.SizePick | null>(null);
const pickedText = computed(() => {
  const p = picked.value; if (!p) return "";
  const list = (v: number[]) => v.map((x) => `${x}%`).join(" and ");
  const used = (i: number) => p.used[i].filter((u) => !p.chosen[i].includes(u[0]) && u[1] >= 0.005).map((u) => `${u[0]}% (${Math.round(u[1] * 100)}% of hands)`).join(", ");
  const who = p.street === "flop"
    ? `${built.value.oopName} bets ${list(p.chosen[0])} of the pot, ${built.value.ipName} ${list(p.chosen[1])}`
    : `both players bet ${list(p.chosen[0])} of the pot`;
  const dropped = [used(0), used(1)].filter(Boolean);
  return `Bet sizes on the ${p.street} were picked for this spot: ${who}. A rough first solve (${secs(p.secs)}) tried ${SIZES_TRIED.join(", ")}% and these were used most${dropped.length ? `; left out: ${p.street === "flop" ? `${built.value.oopName} ${used(0) || "none"}; ${built.value.ipName} ${used(1) || "none"}` : dropped.join("; ")}` : ""}.`;
});
/** Memory a spot from the spot builder needs, without touching what is on this screen. */
async function previewCustom(p: Custom2): Promise<{ text: string; problem: string }> {
  const sp = { ...specCore.value, oop_range: p.res.oopRange, ip_range: p.res.ipRange, board: p.board.map((c) => RANK_CHARS[c >> 2] + SUIT_CHARS[c & 3]).join(""), pot: p.res.pot, stack: p.res.stack, leaks: [], ...p.overrides } as api.StudySpec;
  try {
    const [mb] = await api.studyMemory(sp);
    const need = p.profile ? mb * 2 : mb;
    const size = (x: number) => (x >= 1024 ? (x / 1024).toFixed(1) + " GB" : x.toFixed(0) + " MB");
    const free = freeMb.value;
    const problem = free && mb > free * 0.85 ? `This needs about ${size(mb)} of memory but only ${size(free)} is free right now. Use fewer bet sizes, or close other programs.` : "";
    return { text: `Every turn and river card. About ${size(need)} of memory${free ? `, ${size(free)} free` : ""}. Usually seconds to a few minutes${p.profile ? ", twice over for the habit" : ""}.`, problem };
  } catch (e) { return { text: "", problem: String((e as Error).message ?? e) }; }
}
/** Takes a spot from the spot builder and solves it. Resolves when the solve is over. */
async function loadCustom(p: Custom2): Promise<{ ok: boolean; error: string }> {
  fromFile.value = false; chipsPerBb.value = p.chips && p.format === "hu" ? p.chips : 10; turnNote.value = null;
  lastPayload = p; manualLocks.value = []; lockStale.value = false; savedId.value = ""; lockRuns.value = [];
  custom.value = { res: p.res, format: p.format }; override.value = p.overrides; board.value = [...p.board];
  autoSizes.value = !!p.autoSizes; picked.value = null;
  if (p.profile) applyProfile(p.profile); else exploitOn.value = false;
  await nextTick();
  oopRangeEdit.value = p.res.oopRange; ipRangeEdit.value = p.res.ipRange;
  await nextTick();
  await startSolve();
  if (step.value === "results" && p.heroHand) { myHandInput.value = p.heroHand; await trackHand(); }
  if (step.value === "results") await runReview();
  return { ok: step.value === "results", error: error.value };
}

// ---------- review of a pasted hand ----------
const lastHeroHand = ref("");
const review = ref<api.Review | null>(null); const reviewError = ref("");
const reviewRows = computed(() => (review.value?.rows ?? []).map((r, i) => ({ r, i })).filter((x) => x.r.hero));
/** Prices each of the hero's decisions in the pasted hand against the normal solve. */
async function runReview() {
  review.value = null; reviewError.value = "";
  const p = lastPayload;
  if (!p?.line?.length || !p.heroHand) return;
  lastHeroHand.value = p.heroHand;
  try { review.value = await api.studyReview("base", heroIdx.value, p.heroHand, p.line); }
  catch (e) { reviewError.value = String((e as Error).message ?? e); }
}
/** bets by the other player that the solve has no exact size for */
const reviewSizeNotes = computed(() => (review.value?.rows ?? []).filter((r) => !r.hero && r.played_as).map((r) =>
  `On the ${r.street} the other player's ${r.played_as!.split(" to ")[0]} of ${fmt(Number(r.played_as!.split(" to ")[1]))} is read as ${reviewAction(r, r.chosen).toLowerCase()}, the closest size in the solve, so the figures after it are for that size.`));
const reviewAction = (r: api.ReviewRow, a: number) => r.actions[a].replace(/^(Bet|Raise to|All-in) (\d+)$/, (_m, v, n) => `${v} ${fmt(Number(n))}`);
const reviewVerdict = (r: api.ReviewRow) => {
  const share = r.freqs[r.chosen] ?? 0; const small = r.loss <= 0.005 * r.pot;
  if (r.chosen === r.best || small) return share >= 0.05 ? "Fine: the solve does this too" : "Costs almost nothing, though the solve rarely does it";
  return r.loss >= 0.05 * r.pot ? "A real mistake" : "A small mistake";
};
/** Opens the result at one decision of the reviewed hand. */
async function showReviewRow(r: api.ReviewRow) {
  if (view.value !== "base") view.value = "base";
  await goTo(0);
  for (const h of r.history) { const v = node.value; if (!v) break; await play(h, v.kind === "player" ? actionLabel(h) : cardLabel(h)); }
  if (lastPayload?.heroHand && myHand.value !== lastPayload.heroHand) { myHandInput.value = lastPayload.heroHand; await trackHand(); }
}
/** One quick solve of this spot on another flop, for the flop report. Leaves this screen empty afterwards. */
async function quickFlop(p: Custom2, flop: number[]) {
  const sp = { ...specCore.value, oop_range: p.res.oopRange, ip_range: p.res.ipRange, board: flop.map((c) => RANK_CHARS[c >> 2] + SUIT_CHARS[c & 3]).join(""), pot: p.res.pot, stack: p.res.stack, ...p.overrides, leaks: [], low_memory: false, accuracy_pct: Math.max(1, Number(p.overrides.accuracy_pct ?? 0)), max_iter: 300 } as api.StudySpec;
  step.value = "setup"; node.value = null;
  const s = await api.studySolve(sp);
  if (s.error) throw new Error(s.error);
  const root = await api.studyNode("base", []);
  const bets = (v: api.NodeView) => v.actions.reduce((a, n, i) => a + (/^(Bet|Raise|All-in)/.test(n) ? v.action_freqs[i] : 0), 0);
  const check = root.actions.findIndex((a) => a.startsWith("Check"));
  const after = check >= 0 ? await api.studyNode("base", [check]) : root;
  let best = -1; after.actions.forEach((n, i) => { if (/^(Bet|All-in)/.test(n) && (best < 0 || after.action_freqs[i] > after.action_freqs[best])) best = i; });
  const h = p.res.heroIsOop ? 0 : 1;
  return { heroEq: root.equity[h], heroEv: s.base_ev[h] / p.res.pot, oopBet: bets(root), ipBet: after.kind === "player" ? bets(after) : 0,
    ipSize: best >= 0 && after.action_freqs[best] > 0.02 ? (after.actions[best].startsWith("All-in") ? "all-in" : Math.round((after.action_amounts[best] / after.pot) * 100) + "% of the pot") : "–" };
}

// ---------- saving, and locks made by hand ----------
let lastPayload: Custom2 | null = null;
const chartMode = ref<"strategy" | "ev" | "equity">("strategy");
const manualLocks = ref<api.ManualLock[]>([]);
const lockOpen = ref(false); const lockScope = ref<"all" | "cell">("all"); const lockPct = ref<number[]>([]);
const lockStale = ref(false); const relocking = ref(false); const lockError = ref("");
const lockedLabel = computed(() => (manualLocks.value.length && summary.value?.locked_nodes ? (leakList.value.length ? "Vs habit and your locks" : "With your locks") : "Vs " + leakSummary.value.toLowerCase()));
const lockTotal = computed(() => lockPct.value.reduce((a, v) => a + (Number(v) || 0), 0));
function fillLock() {
  const v = node.value; if (!v || v.kind !== "player") return;
  if (lockScope.value === "cell" && selectedCell.value === null) lockScope.value = "all";
  const src = lockScope.value === "cell" && selectedCell.value !== null && v.grid[selectedCell.value].weight > 0 ? v.grid[selectedCell.value].freqs : v.action_freqs;
  const out = src.map((f) => Math.round(f * 100));
  const off = 100 - out.reduce((a, x) => a + x, 0);
  if (out.length) out[out.indexOf(Math.max(...out))] += off;
  lockPct.value = out;
}
onMounted(() => watch([node, selectedCell, lockScope, lockOpen], fillLock));
function addLock() {
  const v = node.value!; const cell = lockScope.value === "cell" ? selectedCell.value : null;
  const what = lockPct.value.map((p, i) => (p > 0 ? `${shortAction(i).toLowerCase()} ${p}%` : "")).filter(Boolean).join(", ");
  const where = crumbs.value.length ? crumbs.value.join(" › ") : "First decision";
  manualLocks.value = [...manualLocks.value.filter((l) => !(l.history.join() === history.value.join() && l.cells.join() === (cell === null ? "" : String(cell)))),
    { history: [...history.value], cells: cell === null ? [] : [cell], freqs: lockPct.value.map((p) => (Number(p) || 0) / 100), label: `${where}: ${v.player === 0 ? built.value.oopName : built.value.ipName} with ${cell === null ? "every hand" : cellName(cell)}: ${what}` }];
  lockStale.value = true; savedId.value = "";
}
const removeLock = (i: number) => { manualLocks.value = manualLocks.value.filter((_l, j) => j !== i); lockStale.value = true; };
async function relockNow() {
  relocking.value = true; lockError.value = ""; iter.value = 0;
  const t = window.setInterval(async () => { try { const [, it, mx] = await api.studyProgress(); iter.value = it; maxIter.value = mx || 1; } catch { /* ignore */ } }, 500);
  try {
    const s = await api.studyRelock(spec.value, manualLocks.value);
    if (s.error) { lockError.value = s.error === "Solve cancelled" ? "Stopped. The normal solve is still here." : s.error; summary.value = { ...s, error: null }; view.value = "base"; }
    else { summary.value = s; view.value = "locked"; compareOn.value = true; lockStale.value = false; savedId.value = ""; }
    await load();
    if (!s.error) keepRun();
  } catch (e) { lockError.value = String((e as Error).message ?? e); }
  finally { window.clearInterval(t); relocking.value = false; }
}
/** one row per locked solve, so several lock sets can be read side by side */
interface LockRun { locks: api.ManualLock[]; ev: number; baseEv: number; where: string; who: string; acts: { name: string; f: number; base: number }[] }
const lockRuns = ref<LockRun[]>([]);
function keepRun() {
  const s = summary.value; const v = node.value; if (!s?.locked) return;
  const c = compareNode.value;
  const acts = v && v.kind === "player" ? v.actions.map((_a, i) => ({ name: shortAction(i), f: v.action_freqs[i] ?? 0, base: c && c.kind === "player" ? c.action_freqs[i] ?? 0 : 0 })) : [];
  const key = JSON.stringify(manualLocks.value.map((l) => [l.history, l.cells, l.freqs]));
  const run: LockRun = { locks: manualLocks.value.map((l) => ({ ...l })), ev: s.locked_ev[heroIdx.value], baseEv: s.base_ev[heroIdx.value], where: crumbs.value.length ? crumbs.value.join(" › ") : "First decision", who: v && v.kind === "player" ? (v.player === 0 ? built.value.oopName : built.value.ipName) : "", acts };
  lockRuns.value = [...lockRuns.value.filter((r) => JSON.stringify(r.locks.map((l) => [l.history, l.cells, l.freqs])) !== key), run].slice(-8);
}
const useRun = (i: number) => { manualLocks.value = lockRuns.value[i].locks.map((l) => ({ ...l })); lockStale.value = true; };
/** the other player's hands and replies after one of this player's actions */
async function loadReply(action: number) { const v = await nodeAt(view.value, [...history.value, action]); return { actions: v.actions, combos: v.kind === "player" ? v.combos : [] }; }

const saveOpen = ref(false); const saveName = ref(""); const saveBusy = ref(false); const saveError = ref(""); const savedId = ref("");
function openSave() { saveName.value = `${built.value.description} · ${boardText(node.value?.board.slice(0, 3) ?? [])}`; saveError.value = ""; saveOpen.value = true; }
async function saveNow() {
  if (!lastPayload) { saveError.value = "Only spots set up in Study a spot can be saved."; return; }
  saveBusy.value = true; saveError.value = "";
  try { const e = await api.librarySaveTwo(saveName.value, { payload: lastPayload, locks: lockStale.value ? [] : manualLocks.value, board: boardStr.value }); savedId.value = e.id; saveOpen.value = false; }
  catch (e) { saveError.value = String((e as Error).message ?? e); }
  finally { saveBusy.value = false; }
}
/** Keeps the flop of this solve as a small library file (see the Library page to get the file itself). */
async function shareNow() {
  if (!lastPayload) { saveError.value = "Only spots set up in Study a spot can be saved."; return; }
  saveBusy.value = true; saveError.value = "";
  try { const e = await api.libraryShare(saveName.value, { payload: lastPayload, board: boardStr.value }); savedId.value = e.id; saveOpen.value = false; }
  catch (e) { saveError.value = String((e as Error).message ?? e); }
  finally { saveBusy.value = false; }
}

// ---------- library files: the flop is stored, a turn is solved when its card is picked ----------
const fromFile = ref(false);
const turnBusy = ref(""); const turnError = ref("");
let pendingCard = "";
const turnNote = ref<api.TurnSolve | null>(null);
/** A node of the solve on screen. With a library file, a turn that is not solved yet is solved first. */
async function nodeAt(v: "base" | "locked", hist: number[]): Promise<api.NodeView> {
  try { return await api.studyNode(v, hist); }
  catch (e) {
    if (!fromFile.value || !String((e as Error).message ?? e).includes(api.TURN_NOT_SOLVED)) throw e;
    await solveTurn(hist);
    return await api.studyNode(v, hist);
  }
}
async function solveTurn(hist: number[]) {
  turnError.value = ""; iter.value = 0; expl.value = 100; turnBusy.value = pendingCard || "next";
  const t = window.setInterval(async () => { try { const [, it, , ex] = await api.studyProgress(); iter.value = it; expl.value = ex > 1e5 || it === 0 ? 100 : ex; } catch { /* ignore */ } }, 300);
  try { await api.studyTurn(hist, 0.1, 60, threads.value); }
  catch (e) { const m = String((e as Error).message ?? e); turnError.value = m.includes("cancelled") ? "Stopped. Pick a card to try again." : m; throw e; }
  finally { window.clearInterval(t); turnBusy.value = ""; }
}
/** The setup the result page needs, made from a library file written by the headless program. */
function payloadFromHeadless(h: Record<string, unknown>): Custom2 | null {
  const names = h.names as string[] | undefined; const spot = h.spot as { ranges?: Record<string, string> } | undefined; const tree = h.tree as { starting_pot?: number; effective_stack?: number } | undefined; const b = String(h.board ?? "");
  if (!names || names.length !== 2 || !spot?.ranges || !tree?.starting_pot || b.length !== 6) return null;
  const ids = [0, 2, 4].map((i) => RANK_CHARS.indexOf(b[i]) * 4 + SUIT_CHARS.indexOf(b[i + 1]));
  const res: ScenarioResult = { heroIsOop: false, pfrIsOop: false, oopRange: spot.ranges[names[0]] ?? "", ipRange: spot.ranges[names[1]] ?? "", pot: tree.starting_pot, stack: tree.effective_stack ?? 0,
    description: `${seatLabel(names[1])} against ${seatLabel(names[0]).toLowerCase()}`, oopName: seatLabel(names[0]), ipName: seatLabel(names[1]) };
  return { res, format: "hu", board: ids, profile: null, overrides: {} };
}

/** Re-opens a solve from the library without solving again. */
async function openSaved(e: api.LibraryEntry): Promise<{ ok: boolean; error: string }> {
  const headless = e.meta.headless as Record<string, unknown> | undefined;
  const p = (e.meta.payload as Custom2 | undefined) ?? (headless ? payloadFromHeadless(headless) ?? undefined : undefined);
  if (!p || !e.summary) return { ok: false, error: "That entry cannot be opened." };
  try {
    await api.libraryOpenTwo(e.id);
    fromFile.value = e.kind === "flop"; chipsPerBb.value = headless ? Number(headless.chips_per_bb) || 100 : p.chips && p.format === "hu" ? p.chips : 10; turnNote.value = null; turnError.value = "";
    lastPayload = p; custom.value = { res: p.res, format: p.format }; override.value = p.overrides; board.value = [...p.board];
    if (p.profile) applyProfile(p.profile); else exploitOn.value = false;
    await nextTick();
    oopRangeEdit.value = p.res.oopRange; ipRangeEdit.value = p.res.ipRange;
    manualLocks.value = (e.meta.locks as api.ManualLock[] | undefined) ?? []; lockStale.value = false; savedId.value = e.id; lockRuns.value = [];
    summary.value = { ...e.summary, swap_mode: false }; view.value = e.summary.locked ? "locked" : "base"; compareOn.value = true;
    history.value = []; crumbs.value = []; probs.value = []; takeaway.value = ""; saved.value = false; error.value = "";
    await load();
    step.value = "results";
    if (p.heroHand) { myHandInput.value = p.heroHand; await trackHand(); }
    await runReview();
    return { ok: true, error: "" };
  } catch (x) { return { ok: false, error: String((x as Error).message ?? x) }; }
}
defineExpose({ loadCustom, previewCustom, quickFlop, openSaved });
const oopRangeEdit = ref(""), ipRangeEdit = ref("");
const resetRanges = () => { oopRangeEdit.value = built.value.oopRange; ipRangeEdit.value = built.value.ipRange; };
watch(() => [built.value.oopRange, built.value.ipRange], resetRanges, { immediate: true });
const oppHelp = computed(() => OPPONENTS.find((o) => o.id === opponent.value)?.help ?? "");
const heroIdx = computed(() => (built.value.heroIsOop ? 0 : 1));
const villIdx = computed(() => 1 - heroIdx.value);
const villainName = computed(() => (villIdx.value === 0 ? built.value.oopName : built.value.ipName));
const sizingList = computed(() => SIZINGS[fmtKind.value]);
const sizing = computed(() => sizingList.value.find((s) => s.id === sizingId.value) ?? sizingList.value[0]);
const accuracy = computed(() => ACCURACY.find((a) => a.id === accuracyId.value)!);

const toggleBoard = (c: number) => {
  const i = board.value.indexOf(c);
  if (i >= 0) board.value.splice(i, 1);
  else if (board.value.length < 3) board.value.push(c);
};
const boardCards = computed(() =>
  [...board.value].sort((a, b) => b - a).map((c) => RANK_CHARS[c >> 2] + SUIT_CHARS[c & 3])
);

const applyProfile = (id: string) => {
  exploitOn.value = true;
  profileId.value = id;
  if (id === "custom") return;
  const p = PROFILES.find((x) => x.id === id)!;
  LEAKS.forEach((l) => {
    leakOn[l.id] = l.id in p.leaks;
    if (l.id in p.leaks) leakStrength[l.id] = p.leaks[l.id];
  });
};
const leakList = computed<api.Leak[]>(() =>
  exploitOn.value
    ? LEAKS.filter((l) => leakOn[l.id]).map((l) => ({ player: villIdx.value, street: l.street, effect: l.effect, strength: leakStrength[l.id] }))
    : []
);
const leakSummary = computed(() => {
  if (profileId.value !== "custom") return PROFILES.find((p) => p.id === profileId.value)?.label ?? "";
  return LEAKS.filter((l) => leakOn[l.id]).map((l) => l.label.split(" — ")[0].toLowerCase()).join(", ");
});
// live reading; this tab's own stored solve is dropped when the next one starts, so it counts as free
startMemoryWatch();
const freeMb = computed(() => (memory.availMb ? memory.availMb + memory.heldGuidedMb : 0));
watch(() => memory.released.guided, () => { step.value = "setup"; });
// memory saver: park the normal solve on disk while the exploit solve runs, and keep only one solve
// in memory while browsing. Turned on automatically when a normal solve won't fit comfortably.
const lowMemory = computed(() => leakList.value.length > 0 && (forceSaver.value || (freeMb.value > 0 && memMb.value > freeMb.value * 0.6)));
const memNeeded = computed(() => (lowMemory.value ? memLow.value : memMb.value));
const setupError = computed(() => {
  if (exploitOn.value && leakList.value.length === 0) return "Tick at least one habit, or choose “No habit”.";
  if (freeMb.value && memNeeded.value > freeMb.value * 0.85)
    return `This needs about ${(memNeeded.value / 1024).toFixed(1)} GB of memory but only ${(freeMb.value / 1024).toFixed(1)} GB of your ${gb(memory.totalMb)} is free right now. Choose “One size per street”, a smaller stack or fewer habits, or close other programs; the figure updates by itself.`;
  return "";
});

const boardStr = computed(() => board.value.map((c) => RANK_CHARS[c >> 2] + SUIT_CHARS[c & 3]).join(""));
const threads = ref(4);
api.cpuThreads().then((n) => (threads.value = n)).catch(() => {});

const specCore = computed<api.StudySpec>(() => {
  const s = sizing.value;
  const pfrOop = built.value.pfrIsOop;
  return {
    oop_range: oopRangeEdit.value, ip_range: ipRangeEdit.value, board: boardStr.value,
    pot: built.value.pot, stack: built.value.stack,
    oop_flop_bet: pfrOop ? s.pfrFlop : s.callerFlop, ip_flop_bet: pfrOop ? s.callerFlop : s.pfrFlop,
    turn_bet: s.turn, river_bet: s.river, raise: s.raise, allin_threshold: 1.5,
    accuracy_pct: accuracy.value.pct, max_iter: accuracy.value.iters, leaks: leakList.value, threads: threads.value,
    low_memory: false, freeze_rest: freezeRest.value, confidence: confidence.value,
    ...(custom.value ? override.value : {}),
    ...(picked.value?.overrides ?? {}),
  };
});
const spec = computed<api.StudySpec>(() => ({ ...specCore.value, low_memory: lowMemory.value }));
watch(specCore, async (sp) => {
  if (sp.board.length !== 6) { memMb.value = 0; memLow.value = 0; return; }
  try { [memMb.value, memLow.value] = await api.studyMemory(sp); error.value = ""; }
  catch (e) { memMb.value = 0; memLow.value = 0; error.value = String(e); }
}, { immediate: true, deep: true });

// ---------- solving ----------
const phase = ref(0), iter = ref(0), maxIter = ref(1), expl = ref(100);
const startedAt = ref(0), phaseStartedAt = ref(0), now = ref(0);
let timer: number | undefined;
const phaseText = computed(() =>
  picking.value ? `First, a rough solve to pick bet sizes: trying ${SIZES_TRIED.join(", ")}% of the pot` :
  ["Starting", "Step 1 of " + (leakList.value.length ? 2 : 1) + ": the normal (game-theory) strategy", "Applying the habit to every decision", "Step 2 of 2: your best counter-strategy", "Done", "Stopped"][phase.value] ?? ""
);
const progressPct = computed(() => {
  const target = accuracy.value.pct;
  const conv = expl.value <= target ? 1 : Math.max(0, Math.min(1, Math.log(100 / expl.value) / Math.log(100 / target)));
  const p = Math.max(iter.value / maxIter.value, conv);
  if (!leakList.value.length) return Math.round(p * 100);
  if (phase.value <= 1) return Math.round(p * 50);
  if (phase.value === 2) return 50;
  return Math.round(50 + p * 50);
});
const elapsedText = computed(() => secs((now.value - startedAt.value) / 1000));
const etaText = computed(() => {
  const t = (now.value - phaseStartedAt.value) / 1000;
  if (iter.value < 10 || t < 3) return "Estimating time…";
  const rate = iter.value / t;
  let left = (maxIter.value - iter.value) / rate;
  if (leakList.value.length && phase.value <= 1) left += maxIter.value / rate; // second solve to come
  return `at most ~${secs(left)} left, often less`;
});

const summary = ref<api.StudySummary | null>(null);
const startSolve = async () => {
  error.value = "";
  step.value = "solving";
  phase.value = 0; iter.value = 0; expl.value = 100;
  startedAt.value = phaseStartedAt.value = now.value = Date.now();
  let lastPhase = 0;
  timer = window.setInterval(async () => {
    now.value = Date.now();
    try {
      const [ph, it, mx, ex] = await api.studyProgress();
      if (ph !== lastPhase) { lastPhase = ph; phaseStartedAt.value = Date.now(); }
      phase.value = ph; iter.value = it; maxIter.value = mx || 1; expl.value = ex > 1e5 || it === 0 ? 100 : ex;
    } catch { /* ignore */ }
  }, 500);
  try {
    if (autoSizes.value && custom.value) {
      picked.value = null; picking.value = true;
      try { picked.value = await api.studyPickSizes(spec.value, SIZES_TRIED, 2, freeMb.value ? freeMb.value * 0.85 : 0); }
      finally { picking.value = false; }
      phaseStartedAt.value = Date.now(); iter.value = 0; expl.value = 100;
    }
    const s = await api.studySolve(spec.value);
    window.clearInterval(timer);
    if (s.error) { error.value = s.error; step.value = "setup"; return; }
    summary.value = s;
    view.value = s.locked ? "locked" : "base";
    compareOn.value = !s.swap_mode;
    history.value = []; crumbs.value = []; probs.value = [];
    takeaway.value = ""; saved.value = false;
    await load();
    step.value = "results";
  } catch (e) {
    window.clearInterval(timer);
    error.value = String(e);
    step.value = "setup";
  }
};
const cancel = () => api.studyCancel();

// ---------- results ----------
const view = ref<"base" | "locked">("base");
const history = ref<number[]>([]);
const crumbs = ref<string[]>([]);
const probs = ref<number[]>([]); // probability of reaching each step of the line
const node = ref<api.NodeView | null>(null);
const compareNode = ref<api.NodeView | null>(null);
const selectedCell = ref<number | null>(null);
const takeaway = ref("");
const copied = ref(false);
const saved = ref(false);
const compareOn = ref(true);
const busy = ref(false);
const introDismissed = ref(store("ns-intro") === "1");
const dismissIntro = () => { introDismissed.value = true; persist("ns-intro", "1"); };

const load = async () => {
  busy.value = true;
  try {
    node.value = await nodeAt(view.value, history.value);
    compareNode.value = summary.value?.locked && compareOn.value
      ? await api.studyNode(view.value === "locked" ? "base" : "locked", history.value)
      : null;
    turnNote.value = fromFile.value && node.value.board.length > 3 ? await api.studyTurnNote(history.value) : null;
    await refreshMyHand();
  } finally { busy.value = false; }
  if (!takeaway.value) takeaway.value = await suggestTakeaway();
};
const setView = async (v: "base" | "locked") => { view.value = v; await load(); };
const play = async (action: number, label: string) => {
  const v = node.value!;
  const actor = v.kind === "player" ? actorName.value : "";
  const p = (probs.value[probs.value.length - 1] ?? 1) * (v.kind === "player" ? v.action_freqs[action] : 1);
  history.value = [...history.value, action];
  probs.value = [...probs.value, p];
  crumbs.value = [...crumbs.value, actor ? `${actor}: ${label}` : `${v.board.length === 3 ? "Turn" : "River"} ${label}`];
  selectedCell.value = null;
  pendingCard = v.kind === "chance" ? label : "";
  try { await load(); }
  catch (e) {
    // a library file's turn could not be solved (or was stopped): stay where the card is picked
    if (!(fromFile.value && v.kind === "chance")) throw e;
    history.value = history.value.slice(0, -1); probs.value = probs.value.slice(0, -1); crumbs.value = crumbs.value.slice(0, -1);
  }
};
const goTo = async (n: number) => {
  history.value = history.value.slice(0, n);
  crumbs.value = crumbs.value.slice(0, n);
  probs.value = probs.value.slice(0, n);
  selectedCell.value = null;
  await load();
};
const lineProb = computed(() => probs.value[probs.value.length - 1] ?? 1);
const probText = (p: number) => (p >= 0.1 ? pct(p) : p >= 0.01 ? `${(p * 100).toFixed(1)}%` : "under 1%");

const actorName = computed(() => {
  if (!node.value || node.value.kind !== "player") return "";
  return node.value.player === 0 ? built.value.oopName : built.value.ipName;
});
const colors = computed(() => actionColors(node.value?.actions ?? []));
const actionLabel = (i: number) => {
  const v = node.value!;
  const a = v.actions[i];
  if (a === undefined) return ""; // a list from the previous decision can still be on screen for a moment
  const amt = v.action_amounts[i];
  if (a.startsWith("Bet")) return `Bet ${fmt(amt)} (${Math.round((amt / v.pot) * 100)}% pot)`;
  if (a.startsWith("Raise")) return `Raise to ${fmt(amt)}`;
  if (a.startsWith("All-in")) return `All-in ${fmt(amt)}`;
  return a;
};
const shortAction = (i: number) => {
  const v = node.value!;
  const a = v.actions[i];
  if (a.startsWith("Bet")) return `bet ${Math.round((v.action_amounts[i] / v.pot) * 100)}%`;
  if (a.startsWith("Raise")) return "raise";
  if (a.startsWith("All-in")) return "all-in";
  return a.toLowerCase();
};
const notes = computed(() => (node.value ? describeNode(node.value, actorName.value, fmt) : []));
const compareNotes = computed(() => {
  if (!node.value || !compareNode.value || view.value !== "locked") return [];
  return compareNodes(compareNode.value, node.value, actorName.value, node.value.player === heroIdx.value, fmt);
});
const evGain = computed(() => (summary.value ? summary.value.locked_ev[heroIdx.value] - summary.value.base_ev[heroIdx.value] : 0));
const dealable = computed(() => new Set((node.value?.cards ?? []).map((c) => c[1])));
const terminalText = computed(() => {
  const last = crumbs.value[crumbs.value.length - 1] ?? "";
  return last.includes("Fold") ? "Someone folded, so the other player wins the pot." : "The hand goes to showdown and the best hand wins.";
});

const cardLabel = (id: number) => RANK_CHARS[id >> 2] + SUIT_SYMBOL[SUIT_CHARS[id & 3]];
const suitColor = (id: number) => SUIT_COLOR[SUIT_CHARS[id & 3]];
const RANK_ORDER = "23456789TJQKA";
const sortFlop = (b: string[]) => [...b.slice(0, 3).sort((x, y) => RANK_ORDER.indexOf(y[0]) - RANK_ORDER.indexOf(x[0])), ...b.slice(3)];
/** what the next player does, and who is ahead, if this card comes */
let cardNotes: { key: string; notes: Map<number, api.CardNote> } | null = null;
async function loadCard(id: number): Promise<CardRow> {
  if (fromFile.value && node.value?.board.length === 3) {
    // a library file keeps what the whole solve said about every turn card, so none has to be solved for this
    const key = history.value.join(",");
    if (cardNotes?.key !== key) cardNotes = { key, notes: new Map((await api.studyCards(history.value)).map((c) => [c.id, c])) };
    const c = cardNotes.notes.get(id);
    if (c) return { actor: built.value.oopName, bet: c.bet, equity: [...c.equity] };
  }
  const v = await nodeAt(view.value, [...history.value, id]);
  const bet = v.actions.reduce((a, name, i) => a + (/^(Bet|Raise|All-in)/.test(name) ? v.action_freqs[i] : 0), 0);
  return { actor: v.player === 0 ? built.value.oopName : built.value.ipName, bet, equity: [...v.equity] };
}
const displayBoard = computed(() => sortFlop(node.value?.board ?? []));
const boardText = (b: string[]) => sortFlop(b).map((c) => c[0] + SUIT_SYMBOL[c[1]]).join(" ");
const R = "AKQJT98765432";
const cellName = (i: number) => {
  const r = Math.floor(i / 13), c = i % 13;
  return r === c ? R[r] + R[c] : r < c ? R[r] + R[c] + " suited" : R[c] + R[r] + " offsuit";
};
const SYMBOL_SUIT: Record<string, string> = { "♣": "c", "♦": "d", "♥": "h", "♠": "s" };
const steps = computed<Step[]>(() => crumbs.value.map((c) => {
  const k = c.indexOf(": ");
  if (k >= 0) return { who: c.slice(0, k), text: c.slice(k + 2) };
  const [street, card = ""] = c.split(" ");
  const suit = SYMBOL_SUIT[card.slice(-1)]; const r = card.slice(0, -1) === "10" ? "T" : card.slice(0, -1);
  return suit ? { who: street, text: card, card: r + suit } : { who: street, text: card };
}));
const nowText = computed(() => {
  const v = node.value; if (!v) return "";
  return v.kind === "player" ? `${actorName.value} to act` : v.kind === "chance" ? `Deal the ${v.board.length === 3 ? "turn" : "river"}` : "Hand over";
});
const cellCombos = computed(() => (node.value?.combos ?? []).filter((c) => c.cell === selectedCell.value));
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
// the other player's range beside the deciding player's; open or closed is remembered on this computer
const otherOn = ref(false);
try { otherOn.value = localStorage.getItem("pontiac.other") === "on"; } catch { /* no storage: closed */ }
function setOther(open: boolean) { if (open === otherOn.value) return; otherOn.value = open; try { localStorage.setItem("pontiac.other", open ? "on" : "off"); } catch { /* not remembered */ } }
/** one neutral bar per square: how much of the hand is left */
const otherGrid = computed(() => (node.value?.kind === "player" && node.value.other_grid?.length === 169 ? node.value.other_grid.map((c) => ({ ...c, freqs: c.weight > 0 ? [1] : [] })) : null));
const otherCombos = computed(() => (node.value?.other_grid ?? []).reduce((s, c) => s + (c.reach ?? 0), 0));
// a type of hand picked in "play by type of hand": the chart fades every other square and the hands table lists only that type
const typePick = ref<Record<string, "show" | "hide">>({});
const typeOn = computed(() => Object.keys(typePick.value).length > 0);
const typeText = computed(() => {
  const of = (m: string) => Object.entries(typePick.value).filter(([, v]) => v === m).map(([k]) => k.toLowerCase());
  const [show, hide] = [of("show"), of("hide")];
  return [show.length ? `Showing ${show.join(", ")} only.` : "", hide.length ? `Hiding ${hide.join(", ")}.` : ""].filter(Boolean).join(" ");
});
const typeHands = computed<Set<string> | null>(() => {
  const n = node.value;
  if (!n || !typeOn.value || n.kind !== "player") return null;
  const show = new Set(Object.entries(typePick.value).filter(([, v]) => v === "show").map(([k]) => k));
  const hide = new Set(Object.entries(typePick.value).filter(([, v]) => v === "hide").map(([k]) => k));
  const out = new Set<string>();
  for (const c of n.combos) {
    if (c.weight <= 0 || c.hand.length !== 4) continue;
    const k = classify(c.hand, n.board); const types = [k.made as string, ...(k.draws as string[])];
    if (types.some((t) => hide.has(t))) continue;
    if (!show.size || types.some((t) => show.has(t))) out.add(c.hand);
  }
  return out;
});
/** per chart square: does any hand of the picked type sit in it? null when nothing is picked or the type is not held here */
const typeLit = computed<boolean[] | null>(() => {
  const n = node.value; const hands = typeHands.value;
  if (!n || !hands) return null;
  const lit = new Array<boolean>(169).fill(false);
  for (const c of n.combos) if (hands.has(c.hand)) lit[c.cell] = true;
  return lit;
});
// the table drawing can be hidden; the choice is remembered on this computer
const tableOn = ref(true);
try { tableOn.value = localStorage.getItem("pontiac.table") !== "off"; } catch { /* no storage: keep it on */ }
function toggleTable() { tableOn.value = !tableOn.value; try { localStorage.setItem("pontiac.table", tableOn.value ? "on" : "off"); } catch { /* not remembered */ } }
const selectCell = (i: number) => { selectedCell.value = selectedCell.value === i ? null : i; };

// ---------- your hand ----------
const myHandInput = ref("");
const myHand = ref("");
const myHandError = ref("");
const myView = ref<api.HandView | null>(null);
const myCompare = ref<api.HandView | null>(null);
const normHand = (s: string) => {
  const t = s.replace(/\s/g, "");
  if (!/^[2-9tjqkaTJQKA][cdhsCDHS][2-9tjqkaTJQKA][cdhsCDHS]$/.test(t)) return "";
  return t[0].toUpperCase() + t[1].toLowerCase() + t[2].toUpperCase() + t[3].toLowerCase();
};
const myCell = computed(() => {
  if (!myHand.value) return null;
  const r1 = R.indexOf(myHand.value[0]), r2 = R.indexOf(myHand.value[2]);
  const suited = myHand.value[1] === myHand.value[3];
  const [hi, lo] = r1 <= r2 ? [r1, r2] : [r2, r1];
  if (hi === lo) return hi * 13 + hi;
  return suited ? hi * 13 + lo : lo * 13 + hi;
});
const trackHand = async () => {
  const h = normHand(myHandInput.value);
  if (!h) { myHandError.value = "Type two cards with suits, like AhKd or 7c7d."; return; }
  if (h.slice(0, 2) === h.slice(2)) { myHandError.value = "Those are the same card twice."; return; }
  myHandError.value = "";
  myHand.value = h;
  myHandInput.value = h;
  await refreshMyHand();
};
const trackCell = async (i: number) => {
  const r = Math.floor(i / 13), c = i % 13;
  const used = new Set(node.value?.board ?? []);
  const suits = ["s", "h", "d", "c"];
  let pick = "";
  if (r === c || r > c) {
    // pair or offsuit: two different suits
    const [a, b] = r === c ? [R[r], R[r]] : [R[c], R[r]];
    outer: for (const s1 of suits) for (const s2 of suits) {
      if (s1 === s2) continue;
      if (!used.has(a + s1) && !used.has(b + s2)) { pick = a + s1 + b + s2; break outer; }
    }
  } else {
    for (const s of suits) if (!used.has(R[r] + s) && !used.has(R[c] + s)) { pick = R[r] + s + R[c] + s; break; }
  }
  if (pick) { myHandInput.value = pick; await trackHand(); }
};
async function refreshMyHand() {
  if (!myHand.value || !summary.value) { myView.value = null; myCompare.value = null; return; }
  try {
    myView.value = await api.studyHand(view.value, history.value, heroIdx.value, myHand.value);
    myCompare.value = compareOn.value && summary.value.locked
      ? await api.studyHand(view.value === "locked" ? "base" : "locked", history.value, heroIdx.value, myHand.value)
      : null;
  } catch (e) { myHandError.value = String(e); }
}

// ---------- takeaway & notebook ----------
async function suggestTakeaway(): Promise<string> {
  const hist = history.value;
  let hv = await api.studyNode(view.value, hist);
  let path = hist;
  let facing = "";
  if (hv.kind === "player" && hv.player !== heroIdx.value) {
    // follow the other player's most common action, so the note is about a spot that actually comes up
    const top = hv.action_freqs.indexOf(Math.max(...hv.action_freqs));
    const name = hv.actions[top];
    facing = name === "Check" ? " when checked to" : ` facing a ${/^All-in/.test(name) ? "shove" : "bet"} of ${fmt(hv.action_amounts[top])}`;
    path = [...hist, top];
    hv = await api.studyNode(view.value, path);
  }
  if (hv.kind !== "player") return "";
  const words = (v: api.NodeView) => {
    const ag = (fr: number[]) => fr.map((f, i) => (/^(Bet|Raise|All-in)/.test(v.actions[i]) ? f : 0)).reduce((x, y) => x + y, 0);
    if (v.actions[0] === "Fold") {
      const call = v.actions.findIndex((x) => x === "Call");
      return `fold ${pct(v.action_freqs[0])}, call ${pct(call >= 0 ? v.action_freqs[call] : 0)}, raise ${pct(ag(v.action_freqs))}`;
    }
    return `bet ${pct(ag(v.action_freqs))} (strong hands ${pct(ag(v.buckets[0].freqs))}, weak hands ${pct(ag(v.buckets[2].freqs))})`;
  };
  const where = facing;
  const base = view.value === "base" ? hv : await api.studyNode("base", path);
  let text = `${built.value.description} On ${boardText(hv.board)}${where}: game-theory play is to ${words(base)}.`;
  if (summary.value?.locked) {
    const lv = view.value === "locked" ? hv : await api.studyNode("locked", path);
    if (lv.kind === "player") text += ` Against ${leakSummary.value.toLowerCase()}: ${words(lv)}, worth about ${evGain.value >= 0 ? "+" : ""}${fmt(evGain.value)} a hand if the habit is that strong.`;
  }
  return text + " [Pontiac Solver solve]";
}
const copyText = async (t: string) => {
  try { await navigator.clipboard.writeText(t); copied.value = true; setTimeout(() => (copied.value = false), 1500); } catch { /* ignore */ }
};
interface Note { at: number; text: string }
const notebook = ref<Note[]>(JSON.parse(store("ns-notebook") ?? "[]"));
const notebookOpen = ref(false);
watch(notebook, (n) => persist("ns-notebook", JSON.stringify(n)), { deep: true });
const saveTakeaway = () => {
  if (!takeaway.value.trim()) return;
  notebook.value = [{ at: Date.now(), text: takeaway.value.trim() }, ...notebook.value];
  saved.value = true;
  setTimeout(() => (saved.value = false), 1800);
};
const removeNote = (i: number) => { notebook.value = notebook.value.filter((_, j) => j !== i); };
const notebookText = computed(() => notebook.value.map((n) => `${new Date(n.at).toLocaleDateString()}\n${n.text}`).join("\n\n"));
const downloadNotebook = () => {
  const a = document.createElement("a");
  a.href = URL.createObjectURL(new Blob([notebookText.value], { type: "text/plain" }));
  a.download = "pontiac-solver-notebook.txt";
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 2000);
};
</script>

<style scoped>
.step-title { @apply font-display font-extrabold text-lg flex items-center gap-2.5; }
.step-num { @apply inline-flex items-center justify-center w-7 h-7 rounded-full bg-ink text-white text-sm font-body font-bold; }
.choice { @apply w-full text-left bg-surface border border-rule rounded-lg px-3.5 py-3 hover:border-felt transition-colors; }
.choice-on { @apply border-felt ring-1 ring-felt bg-felt-light; }
.pill { @apply px-3 py-1.5 rounded-full border border-rule bg-surface text-sm hover:border-felt transition-colors; }
.pill-on { @apply border-felt bg-felt text-white hover:border-felt; }
.field { @apply flex flex-col text-sm gap-1 font-bold; }
.input { @apply rounded-md border-rule text-sm font-normal focus:border-felt focus:ring-felt; }
.hint { @apply text-sm text-muted; }
.deck-btn { @apply h-9 rounded-md border border-rule bg-surface text-sm font-bold font-display hover:border-felt disabled:opacity-25 disabled:hover:border-rule; }
.deck-on { @apply !bg-ink !text-white border-ink; }
.stat { @apply rounded-lg bg-paper py-2; }
.stat dt { @apply text-xs text-muted; }
.stat dd { @apply font-display font-extrabold text-lg tabular-nums; }
.btn-solve { @apply w-full py-3 rounded-lg bg-felt text-white text-lg font-bold hover:bg-felt-dark disabled:opacity-40 disabled:hover:bg-felt; }
.btn-primary { @apply px-3.5 py-1.5 rounded-md bg-felt text-white text-sm font-bold hover:bg-felt-dark; }
.btn-quiet { @apply px-3 py-1.5 rounded-md border border-rule bg-surface text-sm font-bold hover:border-felt disabled:opacity-40; }
.crumb { @apply px-2.5 py-1 rounded-md bg-surface border border-rule hover:border-felt; }
.panel { @apply bg-surface rounded-xl border border-rule p-4; }
.action-row { @apply w-full flex items-center gap-3 rounded-lg border border-rule px-3 py-2 hover:border-felt hover:bg-felt-light/40 transition-colors; }
.seg { @apply inline-flex rounded-lg border border-ink overflow-hidden; }
.seg button { @apply px-3 py-1.5 text-sm font-bold bg-surface; }
.seg .seg-on { @apply bg-felt text-white; }
button:focus-visible, input:focus-visible, select:focus-visible, summary:focus-visible { @apply outline-none ring-2 ring-felt ring-offset-1; }
</style>
