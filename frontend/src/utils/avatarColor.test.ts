import { describe, expect, it } from "vitest";
import { AVATAR_COLOR_SLUGS, avatarColorClass, avatarColorClassForName } from "./avatarColor";

describe("avatarColorClassForName", () => {
  it("is stable for the same name", () => {
    expect(avatarColorClassForName("Quinn Baker")).toBe(avatarColorClassForName("Quinn Baker"));
  });

  it("always returns one of the eight avatar color classes", () => {
    const allClasses = new Set(AVATAR_COLOR_SLUGS.map(avatarColorClass));
    for (const name of ["Alice", "Bob", "Charlie", "", "😀", "A very long display name indeed"]) {
      expect(allClasses).toContain(avatarColorClassForName(name));
    }
  });

  it("spreads different names across more than one color", () => {
    const names = ["Alice", "Bob", "Charlie", "Dana", "Eve", "Frank", "Grace", "Heidi"];
    const colors = new Set(names.map(avatarColorClassForName));
    expect(colors.size).toBeGreaterThan(1);
  });
});
