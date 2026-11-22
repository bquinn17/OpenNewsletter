import { afterEach, describe, expect, it, vi } from "vitest";
import { prefersReducedMotion, withViewTransition } from "./motion";

afterEach(() => {
  vi.unstubAllGlobals();
  // @ts-expect-error -- test-only cleanup of a property tests may have added.
  delete window.matchMedia;
});

describe("prefersReducedMotion", () => {
  it("is false when matchMedia is unavailable (jsdom's default)", () => {
    expect(prefersReducedMotion()).toBe(false);
  });

  it("reflects a matching prefers-reduced-motion query", () => {
    window.matchMedia = vi.fn().mockReturnValue({ matches: true });
    expect(prefersReducedMotion()).toBe(true);
    expect(window.matchMedia).toHaveBeenCalledWith("(prefers-reduced-motion: reduce)");
  });

  it("is false when matchMedia throws", () => {
    window.matchMedia = vi.fn(() => {
      throw new Error("not supported");
    });
    expect(prefersReducedMotion()).toBe(false);
  });
});

describe("withViewTransition", () => {
  it("runs the update directly when startViewTransition is unsupported (jsdom)", () => {
    const update = vi.fn();
    withViewTransition(update);
    expect(update).toHaveBeenCalledTimes(1);
  });

  it("runs the update directly when reduced motion is on, even if startViewTransition exists", () => {
    window.matchMedia = vi.fn().mockReturnValue({ matches: true });
    const startViewTransition = vi.fn();
    document.startViewTransition = startViewTransition;
    const update = vi.fn();

    withViewTransition(update);

    expect(update).toHaveBeenCalledTimes(1);
    expect(startViewTransition).not.toHaveBeenCalled();
    // @ts-expect-error -- test-only cleanup.
    delete document.startViewTransition;
  });

  it("wraps the update in startViewTransition when supported and motion isn't reduced", () => {
    const startViewTransition = vi.fn((cb: () => void) => {
      cb();
      return {} as ViewTransition;
    });
    document.startViewTransition = startViewTransition;
    const update = vi.fn();

    withViewTransition(update);

    expect(startViewTransition).toHaveBeenCalledTimes(1);
    expect(update).toHaveBeenCalledTimes(1);
    // @ts-expect-error -- test-only cleanup.
    delete document.startViewTransition;
  });
});
