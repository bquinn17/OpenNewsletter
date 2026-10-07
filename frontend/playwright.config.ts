import { defineConfig, devices } from "@playwright/test";

/**
 * `11-testing-ci-cd.md` §4.3 golden paths, split by what the run target can
 * support (`04-frontend-architecture.md` §16 names the two targets):
 *
 * - `E2E_BASE_URL` unset: builds the app in mock mode (`vite build --mode
 *   e2e`, `.env.e2e`) and serves it with `vite preview` on :4173. Only the
 *   `mock` project runs — it has no real backend, no `/admin/dev/tick`, and
 *   no real Cognito session, so `e2e/mock/**` sticks to what the in-memory
 *   mock transport can actually do.
 * - `E2E_BASE_URL` set: points at a deployed `dev` stack with no `webServer`
 *   step. Only the `dev` project runs, against `e2e/dev/**`. Those tests
 *   need a signed-in session a deployed Cognito pool provides — blocked on
 *   operator setup — so they're scaffolded with `test.skip` guards rather
 *   than faked (see `e2e/dev/README` comment in each file).
 */
const baseURL = process.env.E2E_BASE_URL;

/**
 * `components/layout/BottomNav.tsx` (the primary nav these tests drive) is
 * `md:hidden` (Tailwind `md` = 768px) — a desktop viewport hides it
 * entirely. Its own tab labels are `hidden sm:inline` (Tailwind `sm` =
 * 640px), so a narrower "mobile" viewport loses the accessible *name*
 * instead (icon only, `aria-hidden`). 700px threads both: narrower than
 * `md` (nav renders) and no narrower than `sm` (labels stay in the a11y
 * tree). This app is mobile-first PWA — a mid-width viewport is also the
 * more representative target than desktop Chrome's default 1280px.
 */
const mobileViewport = { width: 700, height: 900 };

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
      ],
  webServer: baseURL
    ? undefined
    : {
        command: "npx vite build --mode e2e && npx vite preview --port 4173 --strict-port",
        port: 4173,
        reuseExistingServer: !process.env.CI,
        timeout: 120_000,
      },
});
