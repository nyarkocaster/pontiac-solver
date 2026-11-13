import { defineStore } from "pinia";
import { sanitizeBetString } from "./utils";

export type NavView = "guided" | "threeway" | "solver" | "results";

export type SideView =
  | "oop-range"
  | "ip-range"
  | "board"
  | "tree-config"
  | "bunching"
  | "run-solver"
  | "about";

export const saveConfigTmp = () => {
  const config = useConfigStore();
  const tmpConfig = useTmpConfigStore();

  tmpConfig.$patch({
    board: [...config.board],
    startingPot: config.startingPot,
    effectiveStack: config.effectiveStack,
    rakePercent: config.rakePercent,
    rakeCap: config.rakeCap,
    donkOption: config.donkOption,
    oopFlopBet: config.oopFlopBet,
    oopFlopRaise: config.oopFlopRaise,
    oopTurnBet: config.oopTurnBet,
    oopTurnRaise: config.oopTurnRaise,
    oopTurnDonk: config.oopTurnDonk,
    oopRiverBet: config.oopRiverBet,
    oopRiverRaise: config.oopRiverRaise,
    oopRiverDonk: config.oopRiverDonk,
    ipFlopBet: config.ipFlopBet,
    ipFlopRaise: config.ipFlopRaise,
    ipTurnBet: config.ipTurnBet,
    ipTurnRaise: config.ipTurnRaise,
    ipRiverBet: config.ipRiverBet,
    ipRiverRaise: config.ipRiverRaise,
    addAllInThreshold: config.addAllInThreshold,
    forceAllInThreshold: config.forceAllInThreshold,
    mergingThreshold: config.mergingThreshold,
    expectedBoardLength: config.expectedBoardLength,
    addedLines: config.addedLines,
    removedLines: config.removedLines,
  });
};

export const saveConfig = () => {
  const tmpConfig = useTmpConfigStore();
  const savedConfig = useSavedConfigStore();

  savedConfig.$patch({
    board: tmpConfig.board,
    startingPot: tmpConfig.startingPot,
    effectiveStack: tmpConfig.effectiveStack,
    rakePercent: tmpConfig.rakePercent,
    rakeCap: tmpConfig.rakeCap,
    donkOption: tmpConfig.donkOption,
    oopFlopBet: tmpConfig.oopFlopBet,
    oopFlopRaise: tmpConfig.oopFlopRaise,
    oopTurnBet: tmpConfig.oopTurnBet,
    oopTurnRaise: tmpConfig.oopTurnRaise,
    oopTurnDonk: tmpConfig.oopTurnDonk,
    oopRiverBet: tmpConfig.oopRiverBet,
    oopRiverRaise: tmpConfig.oopRiverRaise,
    oopRiverDonk: tmpConfig.oopRiverDonk,
    ipFlopBet: tmpConfig.ipFlopBet,
    ipFlopRaise: tmpConfig.ipFlopRaise,
    ipTurnBet: tmpConfig.ipTurnBet,
    ipTurnRaise: tmpConfig.ipTurnRaise,
    ipRiverBet: tmpConfig.ipRiverBet,
    ipRiverRaise: tmpConfig.ipRiverRaise,
    addAllInThreshold: tmpConfig.addAllInThreshold,
    forceAllInThreshold: tmpConfig.forceAllInThreshold,
    mergingThreshold: tmpConfig.mergingThreshold,
    expectedBoardLength: tmpConfig.expectedBoardLength,
    addedLines: tmpConfig.addedLines,
    removedLines: tmpConfig.removedLines,
  });
};

/* Node Locking (Phase 1/2/3) */

export type NodeLock = {
  /** Action indices from the root, e.g. [] (root) or [0, 2]. */
  history: number[];
  /** Flattened strategy in engine layout: [action * numHands + hand]. */
  strategy: number[];
  numActions: number;
  numHands: number;
  /** Human-readable node path, for display only. */
  label: string;
  /** Phase 3: soft lock / RRN — treat as a target instead of a hard lock. */
  isSoft: boolean;
  /** Phase 3: locking weight, 1.0 = hard lock, (0,1) = relaxed target. */
  weight: number;
  /** Phase 2 frequency targets (per action); null = derive from averaged strategy. */
  targetFreqs: number[] | null;
};

