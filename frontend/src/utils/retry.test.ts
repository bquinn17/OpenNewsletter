import { describe, expect, it } from "vitest";
import { backoffDelayMs } from "./retry";

describe("backoffDelayMs", () => {
  it("doubles the delay each attempt starting at 1s", () => {
    expect(backoffDelayMs(0)).toBe(1000);
    expect(backoffDelayMs(1)).toBe(2000);
    expect(backoffDelayMs(2)).toBe(4000);
    expect(backoffDelayMs(3)).toBe(8000);
    expect(backoffDelayMs(4)).toBe(16_000);
  });

  it("caps at 30s", () => {
    expect(backoffDelayMs(5)).toBe(30_000);
    expect(backoffDelayMs(10)).toBe(30_000);
  });
});
