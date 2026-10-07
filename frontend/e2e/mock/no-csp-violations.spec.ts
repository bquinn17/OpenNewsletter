import { expect, test } from "@playwright/test";
import { collectCspViolations, gotoHome } from "./helpers";

/**
 * Walks Home → a group's open edition ("a newsletter") → candidates
 * ("Upcoming") → settings and asserts the CSP (`src/csp/cspPlugin.ts`)
 * never actually blocks anything the app does on the way — the meta tag
 * existing (`csp.spec.ts`) doesn't prove that by itself.
 */
test("no CSP violation walking Home → group → newsletter → candidates → settings", async ({
  page,
}) => {
  const { violations } = await collectCspViolations(page);

  await gotoHome(page);

  // Home → a group's open edition, via the bottom nav's "Respond" tab
  // (lands on the redirect route, which forwards to the open cycle — `04`
  // §4.1's `BottomNav`).
  await page.getByRole("link", { name: "Respond" }).click();
  await expect(page.getByText("Open for responses")).toBeVisible();

  // → candidates ("Upcoming"). `PageHeader` renders its title as a plain
  // `<div>`, not a heading, so this asserts on page text rather than role.
  await page.getByRole("link", { name: "Upcoming" }).click();
  await expect(page.getByText("Suggested questions")).toBeVisible();

  // → settings.
  await page.getByRole("link", { name: "You" }).click();
  await expect(page.getByRole("button", { name: "Edit profile" })).toBeVisible();

  expect(violations, violations.join("\n")).toEqual([]);
});
