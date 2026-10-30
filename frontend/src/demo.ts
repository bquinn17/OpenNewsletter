import { env } from "./env";
import { assignLocation } from "./utils/navigation";

/** Every demo URL lives under this prefix, which is also the router's `basename` in the demo. */
export const DEMO_BASENAME = "/demo";

export function isDemoPath(pathname: string): boolean {
  return pathname === DEMO_BASENAME || pathname.startsWith(`${DEMO_BASENAME}/`);
}

/**
 * Whether this page load is the demo (`docs/design/frontend.md` §17). Read once: entering
 * and leaving the demo are always full page loads, so it never changes while a page is open.
 */
export const isDemo: boolean = isDemoPath(window.location.pathname);

/** The API and auth run on the in-memory mock: a mock build (`VITE_USE_MOCKS`), or the demo. */
export const isMockMode: boolean = env.useMocks || isDemo;

/** A full page load out of the demo, which discards everything it held in memory. */
export function exitDemo(): void {
  assignLocation("/");
}
