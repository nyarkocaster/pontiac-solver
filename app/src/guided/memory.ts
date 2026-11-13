// Live memory figures shared by both study tabs. Refreshed every few seconds so closing other
// programs, or finishing a solve, shows up without restarting the app.
import { reactive } from "vue";
import { invoke } from "@tauri-apps/api";

const inTauri = () => typeof window !== "undefined" && "__TAURI_IPC__" in window;
async function call<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  if (inTauri()) return (await invoke(cmd, args)) as T;
  const res = await fetch(`http://127.0.0.1:8765/${cmd}`, { method: "POST", body: JSON.stringify(args) });
  return JSON.parse(await res.text()) as T;
}

const MB = 1048576;
export const memory = reactive({
  availMb: 0, // free right now, as Windows reports it
  totalMb: 0, // installed
  heldGuidedMb: 0, // kept by the Guided Study solve you can still browse
  heldThreeMb: 0, // kept by the 3-Way Pots solve you can still browse
  released: { guided: 0, threeway: 0 }, // bumped when a tab's stored solve is dropped
});

export async function refreshMemory() {
  try {
    const [avail, total, guided, three] = await call<[number, number, number, number]>("memory_status");
    memory.availMb = avail / MB; memory.totalMb = total / MB; memory.heldGuidedMb = guided / MB; memory.heldThreeMb = three / MB;
  } catch { /* keep the last reading */ }
}

/** Drops the other tab's stored solve so its memory can be used. */
export async function releaseSolve(which: "guided" | "threeway") {
  await call<void>("release_solve", { which });
  memory.released[which]++;
  await refreshMemory();
}

let started = false;
export function startMemoryWatch() {
  if (started) return;
  started = true;
  refreshMemory();
  window.setInterval(() => { if (!document.hidden) refreshMemory(); }, 4000);
}

export const gb = (mb: number) => `${(mb / 1024).toFixed(1)} GB`;
