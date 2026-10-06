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
 * throwing — a bad push payload shouldn't break the click handler.
 */
export function resolveNotificationUrl(url: string | undefined, origin: string): string {
  try {
    return new URL(url ?? "/", origin).toString();
  } catch {
    return origin;
  }
}
