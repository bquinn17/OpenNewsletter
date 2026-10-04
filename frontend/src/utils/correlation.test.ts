import { describe, expect, it } from "vitest";
import { newCorrelationId } from "./correlation";

const CROCKFORD_PATTERN = /^[0-9A-HJKMNP-TV-Z]{26}$/;

describe("newCorrelationId", () => {
  it("returns a 26-character Crockford base32 string", () => {
    const id = newCorrelationId();
    expect(id).toHaveLength(26);
    expect(id).toMatch(CROCKFORD_PATTERN);
  });

  it("produces a non-decreasing time prefix across successive calls", () => {
    const first = newCorrelationId().slice(0, 10);
    const second = newCorrelationId().slice(0, 10);
    expect(second >= first).toBe(true);
  });

  it("varies the random suffix across calls", () => {
    const a = newCorrelationId();
    const b = newCorrelationId();
    expect(a).not.toBe(b);
  });
});
