import type { components } from "../types/api";

/** The phases a member can open from Home; `archived` editions never render there. */
export type EditionPhase = Exclude<components["schemas"]["NewsletterStatus"], "archived">;

type PhaseStyle = {
  label: string;
  /** Page-header background, opaque so content scrolling under the sticky header never ghosts through. */
  header: string;
  /** Tinted tile/badge: pastel background with a readable accent text color. */
  chip: string;
};

// Color coding for an edition's phase (`docs/design/frontend.md` §14a). Full class
// strings, not interpolated, so Tailwind's scanner sees them.
const PHASE_STYLES: Record<EditionPhase, PhaseStyle> = {
  voting: {
    label: "Voting",
    header: "bg-phase-voting",
    chip: "bg-phase-voting text-grape-ink",
  },
  open: { label: "Open", header: "bg-phase-open", chip: "bg-phase-open text-coral-ink" },
  published: {
    label: "Published",
    header: "bg-phase-published",
    chip: "bg-phase-published text-mint-ink",
  },
};

export function phaseStyle(phase: EditionPhase): PhaseStyle {
  return PHASE_STYLES[phase];
}
