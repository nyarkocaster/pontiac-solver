<template>
  <div class="h-full overflow-y-auto bg-paper text-ink font-body">
    <div class="max-w-[1180px] mx-auto px-6 py-6">
      <!-- top row (results) -->
      <div v-if="step === 'results'" class="flex items-center justify-between gap-4">
        <button class="btn-quiet" @click="step = 'setup'">New spot</button>
        <button class="btn-quiet" @click="notebookOpen = true">
          Notebook<span v-if="notebook.length" class="ml-1.5 inline-flex min-w-[1.25rem] justify-center rounded-full bg-felt text-white text-xs px-1.5">{{ notebook.length }}</span>
        </button>
      </div>

      <!-- ================= SETUP ================= -->
      <div v-if="step === 'setup'" class="grid grid-cols-12 gap-8">
        <div class="col-span-7">
          <div class="flex items-center justify-between">
            <h1 class="font-display font-extrabold text-[2rem] leading-tight tracking-tight">Study a spot</h1>
            <button class="btn-quiet" @click="notebookOpen = true">
              Notebook<span v-if="notebook.length" class="ml-1.5 inline-flex min-w-[1.25rem] justify-center rounded-full bg-felt text-white text-xs px-1.5">{{ notebook.length }}</span>
            </button>
          </div>
          <p class="text-muted mt-1 max-w-[60ch]">
            Set up a hand you play often. The solver finds the soundest strategy for both players, then — if you
            choose — how to beat an opponent with a specific habit.
          </p>

          <section class="mt-7">
            <h2 class="step-title"><span class="step-num">1</span>What happened before the flop?</h2>
            <div class="grid grid-cols-2 gap-2.5 mt-3">
              <button
                v-for="s in SCENARIOS"
                :key="s.id"
                class="choice"
                :class="scenarioId === s.id ? 'choice-on' : ''"
                :aria-pressed="scenarioId === s.id"
                @click="pickScenario(s.id)"
              >
                <div class="font-bold">{{ s.label }}</div>
                <div class="text-sm text-muted mt-0.5">{{ s.help }}</div>
              </button>
            </div>
            <div class="flex flex-wrap gap-5 mt-4 items-end">
              <label class="field">
                <span>{{ scenario.seatQuestion }}</span>
                <select v-model="seat" class="input">
                  <option v-for="s in scenario.seats" :key="s" :value="s">{{ seatLabel(s) }}</option>
                </select>
              </label>
              <label v-if="scenario.format === 'live'" class="field">
                <span>Opponent</span>
                <select v-model="opponent" class="input">
                  <option v-for="o in OPPONENTS" :key="o.id" :value="o.id">{{ o.label }}</option>
                </select>
              </label>
              <label v-if="scenario.format === 'live'" class="field">
                <span>Open size ($)</span>
                <input v-model.number="openSize" type="number" min="4" max="60" class="input w-24" />
              </label>
              <label class="field">
                <span>{{ scenario.format === 'live' ? 'Starting stack ($)' : 'Stack (big blinds)' }}</span>
                <input v-model.number="stackIn" type="number" min="5" max="2000" class="input w-24" />
              </label>
            </div>
            <p v-if="scenario.format === 'live'" class="hint mt-2 max-w-[70ch]">{{ oppHelp }}</p>
            <details class="mt-2 text-sm">
              <summary class="cursor-pointer text-felt font-bold">Edit the ranges</summary>
              <div class="grid grid-cols-2 gap-4 mt-2">
                <label class="field">
                  <span>{{ built.oopName }} (acts first after the flop)</span>
                  <textarea v-model="oopRangeEdit" rows="3" class="input font-body text-xs"></textarea>
                </label>
                <label class="field">
                  <span>{{ built.ipName }} (acts last)</span>
                  <textarea v-model="ipRangeEdit" rows="3" class="input font-body text-xs"></textarea>
                </label>
              </div>
              <button class="text-felt underline mt-1" @click="resetRanges">Reset to the built-in ranges</button>
            </details>
          </section>

          <section class="mt-8">
            <h2 class="step-title"><span class="step-num">2</span>Pick the flop</h2>
            <div class="flex flex-wrap gap-2 mt-3 items-center">
              <span class="text-sm text-muted">Deal a random</span>
              <button v-for="t in TEXTURES" :key="t.id" class="pill" @click="board = randomFlop(t.id)">
                {{ t.label.replace('Any random flop', 'any flop') }}
              </button>
            </div>
            <div class="mt-3 inline-grid gap-1" style="grid-template-columns: repeat(13, 2.25rem)" role="group" aria-label="Choose flop cards">
              <template v-for="s in [3, 2, 1, 0]" :key="s">
                <button
                  v-for="r in RANKS_DESC"
                  :key="r * 4 + s"
                  class="deck-btn"
                  :class="[suitColor(r * 4 + s), board.includes(r * 4 + s) ? 'deck-on' : '']"
                  :disabled="!board.includes(r * 4 + s) && board.length >= 3"
                  :aria-pressed="board.includes(r * 4 + s)"
                  @click="toggleBoard(r * 4 + s)"
                >
                  {{ cardLabel(r * 4 + s) }}
                </button>
              </template>
            </div>
            <p class="hint mt-1">Click cards to add or remove them. You need exactly three.</p>
          </section>

          <section class="mt-8">
            <h2 class="step-title"><span class="step-num">3</span>Exploit a habit? <span class="font-normal text-muted text-base">(optional)</span></h2>
            <p class="hint mt-1 max-w-[70ch]">
              This is node locking. We make {{ villainName.toLowerCase() }} play with the habit at every decision in the
              hand, then the solver works out your best counter-strategy. Start without a habit to learn the baseline.
            </p>
            <div class="flex flex-wrap gap-2 mt-3">
              <button class="pill" :class="!exploitOn ? 'pill-on' : ''" @click="exploitOn = false">No habit</button>
              <button
                v-for="p in PROFILES"
                :key="p.id"
                class="pill"
                :class="exploitOn && profileId === p.id ? 'pill-on' : ''"
                :title="p.help"
                @click="applyProfile(p.id)"
              >
                {{ p.label }}
              </button>
              <button class="pill" :class="exploitOn && profileId === 'custom' ? 'pill-on' : ''" @click="applyProfile('custom')">
                Pick habits myself
              </button>
            </div>
            <div v-if="exploitOn" class="mt-3 bg-white rounded-lg border border-rule">
              <div
                v-for="l in LEAKS"
                :key="l.id"
                class="flex items-center gap-3 px-4 py-2.5 border-b border-rule last:border-0"
                :class="leakOn[l.id] ? 'bg-felt-light/60' : ''"
              >
                <input :id="'lk' + l.id" v-model="leakOn[l.id]" type="checkbox" class="rounded text-felt" @change="profileId = 'custom'" />
                <label :for="'lk' + l.id" class="flex-1 cursor-pointer">
                  <div class="font-bold">{{ l.label }}</div>
                  <div class="text-sm text-muted">{{ l.help }}</div>
                </label>
                <select v-model.number="leakStrength[l.id]" class="input py-1 text-sm" :disabled="!leakOn[l.id]" :aria-label="'How strong: ' + l.label" @change="profileId = 'custom'">
                  <option v-for="s in STRENGTHS" :key="s.value" :value="s.value">{{ s.label }}</option>
                </select>
              </div>
              <label class="flex items-start gap-3 px-4 py-3 border-t border-rule cursor-pointer">
                <input v-model="freezeRest" type="checkbox" class="rounded text-felt mt-1" />
                <span>
                  <span class="font-bold">Keep the rest of their game normal</span> <span class="text-muted">(recommended)</span>
                  <span class="block text-sm text-muted">
                    Everything else they do stays at normal strategy, so they play like a regular player with this one
                    leak. Untick it to let the solver re-plan the rest of their game around the habit — mathematically
                    stronger for them, but nothing like how real players adjust.
                  </span>
                </span>
              </label>
              <div class="px-4 py-3 border-t border-rule">
                <div class="font-bold">How sure are you about this read?</div>
                <div class="flex flex-wrap gap-2 mt-2" role="group" aria-label="Read confidence">
                  <button
                    v-for="c in CONFIDENCE"
                    :key="c.value"
                    class="pill"
                    :class="confidence === c.value ? 'pill-on' : ''"
                    @click="confidence = c.value"
                  >
                    {{ c.label }}
                  </button>
                </div>
                <p class="text-sm text-muted mt-1.5">{{ CONFIDENCE.find((c) => c.value === confidence)?.help }}</p>
              </div>
              <p class="hint px-4 py-3 border-t border-rule">
                Strength moves that share of the relevant frequency: a little 30%, a lot 60%, extreme 90%.
                Test one habit at a time first — combinations can produce surprising answers. Exploits often need a
                bigger bet size, so use a sizing with two flop sizes: with only a small bet available, the solver may
                prefer to check and trap instead.
              </p>
            </div>
          </section>

          <section class="mt-8 mb-6">
            <h2 class="step-title"><span class="step-num">4</span>Bet sizes and accuracy</h2>
            <div class="grid grid-cols-3 gap-2.5 mt-3">
              <button
                v-for="s in sizingList"
                :key="s.id"
                class="choice"
                :class="sizingId === s.id ? 'choice-on' : ''"
                :aria-pressed="sizingId === s.id"
                @click="sizingId = s.id"
              >
                <div class="font-bold">{{ s.label }}</div>
                <div class="text-sm text-muted mt-0.5">{{ s.help }}</div>
              </button>
            </div>
            <div class="flex flex-wrap gap-2 mt-3" role="group" aria-label="Accuracy">
              <button
                v-for="a in ACCURACY"
                :key="a.id"
                class="pill"
                :class="accuracyId === a.id ? 'pill-on' : ''"
                @click="accuracyId = a.id"
              >
                {{ a.label }}<span class="opacity-75"> · {{ a.help.replace('.', '') }}</span>
              </button>
            </div>
          </section>
        </div>

        <!-- summary rail -->
        <aside class="col-span-5">
          <div class="sticky top-4 bg-white border border-rule rounded-xl p-5">
            <div class="flex gap-2">
              <PlayingCard v-for="i in 3" :key="i" :card="boardCards[i - 1] ?? ''" size="lg" />
            </div>
            <p class="mt-4 font-bold leading-snug">{{ built.description }}</p>
            <dl class="grid grid-cols-3 gap-2 mt-3 text-center">
              <div class="stat"><dt>Pot</dt><dd>{{ fmt(built.pot) }}</dd></div>
              <div class="stat"><dt>Behind</dt><dd>{{ fmt(built.stack) }}</dd></div>
              <div class="stat" title="Stack-to-pot ratio: how many pots are left to bet. Low = easy to get all-in.">
                <dt>SPR</dt><dd>{{ (built.stack / built.pot).toFixed(1) }}</dd>
              </div>
            </dl>
            <div class="grid grid-cols-2 gap-4 mt-4">
              <RangePreview :title="built.oopName" :range="oopRangeEdit" />
              <RangePreview :title="built.ipName" :range="ipRangeEdit" />
            </div>
            <div v-if="exploitOn && leakList.length" class="mt-4 text-sm rounded-lg bg-mine-light px-3 py-2">
              <b>Exploiting:</b> {{ leakSummary }} ({{ villainName.toLowerCase() }})
            </div>
            <button class="btn-solve mt-5" :disabled="board.length !== 3 || !!setupError" @click="startSolve">
              Solve this spot
            </button>
            <p v-if="board.length !== 3" class="text-sm text-[#B42318] mt-2">Pick three flop cards.</p>
            <p v-else-if="setupError" class="text-sm text-[#B42318] mt-2">{{ setupError }}</p>
            <p v-else-if="memNeeded" class="hint mt-2">
              Uses about {{ memNeeded >= 1024 ? (memNeeded / 1024).toFixed(1) + ' GB' : memNeeded.toFixed(0) + ' MB' }} of memory<template v-if="freeMb">, {{ (freeMb / 1024).toFixed(1) }} GB free</template>.
              <template v-if="lowMemory"> Memory saver is on, so switching views later takes a few seconds.</template>
            </p>
            <label v-if="leakList.length" class="hint flex items-center gap-1.5 mt-1">
              <input v-model="forceSaver" type="checkbox" class="rounded text-felt" /> Always use memory saver
            </label>
          </div>
        </aside>
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
            <p class="text-sm text-muted mt-1">
              Accurate to within {{ summary?.base_expl_pct.toFixed(2) }}% of the pot. Solved in {{ secs(summary?.base_secs ?? 0) }}<template v-if="summary?.locked">, exploit solve {{ secs(summary.locked_secs) }}</template>.
            </p>
          </div>
          <div class="flex gap-1.5 shrink-0" :aria-label="'Board ' + boardText(node.board)">
            <PlayingCard v-for="(c, i) in displayBoard" :key="i" :card="c" size="md" />
          </div>
        </div>

        <!-- first-run guide -->
        <div v-if="!introDismissed" class="mt-4 bg-white border border-rule rounded-xl p-4 flex gap-4">
          <div class="text-sm leading-relaxed max-w-[90ch]">
            <b>How to read this.</b> Each decision shows what the player does and how often. Click an action to follow
            that line; click a turn or river card when one is dealt. The chart colours every starting hand by what it
            does here — split colours mean the solver mixes. Type your own hand under <b>Your hand</b> to see exactly what
            to do with it at each step.
          </div>
          <button class="btn-quiet self-start shrink-0" @click="dismissIntro">Got it</button>
        </div>

        <!-- exploit bar -->
        <div v-if="summary?.locked" class="mt-4 rounded-xl border border-[#E8C760] bg-mine-light p-4">
          <div class="flex flex-wrap items-center gap-3">
            <div class="seg" role="group" aria-label="Which strategy to show">
              <button :class="view === 'base' ? 'seg-on' : ''" @click="setView('base')">Normal strategy</button>
              <button :class="view === 'locked' ? 'seg-on' : ''" @click="setView('locked')">Vs {{ leakSummary.toLowerCase() }}</button>
            </div>
            <label class="text-sm flex items-center gap-1.5">
              <input v-model="compareOn" type="checkbox" class="rounded text-felt" @change="load" />
              Show what changes at each step
            </label>
            <span v-if="busy" class="text-sm text-felt font-bold" role="status">Loading…</span>
          </div>
          <p class="text-sm mt-2.5">
            You win on average <b>{{ fmt(summary.base_ev[heroIdx]) }}</b> of this {{ fmt(built.pot) }} pot normally, and
            <b>{{ fmt(summary.locked_ev[heroIdx]) }}</b> against this player —
            <b :class="evGain >= 0 ? 'text-felt' : 'text-[#B42318]'">{{ evGain >= 0 ? '+' : '' }}{{ fmt(evGain) }}</b> a hand,
            if the habit is as strong as you set it. Real players are less consistent, so trust the direction of each
            change more than its size.
          </p>
        </div>

        <!-- line -->
        <nav class="mt-4 flex flex-wrap items-center gap-1 text-sm" aria-label="Hand so far">
          <button class="crumb" @click="goTo(0)">Flop</button>
          <template v-for="(c, i) in crumbs" :key="i">
            <span class="text-muted" aria-hidden="true">›</span>
            <button class="crumb" @click="goTo(i + 1)">{{ c }}</button>
          </template>
        </nav>

        <div class="mt-4 grid grid-cols-12 gap-6">
          <!-- decision -->
          <div class="col-span-7">
            <div class="bg-white rounded-xl border border-rule p-5">
              <template v-if="node.kind === 'player'">
                <div class="flex items-baseline justify-between gap-3">
                  <h2 class="font-display font-extrabold text-xl">
                    {{ actorName }}
                    <span v-if="node.locked && view === 'locked'" class="ml-1 align-middle text-xs font-body font-bold bg-mine text-ink px-2 py-0.5 rounded">habit applied</span>
                  </h2>
                  <span class="text-sm text-muted">Pot {{ fmt(node.pot) }}</span>
                </div>
                <p class="text-sm mt-1" :class="lineProb < 0.03 ? 'text-[#B42318]' : 'text-muted'">
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
                      {{ view === 'locked' ? 'normally' : 'vs habit' }} {{ pct(compareNode.action_freqs[i] ?? 0) }}
                    </span>
                  </button>
                </div>

                <div class="mt-5 flex items-center justify-between">
                  <h3 class="text-sm font-bold">{{ actorName }}'s hands</h3>
                  <ul class="flex flex-wrap gap-x-3 gap-y-1 text-xs text-muted" aria-label="Colour key">
                    <li v-for="(a, i) in node.actions" :key="i" class="flex items-center gap-1">
                      <span class="w-2.5 h-2.5 rounded-sm" :style="{ background: colors[i] }"></span>{{ shortAction(i) }}
                    </li>
                    <li class="flex items-center gap-1"><span class="w-2.5 h-2.5 rounded-sm bg-paper border border-rule"></span>not in range</li>
                  </ul>
                </div>
                <div class="mt-2">
                  <HandGrid :grid="node.grid" :colors="colors" :selected="selectedCell" :mine="myCell" @select="selectCell" />
                  <p class="hint mt-1.5">The fill height shows how much of each hand is still in the range here. A sliver means that hand rarely reaches this point.</p>
                </div>
              </template>

              <template v-else-if="node.kind === 'chance'">
                <h2 class="font-display font-extrabold text-xl">Deal the {{ node.board.length === 3 ? 'turn' : 'river' }}</h2>
                <p class="text-sm text-muted mt-1">Pick the card that came. Pot {{ fmt(node.pot) }}.</p>
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
              </template>

              <template v-else>
                <h2 class="font-display font-extrabold text-xl">Hand over</h2>
                <p class="text-muted mt-1">{{ terminalText }}</p>
                <button class="btn-quiet mt-3" @click="goTo(crumbs.length - 1)">Back one step</button>
              </template>
            </div>
          </div>

          <!-- side panels -->
          <div class="col-span-5 space-y-4">
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
              <p v-if="myHandError" class="text-sm text-[#B42318] mt-2">{{ myHandError }}</p>
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
                      <span v-if="myCompare && myCompare.acting" class="w-20 text-right text-xs text-muted tabular-nums">{{ view === 'locked' ? 'normally' : 'vs habit' }} {{ pct(myCompare.freqs[a] ?? 0) }}</span>
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
              </template>
              <p v-else class="text-sm text-muted">Not in {{ actorName.toLowerCase() }}'s range at this point.</p>
            </section>

            <section v-if="node.kind === 'player'" class="panel">
              <h3 class="font-bold">Coach notes</h3>
              <ul class="mt-2 space-y-1.5 text-sm list-disc pl-5 marker:text-felt">
                <li v-for="(n, i) in notes" :key="i">{{ n }}</li>
              </ul>
              <template v-if="view === 'locked' && compareNotes.length">
                <h3 class="font-bold mt-4">What changes against this player</h3>
                <ul class="mt-2 space-y-1.5 text-sm list-disc pl-5 marker:text-mine">
                  <li v-for="(n, i) in compareNotes" :key="i">{{ n }}</li>
                </ul>
              </template>
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
      </div>

      <div v-if="error" class="mt-6 bg-[#FEF3F2] border border-[#FDA29B] text-[#912018] rounded-lg p-3" role="alert">{{ error }}</div>
    </div>

    <!-- notebook drawer -->
    <div v-if="notebookOpen" class="fixed inset-0 z-40 flex justify-end bg-ink/30" @click.self="notebookOpen = false">
      <aside class="w-[440px] h-full bg-white shadow-xl p-6 overflow-y-auto" aria-label="Notebook">
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
            <button class="text-xs text-[#B42318] underline mt-2" @click="removeNote(i)">Delete</button>
          </li>
        </ul>
      </aside>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import HandGrid from "./HandGrid.vue";
