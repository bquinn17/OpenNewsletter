/// <reference lib="webworker" />
// Custom service worker, injected into Workbox's precache manifest via
// `vite-plugin-pwa`'s `injectManifest` strategy (`docs/design/frontend.md`
// §14.2). Handles: prompt activation of a new deploy (D6), precaching the
// build, `CacheFirst` for media images only, and Web Push
// `push`/`notificationclick` (`docs/design/notifications.md` §5, M11 D11).
//
// Deliberately does NOT runtime-cache any API response (M11 D11 deviation
// from `docs/design/frontend.md` §5.2): authenticated JSON sitting in Cache Storage would outlive
// logout and could leak between users on a shared device.
import { precacheAndRoute } from "workbox-precaching";
import { registerRoute } from "workbox-routing";
import { CacheFirst } from "workbox-strategies";
import { CacheableResponsePlugin } from "workbox-cacheable-response";
import { ExpirationPlugin } from "workbox-expiration";
import {
  activatePromptly,
  buildNotificationOptions,
  mediaCacheKey,
  parsePushPayload,
  resolveNotificationUrl,
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

/** The media bucket's own origin, or `null` if the build-time env var is missing/malformed. */
const MEDIA_ORIGIN: string | null = (() => {
  try {
    return new URL(import.meta.env.VITE_MEDIA_BASE_URL).origin;
  } catch {
    return null;
  }
})();

const DAY_SECONDS = 24 * 60 * 60;

// Processed media only (`/img/*`, `/avatar/*`) — a presigned S3 GET URL has the same path
// regardless of its re-signed query string, so caching by path is effective (`docs/design/media.md`
// §4.4). `MediaImage` and `Avatar` load these in CORS mode (`crossorigin="anonymous"`), so
// responses aren't opaque and `CacheableResponsePlugin` stores only status 200s: an expired
// presigned URL's 403 is never pinned. Keys drop the whole query string, or a fresh signature for
// the same object would never hit.
function mediaCacheFirst(cacheName: string, maxAgeSeconds: number): CacheFirst {
  return new CacheFirst({
    cacheName,
    plugins: [
      { cacheKeyWillBeUsed: async ({ request }) => mediaCacheKey(request.url) },
      new CacheableResponsePlugin({ statuses: [200] }),
      new ExpirationPlugin({ maxEntries: 200, maxAgeSeconds }),
    ],
  });
}

function isMediaPath(url: URL, prefix: "/img/" | "/avatar/"): boolean {
  return MEDIA_ORIGIN !== null && url.origin === MEDIA_ORIGIN && url.pathname.startsWith(prefix);
}

registerRoute(
  ({ url }) => isMediaPath(url, "/img/"),
  mediaCacheFirst("media-images", 30 * DAY_SECONDS),
);
// Longer for avatars (`docs/design/media.md` §11.3): a new avatar gets a new `avatarId`, so a new
// path, and a cached path's bytes never change.
registerRoute(
  ({ url }) => isMediaPath(url, "/avatar/"),
  mediaCacheFirst("media-avatars", 90 * DAY_SECONDS),
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
