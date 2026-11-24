<template>
  <!-- the bet tree, one decision at a time: take an action out, or add a size, at this exact point -->
  <div>
    <div class="flex flex-wrap items-center gap-1 text-sm">
      <button class="underline" :class="line.length ? 'text-felt font-bold' : 'font-bold'" @click="go(0)">Start</button>
      <template v-for="(c, i) in line" :key="i"><span class="text-muted" aria-hidden="true">›</span><button class="underline" :class="i < line.length - 1 ? 'text-felt font-bold' : 'font-bold'" @click="go(i + 1)">{{ words(c) }}</button></template>
    </div>
    <p v-if="error" class="text-sm font-bold text-danger mt-2" role="alert">{{ error }}</p>
    <template v-if="look">
      <p class="text-sm text-muted mt-2">
        <template v-if="look.over">The hand ends here.</template>
        <template v-else><span class="capitalize">{{ look.street }}</span>: {{ names[look.player] }} to act. Pot {{ fmt(look.pot) }}.</template>
        The whole tree has {{ look.lines >= 20000 ? "more than 20,000" : look.lines.toLocaleString() }} decision points.
      </p>
      <ul v-if="!look.over" class="mt-2 space-y-1">
        <li v-for="a in look.actions" :key="a.code" class="flex items-center gap-2">
          <button class="btn-quiet min-w-[11rem] text-left" :disabled="a.ends" :title="a.ends ? 'The hand ends after this action' : 'Go to the decision after this action'" @click="into(a.code)">{{ label(a) }}<span v-if="!a.ends" class="float-right text-muted" aria-hidden="true">›</span></button>
          <button v-if="a.kind !== 'fold' && a.kind !== 'call'" class="text-xs text-danger underline" :title="`Take '${label(a)}' out of the tree at this point, with everything after it`" @click="change('-' + [...line, a.code].join('-'))">Take out</button>
        </li>
      </ul>
      <div v-if="!look.over" class="flex flex-wrap items-end gap-2 mt-3">
        <label class="field"><span>{{ facing ? "Add a raise to" : "Add a bet of" }} ({{ unit }})</span><input v-model.number="amount" type="number" min="0" step="0.1" class="input w-28" @keyup.enter="add" /></label>
        <button class="btn-quiet" :disabled="!(amount > 0)" @click="add">Add here</button>
        <span v-if="amount > 0 && !facing" class="text-xs text-muted pb-2">{{ Math.round((amount * scale) / look.pot * 100) }}% of the pot</span>
      </div>
    </template>
    <div v-if="edits.length" class="mt-3 border-t border-rule pt-2">
      <div class="flex items-center justify-between"><h4 class="text-sm font-bold">Your changes</h4><button class="text-xs underline text-muted" @click="emit('update:edits', []); go(0)">Undo all</button></div>
      <ul class="mt-1 space-y-0.5 text-sm">
        <li v-for="(e, i) in edits" :key="i" class="flex items-center gap-2">
          <span :class="e[0] === '+' ? 'text-felt font-bold' : 'text-danger font-bold'">{{ e[0] === "+" ? "Added" : "Took out" }}</span>
          <span>{{ e.slice(1).split("-").map(words).join(" › ") }}</span>
          <button class="text-xs underline text-muted" @click="undo(i)">Undo</button>
        </li>
      </ul>
    </div>
    <p class="hint mt-2">A change applies at this exact point of the hand only. Sizes typed above still build the rest of the tree, so changing them, the pot or the stacks can leave a change with nothing to attach to; the message says which one to undo.</p>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import * as api from "../../guided/api";

const props = defineProps<{
  /** the spot as it would be solved, without the changes */
  spec: api.StudySpec;
  edits: string[];
  names: [string, string];
  /** chips to text */
  fmt: (chips: number) => string;
  /** chips per unit typed in the amount box, and that unit's name */
  scale: number; unit: string;
}>();
const emit = defineEmits<{ (e: "update:edits", v: string[]): void }>();

const line = ref<string[]>([]);
const look = ref<api.TreeLook | null>(null);
const error = ref(""); const amount = ref(0);
const facing = computed(() => !!look.value && look.value.street_bets[0] !== look.value.street_bets[1]);

const KIND: Record<string, string> = { F: "fold", X: "check", C: "call", B: "bet", R: "raise to", A: "all-in" };
const words = (code: string) => (code.length > 1 ? `${KIND[code[0]]} ${props.fmt(Number(code.slice(1)))}` : KIND[code] ?? code);
const label = (a: api.TreeAction) => { const w = words(a.code); return w[0].toUpperCase() + w.slice(1) + (a.kind === "bet" && look.value ? ` (${Math.round((a.amount / look.value.pot) * 100)}% pot)` : ""); };

let ticket = 0;
async function refresh() {
  const mine = ++ticket;
  try {
    const r = await api.studyTreeLook({ ...props.spec, tree_edits: props.edits }, line.value);
    if (mine === ticket) { look.value = r; error.value = ""; }
  } catch (e) {
    if (mine !== ticket) return;
    const m = String((e as Error).message ?? e);
    // the place being looked at was itself taken out: go back to the start once
    if (line.value.length && m.includes("not in the tree")) { line.value = []; void refresh(); return; }
    look.value = null; error.value = m;
  }
}
watch([() => JSON.stringify(props.spec), () => props.edits.join(",")], refresh, { immediate: true });
function go(n: number) { line.value = line.value.slice(0, n); void refresh(); }
function into(code: string) { line.value = [...line.value, code]; void refresh(); }
function change(e: string) { emit("update:edits", [...props.edits, e]); }
function undo(i: number) { emit("update:edits", props.edits.filter((_x, k) => k !== i)); }
function add() {
  if (!(amount.value > 0)) return;
  change(`+${[...line.value, (facing.value ? "R" : "B") + Math.round(amount.value * props.scale)].join("-")}`);
  amount.value = 0;
}
</script>

<style scoped>
.btn-quiet { @apply px-3 py-1.5 rounded-lg border border-rule bg-surface text-sm font-bold hover:border-felt disabled:opacity-50; }
.field { @apply flex flex-col text-sm gap-1 font-bold; }
.input { @apply rounded-md border-rule text-sm font-normal focus:border-felt focus:ring-felt; }
.hint { @apply text-sm text-muted; }
</style>
