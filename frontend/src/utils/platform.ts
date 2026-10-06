/**
 * iOS/standalone-display detection, shared by the install-prompt banner
 * (`04-frontend-architecture.md` §14.4) and the Settings notifications
 * section's "add to Home Screen" copy (`07-notifications.md` §14, M11 D11).
 * iOS Safari doesn't fire `beforeinstallprompt` and (pre-install) doesn't
 * expose the Push API at all, so both surfaces need to tell "iOS, not yet
 * installed" apart from "unsupported browser".
 */

export function isIOS(): boolean {
  if (typeof navigator === "undefined") return false;
  const ua = navigator.userAgent;
  const isAppleMobile = /iphone|ipad|ipod/i.test(ua);
  // iPadOS 13+ reports as "MacIntel" in the UA string; touch support is the
  // tell that it's actually an iPad, not a real Mac.
  const isIpadOs = navigator.platform === "MacIntel" && navigator.maxTouchPoints > 1;
  return isAppleMobile || isIpadOs;
}

export function isStandaloneDisplay(): boolean {
  if (typeof window === "undefined") return false;
  const iosStandalone = (navigator as Navigator & { standalone?: boolean }).standalone;
  return (
    window.matchMedia?.("(display-mode: standalone)").matches === true || iosStandalone === true
  );
}
