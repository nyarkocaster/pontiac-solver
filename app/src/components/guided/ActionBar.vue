<template>
  <nav class="action-bar" aria-label="Hand so far">
    <div ref="strip" class="flex items-stretch gap-1.5 overflow-x-auto pb-1">
      <div v-if="board && board.length" class="board" :aria-label="'Board: ' + board.join(' ')">
        <span class="tile-top">Board</span>
        <span class="flex gap-1">
          <span v-for="c in board" :key="c" class="bcard" :class="SUIT_COLOR[c[1]]">{{ c[0] === "T" ? "10" : c[0] }}<span class="text-[0.8em]">{{ SUIT_SYMBOL[c[1]] }}</span></span>
          <span v-for="k in Math.max(0, 5 - board.length)" :key="'e' + k" class="bcard bcard-empty" aria-hidden="true"></span>
        </span>
      </div>
      <button class="tile" :class="steps.length === 0 ? 'tile-here' : ''" title="Back to the start" @click="$emit('goto', 0)">
        <span class="tile-top">Start</span>
        <span class="tile-main">{{ start }}</span>
      </button>
      <template v-for="(s, i) in steps" :key="i">
        <span class="shrink-0 self-center text-muted" aria-hidden="true">›</span>
        <button v-if="s.card" class="tile tile-card" :title="`Back to just after the ${s.who.toLowerCase()} card`" @click="$emit('goto', i + 1)">
          <span class="tile-top">{{ s.who }}</span>
          <span class="tile-main" :class="SUIT_COLOR[s.card[1]]">{{ s.card[0] === "T" ? "10" : s.card[0] }}{{ SUIT_SYMBOL[s.card[1]] }}</span>
        </button>
        <button v-else class="tile" :title="`Back to just after ${s.who} ${s.text.toLowerCase()}`" @click="$emit('goto', i + 1)">
          <span class="tile-top"><span class="dot" :style="{ background: s.color || '#98A2B3' }"></span>{{ s.who }}</span>
          <span class="tile-main">{{ s.text }}</span>
        </button>
      </template>
      <span class="shrink-0 self-center text-muted" aria-hidden="true">›</span>
      <div class="tile tile-now" aria-current="step">
        <span class="tile-top">Now</span>
        <span class="tile-main">{{ now }}</span>
        <span v-if="pot" class="tile-sub">Pot {{ pot }}</span>
      </div>
      <button v-if="steps.length" class="shrink-0 self-center ml-2 text-sm text-felt underline font-bold whitespace-nowrap" @click="$emit('goto', steps.length - 1)">Back one step</button>
      <slot />
    </div>
  </nav>
</template>

<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import { SUIT_COLOR, SUIT_SYMBOL } from "../../guided/data";
export interface Step { who: string; text: string; card?: string; color?: string }
const props = defineProps<{ start: string; steps: Step[]; now: string; pot?: string; board?: string[] }>();
// A long line is wider than the strip: keep the newest step in view.
const strip = ref<HTMLElement | null>(null);
watch(() => props.steps.length, () => nextTick(() => { if (strip.value) strip.value.scrollLeft = strip.value.scrollWidth; }));
defineEmits<{ (e: "goto", n: number): void }>();
</script>

<style scoped>
.action-bar { @apply sticky top-0 z-30 -mx-2 px-2 pt-2 pb-1 bg-paper/95 backdrop-blur border-b border-rule; }
.board { @apply shrink-0 sticky left-0 z-10 flex flex-col justify-center gap-0.5 pl-1 pr-3 mr-1 bg-paper border-r border-rule; }
.bcard { @apply inline-flex items-center justify-center w-9 h-11 rounded-md bg-surface border border-rule font-display font-extrabold text-base leading-none shadow-sm; }
.bcard-empty { @apply bg-transparent border-dashed shadow-none; }
.tile { @apply shrink-0 flex flex-col justify-center text-left min-w-[5.5rem] px-3 py-1.5 rounded-lg bg-surface border border-rule hover:border-felt transition-colors whitespace-nowrap; }
.tile-card { @apply min-w-[3.75rem] items-center text-center; }
.tile-now { @apply border-felt bg-felt-light hover:border-felt cursor-default; }
.tile-here { @apply border-ink; }
.tile-top { @apply flex items-center gap-1.5 text-[11px] leading-tight text-muted; }
.tile-main { @apply font-display font-extrabold text-sm leading-snug; }
.tile-sub { @apply text-[11px] leading-tight text-muted tabular-nums; }
.dot { @apply inline-block w-2 h-2 rounded-full; }
button:focus-visible { @apply outline-none ring-2 ring-felt ring-offset-1; }
</style>
