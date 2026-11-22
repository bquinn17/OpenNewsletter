/** A plain, one-shot read of `prefers-reduced-motion` — not reactive, matching how `GiphyEmbed`'s
 * play/pause state is read once on mount (`docs/design/frontend.md` §18.3). */
export function prefersReducedMotion(): boolean {
  if (typeof window === "undefined" || !window.matchMedia) return false;
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}
