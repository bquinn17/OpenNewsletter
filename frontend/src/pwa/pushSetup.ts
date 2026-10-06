import { api } from "../api/client";

/**
 * Web Push subscription lifecycle (`plans/07-notifications.md` §3/§11, M11
 * decision D11). Talks to `navigator.serviceWorker` / `PushManager` /
 * `Notification` directly — there's no React involved here, so this module
 * stays plain functions that `App.tsx` (resubscribe-on-load), `SettingsPage`'s
 * notifications section (the master toggle), and `AuthProvider.logout`
 * (best-effort unsubscribe) call imperatively.
 */

export function isPushSupported(): boolean {
  return (
    typeof navigator !== "undefined" &&
    "serviceWorker" in navigator &&
    typeof window !== "undefined" &&
    "PushManager" in window &&
    "Notification" in window
  );
}

/** Standard VAPID-key base64url → `Uint8Array` decode (RFC 4648 §5, unpadded). */
export function urlB64ToUint8Array(base64String: string): Uint8Array {
  const padding = "=".repeat((4 - (base64String.length % 4)) % 4);
  const base64 = (base64String + padding).replace(/-/g, "+").replace(/_/g, "/");
  const rawData = atob(base64);
  const outputArray = new Uint8Array(rawData.length);
  for (let i = 0; i < rawData.length; i++) outputArray[i] = rawData.charCodeAt(i);
  return outputArray;
}

function keyMatches(subscription: PushSubscription, desired: Uint8Array): boolean {
  const current = subscription.options.applicationServerKey;
  if (!current) return false;
  const currentArr = new Uint8Array(current as ArrayBuffer);
  if (currentArr.length !== desired.length) return false;
  for (let i = 0; i < desired.length; i++) {
    if (currentArr[i] !== desired[i]) return false;
  }
  return true;
}

async function postSubscription(subscription: PushSubscription): Promise<void> {
  const json = subscription.toJSON();
  if (!json.endpoint || !json.keys?.p256dh || !json.keys?.auth) {
    throw new Error("PushSubscription.toJSON() is missing required fields");
  }
  await api.push.subscribe({
    endpoint: json.endpoint,
    expirationTime: json.expirationTime ?? null,
    keys: { p256dh: json.keys.p256dh, auth: json.keys.auth },
    userAgent: navigator.userAgent,
  });
}

export type EnsurePushResult =
  | { status: "unsupported" }
  | { status: "denied" }
  /** `prompt: false` and either permission isn't granted yet, or no local subscription exists. */
  | { status: "skipped" }
  | { status: "subscribed" };

/**
 * `prompt: false` (app load, `07` §11.2): only refreshes an existing,
 * already-granted subscription — never shows the OS permission prompt.
 * `prompt: true` (Settings toggle on): requests permission if needed, then
 * subscribes (or reuses/rekeys) and posts to the server.
 */
export async function ensurePushSubscription(
  vapidPublicKey: string,
  opts: { prompt: boolean },
): Promise<EnsurePushResult> {
  if (!isPushSupported()) return { status: "unsupported" };

  let permission = Notification.permission;
  if (permission !== "granted") {
    if (!opts.prompt) return { status: "skipped" };
    permission = await Notification.requestPermission();
    if (permission !== "granted") return { status: "denied" };
  }

  const registration = await navigator.serviceWorker.ready;
  let subscription = await registration.pushManager.getSubscription();
  if (!opts.prompt && !subscription) return { status: "skipped" };

  const desiredKey = urlB64ToUint8Array(vapidPublicKey);
  if (subscription && !keyMatches(subscription, desiredKey)) {
    await subscription.unsubscribe();
    subscription = null;
  }

  if (!subscription) {
    subscription = await registration.pushManager.subscribe({
      userVisibleOnly: true,
      // lib.dom's `BufferSource` wants an `ArrayBufferView<ArrayBuffer>`
      // specifically (TS 5.7+); plain `new Uint8Array(n)` is inferred as the
      // wider `Uint8Array<ArrayBufferLike>`, which is otherwise identical.
      applicationServerKey: desiredKey as BufferSource,
    });
  }

  await postSubscription(subscription);
  return { status: "subscribed" };
}

/**
 * Best-effort: unsubscribes locally and tells the server, swallowing every
 * failure. Used by the Settings toggle off and by `AuthProvider.logout`
 * (which must never be blocked or broken by a push error, D11).
 */
export async function disablePush(): Promise<void> {
  if (!isPushSupported()) return;
  try {
    const registration = await navigator.serviceWorker.ready;
    const subscription = await registration.pushManager.getSubscription();
    if (!subscription) return;
    try {
      await api.push.unsubscribe({ endpoint: subscription.endpoint });
    } catch {
      // Best-effort — still drop the local subscription below.
    }
    await subscription.unsubscribe();
  } catch {
    // Never throw: callers (notably logout) must proceed regardless.
  }
}

export interface LocalPushState {
  permission: NotificationPermission;
  /** The current browser's subscription endpoint, or `null` if none/not granted. */
  endpoint: string | null;
}

/** The local (this-browser) half of the Settings master toggle's state. */
export async function getLocalPushState(): Promise<LocalPushState> {
  if (!isPushSupported()) return { permission: "default", endpoint: null };
  const permission = Notification.permission;
  if (permission !== "granted") return { permission, endpoint: null };
  const registration = await navigator.serviceWorker.ready;
  const subscription = await registration.pushManager.getSubscription();
  return { permission, endpoint: subscription?.endpoint ?? null };
}
