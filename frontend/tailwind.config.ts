import type { Config } from "tailwindcss";

const config: Config = {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        cream: "#FBF6EE",
        lavender: "#E4EAFF",
        blush: "#FBE1E6",
        mist: "#F4F5F9",
        // Edition phase tints (`utils/phase.ts`): pastel versions of grape, coral, mint.
        phase: {
          voting: "#ECE6FF",
          open: "#FFE3E7",
          published: "#D8F5EA",
        },
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
        float: {
          "0%, 100%": { transform: "translateY(0)" },
          "50%": { transform: "translateY(-6px)" },
        },
        wiggle: {
          "0%, 100%": { transform: "rotate(-6deg)" },
          "50%": { transform: "rotate(6deg)" },
        },
        // `ConfettiBurst` sets each piece's drift and spin.
        confetti: {
          "0%": { transform: "translate3d(0, 0, 0) rotate(0deg)", opacity: "1" },
          "85%": { opacity: "1" },
          "100%": {
            transform: "translate3d(var(--confetti-drift), 105vh, 0) rotate(var(--confetti-spin))",
            opacity: "0",
          },
        },
      },
      animation: {
        pop: "pop .25s ease-out both",
        float: "float 4s ease-in-out infinite",
        wiggle: "wiggle 1.2s ease-in-out infinite",
        confetti: "confetti 3s cubic-bezier(.25,.6,.4,1) both",
      },
    },
  },
  plugins: [],
};

export default config;
