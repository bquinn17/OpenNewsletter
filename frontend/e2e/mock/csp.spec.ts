import { expect, test } from "@playwright/test";

/**
 * Checks the built, served app actually carries the CSP `<meta>` tag
 * `src/csp/cspPlugin.ts` injects, with real values from `.env.e2e`
 * (`04-frontend-architecture.md` §12.1, decided M13).
 */
test.describe("Content-Security-Policy meta tag", () => {
  test("is present exactly once, with the CDN origin in img-src and a locked-down script-src", async ({
    page,
  }) => {
    await page.goto("/");

    const metas = page.locator('meta[http-equiv="Content-Security-Policy"]');
    await expect(metas).toHaveCount(1);

    const content = await metas.getAttribute("content");
    expect(content).toBeTruthy();
    const policy = content!;

    expect(policy).not.toContain("%VITE");

    const directives = new Map(
      policy
        .split(";")
        .map((d) => d.trim())
        .filter(Boolean)
        .map((d) => {
          const [name, ...rest] = d.split(/\s+/);
          return [name!, rest];
        }),
    );

    // `.env.e2e`'s VITE_CDN_BASE_URL, reduced to its origin.
    expect(directives.get("img-src")).toContain("https://cdn.e2e.opennewsletter.test");
    expect(directives.get("script-src")).toEqual(["'self'"]);
  });
});
