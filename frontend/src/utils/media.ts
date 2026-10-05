import { api } from "../api/client";
import { env } from "../env";
import type { components } from "../types/api";

type S = components["schemas"];
type MediaCookie = S["MediaCookieResponse"];

const REFRESH_BEFORE_EXPIRY_MS = 5 * 60_000;

type CacheEntry = {
  cookie: MediaCookie | null;
  inFlight: Promise<MediaCookie> | null;
};

const cache = new Map<string, CacheEntry>();

function entryFor(groupId: string): CacheEntry {
  let entry = cache.get(groupId);
  if (!entry) {
    entry = { cookie: null, inFlight: null };
    cache.set(groupId, entry);
  }
  return entry;
}

function isFresh(cookie: MediaCookie): boolean {
  return new Date(cookie.expiresAt).getTime() - REFRESH_BEFORE_EXPIRY_MS > Date.now();
}

/** Fetches (and caches per group) the CloudFront signed-cookie values, per `08-media-uploads.md` §4.4. No-op in mock mode. */
export async function ensureCookie(groupId: string): Promise<void> {
  if (env.useMocks) return;

  const entry = entryFor(groupId);
  if (entry.cookie && isFresh(entry.cookie)) return;

  if (!entry.inFlight) {
    entry.inFlight = api.media.getMediaCookie(groupId).finally(() => {
      entry.inFlight = null;
    });
  }
  entry.cookie = await entry.inFlight;
}

/**
 * Appends CloudFront signed-URL query params in dev (`08-media-uploads.md` §4.5); pass-through in
 * prod (cookies do the work there) and for any URL this group has no cached cookie for yet
 * (including `/avatar/*`, which is never signed).
 */
export function withMediaAuth(url: string): string {
  if (env.appEnv !== "dev") return url;

  const prefix = `${env.cdnBaseUrl}/img/`;
  if (!url.startsWith(prefix)) return url;

  const groupId = url.slice(prefix.length).split("/")[0];
  const cookie = groupId ? cache.get(groupId)?.cookie : null;
  if (!cookie) return url;

  const separator = url.includes("?") ? "&" : "?";
  return (
    `${url}${separator}Policy=${encodeURIComponent(cookie.policy)}` +
    `&Signature=${encodeURIComponent(cookie.signature)}` +
    `&Key-Pair-Id=${encodeURIComponent(cookie.keyPairId)}`
  );
}

/** Forces a cookie refetch for `groupId`, ignoring freshness — the `<img>` `onError` retry path. */
export async function refreshMediaAuth(groupId: string): Promise<void> {
  if (env.useMocks) return;
  entryFor(groupId).cookie = null;
  await ensureCookie(groupId);
}

export function resetMediaAuthCache(): void {
  cache.clear();
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
 * giving up after ~2 minutes (`08-media-uploads.md` §3.3). Returns a cancel function.
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
