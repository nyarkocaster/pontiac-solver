<template>
  <div class="h-full overflow-y-auto bg-paper text-ink font-body">
    <div class="max-w-4xl mx-auto px-6 py-6">
      <h1 class="font-display font-extrabold text-2xl">Library</h1>
      <p class="text-muted mt-1">Solves you saved from the Result page. They open straight away, without solving again.</p>
      <div class="mt-3 flex items-center gap-3 text-sm">
        <label class="btn-quiet cursor-pointer">Add a library file<input type="file" accept=".plib" class="sr-only" @change="addFile" /></label>
        <span class="text-muted">A small file someone shared with you, or one made by the headless program (.plib).</span>
      </div>
      <p v-if="error" class="mt-3 text-sm font-bold text-danger">{{ error }}</p>
      <p v-if="!entries.length && loaded" class="panel mt-5 text-sm">Nothing saved yet. After a solve, press <b>Save to library</b> at the top of the Result page.</p>
      <ul class="mt-5 space-y-2">
        <li v-for="e in entries" :key="e.id" class="panel flex items-center gap-4">
          <div class="flex-1 min-w-0">
            <div class="font-bold truncate">{{ e.name }}</div>
            <div class="text-xs text-muted mt-0.5">
              {{ when(e.saved_at) }} · {{ e.kind === "flop" ? "two players, flop only: each turn is solved when you pick its card" : e.kind === "two" ? "two players" : stored(e) ? "three players" : "three players, spot only: solved again when opened" }}<template v-if="e.bytes"> · {{ size(e.bytes) }} on disk</template>
              <template v-if="e.summary?.locked"> · includes the locked version</template>
            </div>
          </div>
          <span v-if="busy === e.id" class="text-sm font-bold text-felt" role="status">{{ e.kind !== "three" || stored(e) ? "Opening…" : "Solving again…" }}</span>
          <template v-else>
            <button class="btn-quiet" :disabled="!!busy" @click="open(e)">Open</button>
            <button v-if="e.kind === 'flop'" class="btn-quiet" :disabled="!!busy" title="Saves this entry as one small file you can send to someone" @click="getFile(e)">Get the file</button>
            <button v-if="asking !== e.id" class="text-xs text-muted underline" :disabled="!!busy" @click="asking = e.id">Delete</button>
            <button v-else class="text-xs font-bold text-danger underline" @click="remove(e)">Really delete?</button>
          </template>
        </li>
      </ul>
      <p v-if="entries.length" class="hint mt-4">{{ size(total) }} used in total. Whole flop solves are large files; delete the ones you no longer need. "Flop only" entries are small.</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import * as api from "../../guided/api";

type Outcome = { ok: boolean; error: string };
const props = defineProps<{ active: boolean; open2: (e: api.LibraryEntry) => Promise<Outcome>; open3: (payload: never) => Promise<Outcome>; open3saved: (e: api.LibraryEntry) => Promise<Outcome> }>();
const emit = defineEmits<{ (e: "done", kind: "guided" | "threeway"): void }>();
const entries = ref<api.LibraryEntry[]>([]); const loaded = ref(false); const error = ref(""); const busy = ref(""); const asking = ref("");
const refresh = async () => { try { entries.value = await api.libraryEntries(); loaded.value = true; } catch (e) { error.value = String((e as Error).message ?? e); } };
watch(() => props.active, (on) => { if (on) { error.value = ""; asking.value = ""; refresh(); } }, { immediate: true });
/** Three-player entries from before 0.13.4 keep the spot only. */
const stored = (e: api.LibraryEntry) => e.meta.solved === true;
const total = computed(() => entries.value.reduce((a, e) => a + e.bytes, 0));
const size = (b: number) => (b >= 1 << 30 ? (b / (1 << 30)).toFixed(1) + " GB" : b >= 1 << 20 ? Math.round(b / (1 << 20)) + " MB" : Math.max(1, Math.round(b / 1024)) + " KB");
const when = (s: number) => new Date(s * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
async function open(e: api.LibraryEntry) {
  busy.value = e.id; error.value = "";
  try {
    const r = e.kind !== "three" ? await props.open2(e) : stored(e) ? await props.open3saved(e) : await props.open3(e.meta.payload as never);
    if (r.ok) emit("done", e.kind !== "three" ? "guided" : "threeway"); else error.value = r.error || "That could not be opened.";
  } catch (x) { error.value = String((x as Error).message ?? x); }
  finally { busy.value = ""; }
}
/** Adds a .plib file to the library. */
async function addFile(ev: Event) {
  const input = ev.target as HTMLInputElement; const f = input.files?.[0]; input.value = "";
  if (!f) return;
  error.value = "";
  try { await api.libraryImportFile(Array.from(new Uint8Array(await f.arrayBuffer())), f.name); await refresh(); }
  catch (x) { error.value = String((x as Error).message ?? x); }
}
/** Hands a flop-only entry over as a file. */
async function getFile(e: api.LibraryEntry) {
  error.value = "";
  try {
    const bytes = new Uint8Array(await api.libraryExportFile(e.id));
    const a = document.createElement("a");
    a.href = URL.createObjectURL(new Blob([bytes], { type: "application/octet-stream" }));
    a.download = (e.name.replace(/[^A-Za-z0-9 ._-]+/g, " ").trim().slice(0, 80) || "pontiac") + ".plib"; a.click(); URL.revokeObjectURL(a.href);
  } catch (x) { error.value = String((x as Error).message ?? x); }
}
async function remove(e: api.LibraryEntry) {
  try { await api.libraryRemove(e.id); } catch (x) { error.value = String((x as Error).message ?? x); }
  asking.value = ""; await refresh();
}
</script>

<style scoped>
.panel { @apply bg-surface border border-rule rounded-xl p-4; }
.hint { @apply text-xs text-muted; }
.btn-quiet { @apply px-3 py-1.5 rounded-lg border border-rule bg-surface text-sm font-bold hover:border-felt disabled:opacity-50; }
</style>
