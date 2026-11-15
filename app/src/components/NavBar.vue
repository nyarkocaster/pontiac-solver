<template>
  <nav
    class="metal-bar sticky flex top-0 z-30 w-full h-10 shadow-lg px-4 justify-center text-gray-50 font-body"
  >
    <div class="flex relative w-full">
      <div class="flex items-center z-10">
        <div class="px-4">
          <span class="chrome-text text-lg font-display font-extrabold tracking-tight">Pontiac Solver</span>
          <span class="pl-2 font-semibold text-gray-400">beta</span>
        </div>
      </div>

      <div class="flex ml-auto h-full items-center z-10">
        <button class="flex px-3 items-center font-semibold transition-colors hover:bg-white/10" :title="store.theme === 'dark' ? 'Switch to the light look' : 'Switch to the dark look'" @click="toggleTheme">
          <component :is="store.theme === 'dark' ? SunIcon : MoonIcon" class="w-5 h-5" />
          <span class="pl-2 text-sm">{{ store.theme === "dark" ? "Light" : "Dark" }}</span>
        </button>
        <a
          href="https://github.com/nyarkocaster/pontiac-solver"
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
          :class="'flex relative w-40 items-center justify-center font-semibold transition-colors hover:bg-white/10 hover:text-white ' + (store.navView === 'builder' ? 'nav-on text-white' : '')"
          @click="store.navView = 'builder'"
        >
          <AcademicCapIcon class="w-6 h-6" />
          <span class="pl-3">Study a spot</span>
        </button>
        <button
          v-if="store.lastResult"
          :class="'flex relative w-36 items-center justify-center font-semibold transition-colors hover:bg-white/10 hover:text-white ' + (store.navView === 'guided' || store.navView === 'threeway' ? 'nav-on text-white' : '')"
          @click="store.navView = store.lastResult"
        >
          <ChartBarIcon class="w-6 h-6" />
          <span class="pl-3">Result</span>
        </button>
        <button
          :class="'flex relative w-36 items-center justify-center font-semibold transition-colors hover:bg-white/10 hover:text-white ' + (store.navView === 'library' ? 'nav-on text-white' : '')"
          @click="store.navView = 'library'"
        >
          <BookmarkIcon class="w-6 h-6" />
          <span class="pl-3">Library</span>
        </button>
        <button
          :class="'flex relative w-36 items-center justify-center font-semibold transition-colors hover:bg-white/10 hover:text-white ' + (store.navView === 'pushfold' ? 'nav-on text-white' : '')"
          @click="store.navView = 'pushfold'"
        >
          <BoltIcon class="w-6 h-6" />
          <span class="pl-3">Push / Fold</span>
        </button>
        <button
          :class="'flex relative w-40 items-center justify-center font-semibold transition-colors hover:bg-white/10 hover:text-white ' + (store.navView === 'solver' || store.navView === 'results' ? 'nav-on text-white' : '')"
          @click="store.navView = 'solver'"
        >
          <UserGroupIcon class="w-6 h-6" />
          <span class="pl-3">Classic solver</span>
        </button>
      </div>
    </div>
  </nav>
</template>

<script setup lang="ts">
import { useStore } from "../store";
import { ComputerDesktopIcon, ChartBarIcon, AcademicCapIcon, UserGroupIcon, BoltIcon, SunIcon, MoonIcon, BookmarkIcon } from "@heroicons/vue/24/solid";

const store = useStore();
const applyTheme = () => { document.documentElement.classList.toggle("dark", store.theme === "dark"); };
const toggleTheme = () => { store.theme = store.theme === "dark" ? "light" : "dark"; try { localStorage.setItem("pontiac-theme", store.theme); } catch { /* not remembered */ } applyTheme(); };
applyTheme();
</script>

<style scoped>
.nav-on { background: linear-gradient(180deg, rgb(var(--felt-hi)), rgb(var(--felt-dark))); box-shadow: inset 0 1px 0 rgb(255 255 255 / 0.2); }
</style>
