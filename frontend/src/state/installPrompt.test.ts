import { afterEach, describe, expect, it, vi } from "vitest";
import { useInstallPrompt, type BeforeInstallPromptEvent } from "./installPrompt";

function fakeEvent(outcome: "accepted" | "dismissed"): BeforeInstallPromptEvent {
  return {
    prompt: vi.fn().mockResolvedValue(undefined),
    userChoice: Promise.resolve({ outcome, platform: "web" }),
  } as unknown as BeforeInstallPromptEvent;
}

afterEach(() => {
  localStorage.clear();
  useInstallPrompt.setState({ event: null, iosBannerDismissed: false });
});

describe("useInstallPrompt", () => {
  it("starts with no captured event", () => {
    expect(useInstallPrompt.getState().event).toBeNull();
  });

  it("capture stores the beforeinstallprompt event", () => {
    const event = fakeEvent("accepted");
    useInstallPrompt.getState().capture(event);
    expect(useInstallPrompt.getState().event).toBe(event);
  });

  it("promptInstall resolves unavailable when nothing was captured", async () => {
    await expect(useInstallPrompt.getState().promptInstall()).resolves.toBe("unavailable");
  });

  it("promptInstall calls prompt(), awaits userChoice, and clears the event", async () => {
    const event = fakeEvent("accepted");
    useInstallPrompt.getState().capture(event);

    const outcome = await useInstallPrompt.getState().promptInstall();

    expect(event.prompt).toHaveBeenCalledTimes(1);
    expect(outcome).toBe("accepted");
    expect(useInstallPrompt.getState().event).toBeNull();
  });

  it("promptInstall reports a dismissed choice", async () => {
    useInstallPrompt.getState().capture(fakeEvent("dismissed"));
    await expect(useInstallPrompt.getState().promptInstall()).resolves.toBe("dismissed");
  });

  it("dismissIosBanner flips the flag and persists it to localStorage", () => {
    expect(useInstallPrompt.getState().iosBannerDismissed).toBe(false);

    useInstallPrompt.getState().dismissIosBanner();

    expect(useInstallPrompt.getState().iosBannerDismissed).toBe(true);
    expect(localStorage.getItem("on:iosInstallBannerDismissed")).toBe("1");
  });
});
