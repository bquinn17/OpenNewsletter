import type { IdentityProvider } from "./useAuth";

const STORAGE_KEY = "on:lastProvider";

/** Remembers which IdP this browser last signed in with, so the sign-in buttons can mark it. */
export function rememberProvider(provider: IdentityProvider): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, provider);
  } catch {
    // Storage can throw in private-browsing/locked-down contexts; the hint is only a nicety.
  }
}

export function lastProvider(): IdentityProvider | null {
  try {
    const value = window.localStorage.getItem(STORAGE_KEY);
    return value === "Google" || value === "Facebook" ? value : null;
  } catch {
    return null;
  }
}
