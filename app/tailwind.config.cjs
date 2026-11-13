/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["src/**/*.ts", "src/**/*.vue"],
  theme: {
    extend: {
      colors: {
        ink: "#172033",
        muted: "#5B6475",
        paper: "#F3F5F8",
        rule: "#DCE1E8",
        felt: { DEFAULT: "#0E6B5C", dark: "#0A5246", light: "#E3F1EE" },
        mine: { DEFAULT: "#F2B705", light: "#FFF6D6" },
      },
      fontFamily: {
        body: ['"Atkinson Hyperlegible"', "system-ui", "sans-serif"],
        display: ['"Bricolage Grotesque"', "system-ui", "sans-serif"],
      },
    },
  },
  plugins: [require("@tailwindcss/forms")],
};
