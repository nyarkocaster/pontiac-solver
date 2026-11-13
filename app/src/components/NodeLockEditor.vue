<template>
  <div class="rounded border border-slate-300 bg-white text-xs">
    <!-- Header -->
    <div
      class="flex items-center gap-2 px-3 py-2 cursor-pointer select-none"
      @click="isOpen = !isOpen"
    >
      <span class="font-semibold">Node Lock</span>
      <span class="text-slate-600">
        {{
          headerLabel
        }}
      </span>
      <span v-if="lockStore.locks.length" class="text-emerald-700">
        ({{ lockStore.locks.length }} locked)
      </span>
      <span class="ml-auto">{{ isOpen ? "▲" : "▼" }}</span>
    </div>

    <div v-if="isOpen" class="px-3 pb-3 border-t border-slate-200">
      <!-- Not editable -->
      <div v-if="!editable" class="py-2 text-slate-600">
        {{ notEditableReason }}
      </div>

      <template v-else>
        <!-- Quick actions -->
        <div class="flex flex-wrap items-center gap-2 py-2">
          <button
            class="px-2 py-1 rounded bg-slate-200 hover:bg-slate-300"
            @click="lockAllToCurrent"
          >
            Lock all to current strategy
          </button>
          <button
            class="px-2 py-1 rounded bg-slate-200 hover:bg-slate-300"
            @click="lockAllPure"
          >
            Lock all pure ({{ pureActionName }})
          </button>
          <button
            class="px-2 py-1 rounded bg-slate-200 hover:bg-slate-300"
            @click="zeroAction"
          >
            Zero action ({{ pureActionName }})
          </button>
          <button
            class="px-2 py-1 rounded bg-rose-100 hover:bg-rose-200"
            @click="unlockNode"
          >
            Unlock node
          </button>
          <label class="flex items-center gap-1">
            <input type="checkbox" v-model="showAllHands" />
            show all hands
          </label>
          <label class="flex items-center gap-1" title="Phase 3 (RRN): treat locked frequencies as a soft target">
            <input type="checkbox" v-model="lockStore.softLockEnabled" />
            soft lock
          </label>
          <label
            v-if="lockStore.softLockEnabled"
            class="flex items-center gap-1"
            title="Weight of the soft lock: the node is pinned to weight x target + (1 - weight) x current strategy (restricted Nash refinement)"
          >
            weight
            <input
              type="range"
              min="0.05"
              max="1"
              step="0.05"
              v-model.number="lockStore.softLockWeight"
            />
            <span class="font-mono">{{ lockStore.softLockWeight.toFixed(2) }}</span>
          </label>
        </div>

        <!-- Node-level frequency targets (Phase 3): the shares are distributed
             pro-rata over the hands with iterative proportional fitting. -->
        <div class="flex flex-wrap items-center gap-2 pb-2">
          <span class="text-slate-600">Node freq target (%):</span>
          <label v-for="(action, i) of actions" :key="'freq' + i" class="flex items-center gap-1">
            {{ action }}
            <input
              type="number"
              min="0"
              max="100"
              step="1"
              class="w-14 px-1 py-0.5 text-right border border-slate-300 rounded bg-transparent"
              v-model.number="nodeFreq[i]"
            />
          </label>
          <button
            class="px-2 py-1 rounded bg-slate-200 hover:bg-slate-300"
            @click="applyNodeFreqs"
          >
            Distribute
          </button>
          <button
            class="px-2 py-1 rounded bg-slate-200 hover:bg-slate-300"
            @click="refreshNodeFreq"
          >
            Refresh
          </button>
        </div>

        <!-- Table -->
        <div class="max-h-72 overflow-auto border border-slate-200 rounded">
          <table class="w-full border-collapse">
            <thead class="sticky top-0 bg-slate-100">
              <tr>
                <th class="px-1 py-1 text-left">Hand</th>
                <th class="px-1 py-1 text-right">W</th>
                <th class="px-1 py-1">Lock</th>
                <th v-for="(action, i) of actions" :key="i" class="px-1 py-1 text-right">
                  {{ action }}
                </th>
                <th class="px-1 py-1 text-right">Sum</th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="row of visibleRows"
                :key="row.index"
                :class="row.locked ? 'bg-emerald-50' : ''"
              >
                <td class="px-1 py-0.5 font-mono">{{ row.label }}</td>
                <td class="px-1 py-0.5 text-right text-slate-600">
                  {{ row.weight.toFixed(2) }}
                </td>
                <td class="px-1 py-0.5 text-center">
                  <input type="checkbox" v-model="row.locked" @change="persist" />
                </td>
                <td v-for="(_, i) of actions" :key="i" class="px-1 py-0.5 text-right">
                  <input
                    v-if="row.locked"
                    type="number"
                    min="0"
                    max="100"
                    step="1"
                    class="w-14 px-1 py-0.5 text-right border border-slate-300 rounded bg-transparent"
                    v-model.number="row.freq[i]"
                    @change="persist"
                  />
                  <span v-else class="text-slate-500">
                    {{ (row.freq[i] * 100).toFixed(0) }}
                  </span>
                </td>
                <td class="px-1 py-0.5 text-right" :class="sumClass(row)">
                  {{ rowSum(row).toFixed(0) }}
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <!-- Footer -->
        <div class="flex flex-wrap items-center gap-2 pt-2">
          <button
            class="px-3 py-1 rounded bg-sky-700 text-white hover:bg-sky-800"
            :disabled="isApplying"
            @click="applyAndResolve"
          >
            {{ isApplying ? "Applying…" : "Apply locks & re-solve" }}
          </button>
          <button
            class="px-2 py-1 rounded bg-slate-200 hover:bg-slate-300"
            @click="clearAllLocks"
          >
            Clear all locks
          </button>
          <span class="text-slate-600">
            Locked: {{ lockedRowCount }} hand(s) · the engine pins every locked
            hand; a soft lock blends it with the weight above
          </span>
        </div>

        <!-- Phase 2: what the adjustment costs -->
        <div
          v-if="baseline || lastLocked"
          class="mt-2 rounded border border-slate-200 p-2 space-y-0.5"
        >
          <div class="font-semibold">Locked vs baseline</div>
          <div v-if="baseline" class="text-slate-600">
            Baseline (no locks): EV OOP {{ evText(baseline.ev[0]) }} / IP
            {{ evText(baseline.ev[1]) }} · exploitability
            {{ evText(baseline.exploitability) }} (real
            {{ evText(baseline.unrestricted) }})
          </div>
          <div v-else class="text-slate-600">
            No baseline yet — solve once without locks to measure the cost of the
            adjustment.
          </div>
          <div v-if="lastLocked">
            Locked: EV OOP {{ evText(lastLocked.ev[0]) }} / IP
            {{ evText(lastLocked.ev[1]) }} · exploitability
            {{ evText(lastLocked.exploitability) }} (real
            {{ evText(lastLocked.unrestricted) }})
          </div>
          <div v-if="baseline && lastLocked" class="font-mono">
            Cost of adjustment: ΔEV OOP
            {{ evText(lastLocked.ev[0] - baseline.ev[0]) }} / IP
            {{ evText(lastLocked.ev[1] - baseline.ev[1]) }}
          </div>
        </div>

        <!-- Phase 3: sensitivity sweep — one full re-solve per parameter value -->
        <div class="mt-2 rounded border border-slate-200 p-2 space-y-1">
          <div class="font-semibold">Sensitivity sweep</div>
          <div class="flex flex-wrap items-center gap-2">
            <label class="flex items-center gap-1">
              parameter
              <select
                v-model="sweepMode"
                class="px-1 py-0.5 border border-slate-300 rounded bg-transparent"
              >
                <option value="weight">soft-lock weight p</option>
                <option value="freq">node freq of one action</option>
              </select>
            </label>
            <label v-if="sweepMode === 'freq'" class="flex items-center gap-1">
              action
              <select
                v-model.number="sweepAction"
                class="px-1 py-0.5 border border-slate-300 rounded bg-transparent"
              >
                <option v-for="(action, i) of actions" :key="'sw' + i" :value="i">
                  {{ action }}
                </option>
              </select>
            </label>
            <label class="flex items-center gap-1">
              from
              <input
                type="number"
                step="0.05"
                class="w-16 px-1 py-0.5 text-right border border-slate-300 rounded bg-transparent"
                v-model.number="sweepFrom"
              />
            </label>
            <label class="flex items-center gap-1">
              to
              <input
                type="number"
                step="0.05"
                class="w-16 px-1 py-0.5 text-right border border-slate-300 rounded bg-transparent"
                v-model.number="sweepTo"
              />
            </label>
            <label class="flex items-center gap-1">
              points
              <input
                type="number"
                min="2"
                max="21"
                class="w-14 px-1 py-0.5 text-right border border-slate-300 rounded bg-transparent"
                v-model.number="sweepPoints"
              />
            </label>
            <button
              class="px-2 py-1 rounded bg-sky-700 text-white hover:bg-sky-800"
              :disabled="lockStore.sweepRunning || !lockedRowCount"
              @click="runSweep"
            >
              {{ lockStore.sweepRunning ? "Sweeping…" : "Run sweep" }}
            </button>
            <button
              v-if="lockStore.sweepRunning"
              class="px-2 py-1 rounded bg-rose-100 hover:bg-rose-200"
              @click="lockStore.sweepCancelled = true"
            >
              Cancel
            </button>
            <span v-if="lockStore.sweepRunning" class="text-slate-600">
              {{ Math.round(lockStore.sweepProgress * 100) }}%
            </span>
            <span v-if="sweepMessage" class="text-rose-700">{{ sweepMessage }}</span>
          </div>
          <div v-if="lockStore.sweepResults.length" class="max-h-56 overflow-auto">
            <table class="w-full border-collapse">
              <thead class="sticky top-0 bg-slate-100">
                <tr>
                  <th class="px-1 py-1 text-right">{{ sweepParameterName }}</th>
                  <th class="px-1 py-1 text-right">exploitability</th>
                  <th class="px-1 py-1 text-right">real</th>
                  <th class="px-1 py-1 text-right">EV OOP</th>
                  <th class="px-1 py-1 text-right">EV IP</th>
                  <th v-if="baseline" class="px-1 py-1 text-right">ΔEV OOP</th>
                  <th v-if="baseline" class="px-1 py-1 text-right">ΔEV IP</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="(point, i) of lockStore.sweepResults" :key="'sp' + i">
                  <td class="px-1 py-0.5 text-right font-mono">{{ point.label.toFixed(2) }}</td>
                  <td class="px-1 py-0.5 text-right font-mono">
                    {{ evText(point.exploitability) }}
                  </td>
                  <td class="px-1 py-0.5 text-right font-mono">
                    {{ evText(point.unrestricted) }}
                  </td>
                  <td class="px-1 py-0.5 text-right font-mono">{{ evText(point.ev[0]) }}</td>
                  <td class="px-1 py-0.5 text-right font-mono">{{ evText(point.ev[1]) }}</td>
                  <td v-if="baseline" class="px-1 py-0.5 text-right font-mono">
                    {{ evText(point.ev[0] - baseline.ev[0]) }}
                  </td>
                  <td v-if="baseline" class="px-1 py-0.5 text-right font-mono">
                    {{ evText(point.ev[1] - baseline.ev[1]) }}
                  </td>
                </tr>
              </tbody>
            </table>
            <div v-if="breakEvenText" class="pt-1 text-slate-600">{{ breakEvenText }}</div>
          </div>
        </div>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { Results, Spot, SpotPlayer } from "../result-types";
