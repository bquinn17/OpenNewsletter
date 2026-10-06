/**
 * Turns a stored `userAgent` string into a short "Browser · OS" label for the
 * Settings "My devices" list (`07-notifications.md` §11.1). Best-effort —
 * an unrecognized UA just falls back to a truncated raw string.
 */
export function summarizeUserAgent(userAgent: string): string {
  if (!userAgent) return "Unknown device";

  const browser = (() => {
    if (/edg\//i.test(userAgent)) return "Edge";
    if (/firefox\//i.test(userAgent)) return "Firefox";
    if (/crios\//i.test(userAgent)) return "Chrome";
    if (/chrome\//i.test(userAgent)) return "Chrome";
    if (/version\/.*safari/i.test(userAgent) || /safari\//i.test(userAgent)) return "Safari";
    return null;
  })();

  const os = (() => {
    if (/iphone|ipad|ipod/i.test(userAgent)) return "iOS";
    if (/android/i.test(userAgent)) return "Android";
    if (/mac os x/i.test(userAgent)) return "macOS";
    if (/windows/i.test(userAgent)) return "Windows";
    if (/linux/i.test(userAgent)) return "Linux";
    return null;
  })();

  if (browser && os) return `${browser} · ${os}`;
  if (browser) return browser;
  if (os) return os;
  return userAgent.length > 40 ? `${userAgent.slice(0, 40)}…` : userAgent;
}
