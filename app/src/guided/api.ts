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
export interface StudySpec {
  oop_range: string; ip_range: string; board: string; pot: number; stack: number;
  oop_flop_bet: string; ip_flop_bet: string; turn_bet: string; river_bet: string; raise: string;
  allin_threshold: number; accuracy_pct: number; max_iter: number; leaks: Leak[]; threads: number; low_memory: boolean; freeze_rest: boolean; confidence: number;
}
export interface StudySummary {
  base_ev: [number, number]; base_expl_pct: number; base_secs: number;
  locked: boolean; locked_ev: [number, number]; locked_expl_pct: number; locked_secs: number;
  locked_nodes: number; memory_mb: number; swap_mode: boolean; error: string | null;
}
export interface Cell { full?: number; reach?: number; weight: number; freqs: number[]; equity: number; ev: number }
export interface Bucket { name: string; share: number; freqs: number[] }
export interface NodeView {
  kind: "player" | "chance" | "terminal"; player: number; board: string[]; pot: number;
  bets: [number, number]; stack: number; actions: string[]; action_amounts: number[];
  action_freqs: number[]; grid: Cell[]; buckets: Bucket[]; ev: [number, number];
  equity: [number, number]; cards: [string, number][]; locked: boolean; range_combos: [number, number];
}

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
export const studyRangeGrid = (range: string) => call<number[]>("study_range_grid", { range });
