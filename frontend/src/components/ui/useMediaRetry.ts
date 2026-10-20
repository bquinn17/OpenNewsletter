import { useState } from "react";
import { mediaCacheKey } from "../../pwa/swHelpers";
import { refreshMediaUrls } from "../../utils/media";

type RetryState = {
  /** The path (origin + pathname, no query) this state belongs to, or `null` with no `src`. */
  path: string | null;
  /** Bumped to force the `<img>` to remount after a refresh, so it re-requests `src`. */
  attempt: number;
  /** The path already retried once, or `null`. A second error for this same path gives up. */
  retriedPath: string | null;
  failed: boolean;
};

function initialState(path: string | null): RetryState {
  return { path, attempt: 0, retriedPath: null, failed: false };
}

/**
 * The retry policy `MediaImage` and `Avatar` share for a presigned media URL
 * (`docs/design/media.md` §4.4). The first load error for a path awaits `onExpired` (by default
 * `refreshMediaUrls`), then bumps `attempt`, which the caller uses as its `<img>`'s `key` so it
 * remounts and re-requests `src`. A second error for the same path sets `failed`. State is keyed
 * by path rather than the full URL, so a fresh signature for the same object doesn't reset it.
 */
export function useMediaRetry(
  src: string | null | undefined,
  onExpired: () => Promise<unknown> | void = refreshMediaUrls,
): { attempt: number; failed: boolean; handleError: () => void } {
  const path = src ? mediaCacheKey(src) : null;
  const [state, setState] = useState<RetryState>(() => initialState(path));

  if (state.path !== path) {
    setState(initialState(path));
  }

  function handleError() {
    if (state.retriedPath === path) {
      setState((prev) => ({ ...prev, failed: true }));
      return;
    }
    setState((prev) => ({ ...prev, retriedPath: path }));
    void Promise.resolve(onExpired())
      .catch(() => {
        // The refresh itself failing is no different from the URL still being stale: the
        // remount below will hit this same `handleError` again and give up.
      })
      .finally(() => {
        setState((prev) => (prev.path === path ? { ...prev, attempt: prev.attempt + 1 } : prev));
      });
  }

  return { attempt: state.attempt, failed: state.failed, handleError };
}
