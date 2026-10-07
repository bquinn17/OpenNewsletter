import { describe, expect, it } from "vitest";
import { inviteUrl } from "./inviteUrl";

describe("inviteUrl", () => {
  it("joins origin, base, and the join path with exactly one slash when base is root", () => {
    expect(inviteUrl("ABCD-EFGH-JKMN-PQRS", "/", "https://example.github.io")).toBe(
      "https://example.github.io/join?code=ABCD-EFGH-JKMN-PQRS",
    );
  });

  it("joins correctly when base is a sub-path (GitHub Pages project site)", () => {
    expect(inviteUrl("ABCD-EFGH-JKMN-PQRS", "/OpenNewsletter/", "https://example.github.io")).toBe(
      "https://example.github.io/OpenNewsletter/join?code=ABCD-EFGH-JKMN-PQRS",
    );
  });

  it("normalizes a base missing its trailing slash", () => {
    expect(inviteUrl("ABCD-EFGH-JKMN-PQRS", "/OpenNewsletter", "https://example.github.io")).toBe(
      "https://example.github.io/OpenNewsletter/join?code=ABCD-EFGH-JKMN-PQRS",
    );
  });

  it("URL-encodes the code", () => {
    expect(inviteUrl("AB CD/EF", "/", "https://example.github.io")).toBe(
      "https://example.github.io/join?code=AB%20CD%2FEF",
    );
  });

  it("defaults to window.location.origin and import.meta.env.BASE_URL", () => {
    expect(inviteUrl("ABCD-EFGH-JKMN-PQRS")).toBe(
      `${window.location.origin}${import.meta.env.BASE_URL}join?code=ABCD-EFGH-JKMN-PQRS`,
    );
  });
});