import { lockKey, useLockStore, useStore } from "../store";
import type { NodeLock, SweepStep } from "../store";
import { cardText } from "../utils";

const props = defineProps<{
  spot: Spot | null;
  results: Results | null;
  cards: number[][];
}>();

const lockStore = useLockStore();
const store = useStore();

const isOpen = ref(false);
const showAllHands = ref(false);
const isApplying = ref(false);

type Row = {
  index: number;
  label: string;
  weight: number;
  inRange: boolean;
  locked: boolean;
  freq: number[];
};

const draft = ref<{
  key: string;
  history: number[];
  player: "oop" | "ip";
  actions: string[];
  numHands: number;
  rows: Row[];
} | null>(null);

const spotPlayer = computed(() =>
  props.spot && props.spot.type === "player" ? (props.spot as SpotPlayer) : null
);

const playerIndex = computed(() => (spotPlayer.value?.player === "ip" ? 1 : 0));

const actions = computed(() => spotPlayer.value?.actions.map((a) => a.name) ?? []);

const editable = computed(() => {
  const s = spotPlayer.value;
  const r = props.results;
  if (!s || !r) return false;
  if (r.isEmpty) return false;
  if (r.currentPlayer !== s.player) return false;
  return s.actions.length > 0 && (props.cards[playerIndex.value]?.length ?? 0) > 0;
});

