import { expect, test, type Page } from "@playwright/test";

/**
 * The demo (`docs/design/frontend.md` §17) against a real, non-mock build — the shape of the
 * deployed site — so these prove it needs no backend and leaves a real session alone.
 */

/** `oidc-client-ts`'s key for a stored session, for `.env.e2e`'s placeholder pool and client. */
const SESSION_KEY =
  "oidc.user:https://cognito-idp.us-east-1.amazonaws.com/us-east-1_E2EPOOL:e2e-client-id";

function storedSession(): string {
  return JSON.stringify({
    id_token: "h.e30.s",
    access_token: "access",
    refresh_token: "refresh",
    token_type: "Bearer",
    expires_at: Math.floor(Date.now() / 1000) + 3600,
    profile: { sub: "real-sub", iss: "i", aud: "e2e-client-id", exp: 0, iat: 0 },
  });
}

function browserStorage(page: Page): Promise<Record<string, string>> {
  return page.evaluate(() => {
    const entries: Record<string, string> = {};
    for (const [prefix, store] of [
      ["local:", localStorage],
      ["session:", sessionStorage],
    ] as const) {
      for (let i = 0; i < store.length; i++) {
        const key = store.key(i)!;
        entries[prefix + key] = store.getItem(key)!;
      }
    }
    return entries;
  });
}

async function expectInDemo(page: Page): Promise<void> {
  await expect(page.getByRole("region", { name: "Demo" })).toBeVisible();
  await expect(page.getByRole("heading", { name: /Hey Quinn/ })).toBeVisible();
}

test("a signed-out visitor enters the demo from the home screen", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("link", { name: "See a demo" }).click();

  await expect(page).toHaveURL("/demo");
  await expectInDemo(page);
});

test("a shared deep link opens that page in the demo", async ({ page }) => {
  await page.goto("/demo/g/g_trail/upcoming");

  await expect(page.getByRole("region", { name: "Demo" })).toBeVisible();
  await expect(page.getByText("What's the best trail you hiked this summer?")).toBeVisible();
});

/** `index.html` loads its web fonts from Google; nothing else may leave the site. */
const FONT_HOSTS = ["fonts.googleapis.com", "fonts.gstatic.com"];

test("the demo sends nothing off the site and stores nothing in the browser", async ({
  page,
  baseURL,
}) => {
  const siteOrigin = new URL(baseURL!).origin;
  const offSiteRequests: string[] = [];
  page.on("request", (request) => {
    const url = new URL(request.url());
    const isOffSite = url.protocol.startsWith("http") && url.origin !== siteOrigin;
    if (isOffSite && !FONT_HOSTS.includes(url.hostname)) offSiteRequests.push(request.url());
  });
  await page.goto("/");
  const before = await browserStorage(page);

  await page.getByRole("link", { name: "See a demo" }).click();
  await expectInDemo(page);
  await page.getByRole("link", { name: /you've published/ }).click();
  await page.getByRole("link", { name: /what's been on your mind lately/i }).click();
  await page.getByPlaceholder("Tell us about it…").fill("A demo answer with a photo");
  await page.locator('input[type="file"]').setInputFiles({
    name: "photo.png",
    mimeType: "image/png",
    buffer: Buffer.from(
      "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUAAWJDR3EAAAAASUVORK5CYII=",
      "base64",
    ),
  });
  await expect(page.getByPlaceholder("Describe this image")).toBeVisible();

  expect(await browserStorage(page)).toEqual(before);
  expect(offSiteRequests).toEqual([]);
});

test("exiting the demo returns to the real, signed-out home", async ({ page }) => {
  await page.goto("/demo");
  await expectInDemo(page);

  await page.getByRole("button", { name: "Exit demo" }).click();

  await expect(page).toHaveURL("/");
  await expect(page.getByRole("link", { name: "See a demo" })).toBeVisible();
});

test("a signed-in visitor is told the demo isn't their groups, and keeps their session", async ({
  page,
}) => {
  await page.goto("/");
  await page.evaluate(([key, value]) => localStorage.setItem(key, value), [
    SESSION_KEY,
    storedSession(),
  ] as const);
  const session = await page.evaluate((key) => localStorage.getItem(key), SESSION_KEY);

  await page.goto("/demo");

  await expect(
    page.getByText(/You're still signed in, but these aren't your groups/),
  ).toBeVisible();
  expect(await page.evaluate((key) => localStorage.getItem(key), SESSION_KEY)).toBe(session);
});

test("signing in on another tab ends the demo", async ({ context }) => {
  const demoTab = await context.newPage();
  await demoTab.goto("/demo");
  await expectInDemo(demoTab);
  const otherTab = await context.newPage();
  await otherTab.goto("/");

  await otherTab.evaluate(([key, value]) => localStorage.setItem(key, value), [
    SESSION_KEY,
    storedSession(),
  ] as const);

  await expect(demoTab).toHaveURL("/");
});
