import type { Config } from "tailwindcss";

const config: Config = {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        cream: "#FBF6EE",
        surface: "#FFFFFF",
        ink: "#181821",
        inkmuted: "#6F6E7C",
        coral: "#FF5A6B",
        peach: "#FFB178",
        grape: "#7B5BFF",
        sky: "#5AC8FF",
        mint: "#3DD9A4",
        sun: "#FFD23F",
        line: "#ECE5D8",
      },
      fontFamily: {
        sans: ['"Plus Jakarta Sans"', "system-ui", "sans-serif"],
        display: ['"Fraunces"', "ui-serif", "serif"],
      },
      boxShadow: {
        soft: "0 1px 2px rgba(20,18,30,.04), 0 8px 24px rgba(20,18,30,.06)",
        pop: "0 8px 32px rgba(255,90,107,.25)",
        card: "0 1px 0 rgba(20,18,30,.04), 0 18px 48px -16px rgba(20,18,30,.18)",
      },
      borderRadius: {
        "4xl": "2rem",
        "5xl": "2.5rem",
      },
      keyframes: {
        pop: {
          "0%": { opacity: "0", transform: "translateY(4px)" },
          "100%": { opacity: "1", transform: "none" },
        },
      },
      animation: {
        pop: "pop .25s ease-out both",
      },
    },
  },
  plugins: [],
};

export default config;
