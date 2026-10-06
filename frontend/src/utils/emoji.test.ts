import { describe, expect, it } from "vitest";
import { firstGrapheme, isValidEmoji, normalizeEmoji } from "./emoji";

describe("isValidEmoji", () => {
  it("accepts a single simple emoji", () => {
    expect(isValidEmoji("🔥")).toBe(true);
  });

  it("accepts a ZWJ family sequence (7 codepoints)", () => {
    expect(isValidEmoji("👨‍👩‍👧‍👦")).toBe(true);
  });

  it("accepts a two-codepoint flag (regional indicators)", () => {
    expect(isValidEmoji("🇺🇸")).toBe(true);
  });

  it("rejects an empty string", () => {
    expect(isValidEmoji("")).toBe(false);
  });

  it("rejects plain ASCII letters", () => {
    expect(isValidEmoji("abc")).toBe(false);
  });

  it("rejects digits", () => {
    expect(isValidEmoji("123")).toBe(false);
  });

  it("rejects more than 12 codepoints", () => {
    expect(isValidEmoji("🔥".repeat(13))).toBe(false);
  });

  it("accepts up to 12 codepoints", () => {
    expect(isValidEmoji("🔥".repeat(12))).toBe(true);
  });
});

describe("normalizeEmoji", () => {
  it("NFC-normalizes the input", () => {
    expect(normalizeEmoji("🔥")).toBe("🔥".normalize("NFC"));
  });
});

describe("firstGrapheme", () => {
  it("returns an empty string for empty input", () => {
    expect(firstGrapheme("")).toBe("");
  });

  it("returns the whole family emoji as a single grapheme", () => {
    expect(firstGrapheme("👨‍👩‍👧‍👦")).toBe("👨‍👩‍👧‍👦");
  });

  it("returns the whole flag as a single grapheme", () => {
    expect(firstGrapheme("🇺🇸")).toBe("🇺🇸");
  });

  it("drops everything after the first grapheme", () => {
    expect(firstGrapheme("🔥🤣")).toBe("🔥");
  });

  it("keeps a plain ASCII character as a fallback grapheme", () => {
    expect(firstGrapheme("abc")).toBe("a");
  });
});
