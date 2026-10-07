import { expect, test } from "@playwright/test";
import { gotoHome } from "./helpers";

/**
 * As many of `11-testing-ci-cd.md` §4.3's golden paths as the mock transport
 * supports — it has no `/admin/dev/tick` and no real Cognito session, so the
 * cycle-advance and real-auth paths aren't attempted here (see
 * `e2e/dev/golden-paths.spec.ts`, scaffolded and skipped pending a deployed
 * stack).
 */

test("suggests a candidate question and sees it in the voting list", async ({ page }) => {
  const prompt = `E2E candidate — favorite trail snack ${Date.now()}`;

  await gotoHome(page);
  await page.getByRole("link", { name: "Upcoming" }).click();

  await page.getByRole("link", { name: "＋ Idea" }).click();
  await page.getByLabel("Your question").fill(prompt);
  await page.getByRole("button", { name: "Submit" }).click();

  await expect(page.getByText(prompt)).toBeVisible();
});

test("upvotes a candidate and the vote count increments", async ({ page }) => {
  await gotoHome(page);
  await page.getByRole("link", { name: "Upcoming" }).click();

  const upvote = page.getByRole("button", {
    name: /Upvote "What's the best trail you hiked this summer\?"/,
  });
  await expect(upvote).toHaveAttribute("aria-pressed", "false");

  await upvote.click();

  const removeVote = page.getByRole("button", {
    name: /Remove your vote for "What's the best trail you hiked this summer\?"/,
  });
  await expect(removeVote).toHaveAttribute("aria-pressed", "true");
  await expect(removeVote).toContainText("6");
});

test("reacts to a published answer, toggles it off, then reacts with a different emoji", async ({
  page,
}) => {
  await gotoHome(page);
  // `g_trail` has two published editions on Home; `.first()` is the latest
  // one (`PUBLISHED_CYCLE_ID`, rendered before `morePast` — `GroupEditions.tsx`),
  // which is the one seeded with the rich M10 reaction/comment fixtures.
  await page
    .locator("section", { hasText: "Trail Crew" })
    .getByRole("link", { name: /questions · published/ })
    .first()
    .click();

  const answer = page.locator("article", { hasText: "ridge walk" });
  const firePill = answer.getByRole("button", { name: /React with 🔥, 2 reactions/ });
  await expect(firePill).toHaveAttribute("aria-pressed", "false");

  await firePill.click();
  const firePillOn = answer.getByRole("button", { name: /React with 🔥, 3 reactions/ });
  await expect(firePillOn).toHaveAttribute("aria-pressed", "true");

  await firePillOn.click();
  await expect(answer.getByRole("button", { name: /React with 🔥, 2 reactions/ })).toHaveAttribute(
    "aria-pressed",
    "false",
  );

  await answer.getByRole("button", { name: "React with 🤣" }).click();
  const laughPill = answer.getByRole("button", { name: /React with 🤣, 1 reaction/ });
  await expect(laughPill).toHaveAttribute("aria-pressed", "true");
});

test("comments on a published answer and sees it without reloading", async ({ page }) => {
  const commentBody = `E2E comment ${Date.now()}`;

  await gotoHome(page);
  await page
    .locator("section", { hasText: "Trail Crew" })
    .getByRole("link", { name: /questions · published/ })
    .first()
    .click();

  const answer = page.locator("article", { hasText: "ridge walk" });
  await answer.getByLabel("Add a comment").fill(commentBody);
  await answer.getByRole("button", { name: "Post" }).click();

  await expect(answer.getByText(commentBody)).toBeVisible();
});

test.describe("save draft → reload", () => {
  test("recovers an unsaved draft from the localStorage mirror after a reload", async ({
    page,
  }) => {
    // The literal golden path (`11` §4.3 #5: save → reload → the *saved*
    // answer persists) isn't testable in mock mode: `mockTransport`'s state
    // is a plain in-memory module-level object (`src/api/mockTransport.ts`),
    // so it resets on every real page load, unlike a real backend. What
    // *does* survive a reload in mock mode is the editor's own localStorage
    // tab-crash-recovery mirror (`useResponseEditor.ts`, `04` §8.3), which is
    // the part of the persistence design this test exercises instead.
    const draftText = `E2E unsaved draft ${Date.now()}`;

    await gotoHome(page);
    await page.getByRole("link", { name: "Respond" }).click();
    await page.getByRole("link", { name: /what's been on your mind lately/i }).click();

    const textarea = page.getByPlaceholder("Tell us about it…");
    await textarea.fill(draftText);

    // Reload immediately — before the autosave debounce (1.5s) or the mock
    // transport's simulated 250ms latency could complete a save that would
    // otherwise clear the localStorage mirror.
    await page.reload();

    await expect(page.getByPlaceholder("Tell us about it…")).toHaveValue(draftText);
    await expect(page.getByText("Unsaved changes")).toBeVisible();
  });
});
