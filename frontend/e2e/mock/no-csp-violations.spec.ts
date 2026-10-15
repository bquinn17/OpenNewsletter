import { expect, test } from "@playwright/test";
import { collectCspViolations, gotoHome, openCurrentEdition, openUpcoming } from "./helpers";

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

  // Home → the current group's open edition.
  await openCurrentEdition(page);
  await expect(page.getByText("Open for responses")).toBeVisible();

  // → back Home (the header logo) → candidates ("Upcoming"). `PageHeader`
  // renders its title as a plain `<div>`, not a heading, so this asserts on
  // page text rather than role.
  await page.getByRole("link", { name: "Home" }).click();
  await openUpcoming(page);
  await expect(page.getByText("Suggested questions")).toBeVisible();

  // → settings, via the header's account menu.
  await page.getByRole("button", { name: "Account menu" }).click();
  await page.getByRole("menuitem", { name: "Settings" }).click();
  await expect(page.getByRole("button", { name: "Edit profile" })).toBeVisible();

  expect(violations, violations.join("\n")).toEqual([]);
});
