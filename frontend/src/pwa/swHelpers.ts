/**
 * Pure helpers for `sw.ts`'s `push`/`notificationclick` handlers, pulled out
 * so they're unit-testable without a `ServiceWorkerGlobalScope` (`07-notifications.md`
 * §5, M11 decision D6/D11). Deliberately lib-agnostic (no DOM/webworker
 * ambient types) so this file compiles cleanly under both the app's normal
 * tsconfig and `tsconfig.sw.json`.
 */

/** The push payload shape sent by `push-api` (`07-notifications.md` §5, D6). */
export interface PushPayload {
  title: string;
  body?: string;
  url?: string;
  tag?: string;
  groupName?: string;
}

export interface BuiltNotificationOptions {
  body?: string;
  icon: string;
  badge: string;
  tag?: string;
  data: { url: string };
}

const DEFAULT_TITLE = "OpenNewsletter";
const ICON_PATH = "/icon-192.png";
const BADGE_PATH = "/badge-96.png";

/** Normalizes an untrusted push payload (already-parsed JSON, or anything) into a safe shape. */
export function parsePushPayload(raw: unknown): PushPayload {
  if (!raw || typeof raw !== "object") return { title: DEFAULT_TITLE };
  const data = raw as Record<string, unknown>;
  return {
    title: typeof data.title === "string" && data.title.length > 0 ? data.title : DEFAULT_TITLE,
    body: typeof data.body === "string" ? data.body : undefined,
    url: typeof data.url === "string" ? data.url : undefined,
    tag: typeof data.tag === "string" ? data.tag : undefined,
    groupName: typeof data.groupName === "string" ? data.groupName : undefined,
  };
}

/** `showNotification(title, options)` — options half, per D11's SW `push` handler. */
export function buildNotificationOptions(payload: PushPayload): BuiltNotificationOptions {
  return {
    body: payload.body,
    icon: ICON_PATH,
    badge: BADGE_PATH,
    tag: payload.tag,
    data: { url: payload.url ?? "/" },
  };
}

/**
 * Resolves the payload's root-relative `url` (D6) against the SW's own
 * origin. Falls back to the origin itself on a malformed value rather than
 * throwing — a bad push payload shouldn't break the click handler — and on
 * any URL that resolves off-origin, so a notification can only open the app.
 */
export function resolveNotificationUrl(url: string | undefined, origin: string): string {
  try {
    const resolved = new URL(url ?? "/", origin);
    return resolved.origin === new URL(origin).origin ? resolved.toString() : origin;
  } catch {
    return origin;
  }
}

/**
 * The slice of `ServiceWorkerGlobalScope` that `activatePromptly` touches,
 * typed structurally so this file stays lib-agnostic (see the header).
 */
export interface ActivationScope {
  skipWaiting(): Promise<void>;
  clients: { claim(): Promise<void> };
  addEventListener(
    type: "install" | "activate",
    listener: (event: { waitUntil(promise: Promise<unknown>): void }) => void,
  ): void;
}

/**
 * Makes a newly deployed service worker take over as soon as it installs
 * (`04-frontend-architecture.md` §5.2/§14.2, D6), instead of waiting for
 * every tab of the installed PWA to close: the prod promotion gate ships the
 * backend and frontend together, so clients must pick up the matching
 * frontend quickly. It doesn't reload anything: an open page keeps running
 * the code it loaded and gets the new build on its next load or navigation.
 */
export function activatePromptly(scope: ActivationScope): void {
  scope.addEventListener("install", (event) => event.waitUntil(scope.skipWaiting()));
  scope.addEventListener("activate", (event) => event.waitUntil(scope.clients.claim()));
}

/** CloudFront signed-URL query params (`08-media-uploads.md` §4.5). */
const MEDIA_AUTH_PARAMS = ["Policy", "Signature", "Key-Pair-Id", "Expires"];

/**
 * The `cdn-images` cache key for `url`: the URL without CloudFront's signed-URL
 * params. Dev signs image URLs with query params that change every hour, so
 * keying on the full URL would never hit. Prod URLs carry none (cookies do the
 * work), so this leaves them unchanged.
 */
export function stripMediaAuthParams(url: string): string {
  const parsed = new URL(url);
  for (const param of MEDIA_AUTH_PARAMS) parsed.searchParams.delete(param);
  return parsed.toString();
}