import PlayingCard from "./PlayingCard.vue";
import RangePreview from "./RangePreview.vue";
import {
  SCENARIOS, OPPONENTS, SIZINGS, ACCURACY, LEAKS, STRENGTHS, PROFILES, TEXTURES, SEAT_LABEL,
  RANK_CHARS, SUIT_CHARS, SUIT_SYMBOL, SUIT_COLOR, randomFlop,
} from "../../guided/data";
import * as api from "../../guided/api";
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
const pickScenario = (id: string) => {
  scenarioId.value = id;
  const s = scenario.value;
  if (!s.seats.includes(seat.value)) seat.value = s.seats[s.seats.length - 1];
  if (s.format === "hu") { stackIn.value = 25; sizingId.value = "hu"; }
  else { if (stackIn.value < 40) stackIn.value = 200; if (sizingId.value === "hu") sizingId.value = "clp"; }
};
const seatLabel = (s: string) => SEAT_LABEL[s] ?? (s === "BB" ? "Big blind" : s);
const fmt = (chips: number) =>
  scenario.value.format === "hu" ? `${(chips / 10).toFixed(1)}bb` : `$${Number.isInteger(chips) ? chips : chips.toFixed(1)}`;
const secs = (s: number) => (s < 90 ? `${Math.round(s)}s` : `${Math.round(s / 60)} min`);

