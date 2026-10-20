import { describe, expect, it, vi } from "vitest";
import {
  activatePromptly,
  buildNotificationOptions,
  mediaCacheKey,
  parsePushPayload,
  resolveNotificationUrl,
  type ActivationScope,
} from "./swHelpers";

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

  it("falls back to the bare origin for an off-origin url", () => {
    for (const url of ["https://evil.example/x", "//evil.example/x", "javascript:alert(1)"]) {
      expect(resolveNotificationUrl(url, "https://app.example.com")).toBe(
        "https://app.example.com",
      );
    }
  });
});

describe("activatePromptly (D6)", () => {
  function fakeScope() {
    const listeners = new Map<string, (event: { waitUntil(p: Promise<unknown>): void }) => void>();
    const scope: ActivationScope = {
      skipWaiting: vi.fn().mockResolvedValue(undefined),
      clients: { claim: vi.fn().mockResolvedValue(undefined) },
      addEventListener: (type, listener) => listeners.set(type, listener),
    };
    function fire(type: string): Promise<unknown>[] {
      const waited: Promise<unknown>[] = [];
      listeners.get(type)?.({ waitUntil: (p) => waited.push(p) });
      return waited;
    }
    return { scope, fire };
  }

  it("skips waiting on install, so a new deploy doesn't wait for every tab to close", () => {
    const { scope, fire } = fakeScope();
    activatePromptly(scope);

    expect(fire("install")).toHaveLength(1);
    expect(scope.skipWaiting).toHaveBeenCalledTimes(1);
    expect(scope.clients.claim).not.toHaveBeenCalled();
  });

  it("claims open clients on activate", () => {
    const { scope, fire } = fakeScope();
    activatePromptly(scope);

    expect(fire("activate")).toHaveLength(1);
    expect(scope.clients.claim).toHaveBeenCalledTimes(1);
  });
});

describe("mediaCacheKey", () => {
  it("drops every query param, including the X-Amz-* presign params and x-id", () => {
    expect(
      mediaCacheKey(
        "https://media.example.com/img/g1/c/q/u/i/thumb.webp?X-Amz-Algorithm=AWS4-HMAC-SHA256" +
          "&X-Amz-Credential=abc&X-Amz-Date=20261014T000000Z&X-Amz-Expires=3600" +
          "&X-Amz-SignedHeaders=host&X-Amz-Security-Token=tok&X-Amz-Signature=sig&x-id=GetObject",
      ),
    ).toBe("https://media.example.com/img/g1/c/q/u/i/thumb.webp");
  });

  it("leaves an unsigned URL alone", () => {
    expect(mediaCacheKey("https://media.example.com/avatar/a1/display.webp")).toBe(
      "https://media.example.com/avatar/a1/display.webp",
    );
  });

  it("keeps only the origin and path, dropping any other query param too", () => {
    expect(mediaCacheKey("https://media.example.com/img/x.webp?v=2")).toBe(
      "https://media.example.com/img/x.webp",
    );
  });
});
