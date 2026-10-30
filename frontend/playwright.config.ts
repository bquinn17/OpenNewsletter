import { defineConfig, devices } from "@playwright/test";

/**
 * `docs/development/testing-and-ci.md` §4.3 golden paths, split by what the run target can
 * support (`docs/design/frontend.md` §16 names the two targets):
 *
 * - `E2E_BASE_URL` unset: builds the app in mock mode (`vite build --mode
 *   e2e`, `.env.e2e`) and serves it with `vite preview` on :4173. Only the
 *   `mock` project runs — it has no real backend, no `/admin/dev/tick`, and
 *   no real Cognito session, so `e2e/mock/**` sticks to what the in-memory
 *   mock transport can actually do. The `demo` project runs alongside it,
 *   against the same env with mocks off, served on :4174 (`e2e/demo/**`).
 * - `E2E_BASE_URL` set: points at a deployed `dev` stack with no `webServer`
 *   step. Only the `dev` project runs, against `e2e/dev/**`. Those tests
 *   need a signed-in session a deployed Cognito pool provides — blocked on
 *   operator setup — so they're scaffolded with `test.skip` guards rather
 *   than faked (see the header comment in `e2e/dev/golden-paths.spec.ts`).
 */
const baseURL = process.env.E2E_BASE_URL;

/** This app is a mobile-first PWA, so the tests run at phone width. */
const mobileViewport = { width: 390, height: 844 };

export default defineConfig({
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: "list",
  use: {
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  projects: baseURL
    ? [
        {
          name: "dev",
          testDir: "./e2e/dev",
          use: { ...devices["Desktop Chrome"], viewport: mobileViewport, baseURL },
        },
      ]
    : [
        {
          name: "mock",
          testDir: "./e2e/mock",
          use: {
            ...devices["Desktop Chrome"],
            viewport: mobileViewport,
            baseURL: "http://localhost:4173",
          },
        },
        {
          name: "demo",
          testDir: "./e2e/demo",
          use: {
            ...devices["Desktop Chrome"],
            viewport: mobileViewport,
            baseURL: "http://localhost:4174",
          },
        },
      ],
  webServer: baseURL
    ? undefined
    : [
        {
          command: "npx vite build --mode e2e && npx vite preview --port 4173 --strict-port",
          port: 4173,
          reuseExistingServer: !process.env.CI,
          timeout: 120_000,
        },
        {
          // `.env.e2e` with mocks off, so the demo runs in a real build as on the deployed site
          // (`docs/design/frontend.md` §17); a process env var overrides an `.env` file. Its own
          // `outDir`, so the two builds don't overwrite each other.
          command:
            "npx vite build --mode e2e --outDir dist-e2e-demo && npx vite preview --outDir dist-e2e-demo --port 4174 --strict-port",
          env: { VITE_USE_MOCKS: "false" },
          port: 4174,
          reuseExistingServer: !process.env.CI,
          timeout: 120_000,
        },
      ],
});