const built = computed(() => scenario.value.build(seat.value, opponent.value, openSize.value, stackIn.value));
const oopRangeEdit = ref(""), ipRangeEdit = ref("");
const resetRanges = () => { oopRangeEdit.value = built.value.oopRange; ipRangeEdit.value = built.value.ipRange; };
watch(() => [built.value.oopRange, built.value.ipRange], resetRanges, { immediate: true });
const oppHelp = computed(() => OPPONENTS.find((o) => o.id === opponent.value)?.help ?? "");
const heroIdx = computed(() => (built.value.heroIsOop ? 0 : 1));
const villIdx = computed(() => 1 - heroIdx.value);
const villainName = computed(() => (villIdx.value === 0 ? built.value.oopName : built.value.ipName));
const sizingList = computed(() => SIZINGS[scenario.value.format]);
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
const freeMb = ref(0);
api.systemMemory().then(([avail]) => (freeMb.value = avail / 1_048_576)).catch(() => {});
// memory saver: park the normal solve on disk while the exploit solve runs, and keep only one solve
// in memory while browsing. Turned on automatically when a normal solve won't fit comfortably.
const lowMemory = computed(() => leakList.value.length > 0 && (forceSaver.value || (freeMb.value > 0 && memMb.value > freeMb.value * 0.6)));
const memNeeded = computed(() => (lowMemory.value ? memLow.value : memMb.value));
const setupError = computed(() => {
  if (exploitOn.value && leakList.value.length === 0) return "Tick at least one habit, or choose “No habit”.";
  if (freeMb.value && memNeeded.value > freeMb.value * 0.85)
    return `This needs about ${(memNeeded.value / 1024).toFixed(1)} GB of memory but only ${(freeMb.value / 1024).toFixed(1)} GB is free. Choose “One size per street”, a smaller stack or fewer habits, or close other programs.`;
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
    node.value = await api.studyNode(view.value, history.value);
    compareNode.value = summary.value?.locked && compareOn.value
      ? await api.studyNode(view.value === "locked" ? "base" : "locked", history.value)
      : null;
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
  await load();
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
const displayBoard = computed(() => sortFlop(node.value?.board ?? []));
const boardText = (b: string[]) => sortFlop(b).map((c) => c[0] + SUIT_SYMBOL[c[1]]).join(" ");
const R = "AKQJT98765432";
const cellName = (i: number) => {
  const r = Math.floor(i / 13), c = i % 13;
  return r === c ? R[r] + R[c] : r < c ? R[r] + R[c] + " suited" : R[c] + R[r] + " offsuit";
};
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
  if (hv.kind === "player" && hv.player !== heroIdx.value && hv.actions[0] === "Check") {
    path = [...hist, 0];
    hv = await api.studyNode(view.value, path);
  }
  if (hv.kind !== "player") return "";
  const words = (v: api.NodeView) => {
    const ag = (fr: number[]) => fr.map((f, i) => (/^(Bet|Raise|All-in)/.test(v.actions[i]) ? f : 0)).reduce((x, y) => x + y, 0);
    return `bet ${pct(ag(v.action_freqs))} (strong hands ${pct(ag(v.buckets[0].freqs))}, weak hands ${pct(ag(v.buckets[2].freqs))})`;
  };
  const where = path.length > hist.length ? " when checked to" : "";
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
.choice { @apply w-full text-left bg-white border border-rule rounded-lg px-3.5 py-3 hover:border-felt transition-colors; }
.choice-on { @apply border-felt ring-1 ring-felt bg-felt-light; }
.pill { @apply px-3 py-1.5 rounded-full border border-rule bg-white text-sm hover:border-felt transition-colors; }
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
.btn-primary { @apply px-3.5 py-1.5 rounded-md bg-felt text-white text-sm font-bold hover:bg-felt-dark; }
.btn-quiet { @apply px-3 py-1.5 rounded-md border border-rule bg-white text-sm font-bold hover:border-felt disabled:opacity-40; }
.crumb { @apply px-2.5 py-1 rounded-md bg-white border border-rule hover:border-felt; }
.panel { @apply bg-white rounded-xl border border-rule p-4; }
.action-row { @apply w-full flex items-center gap-3 rounded-lg border border-rule px-3 py-2 hover:border-felt hover:bg-felt-light/40 transition-colors; }
.seg { @apply inline-flex rounded-lg border border-ink overflow-hidden; }
.seg button { @apply px-3 py-1.5 text-sm font-bold bg-white; }
.seg .seg-on { @apply bg-ink text-white; }
button:focus-visible, input:focus-visible, select:focus-visible, summary:focus-visible { @apply outline-none ring-2 ring-felt ring-offset-1; }
</style>
