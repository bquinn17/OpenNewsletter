import { create } from "zustand";

/** `docs/design/frontend.md` §18.4: GIPHY doesn't document when its hourly window resets. */
export const GIPHY_COOLDOWN_MS = 10 * 60_000;

interface State {
  /** Epoch ms the shared cooldown ends, or `null` when no `429` is active. */
  until: number | null;
  /** A `429` from either search or a display lookup starts (or restarts) the cooldown. */
  markRateLimited: () => void;
  /** Not persisted — a reload clears it (`docs/design/frontend.md` §18.4). */
  clear: () => void;
}

/**
 * The shared, in-memory `giphyLimit` store (`docs/design/frontend.md` §18.4): every GIPHY call —
 * search or the batched display lookup — checks this first, and a `429` from either one blocks
 * the other for the same 10-minute cooldown.
 */
export const useGiphyLimit = create<State>((set) => ({
  until: null,
  markRateLimited: () => set({ until: Date.now() + GIPHY_COOLDOWN_MS }),
  clear: () => set({ until: null }),
}));

/** Whether the cooldown is active right now (a plain read, outside a component). */
export function isGiphyRateLimited(): boolean {
  const until = useGiphyLimit.getState().until;
  return until !== null && until > Date.now();
}

/** Milliseconds left in the cooldown, or 0 when it's not active. */
export function giphyLimitRemainingMs(): number {
  const until = useGiphyLimit.getState().until;
  return until === null ? 0 : Math.max(0, until - Date.now());
}
