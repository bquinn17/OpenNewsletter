import type { Page } from "@playwright/test";

/**
 * Collects every CSP violation the page reports from here on — both the
 * `securitypolicyviolation` DOM event (fired for every blocked resource,
 * `blob:`/`img:`/`connect` etc.) and the console `error` a blocked
 * `<script>`/stylesheet load also logs. Call before navigating so nothing
 * is missed; read `violations` any time after.
 */
export async function collectCspViolations(page: Page): Promise<{ violations: string[] }> {
  const state = { violations: [] as string[] };

  await page.exposeFunction("__onCspViolation", (detail: string) => {
    state.violations.push(detail);
  });
  await page.addInitScript(() => {
    document.addEventListener("securitypolicyviolation", (event) => {
      const e = event as SecurityPolicyViolationEvent;
      // @ts-expect-error -- exposed by `exposeFunction` above, not a DOM type.
      window.__onCspViolation(`${e.violatedDirective}: blocked ${e.blockedURI}`);
    });
  });
  page.on("console", (msg) => {
    if (msg.type() === "error" && /content security policy/i.test(msg.text())) {
      state.violations.push(msg.text());
    }
  });

  return state;
}

/** The mock app always auto-authenticates as "Quinn" and defaults to the "Trail Crew" group. */
export async function gotoHome(page: Page): Promise<void> {
  await page.goto("/");
  await page.getByRole("heading", { name: /Hey Quinn/ }).waitFor();
}

/** Home's row for the current group's open edition (`EditionRow.tsx`). */
export async function openCurrentEdition(page: Page): Promise<void> {
  await page.getByRole("link", { name: /you've published/ }).click();
}

/** Home's row for the current group's voting cycle, which links to the candidates page. */
export async function openUpcoming(page: Page): Promise<void> {
  await page.getByRole("link", { name: /Suggest and vote on questions/ }).click();
}
