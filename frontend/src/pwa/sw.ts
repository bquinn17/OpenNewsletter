/// <reference lib="webworker" />
// Custom service worker, injected into Workbox's precache manifest via
// `vite-plugin-pwa`'s `injectManifest` strategy (`plans/04-frontend-architecture.md`
// §14.2). Handles: precaching the build, `CacheFirst` for CDN images only, and
// Web Push `push`/`notificationclick` (`plans/07-notifications.md` §5, M11 D11).
//
// Deliberately does NOT runtime-cache any API response (M11 D11 deviation
// from `04` §5.2): authenticated JSON sitting in Cache Storage would outlive
// logout and could leak between users on a shared device.
import { precacheAndRoute } from "workbox-precaching";
import { registerRoute } from "workbox-routing";
import { CacheFirst } from "workbox-strategies";
import { ExpirationPlugin } from "workbox-expiration";
import { buildNotificationOptions, parsePushPayload, resolveNotificationUrl } from "./swHelpers";

declare let self: ServiceWorkerGlobalScope & {
  __WB_MANIFEST: Array<{ url: string; revision: string | null }>;
};

precacheAndRoute(self.__WB_MANIFEST);

/** The CDN's own origin, or `null` if the build-time env var is missing/malformed. */
const CDN_ORIGIN: string | null = (() => {
  try {
    return new URL(import.meta.env.VITE_CDN_BASE_URL).origin;
  } catch {
    return null;
  }
})();

// CDN images only (`/img/*`, `/avatar/*`) — the signed-cookie URL is the same
// path for every viewer, so caching is effective (`04` §5.2).
registerRoute(
  ({ url }) =>
    CDN_ORIGIN !== null &&
    url.origin === CDN_ORIGIN &&
    (url.pathname.startsWith("/img/") || url.pathname.startsWith("/avatar/")),
  new CacheFirst({
    cacheName: "cdn-images",
    plugins: [new ExpirationPlugin({ maxEntries: 200, maxAgeSeconds: 30 * 24 * 60 * 60 })],
  }),
);

self.addEventListener("push", (event: PushEvent) => {
  let raw: unknown;
  try {
    raw = event.data?.json();
  } catch {
    raw = undefined;
  }
  const payload = parsePushPayload(raw);
  const options = buildNotificationOptions(payload);
  event.waitUntil(self.registration.showNotification(payload.title, options));
});

self.addEventListener("notificationclick", (event: NotificationEvent) => {
  event.notification.close();
  const data = event.notification.data as { url?: string } | undefined;
  const target = resolveNotificationUrl(data?.url, self.location.origin);

  event.waitUntil(
    (async () => {
      const allClients = await self.clients.matchAll({ type: "window" });
      const existing = allClients.find((c) => c.url === target);
      if (existing) {
        await existing.focus();
        return;
      }
      await self.clients.openWindow(target);
    })(),
  );
});