const notEditableReason = computed(() => {
  const s = props.spot;
  const r = props.results;
  if (!s) return "Select a node to lock.";
  if (s.type !== "player") return "Only player nodes (not chance / terminal) can be locked.";
  if (!r || r.isEmpty) return "No results at this node.";
  if (r.currentPlayer !== s.player)
    return `Results belong to '${r.currentPlayer}' — navigate via the results tab to this node to lock it.`;
  return "Nothing to lock.";
});

const handLabel = (pair: number) => {
  const card1 = pair & 0xff;
  const card2 = pair >>> 8;
  const text = (card: number) => {
    const t = cardText(card);
    return `${t.rank}${t.suitLetter}`;
  };
  if (card2 === 0xff) return text(card1);
  return `${text(card2)}${text(card1)}`;
};

const headerLabel = computed(() => {
  const h = lockStore.currentHistory;
  if (!props.spot) return "";
  const path = h.length ? h.join("›") : "root";
  const player = spotPlayer.value?.player ?? props.spot.type;
  return `[${path}] ${player}`;
});

const pureAction = computed(() => {
  const s = spotPlayer.value;
  if (!s) return null;
  return s.actions.find((a) => a.isSelected) ?? s.actions[s.actions.length - 1] ?? null;
});

const pureActionName = computed(() => {
  const name = pureAction.value ? pureAction.value.name : "";
  return name || "—";
});

