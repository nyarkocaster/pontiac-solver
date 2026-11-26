<template>
  <div class="select-none relative" @mouseleave="hover = null">
    <!-- details of the square under the pointer -->
    <div v-if="card" class="absolute z-30 pointer-events-none w-56 rounded-lg border border-rule bg-surface shadow-lg px-3 py-2 text-xs" :style="card.style">
      <div class="flex items-baseline justify-between"><span class="font-display font-extrabold text-base">{{ card.label }}</span><span class="text-muted">{{ card.here }}</span></div>
      <template v-if="card.cell.weight > 0">
        <div v-if="actions && card.cell.freqs.length === actions.length" class="mt-1 space-y-0.5">
          <div v-for="(f, a) in card.cell.freqs" v-show="f >= 0.005" :key="a" class="flex items-center gap-1.5">
            <span class="w-2.5 h-2.5 rounded-sm shrink-0" :style="{ background: colors[a] }"></span>
            <span class="flex-1 truncate">{{ actions[a] }}</span><span class="font-bold tabular-nums">{{ Math.round(f * 100) }}%</span>
          </div>
        </div>
        <div v-if="values !== false" class="mt-1 pt-1 border-t border-rule flex justify-between tabular-nums"><span>Worth {{ fmtEv ? fmtEv(Math.round(card.cell.ev * 10) / 10) : card.cell.ev.toFixed(1) }}</span><span>Wins {{ Math.round(card.cell.equity * 100) }}%</span></div>
      </template>
      <div v-else class="mt-1 text-muted">Not in the range here.</div>
    </div>
    <div class="grid gap-[2px]" style="grid-template-columns: repeat(13, minmax(0, 1fr))">
      <div
        v-for="(cell, i) in cells"
        :key="i"
        class="relative aspect-square rounded-[3px] overflow-hidden cursor-pointer border"
        :class="[mine === i ? 'border-mine ring-[3px] ring-mine z-20' : selected === i ? 'border-ink ring-2 ring-ink z-10' : 'border-transparent', lit && !lit[i] ? 'opacity-20' : '']"
        :aria-label="tooltip(i)"
        @mouseenter="hover = i"
        @click="$emit('select', i)"
      >
        <div v-if="cell.weight <= 0" class="absolute inset-0 bg-paper"></div>
        <div v-else class="absolute inset-0 bg-paper"></div>
        <div v-if="cell.weight > 0 && mode && mode !== 'strategy'" class="absolute inset-0" :style="{ background: 'rgb(var(--felt))', opacity: 0.08 + 0.92 * level(i) }"></div>
        <div v-else-if="cell.weight > 0" class="absolute inset-x-0 bottom-0 flex" :style="{ height: share(i) * 100 + '%' }">
          <div
            v-for="(f, a) in cell.freqs"
            :key="a"
            :style="{ width: (f * 100).toFixed(2) + '%', background: colors[a] }"
          ></div>
        </div>
        <div
          class="absolute inset-0 flex items-center justify-center text-[10px] font-bold font-display"
          :class="cell.weight <= 0 ? 'text-muted opacity-60' : valued ? (level(i) > 0.5 ? 'text-white flex-col leading-none gap-[1px]' : 'text-ink flex-col leading-none gap-[1px]') : share(i) >= 0.5 ? 'text-[#1B1417]' : 'text-ink'"
          :style="cell.weight > 0 && !valued && share(i) >= 0.5 ? 'text-shadow: 0 0 2px #fff, 0 0 2px #fff' : ''"
        >
          <span>{{ label(i) }}</span>
          <span v-if="valued && cell.weight > 0" class="text-[8px] font-body font-normal tabular-nums">{{ shown(i) }}</span>
        </div>
        <div v-if="marks && marks.has(i)" class="absolute top-[2px] right-[2px] w-[6px] h-[6px] rounded-full bg-ink ring-1 ring-white" title="Suits play differently here: click to see each one"></div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import type { Cell } from "../../guided/api";
type GridCell = Cell;

const props = defineProps<{ grid: GridCell[]; colors: string[]; selected: number | null; mine?: number | null; marks?: Set<number>; mode?: "strategy" | "ev" | "equity"; fmtEv?: (v: number) => string; /** action names, for the card shown on hover */ actions?: string[]; /** false when the squares carry no value or chance to win */ values?: boolean; /** when given, squares marked false are faded: a type of hand is being highlighted */ lit?: boolean[] }>();
const hover = ref<number | null>(null);
const valued = computed(() => !!props.mode && props.mode !== "strategy");
// EV is shaded against the best hand in the range, chance to win as it is
const evTop = computed(() => Math.max(1e-9, ...props.grid.filter((c) => c.weight > 0).map((c) => c.ev)));
const level = (i: number) => { const c = props.grid[i]; return Math.max(0, Math.min(1, props.mode === "ev" ? c.ev / evTop.value : c.equity)); };
const shown = (i: number) => { const c = props.grid[i]; return props.mode === "ev" ? (props.fmtEv ? props.fmtEv(c.ev) : c.ev.toFixed(1)) : Math.round(c.equity * 100) + "%"; };
defineEmits<{ (e: "select", i: number): void }>();

const R = "AKQJT98765432";
const label = (i: number) => {
  const r = Math.floor(i / 13), c = i % 13;
  if (r === c) return R[r] + R[c];
  return r < c ? R[r] + R[c] + "s" : R[c] + R[r] + "o";
};
const cells = computed(() => props.grid);
// how much of this hand class is still here (only when the cell says how many combos are possible)
const share = (i: number) => {
  const c = props.grid[i];
  if (!c || !c.full) return 1;
  return Math.max(0.06, Math.min(1, (c.reach ?? c.weight) / c.full));
};
const tooltip = (i: number) => {
  const c = props.grid[i];
  if (!c || c.weight <= 0) return `${label(i)}: not in range here`;
  const f = c.full ? (c.reach ?? c.weight) / c.full : 0;
  const here = c.full ? `, ${Math.min(100, f * 100).toFixed(f < 0.1 ? 1 : 0)}% of these hands get here` : "";
  return `${label(i)}: wins ${(c.equity * 100).toFixed(0)}%, worth ${props.fmtEv ? props.fmtEv(c.ev) : c.ev.toFixed(1)}${here}`;
};
/** the hover card: beside the square, on whichever side has room */
const card = computed(() => {
  const i = hover.value;
  if (i === null || !props.grid[i]) return null;
  const c = props.grid[i]; const row = Math.floor(i / 13), col = i % 13;
  const f = c.full ? (c.reach ?? c.weight) / c.full : 0;
  const style: Record<string, string> = {};
  if (col < 7) style.left = `${((col + 1) / 13) * 100}%`; else style.right = `${((13 - col) / 13) * 100}%`;
  if (row < 8) style.top = `${(row / 13) * 100}%`; else style.bottom = `${((12 - row) / 13) * 100}%`;
  return { cell: c, label: label(i), style, here: c.weight > 0 && c.full ? `${Math.min(100, f * 100).toFixed(f < 0.1 ? 1 : 0)}% in range` : "" };
});
defineExpose({ label });
</script>
