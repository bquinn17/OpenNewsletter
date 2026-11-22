import { describe, expect, it } from "vitest";
import { readingTimeMinutes } from "./readingTime";

describe("readingTimeMinutes", () => {
  it("rounds up to the nearest minute at 200 words a minute", () => {
    const body = Array(201).fill("word").join(" ");
    expect(readingTimeMinutes([body])).toBe(2);
  });

  it("returns at least 1 minute with no words", () => {
    expect(readingTimeMinutes([])).toBe(1);
    expect(readingTimeMinutes([""])).toBe(1);
  });

  it("sums words across every answer body", () => {
    const hundredWords = Array(100).fill("word").join(" ");
    expect(readingTimeMinutes([hundredWords, hundredWords])).toBe(1);
    expect(readingTimeMinutes([hundredWords, hundredWords, hundredWords])).toBe(2);
  });

  it("strips image tokens before counting words", () => {
    expect(readingTimeMinutes(["![alt](image:abc) one two three"])).toBe(1);
  });
});