const buildDraft = () => {
  const s = spotPlayer.value;
  const r = props.results;
  if (!editable.value || !s || !r) {
    draft.value = null;
    return;
  }

  const history = [...lockStore.currentHistory];
  const key = lockKey(history);
  const n = props.cards[playerIndex.value].length;
  const numActions = s.actions.length;

  const existing = lockStore.locks.find((l) => lockKey(l.history) === key);
  const weights = r.weights?.[playerIndex.value] ?? [];

  const rows: Row[] = [];
  for (let j = 0; j < n; ++j) {
    const weight = weights[j] ?? 0;
    const freq: number[] = [];
    let locked = false;
    for (let i = 0; i < numActions; ++i) {
      if (existing && existing.strategy.length === numActions * n) {
        const v = existing.strategy[i * n + j];
        freq.push(v);
        if (v > 0) locked = true;
      } else {
        freq.push(r.strategy[i * n + j] ?? 0);
      }
    }
    rows.push({
      index: j,
      label: handLabel(props.cards[playerIndex.value][j]),
      weight,
      inRange: weight > 0,
      locked: existing ? locked : false,
      freq,
    });
  }

  draft.value = {
    key,
    history,
    player: s.player,
    actions: s.actions.map((a) => a.name),
    numHands: n,
    rows,
  };
};

watch(
  () => [lockStore.currentHistory.join("."), (props.spot as any)?.type, (props.spot as any)?.index, props.results],
  buildDraft,
  { immediate: true }
);

const visibleRows = computed(() => {
  const d = draft.value;
  if (!d) return [];
  return showAllHands.value ? d.rows : d.rows.filter((row) => row.inRange);
});

const rowSum = (row: Row) => row.freq.reduce((a, b) => a + (b > 0 ? b : 0), 0);

const sumClass = (row: Row) => {
  const sum = rowSum(row);
  if (!row.locked) return "text-slate-300";
  return Math.abs(sum - 1) > 1e-6 ? "text-rose-700 font-semibold" : "text-emerald-700";
};

const lockedRowCount = computed(() => draft.value?.rows.filter((r) => r.locked && rowSum(r) > 0).length ?? 0);

