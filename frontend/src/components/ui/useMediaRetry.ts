import { useEffect, useState } from "react";
import { mediaCacheKey } from "../../pwa/swHelpers";
import { refreshMediaUrls } from "../../utils/media";

type RetryState = {
  /** The path (origin + pathname, no query) this state belongs to, or `null` with no `src`. */
  path: string | null;
  /** Bumped to force the `<img>` to remount after a refresh, so it re-requests `src`. */
  attempt: number;
  /** The path already retried once, or `null`. A second error for this same path gives up. */
  retriedPath: string | null;
  /**
   * The exact `src`, signature included, that the hook gave up on, or `null`. A different URL for
   * the same path (a later refetch re-signed it) is loaded again, without another refresh.
   */
  failedSrc: string | null;
};

function initialState(path: string | null): RetryState {
  return { path, attempt: 0, retriedPath: null, failedSrc: null };
}

/**
 * The retry policy `MediaImage` and `Avatar` share for a presigned media URL
 * (`docs/design/media.md` §4.4). The first load error for a path awaits `refreshMediaUrls`, then
 * bumps `attempt`, which the caller uses as its `<img>`'s `key` so it remounts and re-requests
 * `src`. A second error for the same path sets `failed`. State is keyed
 * by path rather than the full URL, so the refresh's own fresh signature doesn't reset it. After
 * giving up, a different URL for the path gets one more load, and coming back online starts over:
 * offline, both errors were the network's rather than the URL's.
 */
export function useMediaRetry(src: string | null | undefined): {
  attempt: number;
  failed: boolean;
  handleError: () => void;
} {
  const path = src ? mediaCacheKey(src) : null;
  const [state, setState] = useState<RetryState>(() => initialState(path));

  if (state.path !== path) {
    setState(initialState(path));
  }

  const failed = !!src && state.failedSrc === src;

  useEffect(() => {
    if (!failed) return;
    const startOver = () =>
      setState((prev) => ({
        ...prev,
        attempt: prev.attempt + 1,
        retriedPath: null,
        failedSrc: null,
      }));
    window.addEventListener("online", startOver);
    return () => window.removeEventListener("online", startOver);
  }, [failed]);

  function handleError() {
    if (state.retriedPath === path) {
      setState((prev) => ({ ...prev, failedSrc: src ?? null }));
      return;
    }
    setState((prev) => ({ ...prev, retriedPath: path }));
    void refreshMediaUrls()
      .catch(() => {
        // The refresh itself failing is no different from the URL still being stale: the
        // remount below will hit this same `handleError` again and give up.
      })
      .finally(() => {
        setState((prev) => (prev.path === path ? { ...prev, attempt: prev.attempt + 1 } : prev));
      });
  }

  return { attempt: state.attempt, failed, handleError };
}
