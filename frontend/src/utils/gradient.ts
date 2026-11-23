import type { components } from "../types/api";

export type GradientSlug = components["schemas"]["GradientSlug"];

/** Every `GradientSlug` value, for rendering a swatch picker (`GroupSettingsForm`). */
export const GRADIENT_SLUGS = [
  "grape-sky",
  "ember-rose",
  "forest-mint",
  "ocean-dusk",
  "citrus-blush",
  "slate-lilac",
  "lavender-blush",
] as const;

// Tailwind classes for each `GradientSlug` (`docs/design/frontend.md` §14a). The slug is the API
// contract; the class string is presentation. `Record` over the literal union
// keeps this exhaustive if the enum changes. Full class strings, not
// interpolated, so Tailwind's scanner sees them.
//
// Each pair of stops is darkened just enough that its text color clears 4.5:1 at BOTH ends
// (`docs/design/frontend.md` §4.2) while staying recognizably the same hue.
const GRADIENT_CLASSES: Record<GradientSlug, string> = {
  "grape-sky": "bg-gradient-to-br from-violet-600 to-sky-700 text-white",
  "ember-rose": "bg-gradient-to-br from-orange-700 to-rose-700 text-white",
  "forest-mint": "bg-gradient-to-br from-green-700 to-emerald-700 text-white",
  "ocean-dusk": "bg-gradient-to-br from-blue-700 to-indigo-700 text-white",
  "citrus-blush": "bg-gradient-to-br from-amber-300 to-pink-400 text-ink",
  "slate-lilac": "bg-gradient-to-br from-slate-600 to-purple-700 text-white",
  "lavender-blush": "bg-gradient-to-br from-lavender to-blush text-ink",
};

export function gradientClasses(slug: GradientSlug): string {
  return GRADIENT_CLASSES[slug];
}