/** Write the draft into the lock store (only hands with a positive sum are locked). */
const persist = () => {
  const d = draft.value;
  if (!d) return;

  const strategy = new Array<number>(d.actions.length * d.numHands).fill(0);
  let any = false;

  for (const row of d.rows) {
    if (!row.locked) continue;
    let sum = 0;
    for (const v of row.freq) sum += v > 0 ? v : 0;
    if (sum <= 0) continue;
    any = true;
    for (let i = 0; i < d.actions.length; ++i) {
      const v = row.freq[i] > 0 ? row.freq[i] : 0;
      strategy[i * d.numHands + row.index] = v / sum;
    }
  }

  if (!any) {
    lockStore.remove(d.history);
    return;
  }

  lockStore.upsert({
    history: d.history,
    strategy,
    numActions: d.actions.length,
    numHands: d.numHands,
    label: headerLabel.value,
    isSoft: lockStore.softLockEnabled,
    weight: lockStore.softLockEnabled ? lockStore.softLockWeight : 1,
    targetFreqs: null,
  });
};

const lockAllToCurrent = () => {
  const d = draft.value;
  const r = props.results;
  if (!d || !r) return;
  for (const row of d.rows) {
    if (!row.inRange) continue;
    row.locked = true;
    for (let i = 0; i < d.actions.length; ++i) {
      row.freq[i] = (r.strategy[i * d.numHands + row.index] ?? 0) * 100;
    }
  }
  persist();
};

const lockAllPure = () => {
  const d = draft.value;
  const action = pureAction.value;
  if (!d || !action) return;
  const pureIndex = d.actions.findIndex((name) => name === action.name);
  if (pureIndex < 0) return;
  for (const row of d.rows) {
    if (!row.inRange) continue;
    row.locked = true;
    for (let i = 0; i < d.actions.length; ++i) row.freq[i] = i === pureIndex ? 100 : 0;
  }
  persist();
};

const zeroAction = () => {
  const d = draft.value;
  const action = pureAction.value;
  if (!d || !action) return;
  const zi = d.actions.findIndex((name) => name === action.name);
  if (zi < 0) return;
  const factor = 100 / Math.max(1, d.actions.length - 1);
  for (const row of d.rows) {
    if (!row.inRange) continue;
    row.freq[zi] = 0;
    let sum = rowSum(row);
    if (sum <= 0) {
      // renormalize the remaining actions equally
      for (let i = 0; i < d.actions.length; ++i)
        row.freq[i] = i === zi ? 0 : factor;
    } else {
      for (let i = 0; i < d.actions.length; ++i)
        row.freq[i] = i === zi ? 0 : row.freq[i];
    }
    row.locked = true;
  }
  persist();
};

const unlockNode = () => {
  const d = draft.value;
  if (!d) return;
  for (const row of d.rows) row.locked = false;
  lockStore.remove(d.history);
};

const clearAllLocks = () => {
  lockStore.clear();
  const d = draft.value;
  if (d) for (const row of d.rows) row.locked = false;
};

const applyAndResolve = () => {
  persist();
  if (!lockStore.hasLocks) return;
  isApplying.value = true;
  // Locks must be applied after game_allocate_memory and before solving, so the
  // whole run has to be redone: RunSolver watches this flag.
  store.nodelockResolveRequested = true;
  store.navView = "solver";
};
// ---------- Phase 3: weight slider, node frequency targets, comparison ----------

/** Node-level frequency targets in percent, indexed like `actions`. */
const nodeFreq = ref<number[]>([]);

/** Weighted node-level frequency of the current draft, as a fraction. */
const currentNodeFreq = computed(() => {
  const d = draft.value;
  if (!d) return [] as number[];
  const sums = d.actions.map(() => 0);
  let total = 0;
  for (const row of d.rows) {
    if (row.weight <= 0) continue;
    total += row.weight;
    for (let i = 0; i < d.actions.length; ++i) {
      sums[i] += row.weight * Math.max(row.freq[i], 0);
    }
  }
  return sums.map((sum) => (total > 0 ? sum / total : 0));
});

watch(
  currentNodeFreq,
  (freqs) => {
    nodeFreq.value = freqs.map((f) => Number((f * 100).toFixed(1)));
  },
  { immediate: true }
);

const refreshNodeFreq = () => {
  nodeFreq.value = currentNodeFreq.value.map((f) => Number((f * 100).toFixed(1)));
};

