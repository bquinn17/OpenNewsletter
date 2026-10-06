import { create } from "zustand";

/**
 * Not in `lib.dom.d.ts` (the spec is still a WICG draft). Minimal shape
 * `App.tsx` needs from the `beforeinstallprompt` event
 * (`04-frontend-architecture.md` §14.4).
 */
export interface BeforeInstallPromptEvent extends Event {
  readonly userChoice: Promise<{ outcome: "accepted" | "dismissed"; platform: string }>;
  prompt(): Promise<void>;
}

const IOS_BANNER_DISMISSED_KEY = "on:iosInstallBannerDismissed";

function readIosBannerDismissed(): boolean {
  try {
    return localStorage.getItem(IOS_BANNER_DISMISSED_KEY) === "1";
  } catch {
    return false;
  }
}

type State = {
  /** The captured `beforeinstallprompt` event, or `null` before it fires / after it's used. */
  event: BeforeInstallPromptEvent | null;
  /** Persisted (`localStorage`) so a dismissed iOS "Add to Home Screen" banner stays dismissed. */
  iosBannerDismissed: boolean;
  capture: (event: BeforeInstallPromptEvent) => void;
  /** Shows the native install prompt. Resolves "unavailable" if no event was captured. */
  promptInstall: () => Promise<"accepted" | "dismissed" | "unavailable">;
  dismissIosBanner: () => void;
};

export const useInstallPrompt = create<State>((set, get) => ({
  event: null,
  iosBannerDismissed: readIosBannerDismissed(),
  capture: (event) => set({ event }),
  promptInstall: async () => {
    const { event } = get();
    if (!event) return "unavailable";
    await event.prompt();
    const choice = await event.userChoice;
    set({ event: null });
    return choice.outcome;
  },
  dismissIosBanner: () => {
    try {
      localStorage.setItem(IOS_BANNER_DISMISSED_KEY, "1");
    } catch {
      // Best effort — the banner just reappears next load.
    }
    set({ iosBannerDismissed: true });
  },
}));
