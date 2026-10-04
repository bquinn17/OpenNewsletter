const STORAGE_KEY = "on:pendingInvite";

/** Fallback storage for an invite code across the OAuth round-trip (see `05-auth-flow.md` §3–§4). */
export function stashInvite(code: string): void {
  try {
    window.sessionStorage.setItem(STORAGE_KEY, code);
  } catch {
    // sessionStorage can throw in private-browsing/locked-down contexts; the
    // OIDC `state` payload is the primary carrier, so losing the fallback is fine.
  }
}

/** Reads and clears the stashed invite code, if any. */
export function takeStashedInvite(): string | null {
  try {
    const code = window.sessionStorage.getItem(STORAGE_KEY);
    if (code !== null) window.sessionStorage.removeItem(STORAGE_KEY);
    return code;
  } catch {
    return null;
  }
}
