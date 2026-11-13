import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./components/App.vue";
import "@fontsource/atkinson-hyperlegible/400.css";
import "@fontsource/atkinson-hyperlegible/700.css";
import "@fontsource/bricolage-grotesque/600.css";
import "@fontsource/bricolage-grotesque/800.css";
import "./style.css";
import "tippy.js/dist/tippy.css";

createApp(App).use(createPinia()).mount("#app");

// disable the right click menu
document.addEventListener("contextmenu", (e) => e.preventDefault());
