import type { components } from "../types/api";

export type GradientSlug = components["schemas"]["GradientSlug"];

// Tailwind classes for each `GradientSlug` (`04` §14a). The slug is the API
// contract; the class string is presentation. `Record` over the literal union
// keeps this exhaustive if the enum changes. Full class strings, not
// interpolated, so Tailwind's scanner sees them.
const GRADIENT_CLASSES: Record<GradientSlug, string> = {
  "grape-sky": "bg-gradient-to-br from-violet-500 to-sky-400 text-white",
  "ember-rose": "bg-gradient-to-br from-orange-500 to-rose-500 text-white",
  "forest-mint": "bg-gradient-to-br from-green-700 to-emerald-300 text-white",
  "ocean-dusk": "bg-gradient-to-br from-blue-700 to-indigo-400 text-white",
  "citrus-blush": "bg-gradient-to-br from-amber-300 to-pink-400 text-ink",
  "slate-lilac": "bg-gradient-to-br from-slate-600 to-purple-300 text-white",
};

export function gradientClasses(slug: GradientSlug): string {
  return GRADIENT_CLASSES[slug];
}