/**
 * Distributes the node-level target frequencies pro-rata over the hands:
 * iterative proportional fitting alternates between "every hand sums to 100"
 * and "the weighted node-level shares match the targets".
 */
const applyNodeFreqs = () => {
  const d = draft.value;
  if (!d) return;
  const rows = d.rows.filter((row) => row.weight > 0);
  let totalWeight = 0;
  for (const row of rows) totalWeight += row.weight;
  const targets = d.actions.map((_, i) => Math.max(nodeFreq.value[i] ?? 0, 0) / 100);
  if (totalWeight <= 0 || targets.reduce((a, b) => a + b, 0) <= 0) return;

  for (const row of rows) {
    row.locked = true;
    for (let i = 0; i < d.actions.length; ++i) row.freq[i] = Math.max(row.freq[i], 0);
  }
  for (let step = 0; step < 40; ++step) {
    for (const row of rows) {
      const sum = row.freq.reduce((a, b) => a + b, 0);
      if (sum > 0) {
        for (let i = 0; i < d.actions.length; ++i) row.freq[i] /= sum;
      }
    }
    for (let i = 0; i < d.actions.length; ++i) {
      let aggregate = 0;
      for (const row of rows) aggregate += row.weight * row.freq[i];
      const wanted = targets[i] * totalWeight;
      if (aggregate > 0 && wanted > 0) {
        const scale = wanted / aggregate;
        for (const row of rows) row.freq[i] *= scale;
      }
    }
  }
  for (const row of rows) {
    const sum = row.freq.reduce((a, b) => a + b, 0);
    if (sum > 0) {
      for (let i = 0; i < d.actions.length; ++i) row.freq[i] /= sum;
    }
  }
  persist();
};

/** The locked run is compared against the last run without locks (Phase 2). */
const baseline = computed(() => lockStore.baseline);
const lastLocked = computed(() => lockStore.lastLocked);
const evText = (value: number) => (Number.isFinite(value) ? value.toFixed(2) : "n/a");

// ---------- Phase 3: sensitivity sweep ----------

const sweepMode = ref<"weight" | "freq">("weight");
const sweepAction = ref(0);
const sweepFrom = ref(0);
const sweepTo = ref(1);
const sweepPoints = ref(5);
const sweepMessage = ref("");

const sweepParameterName = computed(() =>
  sweepMode.value === "weight" ? "weight p" : `freq ${actions.value[sweepAction.value] ?? ""} (%)`
);

watch(sweepMode, (mode) => {
  if (mode === "weight") {
    sweepFrom.value = 0;
    sweepTo.value = 1;
  } else {
    sweepFrom.value = 0;
    sweepTo.value = 100;
  }
});

const cloneLock = (lock: NodeLock): NodeLock => ({
  ...lock,
  strategy: [...lock.strategy],
});

/** Per-hand frequency vectors (ratios, each row summing to 1) of a draft. */
const draftFreqRows = (d: NonNullable<typeof draft.value>, action: number | null, targetPct: number | null) => {
  const work = d.rows.map((row) => row.freq.map((v) => Math.max(v, 0)));
  const rows = d.rows;
  let totalWeight = 0;
  for (const row of rows) totalWeight += row.weight > 0 ? row.weight : 0;

  const normalize = () => {
    for (let j = 0; j < work.length; ++j) {
      const sum = work[j].reduce((a, b) => a + b, 0);
      if (sum > 0) for (let i = 0; i < d.actions.length; ++i) work[j][i] /= sum;
    }
  };

  normalize();
  if (action !== null && targetPct !== null && totalWeight > 0) {
    const target = Math.min(Math.max(targetPct, 0), 100) / 100;
    for (let step = 0; step < 40; ++step) {
      normalize();
      let aggregate = 0;
      for (let j = 0; j < work.length; ++j) {
        if (rows[j].weight > 0) aggregate += rows[j].weight * work[j][action];
      }
      const wanted = target * totalWeight;
      if (wanted <= 0) {
        for (let j = 0; j < work.length; ++j) work[j][action] = 0;
      } else if (aggregate > 0) {
        const scale = wanted / aggregate;
        for (let j = 0; j < work.length; ++j) work[j][action] *= scale;
      }
    }
    normalize();
  }
  return work;
};

