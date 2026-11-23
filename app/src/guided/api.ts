// Thin wrapper over the Study backend. Inside the desktop app it uses Tauri's
// invoke; in a plain browser (development/testing) it talks to a local dev server.
import { invoke } from "@tauri-apps/api";

const inTauri = () => typeof window !== "undefined" && "__TAURI_IPC__" in window;
const DEV_URL = "http://127.0.0.1:8765";

async function call<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  if (inTauri()) return (await invoke(cmd, args)) as T;
  const res = await fetch(`${DEV_URL}/${cmd}`, { method: "POST", body: JSON.stringify(args) });
  const text = await res.text();
  if (!res.ok) throw new Error(text);
  return JSON.parse(text) as T;
}

export interface Leak { player: number; street: string; effect: string; strength: number }
/** Prize-money setup, in the same chip units as the spot's pot and stack. */
export interface IcmSpec { payouts: number[]; oop_stack: number; ip_stack: number; others: number[] }
export interface StudySpec {
  oop_range: string; ip_range: string; board: string; pot: number; stack: number;
  oop_flop_bet: string; ip_flop_bet: string; turn_bet: string; river_bet: string; raise: string;
  allin_threshold: number; accuracy_pct: number; max_iter: number; leaks: Leak[]; threads: number; low_memory: boolean; freeze_rest: boolean; confidence: number;
}
export interface StudySummary {
  base_ev: [number, number]; base_expl_pct: number; base_secs: number;
  locked: boolean; locked_ev: [number, number]; locked_expl_pct: number; locked_secs: number;
  locked_nodes: number; memory_mb: number; swap_mode: boolean; error: string | null;
  /** set when the solve played for prize money: shares of the prize pool in percent, [out of position, in position] */
  icm?: { base_share: [number, number]; locked_share: [number, number]; split: [number, number]; give_up: [number, number] } | null;
}
export interface Cell { full?: number; reach?: number; weight: number; freqs: number[]; equity: number; ev: number }
export interface Bucket { name: string; share: number; freqs: number[] }
export interface NodeView {
  kind: "player" | "chance" | "terminal"; player: number; board: string[]; pot: number;
  bets: [number, number]; stack: number; actions: string[]; action_amounts: number[];
  action_freqs: number[]; grid: Cell[]; buckets: Bucket[]; ev: [number, number];
  equity: [number, number]; cards: [string, number][]; locked: boolean; range_combos: [number, number];
  combos: Combo[]; curves: [number, number][][];
}
export interface Combo { hand: string; cell: number; weight: number; freqs: number[]; equity: number; ev: number }

export const studySolve = (spec: StudySpec) => call<StudySummary>("study_solve", { spec });
export const studyProgress = () => call<[number, number, number, number]>("study_progress");
export const studyCancel = () => call<void>("study_cancel");
export const studyMemory = (spec: StudySpec) => call<[number, number]>("study_memory_estimate", { spec });
export const studyNode = (which: "base" | "locked", history: number[]) =>
  call<NodeView>("study_node", { which, history });
export const cpuThreads = () => call<number>("cpu_threads");
export const systemMemory = () => call<[number, number]>("memory");

export interface HandView { player: number; acting: boolean; in_range: boolean; blocked: boolean; freqs: number[]; equity: number; ev: number }
export const studyHand = (which: "base" | "locked", history: number[], player: number, hand: string) =>
  call<HandView>("study_hand", { which, history, player, hand });
/** One thing that happened after the flop in a hand being reviewed. Amounts are in the solve's chips. */
export type ReviewStep = { kind: "action"; who: number; verb: "fold" | "check" | "call" | "bet" | "raise"; to: number } | { kind: "card"; card: string };
export interface ReviewRow { history: number[]; street: string; board: string[]; pot: number; hero: boolean; actions: string[]; amounts: number[]; chosen: number; played_as: string | null; freqs: number[]; evs: number[]; best: number; loss: number }
export interface Review { rows: ReviewRow[]; total_loss: number; stopped: string | null }
export const studyReview = (which: "base" | "locked", hero: number, hand: string, steps: ReviewStep[]) => call<Review>("study_review", { which, hero, hand, steps });
export const studyRangeGrid = (range: string) => call<number[]>("study_range_grid", { range });

// hand-made locks and the saved-solve library
export interface ManualLock { history: number[]; cells: number[]; freqs: number[]; label?: string }
export interface LibraryEntry { id: string; name: string; saved_at: number; kind: "two" | "three"; bytes: number; summary: StudySummary | null; confidence: number; meta: Record<string, unknown> }
export const studyRelock = (spec: StudySpec, locks: ManualLock[]) => call<StudySummary>("study_relock", { spec, locks: locks.map((l) => ({ history: l.history, cells: l.cells, freqs: l.freqs })) });
export const libraryEntries = () => call<LibraryEntry[]>("library_entries");
export const libraryRemove = (id: string) => call<void>("library_remove", { id });
export const librarySaveTwo = (name: string, meta: Record<string, unknown>) => call<LibraryEntry>("library_save_two", { name, meta });
export const librarySaveThree = (name: string, meta: Record<string, unknown>) => call<LibraryEntry>("library_save_three", { name, meta });
export const libraryOpenTwo = (id: string) => call<LibraryEntry>("library_open_two", { id });
/** Puts a stored three-player solve on the three-player screen; answers with its summary (see api3 Summary3). */
export const libraryOpenThree = <T>(id: string) => call<T>("library_open_three", { id });
