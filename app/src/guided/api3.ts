// Three-way river solver API. Same transport as api.ts (Tauri in the app, dev server in a browser).
import { invoke } from "@tauri-apps/api";

const inTauri = () => typeof window !== "undefined" && "__TAURI_IPC__" in window;
const DEV_URL = "http://127.0.0.1:8765";

async function call<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  if (inTauri()) return (await invoke(cmd, args)) as T;
  const res = await fetch(`${DEV_URL}/${cmd}`, { method: "POST", body: JSON.stringify(args) });
  const text = await res.text();
  if (!res.ok) throw new Error(JSON.parse(text));
  return JSON.parse(text) as T;
}

export interface Leak3 { player: number; street: string; effect: string; strength: number }
export interface Spec3 {
  ranges: [string, string, string]; board: string; pot: number; stack: number;
  flop_bet_sizes: string; flop_max_raises: number; turn_bet_sizes: string; turn_max_raises: number; turn_cards: number; river_cards: number; bet_sizes: string; raise_sizes: string; max_raises: number; allin_spr: number; threads: number;
  accuracy_pct: number; max_iter: number; leaks: Leak3[];
  stacks?: [number, number, number] | null; rake_rate?: number; rake_cap?: number;
}
export interface Summary3 {
  base_ev: number[]; base_gain_pct: number[]; base_secs: number; base_iters: number;
  locked: boolean; locked_ev: number[]; locked_gain_pct: number[]; locked_secs: number; locked_iters: number;
  locked_nodes: number; combos: number[]; nodes: number; memory_mb: number; cancelled: boolean; error: string | null;
}
export interface Cell3 { full: number; weight: number; freqs: number[]; equity: number; ev: number }
export interface Bucket3 { name: string; share: number; freqs: number[] }
export interface Combo3 { hand: string; cell: number; weight: number; freqs: number[]; equity: number; ev: number }
export interface NodeView3 {
  kind: "player" | "chance" | "terminal"; board: string[]; street: string; cards: [string, number][]; result: string; winner: number; player: number; pot: number;
  invested: number[]; folded: boolean[]; stack: number; actions: string[]; action_amounts: number[];
  action_freqs: number[]; grid: Cell3[]; buckets: Bucket3[]; combos: Combo3[]; ev: number[]; equity: number[];
  range_combos: number[]; locked: boolean; has_ev: boolean; cards_possible: number; stacks: number[]; exact: boolean;
}

export const mwSolve = (spec: Spec3) => call<Summary3>("mw_solve", { spec });
export const mwProgress = () => call<[number, number, number, number]>("mw_progress");
export const mwCancel = () => call<void>("mw_cancel");
export interface Estimate3 { nodes: number; memory_mb: number; combos: number[]; secs_per_iter: number; turn: boolean; start: number }
export const mwEstimate = (spec: Spec3) => call<Estimate3>("mw_estimate", { spec });
export const mwNode = (which: "base" | "locked", history: number[], ev = true) => call<NodeView3>("mw_node", { which, history, ev });
/** After a flop solve: solve one turn card exactly (every river card) from the ranges that reach it. */
export const mwResolve = (which: "base" | "locked", history: number[], card: number) => call<Summary3>("mw_resolve", { which, history, card });

export interface PushFold {
  stack_bb: number; small_blind: number; ante: number; jam_pct: number; call_pct: number;
  jam: number[]; call: number[]; jam_edge_bb: number[]; call_edge_bb: number[];
  ev_small_blind_bb: number; ev_big_blind_bb: number; classes: string[];
}
export const pushfoldSolve = (stack: number, sb: number, ante: number) => call<PushFold>("pushfold_solve", { stack, sb, ante });
