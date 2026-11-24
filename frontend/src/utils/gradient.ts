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
// Bright stops with ink text: ink clears 4.5:1 along every gradient, even at 90% opacity
// (`docs/design/frontend.md` §4.2). White text would need stops dark enough to look muddy.
const GRADIENT_CLASSES: Record<GradientSlug, string> = {
  "grape-sky": "bg-gradient-to-br from-violet-400 to-sky-400 text-ink",
  "ember-rose": "bg-gradient-to-br from-orange-400 to-rose-400 text-ink",
  "forest-mint": "bg-gradient-to-br from-green-400 to-teal-300 text-ink",
  "ocean-dusk": "bg-gradient-to-br from-blue-400 to-indigo-300 text-ink",
  "citrus-blush": "bg-gradient-to-br from-amber-300 to-pink-400 text-ink",
  "slate-lilac": "bg-gradient-to-br from-slate-400 to-purple-300 text-ink",
  "lavender-blush": "bg-gradient-to-br from-lavender to-blush text-ink",
};

export function gradientClasses(slug: GradientSlug): string {
  return GRADIENT_CLASSES[slug];
}
