import type { QueryClient } from "@tanstack/react-query";
import { queryClient } from "../api/queryClient";
import type { components } from "../types/api";

type S = components["schemas"];

const REFRESH_THROTTLE_MS = 10_000;

/** Mirrors `THUMB_LONG_EDGE_PX` in `backend/crates/shared/src/config.rs` (`docs/design/media.md` §1). */
const THUMB_LONG_EDGE_PX = 400;

/**
 * The thumb variant's size, from the original's `width`/`height` that the API reports. The
 * pipeline scales the long edge down to `THUMB_LONG_EDGE_PX`, never up (`docs/design/media.md`
 * §5.2). Passing the original's size to a thumb `<img>` would stretch it to its container.
 */
export function thumbSize(
  width: number | null | undefined,
  height: number | null | undefined,
): { width?: number; height?: number } {
  if (!width || !height) return {};
  const scale = Math.min(1, THUMB_LONG_EDGE_PX / Math.max(width, height));
  return { width: Math.round(width * scale), height: Math.round(height * scale) };
}

/**
 * `meta` for every query whose data carries presigned media URLs (`avatarUrl`, `displayUrl`,
 * `thumbUrl`), so `invalidateMediaQueries` can find them (`docs/design/media.md` §4.4).
 */
export const MEDIA_URLS_META = { mediaUrls: true };

/** Invalidates the queries flagged with `MEDIA_URLS_META`, refetching the active ones. */
export function invalidateMediaQueries(client: QueryClient): Promise<void> {
  return client.invalidateQueries({
    predicate: (query) => query.meta?.mediaUrls === true,
    refetchType: "active",
  });
}

let lastRefreshAt = 0;
let inFlight: Promise<void> | null = null;

/**
 * Recovers from an expired S3 presigned media URL (`docs/design/media.md` §4.2/§4.4): refetches
 * the active queries that carry media URLs, so they come back with fresh ones. Concurrent callers
 * share one in-flight refresh, and a call within 10s of the last one resolves immediately without
 * re-invalidating — a burst of `<img onError>`s from the same stale batch of URLs shouldn't each
 * trigger their own refetch.
 */
export function refreshMediaUrls(): Promise<void> {
  // Offline, the load failed on the network rather than on an expired signature, and every
  // refetch would fail too, each with its own error toast.
  if (navigator.onLine === false) return Promise.resolve();
  if (inFlight) return inFlight;
  if (Date.now() - lastRefreshAt < REFRESH_THROTTLE_MS) return Promise.resolve();

  lastRefreshAt = Date.now();
  inFlight = invalidateMediaQueries(queryClient).finally(() => {
    inFlight = null;
  });
  return inFlight;
}

/** Standard-base64 SHA-256 of `file`, or `undefined` when `crypto.subtle` isn't available. */
export async function sha256Base64(file: File): Promise<string | undefined> {
  if (typeof crypto === "undefined" || !crypto.subtle) return undefined;
  const buffer = await file.arrayBuffer();
  const digest = await crypto.subtle.digest("SHA-256", buffer);
  const bytes = new Uint8Array(digest);
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

const POLL_INITIAL_MS = 1500;
const POLL_BACKOFF_AFTER_MS = 10_000;
const POLL_BACKOFF_FACTOR = 1.5;
const POLL_MAX_INTERVAL_MS = 5000;
const POLL_TIMEOUT_MS = 120_000;

type Pollable = { status: S["MediaStatus"]; errorMessage: string | null };

type MediaPollHandlers<T extends Pollable> = {
  onReady: (data: T) => void;
  onFailed: (errorMessage: string | null) => void;
  onTimeout: () => void;
};

/**
 * Polls `fetchStatus` every 1.5s, backing off ×1.5 (capped at 5s) once pending for more than 10s,
 * giving up after ~2 minutes (`docs/design/media.md` §3.3). Returns a cancel function.
 */
export function pollMediaStatus<T extends Pollable>(
  fetchStatus: () => Promise<T>,
  handlers: MediaPollHandlers<T>,
): () => void {
  let cancelled = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const startedAt = Date.now();

  const tick = async (interval: number) => {
    if (cancelled) return;
    const elapsed = Date.now() - startedAt;
    if (elapsed > POLL_TIMEOUT_MS) {
      handlers.onTimeout();
      return;
    }

    try {
      const data = await fetchStatus();
      if (cancelled) return;
      if (data.status === "ready") {
        handlers.onReady(data);
        return;
      }
      if (data.status === "failed") {
        handlers.onFailed(data.errorMessage);
        return;
      }
    } catch {
      // Transient poll failure; keep polling until the timeout above gives up.
    }

    if (cancelled) return;
    const nextInterval =
      elapsed > POLL_BACKOFF_AFTER_MS
        ? Math.min(interval * POLL_BACKOFF_FACTOR, POLL_MAX_INTERVAL_MS)
        : interval;
    timer = setTimeout(() => void tick(nextInterval), nextInterval);
  };

  timer = setTimeout(() => void tick(POLL_INITIAL_MS), POLL_INITIAL_MS);

  return () => {
    cancelled = true;
    if (timer) clearTimeout(timer);
  };
}
