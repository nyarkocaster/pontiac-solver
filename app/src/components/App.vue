<template>
  <div class="min-w-[1080px]" :style="{ height: clientHeight + 'px' }">
    <NavBar />

    <div v-show="store.navView === 'builder'" style="height: calc(100% - 2.5rem)">
      <SpotBuilder :run2="run2" :run3="run3" :preview2="preview2" :preview3="preview3" :quick-flop="quickFlop" @done="showResult" />
    </div>

    <div v-show="store.navView === 'guided'" style="height: calc(100% - 2.5rem)">
      <GuidedStudy ref="guided" @new="store.navView = 'builder'" />
    </div>

    <div v-show="store.navView === 'threeway'" style="height: calc(100% - 2.5rem)">
      <ThreeWayRiver ref="threeway" @new="store.navView = 'builder'" />
    </div>

    <div v-show="store.navView === 'library'" style="height: calc(100% - 2.5rem)">
      <Library :active="store.navView === 'library'" :open2="open2" :open3="run3" :open3saved="open3saved" @done="showResult" />
    </div>

    <div v-show="store.navView === 'pushfold'" style="height: calc(100% - 2.5rem)">
      <PushFold />
    </div>

    <div v-if="store.navView === 'about'" class="overflow-y-auto px-6 py-4" style="height: calc(100% - 2.5rem)">
      <div class="max-w-screen-lg mx-auto"><AboutPage /></div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { useStore } from "../store";

import NavBar from "./NavBar.vue";
import AboutPage from "./AboutPage.vue";
import GuidedStudy from "./guided/GuidedStudy.vue";
import ThreeWayRiver from "./guided/ThreeWayRiver.vue";
import PushFold from "./guided/PushFold.vue";
import Library from "./guided/Library.vue";
import type { LibraryEntry } from "../guided/api";
import SpotBuilder, { type Solve2, type Solve3 } from "./guided/SpotBuilder.vue";

const guided = ref<InstanceType<typeof GuidedStudy> | null>(null);
const threeway = ref<InstanceType<typeof ThreeWayRiver> | null>(null);
const fail = { ok: false, error: "The solver screen is not ready yet. Try again." };
const run2 = async (p: Solve2) => (await guided.value?.loadCustom(p)) ?? fail;
const run3 = async (p: Solve3) => (await threeway.value?.loadCustom(p)) ?? fail;
const preview2 = async (p: Solve2) => (await guided.value?.previewCustom(p)) ?? { text: "", problem: "" };
const preview3 = async (p: Solve3) => (await threeway.value?.previewCustom(p)) ?? { text: "", problem: "" };
const quickFlop = async (p: Solve2, flop: number[]) => { if (store.lastResult === "guided") store.lastResult = ""; if (!guided.value) throw new Error(fail.error); return guided.value.quickFlop(p, flop); };
const open2 = async (e: LibraryEntry) => (await guided.value?.openSaved(e)) ?? fail;
const open3saved = async (e: LibraryEntry) => (await threeway.value?.openSaved(e)) ?? fail;
const showResult = (kind: "guided" | "threeway") => { store.lastResult = kind; store.navView = kind; };

const store = useStore();

const clientHeight = ref(0);
const updateClientHeight = () => {
  clientHeight.value = document.documentElement.clientHeight - 0.01;
};

updateClientHeight();
window.addEventListener("resize", updateClientHeight);
</script>