export const lockKey = (history: number[]) => history.join(".");

/** Metrics of one finished solve, used for the baseline / locked comparison. */
/** One point of a sensitivity sweep (one full re-solve per point). */
export type SweepPoint = {
  /** Swept parameter value (lock weight, or node frequency in percent). */
  label: number;
  exploitability: number;
  unrestricted: number;
  ev: [number, number];
  elapsedMs: number;
};

/** One step of a sweep: the complete lock set that is applied for that step. */
export type SweepStep = {
  label: number;
  locks: NodeLock[];
};

export type RunMetrics = {
  /** Exploitability within the locks (stopping criterion). */
  exploitability: number;
  /** Real exploitability of the delivered profile (locks ignored). */
  unrestricted: number;
  /** Reach-weighted root EV per player. */
  ev: [number, number];
  hasLocks: boolean;
  elapsedMs: number;
};

export const useLockStore = defineStore("locks", {
  state: () => ({
    locks: [] as NodeLock[],
    softLockEnabled: false,
    /** Weight applied to new soft locks (Phase 3). */
    softLockWeight: 0.5,
    /** History of the node currently displayed in the results view (set by ResultNav). */
    currentHistory: [] as number[],
    /** Metrics of the last solve without locks (Phase 2 comparison). */
    baseline: null as RunMetrics | null,
    /** Metrics of the last solve with locks (Phase 2 comparison). */
    lastLocked: null as RunMetrics | null,
    /** Sensitivity sweep (Phase 3): the steps to run, if any. */
    sweepRequest: null as SweepStep[] | null,
    sweepRunning: false,
    sweepCancelled: false,
    /** Fraction of the sweep that is done (0..1). */
    sweepProgress: 0,
    sweepResults: [] as SweepPoint[],
  }),

  getters: {
    hasLocks: (state) => state.locks.length > 0,
  },

  actions: {
    setCurrentHistory(history: number[]) {
      this.currentHistory = [...history];
    },

    upsert(lock: NodeLock) {
      const key = lockKey(lock.history);
      const index = this.locks.findIndex((l) => lockKey(l.history) === key);
      if (index >= 0) {
        this.locks[index] = lock;
      } else {
        this.locks.push(lock);
      }
    },

    remove(history: number[]) {
      const key = lockKey(history);
      this.locks = this.locks.filter((l) => lockKey(l.history) !== key);
    },

    clear() {
      this.locks = [];
    },

    startSweep(steps: SweepStep[]) {
      this.sweepRequest = steps;
      this.sweepResults = [];
      this.sweepProgress = 0;
      this.sweepCancelled = false;
      this.sweepRunning = true;
    },

    finishSweep() {
      this.sweepRequest = null;
      this.sweepRunning = false;
      this.sweepProgress = 1;
    },

    /** Records the metrics of a finished solve for the comparison panel. */
    recordRun(metrics: RunMetrics) {
      if (metrics.hasLocks) {
        this.lastLocked = metrics;
      } else {
        this.baseline = metrics;
      }
    },
  },
});

export const useStore = defineStore("app", {
  state: () => ({
    navView: "guided" as NavView,
    /** Set by the node-lock editor to ask RunSolver to re-init + re-solve with locks. */
    nodelockResolveRequested: false,
    sideView: "oop-range" as SideView,
    headers: {
      about: ["About"],
      "oop-range": ["OOP Range"],
      "ip-range": ["IP Range"],
      board: ["Board"],
      "tree-config": ["Tree Configuration"],
      bunching: ["Bunching Effect"],
      "run-solver": ["Run Solver"],
    },
    ranges: Array.from({ length: 6 }, () =>
      Array.from({ length: 13 * 13 }, () => 0)
    ),
    isBunchingEnabled: false,
    isBunchingRunning: false,
    bunchingFlop: [] as number[],
    isSolverRunning: false,
    isSolverPaused: false,
    isSolverFinished: false,
    isSolverError: false,
    isFinalizing: false,
  }),

  getters: {
    hasSolverRun: (state) => {
      return (
        state.isSolverRunning ||
        state.isSolverPaused ||
        state.isSolverFinished ||
        state.isSolverError ||
        state.isFinalizing
      );
    },
  },
});

