<template>
  <nav
    class="sticky flex top-0 z-30 w-full h-10 shadow-lg px-4 justify-center bg-ink text-gray-50 font-body"
  >
    <div class="flex relative w-full">
      <div class="flex items-center z-10">
        <div class="px-4">
          <span class="text-lg font-display font-extrabold tracking-tight">Pontiac Solver</span>
          <span class="pl-2 font-semibold text-gray-400">beta</span>
        </div>
      </div>

      <div class="flex ml-auto h-full items-center z-10">
        <a
          href="https://github.com/b-inary/desktop-postflop"
          class="flex px-4 h-full items-center font-semibold hover:bg-slate-700"
          target="_blank"
        >
          <img
            src="../assets/GitHub-Mark-Light-32px.png"
            class="inline-block w-5 h-5 mr-2.5"
          />
          GitHub
        </a>
      </div>

      <div
        class="flex absolute w-full h-full left-0 top-0 gap-3 justify-center"
      >
        <button
          :class="
            'flex relative w-36 items-center justify-center font-semibold ' +
            'transition-colors hover:bg-slate-700 hover:text-blue-200 ' +
            (store.navView === 'guided' ? 'bg-slate-700 text-blue-200' : '')
          "
          @click="store.navView = 'guided'"
        >
          <AcademicCapIcon class="w-6 h-6" />
          <span class="pl-3">Guided Study</span>
        </button>
        <button
          :class="
            'flex relative w-44 items-center justify-center font-semibold ' +
            'transition-colors hover:bg-slate-700 hover:text-blue-200 ' +
            (store.navView === 'threeway' ? 'bg-slate-700 text-blue-200' : '')
          "
          @click="store.navView = 'threeway'"
        >
          <UserGroupIcon class="w-6 h-6" />
          <span class="pl-3">3-Way Pots</span>
        </button>
        <button
          :class="
            'flex relative w-32 items-center justify-center font-semibold ' +
            'transition-colors hover:bg-slate-700 hover:text-blue-200 ' +
            (store.navView === 'solver' ? 'bg-slate-700 text-blue-200' : '')
          "
          @click="store.navView = 'solver'"
        >
          <ComputerDesktopIcon class="w-6 h-6" />
          <span class="pl-3">Advanced</span>
        </button>
        <button
          :class="
            'flex relative w-48 items-center justify-center font-semibold ' +
            'transition-colors hover:bg-slate-700 hover:text-blue-200 ' +
            (store.navView === 'results' ? 'bg-slate-700 text-blue-200' : '')
          "
          @click="store.navView = 'results'"
        >
          <ChartBarIcon class="w-6 h-6" />
          <span class="pl-3">Advanced results</span>
        </button>
      </div>
    </div>
  </nav>
</template>

<script setup lang="ts">
import { useStore } from "../store";
import { ComputerDesktopIcon, ChartBarIcon, AcademicCapIcon, UserGroupIcon } from "@heroicons/vue/24/solid";

const store = useStore();
</script>