/** Builds the lock of the current node from a set of per-hand frequency rows. */
const lockFromFreqRows = (d: NonNullable<typeof draft.value>, freqRows: number[][]): NodeLock => {
  const n = d.numHands;
  const numActions = d.actions.length;
  const strategy = new Array<number>(numActions * n).fill(0);
  for (let j = 0; j < n; ++j) {
    const sum = freqRows[j].reduce((a, b) => a + (b > 0 ? b : 0), 0);
    if (sum <= 0) continue;
    for (let i = 0; i < numActions; ++i) {
      strategy[i * n + j] = (freqRows[j][i] > 0 ? freqRows[j][i] : 0) / sum;
    }
  }
  return {
    history: [...d.history],
    strategy,
    numActions,
    numHands: n,
    label: headerLabel.value,
    isSoft: lockStore.softLockEnabled,
    weight: lockStore.softLockEnabled ? lockStore.softLockWeight : 1,
    targetFreqs: null,
  };
};

const runSweep = () => {
  const d = draft.value;
  sweepMessage.value = "";
  if (!d) {
    sweepMessage.value = "Select a node and lock at least one hand first.";
    return;
  }
  const count = Math.max(2, Math.min(21, Math.round(sweepPoints.value)));
  const labels: number[] = [];
  for (let i = 0; i < count; ++i) {
    labels.push(sweepFrom.value + ((sweepTo.value - sweepFrom.value) * i) / (count - 1));
  }

  // every lock of another node stays untouched during the sweep
  const others = lockStore.locks.filter((l) => lockKey(l.history) !== d.key).map(cloneLock);
  const steps: SweepStep[] = [];

  if (sweepMode.value === "weight") {
    const base = lockStore.locks.find((l) => lockKey(l.history) === d.key);
    if (!base) {
      sweepMessage.value = "Apply a lock on this node first (the sweep varies its weight).";
      return;
    }
    for (const label of labels) {
      const weight = Math.min(Math.max(label, 0), 1);
      steps.push({
        label: weight,
        locks: [...others, { ...cloneLock(base), isSoft: true, weight }],
      });
    }
  } else {
    const targetBase = lockStore.locks.find((l) => lockKey(l.history) === d.key);
    const baseRows = draftFreqRows(d, null, null);
    if (!targetBase) {
      const anyLocked = d.rows.some((row) => row.locked && row.weight > 0);
      if (!anyLocked) {
        sweepMessage.value = "Lock at least one hand (or Apply locks) before sweeping a frequency.";
        return;
      }
    }
    for (const label of labels) {
      const freqRows = draftFreqRows(d, sweepAction.value, label);
      const lock = targetBase
        ? { ...cloneLock(targetBase), strategy: lockFromFreqRows(d, freqRows).strategy }
        : lockFromFreqRows(d, freqRows);
      steps.push({ label, locks: [...others, lock] });
    }
    void baseRows;
  }

  lockStore.startSweep(steps);
};

/** Linear interpolation of the value where EV crosses the baseline EV. */
const breakEvenAt = (player: 0 | 1) => {
  const base = baseline.value;
  const points = lockStore.sweepResults;
  if (!base || points.length < 2) return Number.NaN;
  for (let i = 1; i < points.length; ++i) {
    const a = points[i - 1].ev[player] - base.ev[player];
    const b = points[i].ev[player] - base.ev[player];
    if (a === 0) return points[i - 1].label;
    if (a * b < 0) {
      const t = a / (a - b);
      return points[i - 1].label + (points[i].label - points[i - 1].label) * t;
    }
  }
  return Number.NaN;
};

const breakEvenText = computed(() => {
  if (!baseline.value || lockStore.sweepResults.length < 2) return "";
  const oop = breakEvenAt(0);
  const ip = breakEvenAt(1);
  if (!Number.isFinite(oop) && !Number.isFinite(ip)) {
    return "No EV break-even point inside the swept range (the adjustment costs or gains EV throughout).";
  }
  const parts: string[] = [];
  if (Number.isFinite(oop)) parts.push(`OOP at ${oop.toFixed(2)}`);
  if (Number.isFinite(ip)) parts.push(`IP at ${ip.toFixed(2)}`);
  return `EV break-even (vs the unlocked baseline): ${parts.join(", ")}.`;
});
</script>
