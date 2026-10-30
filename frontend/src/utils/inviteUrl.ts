import { DEMO_BASENAME, isDemo } from "../demo";

/**
 * The shareable join URL for an invite code. Built from this build's own
 * origin and base path rather than a hardcoded domain, so it works the same
 * in local dev, GitHub Pages (`BASE_URL` is `/OpenNewsletter/`), and any
 * future deploy target. In the demo it's a demo URL, so a copied invite stays in the demo.
 */
export function inviteUrl(
  code: string,
  base: string = isDemo ? DEMO_BASENAME : import.meta.env.BASE_URL,
  origin: string = window.location.origin,
): string {
  const normalizedBase = base.endsWith("/") ? base : `${base}/`;
  return `${origin}${normalizedBase}join?code=${encodeURIComponent(code)}`;
}
