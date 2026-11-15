import { describe, expect, it } from "vitest";
import { safeReturnTo } from "./returnTo";

describe("safeReturnTo", () => {
  it("keeps an in-app path with its query", () => {
    expect(safeReturnTo("/g/g_trail/upcoming?tab=vote")).toBe("/g/g_trail/upcoming?tab=vote");
  });

  it.each([null, undefined, "", "https://evil.example", "//evil.example", "/\\evil.example", "g"])(
    "sends %j Home instead",
    (value) => {
      expect(safeReturnTo(value)).toBe("/");
    },
  );
});
