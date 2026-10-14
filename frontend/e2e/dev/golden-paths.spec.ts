import { test } from "@playwright/test";

/**
 * `docs/development/testing-and-ci.md` §4.3's full golden-path list, against a deployed
 * `dev` stack (this file's `dev` Playwright project only runs when
 * `E2E_BASE_URL` is set — see `playwright.config.ts`).
 *
 * Every one of these needs a real, signed-in Cognito session, which needs
 * `E2E_USERNAME`/`E2E_PASSWORD` (`docs/development/testing-and-ci.md` §4.3, "until B5
 * clears") — operator setup that hasn't happened yet. Scaffolded with
 * `test.skip` rather than faked: filling these in is just writing the test
 * body once a session-helper (`signIn(page)`, not written here) exists.
 */
const hasDevCredentials = !!process.env.E2E_USERNAME && !!process.env.E2E_PASSWORD;

test.describe("dev-stack golden paths (`docs/development/testing-and-ci.md` §4.3)", () => {
  test.skip(!hasDevCredentials, "needs E2E_USERNAME/E2E_PASSWORD against a deployed dev stack");

  test("1. sign in → land on home → see no groups → redeem invite → see group", async () => {
    test.fixme();
  });

  test("2. suggest a candidate question → see it in the list", async () => {
    test.fixme();
  });

  test("3. upvote a candidate → vote count increments", async () => {
    test.fixme();
  });

  test("4. cycle opens (POST /admin/dev/tick/cycle) → see questions", async () => {
    test.fixme();
  });

  test("5. save draft → close tab → reopen → draft persists", async () => {
    test.fixme();
  });

  test("6. upload an image → wait for ready → image renders in preview", async () => {
    test.fixme();
  });

  test("7. publish answer → cycle closes → comments enabled", async () => {
    test.fixme();
  });

  test("8. comment on an answer → see it on reload", async () => {
    test.fixme();
  });

  test("9. react with emoji → toggle off → toggle on different emoji", async () => {
    test.fixme();
  });

  test("10. upload avatar → poll until ready → PATCH /me → avatar renders in member listing", async () => {
    test.fixme();
  });
});
