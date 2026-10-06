import { describe, expect, it } from "vitest";
import { buildNotificationOptions, parsePushPayload, resolveNotificationUrl } from "./swHelpers";

describe("parsePushPayload", () => {
  it("passes through a well-formed payload", () => {
    const payload = parsePushPayload({
      title: "Trail Crew: time to write",
      body: "5 questions are waiting.",
      url: "/g/g1/n/202610",
      tag: "g1:202610:cycle_open",
      groupName: "Trail Crew",
    });
    expect(payload).toEqual({
      title: "Trail Crew: time to write",
      body: "5 questions are waiting.",
      url: "/g/g1/n/202610",
      tag: "g1:202610:cycle_open",
      groupName: "Trail Crew",
    });
  });

  it("falls back to a default title and drops non-string fields", () => {
    expect(parsePushPayload({ title: 42, body: "ok" })).toEqual({
      title: "OpenNewsletter",
      body: "ok",
      url: undefined,
      tag: undefined,
      groupName: undefined,
    });
  });

  it("handles null, undefined, and non-object payloads", () => {
    expect(parsePushPayload(null)).toEqual({ title: "OpenNewsletter" });
    expect(parsePushPayload(undefined)).toEqual({ title: "OpenNewsletter" });
    expect(parsePushPayload("not an object")).toEqual({ title: "OpenNewsletter" });
  });

  it("treats an empty-string title as missing", () => {
    expect(parsePushPayload({ title: "" }).title).toBe("OpenNewsletter");
  });
});

describe("buildNotificationOptions", () => {
  it("builds the showNotification options from a payload", () => {
    const options = buildNotificationOptions({
      title: "t",
      body: "b",
      url: "/g/g1/n/202610",
      tag: "g1:202610:cycle_open",
    });
    expect(options).toEqual({
      body: "b",
      icon: "/icon-192.png",
      badge: "/badge-96.png",
      tag: "g1:202610:cycle_open",
      data: { url: "/g/g1/n/202610" },
    });
  });

  it("defaults the notification's url to / when the payload has none", () => {
    const options = buildNotificationOptions({ title: "t" });
    expect(options.data).toEqual({ url: "/" });
  });
});

describe("resolveNotificationUrl", () => {
  it("resolves a root-relative url against the given origin", () => {
    expect(resolveNotificationUrl("/g/g1/n/202610", "https://app.example.com")).toBe(
      "https://app.example.com/g/g1/n/202610",
    );
  });

  it("defaults to / when url is undefined", () => {
    expect(resolveNotificationUrl(undefined, "https://app.example.com")).toBe(
      "https://app.example.com/",
    );
  });

  it("falls back to the bare origin on a malformed url", () => {
    // A string that itself parses as an (invalid) absolute URL makes `new
    // URL(x, base)` ignore `base` entirely and throw on `x`'s own malformed
    // IPv6 host — the catch branch returns the origin verbatim (no path).
    expect(resolveNotificationUrl("http://[::1", "https://app.example.com")).toBe(
      "https://app.example.com",
    );
  });
});