export const useConfigStore = defineStore("config", {
  state: () => ({
    board: [] as number[],
    startingPot: 20,
    effectiveStack: 100,
    rakePercent: 0,
    rakeCap: 0,
    donkOption: false,
    oopFlopBet: "",
    oopFlopRaise: "",
    oopTurnBet: "",
    oopTurnRaise: "",
    oopTurnDonk: "",
    oopRiverBet: "",
    oopRiverRaise: "",
    oopRiverDonk: "",
    ipFlopBet: "",
    ipFlopRaise: "",
    ipTurnBet: "",
    ipTurnRaise: "",
    ipRiverBet: "",
    ipRiverRaise: "",
    addAllInThreshold: 150,
    forceAllInThreshold: 20,
    mergingThreshold: 10,
    expectedBoardLength: 0,
    addedLines: "",
    removedLines: "",
  }),

  getters: {
    oopFlopBetSanitized: (s) => sanitizeBetString(s.oopFlopBet, false),
    oopFlopRaiseSanitized: (s) => sanitizeBetString(s.oopFlopRaise, true),
    oopTurnBetSanitized: (s) => sanitizeBetString(s.oopTurnBet, false),
    oopTurnRaiseSanitized: (s) => sanitizeBetString(s.oopTurnRaise, true),
    oopTurnDonkSanitized: (s) => sanitizeBetString(s.oopTurnDonk, false),
    oopRiverBetSanitized: (s) => sanitizeBetString(s.oopRiverBet, false),
    oopRiverRaiseSanitized: (s) => sanitizeBetString(s.oopRiverRaise, true),
    oopRiverDonkSanitized: (s) => sanitizeBetString(s.oopRiverDonk, false),
    ipFlopBetSanitized: (s) => sanitizeBetString(s.ipFlopBet, false),
    ipFlopRaiseSanitized: (s) => sanitizeBetString(s.ipFlopRaise, true),
    ipTurnBetSanitized: (s) => sanitizeBetString(s.ipTurnBet, false),
    ipTurnRaiseSanitized: (s) => sanitizeBetString(s.ipTurnRaise, true),
    ipRiverBetSanitized: (s) => sanitizeBetString(s.ipRiverBet, false),
    ipRiverRaiseSanitized: (s) => sanitizeBetString(s.ipRiverRaise, true),
  },
});

export const useTmpConfigStore = defineStore("tmpConfig", {
  state: () => ({
    board: [] as number[],
    startingPot: 0,
    effectiveStack: 0,
    rakePercent: 0,
    rakeCap: 0,
    donkOption: false,
    oopFlopBet: "",
    oopFlopRaise: "",
    oopTurnBet: "",
    oopTurnRaise: "",
    oopTurnDonk: "",
    oopRiverBet: "",
    oopRiverRaise: "",
    oopRiverDonk: "",
    ipFlopBet: "",
    ipFlopRaise: "",
    ipTurnBet: "",
    ipTurnRaise: "",
    ipRiverBet: "",
    ipRiverRaise: "",
    addAllInThreshold: 0,
    forceAllInThreshold: 0,
    mergingThreshold: 0,
    expectedBoardLength: 0,
    addedLines: "",
    removedLines: "",
  }),
});

export const useSavedConfigStore = defineStore("savedConfig", {
  state: () => ({
    board: [] as number[],
    startingPot: 0,
    effectiveStack: 0,
    rakePercent: 0,
    rakeCap: 0,
    donkOption: false,
    oopFlopBet: "",
    oopFlopRaise: "",
    oopTurnBet: "",
    oopTurnRaise: "",
    oopTurnDonk: "",
    oopRiverBet: "",
    oopRiverRaise: "",
    oopRiverDonk: "",
    ipFlopBet: "",
    ipFlopRaise: "",
    ipTurnBet: "",
    ipTurnRaise: "",
    ipRiverBet: "",
    ipRiverRaise: "",
    addAllInThreshold: 0,
    forceAllInThreshold: 0,
    mergingThreshold: 0,
    expectedBoardLength: 0,
    addedLines: "",
    removedLines: "",
  }),
});
