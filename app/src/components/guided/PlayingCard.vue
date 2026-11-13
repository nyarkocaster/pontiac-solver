<template>
  <div
    class="relative inline-flex flex-col justify-between bg-white border border-rule rounded-[10%/7%] shadow-[0_1px_0_#c9d0db,0_2px_6px_rgba(23,32,51,0.08)] select-none font-display"
    :class="[sizeClass, colorClass, empty ? '!bg-paper border-dashed shadow-none' : '']"
    :aria-label="empty ? 'empty card slot' : name"
  >
    <template v-if="!empty">
      <span class="leading-none font-extrabold pl-[12%] pt-[8%]">{{ rankLabel }}</span>
      <span class="self-center leading-none -mt-[8%] mb-[14%]" :class="suitSize">{{ suitSym }}</span>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";

const props = withDefaults(defineProps<{ card?: string; size?: "sm" | "md" | "lg" }>(), { card: "", size: "md" });
const empty = computed(() => props.card.length !== 2);
const rankLabel = computed(() => (props.card[0] === "T" ? "10" : props.card[0]));
const SYM: Record<string, string> = { s: "♠", h: "♥", d: "♦", c: "♣" };
const NAME: Record<string, string> = { s: "spades", h: "hearts", d: "diamonds", c: "clubs" };
const suitSym = computed(() => SYM[props.card[1]] ?? "");
const name = computed(() => `${rankLabel.value} of ${NAME[props.card[1]] ?? ""}`);
// four-colour deck: easier to read suits at a glance
const colorClass = computed(() => ({ s: "text-ink", h: "text-[#C8102E]", d: "text-[#1F5FBF]", c: "text-[#1E7A3C]" } as Record<string, string>)[props.card[1]] ?? "");
const sizeClass = computed(() => ({ sm: "w-8 h-11 text-sm", md: "w-12 h-[4.25rem] text-xl", lg: "w-16 h-[5.75rem] text-2xl" })[props.size]);
const suitSize = computed(() => ({ sm: "text-base", md: "text-3xl", lg: "text-4xl" })[props.size]);
</script>
