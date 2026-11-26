import { afterEach, describe, expect, it, vi } from "vitest";
import {
  GIPHY_COOLDOWN_MS,
  giphyLimitRemainingMs,
  isGiphyRateLimited,
  useGiphyLimit,
} from "./giphyLimit";

describe("giphyLimit", () => {
  afterEach(() => {
    useGiphyLimit.getState().clear();
    vi.useRealTimers();
  });

  it("starts with no cooldown active", () => {
    expect(isGiphyRateLimited()).toBe(false);
    expect(giphyLimitRemainingMs()).toBe(0);
  });

  it("marks the shared cooldown active for the full window on a 429, from either caller", () => {
    useGiphyLimit.getState().markRateLimited();
    expect(isGiphyRateLimited()).toBe(true);
    expect(giphyLimitRemainingMs()).toBeGreaterThan(GIPHY_COOLDOWN_MS - 1000);
  });

  it("re-enables after the cooldown elapses, without anyone calling clear", () => {
    vi.useFakeTimers();
    useGiphyLimit.getState().markRateLimited();
    expect(isGiphyRateLimited()).toBe(true);

    vi.advanceTimersByTime(GIPHY_COOLDOWN_MS + 1);

    expect(isGiphyRateLimited()).toBe(false);
    expect(giphyLimitRemainingMs()).toBe(0);
  });

  it("a later 429 restarts the cooldown", () => {
    vi.useFakeTimers();
    useGiphyLimit.getState().markRateLimited();
    vi.advanceTimersByTime(GIPHY_COOLDOWN_MS + 1);
    expect(isGiphyRateLimited()).toBe(false);

    useGiphyLimit.getState().markRateLimited();
    expect(isGiphyRateLimited()).toBe(true);
  });
});
