<template>
  <!-- the spot drawn as a table: who sits where, what is in the middle, whose turn it is -->
  <div class="relative mx-auto w-full max-w-3xl h-64 select-none" role="img" :aria-label="label">
    <div class="absolute inset-x-10 inset-y-10 rounded-[999px] border-[6px] border-[#3a2a1c] bg-[radial-gradient(ellipse_at_center,#2f7a56_0%,#1f5a3e_70%,#174631_100%)] shadow-[inset_0_0_30px_rgba(0,0,0,0.35)]"></div>

    <!-- middle: board and pot -->
    <div class="absolute inset-0 flex flex-col items-center justify-center gap-1.5">
      <div class="flex gap-1">
        <PlayingCard v-for="i in 5" :key="i" size="sm" :card="board[i - 1] ?? ''" :class="board[i - 1] ? '' : 'opacity-25'" />
      </div>
      <div class="text-sm font-bold text-white bg-black/35 rounded-full px-3 py-0.5 tabular-nums">Pot {{ fmt(pot) }}</div>
    </div>

    <!-- chips each player has put in, on the felt in front of the seat -->
    <div v-for="s in seats" :key="'b' + s.idx" class="absolute left-1/2 -translate-x-1/2 text-xs font-bold text-white tabular-nums" :class="s.top ? 'top-[4.4rem]' : 'bottom-[4.4rem]'">
      <span v-if="s.bet > 0" class="bg-black/35 rounded-full px-2 py-0.5"><span class="inline-block w-2 h-2 rounded-full bg-mine mr-1 align-middle"></span>{{ fmt(s.bet) }} in</span>
    </div>

    <!-- seats -->
    <div v-for="s in seats" :key="s.idx" class="absolute left-1/2 -translate-x-1/2 flex items-center gap-2" :class="s.top ? 'top-0' : 'bottom-0'">
      <div v-if="s.cards.length" class="flex gap-0.5"><PlayingCard v-for="c in s.cards" :key="c" size="sm" :card="c" /></div>
      <div class="rounded-xl border px-3 py-1.5 bg-surface text-center min-w-[9.5rem]" :class="s.acting ? 'border-felt ring-2 ring-felt' : 'border-rule'">
        <div class="text-sm font-bold leading-tight">{{ s.name }}<span v-if="s.you" class="ml-1 text-xs font-normal text-muted">you</span></div>
        <div class="text-xs tabular-nums">{{ fmt(s.left) }} behind</div>
        <div class="text-[11px] text-muted tabular-nums leading-tight">wins {{ Math.round(s.equity * 100) }}% · worth {{ fmt(Math.round(s.ev * 10) / 10) }}</div>
      </div>
      <span v-if="s.acting" class="text-xs font-bold text-felt whitespace-nowrap">to act</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import PlayingCard from "./PlayingCard.vue";

const props = defineProps<{
  board: string[];
  pot: number;
  /** chips each player has put in since the spot began [first to act, last to act] */
  bets: [number, number];
  stack: number;
  /** who decides now, or -1 when nobody does (a card is dealt, or the hand is over) */
  acting: number;
  names: [string, string];
  /** which of the two you are; drawn at the bottom */
  hero: number;
  /** your two cards when known, e.g. "AhKd" */
  heroHand?: string;
  ev: [number, number];
  equity: [number, number];
  fmt: (chips: number) => string;
}>();

const seats = computed(() =>
  [1 - props.hero, props.hero].map((idx, i) => ({
    idx, top: i === 0, you: idx === props.hero, name: props.names[idx],
    bet: props.bets[idx], left: Math.max(0, props.stack - props.bets[idx]),
    acting: props.acting === idx, ev: props.ev[idx], equity: props.equity[idx],
    cards: idx === props.hero && props.heroHand?.length === 4 ? [props.heroHand.slice(0, 2), props.heroHand.slice(2)] : [],
  })),
);
const label = computed(() => `Table: pot ${props.fmt(props.pot)}, board ${props.board.join(" ") || "not dealt"}. ` + seats.value.map((s) => `${s.name} has ${props.fmt(s.left)} behind${s.acting ? " and is to act" : ""}`).join("; ") + ".");
</script>
