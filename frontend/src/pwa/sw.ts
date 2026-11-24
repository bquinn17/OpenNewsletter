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
import { RangeRequestsPlugin } from "workbox-range-requests";
import {
  activatePromptly,
  buildNotificationOptions,
  isCacheableMediaStatus,
  isMediaPath,
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
// Derived from `isCacheableMediaStatus` (the unit-tested rule) rather than the literal `[200]`
// `CacheableResponsePlugin` wants, so the two never drift (`docs/design/frontend.md` §19.3).
const CACHEABLE_MEDIA_STATUSES = [200, 206].filter(isCacheableMediaStatus);

// Processed media only (`/img/*`, `/avatar/*`, `/aud/*`) — a presigned S3 GET URL has the same path
// regardless of its re-signed query string, so caching by path is effective (`docs/design/media.md`
// §4.4). `MediaImage`/`Avatar`/`VoiceClip` load these in CORS mode (`crossorigin="anonymous"`), so
// responses aren't opaque and `CacheableResponsePlugin` stores only status 200s (`isCacheableMediaStatus`):
// an expired presigned URL's 403, or a `206` partial, is never pinned. Keys drop the whole query
// string, or a fresh signature for the same object would never hit.
function mediaCacheFirst(
  cacheName: string,
  maxAgeSeconds: number,
  rangeRequests = false,
): CacheFirst {
  return new CacheFirst({
    cacheName,
    plugins: [
      { cacheKeyWillBeUsed: async ({ request }) => mediaCacheKey(request.url) },
      new CacheableResponsePlugin({ statuses: CACHEABLE_MEDIA_STATUSES }),
      new ExpirationPlugin({ maxEntries: 200, maxAgeSeconds }),
      // `RangeRequestsPlugin` serves a `Range` request (Safari always sends one for `<audio>`)
      // from the one full `200` this cache ever stores (`docs/design/frontend.md` §19.3).
      ...(rangeRequests ? [new RangeRequestsPlugin()] : []),
    ],
  });
}

registerRoute(
  ({ url }) => isMediaPath(url, MEDIA_ORIGIN, "/img/"),
  mediaCacheFirst("media-images", 30 * DAY_SECONDS),
);
// Longer for avatars (`docs/design/media.md` §11.3): a new avatar gets a new `avatarId`, so a new
// path, and a cached path's bytes never change. Mirrors `AVATAR_CACHE_MAX_AGE_SECONDS` in
// `backend/crates/shared/src/config.rs`.
registerRoute(
  ({ url }) => isMediaPath(url, MEDIA_ORIGIN, "/avatar/"),
  mediaCacheFirst("media-avatars", 90 * DAY_SECONDS),
);
// Voice clips (`docs/design/frontend.md` §19.3, `docs/design/media.md` §12.4): 30-day expiry, same
// as images — a `clip.webm`'s own id (not its content) is stable, same as `imageId`.
registerRoute(
  ({ url }) => isMediaPath(url, MEDIA_ORIGIN, "/aud/"),
  mediaCacheFirst("media-audio", 30 * DAY_SECONDS, true),
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
