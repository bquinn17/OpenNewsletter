/// <reference lib="webworker" />
// Custom service worker, injected into Workbox's precache manifest via
// `vite-plugin-pwa`'s `injectManifest` strategy (`docs/design/frontend.md`
// §14.2). Handles: prompt activation of a new deploy (D6), precaching the
// build, `CacheFirst` for CDN images only, and Web Push
// `push`/`notificationclick` (`docs/design/notifications.md` §5, M11 D11).
//
// Deliberately does NOT runtime-cache any API response (M11 D11 deviation
// from `docs/design/frontend.md` §5.2): authenticated JSON sitting in Cache Storage would outlive
// logout and could leak between users on a shared device.
import { precacheAndRoute } from "workbox-precaching";
import { registerRoute } from "workbox-routing";
import { CacheFirst } from "workbox-strategies";
import { ExpirationPlugin } from "workbox-expiration";
import {
  activatePromptly,
  buildNotificationOptions,
  parsePushPayload,
  resolveNotificationUrl,
  stripMediaAuthParams,
} from "./swHelpers";

declare let self: ServiceWorkerGlobalScope & {
  __WB_MANIFEST: Array<{ url: string; revision: string | null }>;
};

// D6: activate a new deploy right away (skipWaiting + clients.claim) rather
// than after every tab closes. `main.tsx` keeps vite-plugin-pwa's default
// `prompt` registerType, whose `autoUpdate` alternative would reload open
// pages mid-session; here a page picks up the new build on its next load.
// Activating drops the old build's precache, and GitHub Pages no longer
// serves its hashed files, so an open tab can't fetch a lazy chunk of the
// build it's running. Keep the app free of `import()`/`React.lazy` chunks.
activatePromptly(self);

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
// path for every viewer, so caching is effective (`docs/design/frontend.md` §5.2). `CdnImage` and
// `Avatar` load these in CORS mode, so responses aren't opaque and `CacheFirst`'s
// default stores only status 200s: a 403 from a stale cookie is never pinned.
// Keys drop dev's rotating signed-URL params, or dev would never get a hit.
registerRoute(
  ({ url }) =>
    CDN_ORIGIN !== null &&
    url.origin === CDN_ORIGIN &&
    (url.pathname.startsWith("/img/") || url.pathname.startsWith("/avatar/")),
  new CacheFirst({
    cacheName: "cdn-images",
    plugins: [
      { cacheKeyWillBeUsed: async ({ request }) => stripMediaAuthParams(request.url) },
      new ExpirationPlugin({ maxEntries: 200, maxAgeSeconds: 30 * 24 * 60 * 60 }),
    ],
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
