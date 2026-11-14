/** @type {import('tailwindcss').Config} */
module.exports = {
  darkMode: "class",
  content: ["src/**/*.ts", "src/**/*.vue"],
  theme: {
    extend: {
      colors: {
        // theme colours live in style.css as variables, one set for light and one for dark
        ink: "rgb(var(--ink) / <alpha-value>)",
        muted: "rgb(var(--muted) / <alpha-value>)",
        paper: "rgb(var(--paper) / <alpha-value>)",
        surface: "rgb(var(--surface) / <alpha-value>)",
        rule: "rgb(var(--rule) / <alpha-value>)",
        danger: "rgb(var(--danger) / <alpha-value>)",
        chrome: "rgb(var(--chrome) / <alpha-value>)",
        // "felt" is the accent: deep burgundy
        felt: { DEFAULT: "rgb(var(--felt) / <alpha-value>)", dark: "rgb(var(--felt-dark) / <alpha-value>)", light: "rgb(var(--felt-light) / <alpha-value>)" },
        mine: { DEFAULT: "rgb(var(--mine) / <alpha-value>)", light: "rgb(var(--mine-light) / <alpha-value>)" },
      },
      fontFamily: {
        body: ['"Atkinson Hyperlegible"', "system-ui", "sans-serif"],
        display: ['"Bricolage Grotesque"', "system-ui", "sans-serif"],
      },
    },
  },
  plugins: [require("@tailwindcss/forms")],
};
