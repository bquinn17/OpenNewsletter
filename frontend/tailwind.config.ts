import type { Config } from "tailwindcss";

// Reads each token's CSS variable (`src/styles/tailwind.css` `:root`), so
// opacity modifiers like `bg-coral/10` keep working (`docs/design/frontend.md` §4.2).
function withOpacity(variable: string) {
  return `rgb(var(${variable}) / <alpha-value>)`;
}

const config: Config = {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        cream: withOpacity("--cream"),
        lavender: withOpacity("--lavender"),
        blush: withOpacity("--blush"),
        mist: withOpacity("--mist"),
        // Edition phase tints (`utils/phase.ts`): pastel versions of grape, coral, mint.
        phase: {
          voting: withOpacity("--phase-voting"),
          open: withOpacity("--phase-open"),
          published: withOpacity("--phase-published"),
        },
        surface: withOpacity("--surface"),
        ink: withOpacity("--ink"),
        inkmuted: withOpacity("--inkmuted"),
        coral: {
          DEFAULT: withOpacity("--coral"),
          ink: withOpacity("--coral-ink"),
        },
        peach: withOpacity("--peach"),
        grape: {
          DEFAULT: withOpacity("--grape"),
          ink: withOpacity("--grape-ink"),
        },
        sky: {
          DEFAULT: withOpacity("--sky"),
          ink: withOpacity("--sky-ink"),
        },
        mint: {
          DEFAULT: withOpacity("--mint"),
          ink: withOpacity("--mint-ink"),
        },
        sun: {
          DEFAULT: withOpacity("--sun"),
          ink: withOpacity("--sun-ink"),
        },
        danger: withOpacity("--danger"),
        line: withOpacity("--line"),
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
      transitionDuration: {
        fast: "120ms",
        base: "220ms",
        slow: "400ms",
      },
      transitionTimingFunction: {
        spring: "cubic-bezier(.34,1.56,.64,1)",
        smooth: "cubic-bezier(.2,.8,.2,1)",
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
        "pop-in": {
          "0%": { opacity: "0", transform: "scale(.6)" },
          "60%": { opacity: "1", transform: "scale(1.08)" },
          "100%": { opacity: "1", transform: "scale(1)" },
        },
        rise: {
          "0%": { opacity: "0", transform: "translateY(8px)" },
          "100%": { opacity: "1", transform: "none" },
        },
        shimmer: {
          "0%": { backgroundPosition: "200% 0" },
          "100%": { backgroundPosition: "-200% 0" },
        },
        "float-up": {
          "0%": { opacity: "1", transform: "translateY(0)" },
          "100%": { opacity: "0", transform: "translateY(-32px)" },
        },
        "pulse-soft": {
          "0%, 100%": { transform: "scale(1)", opacity: "1" },
          "50%": { transform: "scale(1.04)", opacity: "0.85" },
        },
        draw: {
          "0%": { strokeDashoffset: "1" },
          "100%": { strokeDashoffset: "0" },
        },
      },
      animation: {
        pop: "pop .25s ease-out both",
        float: "float 4s ease-in-out infinite",
        wiggle: "wiggle 1.2s ease-in-out infinite",
        confetti: "confetti 3s cubic-bezier(.25,.6,.4,1) both",
        "pop-in": "pop-in .4s cubic-bezier(.34,1.56,.64,1) both",
        rise: "rise .4s cubic-bezier(.2,.8,.2,1) both",
        shimmer: "shimmer 1.6s linear infinite",
        "float-up": "float-up .8s cubic-bezier(.2,.8,.2,1) both",
        "pulse-soft": "pulse-soft 2s cubic-bezier(.2,.8,.2,1) infinite",
        draw: "draw .5s cubic-bezier(.2,.8,.2,1) forwards",
      },
    },
  },
  plugins: [],
};

export default config;
