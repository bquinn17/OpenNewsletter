// Vitest-only stand-in for vite-plugin-pwa's `virtual:pwa-register` module,
// aliased in `vite.config.ts`'s `test.alias` so `main.tsx`'s real import
// doesn't pull Workbox's registration machinery (or the virtual module
// resolution vite-plugin-pwa only wires up for `vite dev`/`vite build`) into
// the test environment.
// eslint-disable-next-line @typescript-eslint/no-unused-vars -- matches the real module's signature
export function registerSW(_options?: unknown): (reloadPage?: boolean) => Promise<void> {
  return async () => {};
}
