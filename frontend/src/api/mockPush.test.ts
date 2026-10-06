import { describe, expect, it } from "vitest";
import type { components } from "../types/api";
import { mockPushRoute } from "./mockPush";
import { NO_MOCK_ROUTE, type MockRequest } from "./mockShared";

type S = components["schemas"];

function req(method: MockRequest["method"], rawPath: string, body?: unknown): MockRequest {
  const [path, queryString] = rawPath.split("?");
  return {
    method,
    rawPath: path!,
    segments: path!.split("/").filter(Boolean),
    query: new URLSearchParams(queryString ?? ""),
    body,
  };
}

const VALID_SUBSCRIBE_BODY: S["PushSubscribeRequest"] = {
  endpoint: "https://fcm.googleapis.com/fcm/send/unit-test-endpoint",
  keys: { p256dh: "p256dh-value", auth: "auth-value" },
  userAgent: "Mozilla/5.0 (Test)",
};

describe("mockPushRoute", () => {
  it("returns NO_MOCK_ROUTE for an unrelated path", () => {
    expect(mockPushRoute(req("GET", "/groups/g_trail/candidate-questions"))).toBe(NO_MOCK_ROUTE);
  });

  describe("POST /push/subscribe", () => {
    it("422s when a required field is missing", () => {
      expect(() => mockPushRoute(req("POST", "/push/subscribe", { keys: { auth: "a" } }))).toThrow(
        expect.objectContaining({ status: 422, code: "VALIDATION_FAILED" }),
      );
    });

    it("stores a subscription and returns a stable subscriptionId", () => {
      const first = mockPushRoute(
        req("POST", "/push/subscribe", VALID_SUBSCRIBE_BODY),
      ) as S["PushSubscribeResponse"];
      expect(first.subscriptionId).toBeTruthy();

      const second = mockPushRoute(
        req("POST", "/push/subscribe", VALID_SUBSCRIBE_BODY),
      ) as S["PushSubscribeResponse"];
      // Same endpoint -> same (upserted) subscriptionId, not a duplicate row.
      expect(second.subscriptionId).toBe(first.subscriptionId);

      const list = mockPushRoute(
        req("GET", "/push/subscriptions"),
      ) as S["PushSubscriptionListResponse"];
      const matches = list.items.filter((i) => i.endpoint === VALID_SUBSCRIBE_BODY.endpoint);
      expect(matches).toHaveLength(1);
    });
  });

  describe("POST /push/unsubscribe", () => {
    it("is idempotent — 204/undefined even for an unknown endpoint", () => {
      expect(
        mockPushRoute(req("POST", "/push/unsubscribe", { endpoint: "https://no-such/endpoint" })),
      ).toBeUndefined();
    });

    it("removes the subscription matching the endpoint", () => {
      mockPushRoute(req("POST", "/push/subscribe", VALID_SUBSCRIBE_BODY));
      mockPushRoute(req("POST", "/push/unsubscribe", { endpoint: VALID_SUBSCRIBE_BODY.endpoint }));

      const list = mockPushRoute(
        req("GET", "/push/subscriptions"),
      ) as S["PushSubscriptionListResponse"];
      expect(list.items.some((i) => i.endpoint === VALID_SUBSCRIBE_BODY.endpoint)).toBe(false);
    });
  });

  describe("GET /push/subscriptions", () => {
    it("never returns keys, and includes the seeded demo device", () => {
      const list = mockPushRoute(
        req("GET", "/push/subscriptions"),
      ) as S["PushSubscriptionListResponse"];
      expect(list.items.length).toBeGreaterThan(0);
      for (const item of list.items) {
        expect(item).not.toHaveProperty("keys");
      }
    });
  });

  describe("POST /push/test", () => {
    it("returns a delivered result per subscription", () => {
      const result = mockPushRoute(req("POST", "/push/test")) as S["PushTestResponse"];
      expect(result.results.length).toBeGreaterThan(0);
      for (const r of result.results) {
        expect(r.outcome).toBe("delivered");
        expect(r.statusCode).toBe(201);
      }
    });
  });

  describe("GET /push/preferences", () => {
    it("returns one entry per fixture group, defaulting missing prefs to true", () => {
      const result = mockPushRoute(
        req("GET", "/push/preferences"),
      ) as S["PushPreferenceListResponse"];
      const groupIds = result.items.map((i) => i.groupId);
      expect(groupIds).toEqual(expect.arrayContaining(["g_trail", "g_game", "g_meeple"]));
    });
  });

  describe("PUT /push/preferences/{groupId}", () => {
    it("403s for a group the caller isn't a member of", () => {
      expect(() =>
        mockPushRoute(
          req("PUT", "/push/preferences/g_nope", {
            cycleOpen: true,
            deadlineReminders: true,
          }),
        ),
      ).toThrow(expect.objectContaining({ status: 403, code: "FORBIDDEN" }));
    });

    it("422s on an unknown field (e.g. publication)", () => {
      expect(() =>
        mockPushRoute(
          req("PUT", "/push/preferences/g_trail", {
            cycleOpen: true,
            deadlineReminders: true,
            publication: true,
          }),
        ),
      ).toThrow(expect.objectContaining({ status: 422, code: "VALIDATION_FAILED" }));
    });

    it("upserts and echoes the stored preference", () => {
      const result = mockPushRoute(
        req("PUT", "/push/preferences/g_trail", { cycleOpen: false, deadlineReminders: true }),
      ) as S["PushPreferenceResponse"];
      expect(result).toEqual({ groupId: "g_trail", cycleOpen: false, deadlineReminders: true });

      const list = mockPushRoute(
        req("GET", "/push/preferences"),
      ) as S["PushPreferenceListResponse"];
      expect(list.items.find((i) => i.groupId === "g_trail")).toEqual(result);
    });
  });
});
