/**
 * Fills the service worker's `media-audio` cache after a clip's first play
 * (`docs/design/frontend.md` §19.3). Safari always requests `<audio>` with a `Range` header,
 * which S3 answers with a `206` — and the SW's `CacheableResponsePlugin` only stores a full `200`
 * (`isCacheableMediaStatus`), so a 206 is never cached on its own. This issues one plain,
 * unranged `GET` per clip so the SW's route sees a cacheable `200` once.
 */
const primed = new Set<string>();

export async function primeAudioCache(url: string, doFetch: typeof fetch = fetch): Promise<void> {
  if (primed.has(url)) return;
  primed.add(url);
  try {
    await doFetch(url, { mode: "cors", credentials: "omit" });
  } catch {
    // Best effort — playback itself doesn't depend on this succeeding.
  }
}
