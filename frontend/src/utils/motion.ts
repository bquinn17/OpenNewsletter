/**
 * Shared guards for script-driven motion (Web Animations API, view transitions):
 * jsdom has none of `matchMedia`/`animate`/`startViewTransition`, and a real
 * browser may lack `startViewTransition` or have reduced motion on, so every
 * caller feature-detects rather than assuming support (`docs/design/frontend.md` §18).
 */
export function prefersReducedMotion(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return false;
  try {
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  } catch {
    return false;
  }
}

type DocumentWithViewTransition = Document & {
  startViewTransition?: (callback: () => void) => unknown;
};

/**
 * Runs `update` inside `document.startViewTransition` when it's supported and
 * motion isn't reduced, otherwise just runs it. View transitions are
 * progressive enhancement (`docs/design/frontend.md` §18): a browser without
 * them, or a caller with reduced motion on, simply skips the animation and
 * applies the update immediately.
 */
export function withViewTransition(update: () => void): void {
  const doc =
    typeof document === "undefined" ? undefined : (document as DocumentWithViewTransition);
  if (prefersReducedMotion() || typeof doc?.startViewTransition !== "function") {
    update();
    return;
  }
  doc.startViewTransition(update);
}
