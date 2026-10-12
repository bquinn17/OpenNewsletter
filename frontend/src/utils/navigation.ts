/**
 * Full-page navigation. A one-line wrapper so tests can stub it: jsdom's
 * `window.location.assign` is unforgeable and not implemented.
 */
export function assignLocation(url: string): void {
  window.location.assign(url);
}
