<template>
  <div class="select-none">
    <div class="grid gap-[2px]" style="grid-template-columns: repeat(13, minmax(0, 1fr))">
      <div
        v-for="(cell, i) in cells"
        :key="i"
        class="relative aspect-square rounded-[3px] overflow-hidden cursor-pointer border"
        :class="mine === i ? 'border-mine ring-[3px] ring-mine z-20' : selected === i ? 'border-ink ring-2 ring-ink z-10' : 'border-transparent'"
        :title="tooltip(i)"
        @click="$emit('select', i)"
      >
        <div v-if="cell.weight <= 0" class="absolute inset-0 bg-paper"></div>
        <div v-else class="absolute inset-0 bg-paper"></div>
        <div v-if="cell.weight > 0" class="absolute inset-x-0 bottom-0 flex" :style="{ height: share(i) * 100 + '%' }">
          <div
            v-for="(f, a) in cell.freqs"
            :key="a"
            :style="{ width: (f * 100).toFixed(2) + '%', background: colors[a] }"
          ></div>
        </div>
        <div
          class="absolute inset-0 flex items-center justify-center text-[10px] font-bold font-display"
          :class="cell.weight <= 0 ? 'text-[#B6BDC9]' : 'text-ink'"
          :style="cell.weight > 0 ? 'text-shadow: 0 0 2px #fff, 0 0 2px #fff' : ''"
        >
          {{ label(i) }}
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import type { Cell } from "../../guided/api";
type GridCell = Cell;

const props = defineProps<{ grid: GridCell[]; colors: string[]; selected: number | null; mine?: number | null }>();
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
  return `${label(i)} — equity ${(c.equity * 100).toFixed(0)}%${here}`;
};
defineExpose({ label });
</script>
