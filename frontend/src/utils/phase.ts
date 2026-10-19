import type { components } from "../types/api";

/** The phases a member can open from Home; `archived` editions never render there. */
export type EditionPhase = Exclude<components["schemas"]["NewsletterStatus"], "archived">;

type PhaseStyle = {
  label: string;
  /** Page-header background, translucent for the sticky backdrop blur. */
  header: string;
  /** Tinted tile/badge: pastel background with a readable accent text color. */
  chip: string;
};

// Color coding for an edition's phase (`docs/design/frontend.md` §14a). Full class
// strings, not interpolated, so Tailwind's scanner sees them.
const PHASE_STYLES: Record<EditionPhase, PhaseStyle> = {
  voting: { label: "Voting", header: "bg-phase-voting/90", chip: "bg-phase-voting text-grape" },
  open: { label: "Open", header: "bg-phase-open/90", chip: "bg-phase-open text-rose-600" },
  published: {
    label: "Published",
    header: "bg-phase-published/90",
    chip: "bg-phase-published text-emerald-700",
  },
};

export function phaseStyle(phase: EditionPhase): PhaseStyle {
  return PHASE_STYLES[phase];
}
