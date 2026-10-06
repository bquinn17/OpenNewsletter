import type { components } from "../types/api";
import { ApiError } from "./client";
import { fail, NO_MOCK_ROUTE, type MockRequest } from "./mockShared";

type S = components["schemas"];

/**
 * Mock routes for `/push/*` (`03-api-contract.md` §10, `07-notifications.md`,
 * M11 D8/D11). Self-contained in-memory fixtures — no real crypto, no real
 * push service. `GET /config`'s `vapidPublicKey` (`mockTransport.ts`) is a
 * placeholder string, so subscribing for real in mock mode may fail the
 * browser's `PushManager.subscribe()` call (not a valid EC point); that's an
 * accepted limitation of the fast offline dev loop.
 */

interface Row {
  subscriptionId: string;
  endpoint: string;
  userAgent: string;
  createdAt: string;
  lastSuccessAt: string | null;
  failureCount: number;
}

const MAX_SUBSCRIPTIONS_PER_USER = 10;

/** One demo device, so the Settings "My devices" list isn't empty in mock mode. */
let subscriptions: Row[] = [
  {
    subscriptionId: "sub_demo_phone",
    endpoint: "https://fcm.googleapis.com/fcm/send/demo-phone-endpoint",
    userAgent: "Mozilla/5.0 (Linux; Android 14; Pixel 8) Chrome/129.0",
    createdAt: new Date(Date.now() - 20 * 24 * 60 * 60 * 1000).toISOString(),
    lastSuccessAt: new Date(Date.now() - 2 * 60 * 60 * 1000).toISOString(),
    failureCount: 0,
  },
];

/** Mirrors the fixture memberships in `mockTransport.ts` (`g_trail`/`g_game`/`g_meeple`). */
const GROUP_IDS = ["g_trail", "g_game", "g_meeple"];

/** `g_game` starts with cycle-open muted, so the per-group toggle fixture isn't all-on. */
let preferences: Record<string, { cycleOpen: boolean; deadlineReminders: boolean }> = {
  g_game: { cycleOpen: false, deadlineReminders: true },
};

function validationFailed(field: string, message: string): never {
  const problem: S["ProblemDetails"] = {
    type: "https://api.opennewsletter.example.com/errors/validation-failed",
    title: "Validation failed",
    status: 422,
    detail: message,
    code: "VALIDATION_FAILED",
    correlationId: "mock-correlation-id",
    fieldErrors: [{ field, code: "INVALID", message }],
  };
  throw new ApiError(422, "VALIDATION_FAILED", problem);
}

/** Deterministic stand-in for the server's `sha256(endpoint)` subscription id. */
function subscriptionIdFor(endpoint: string): string {
  let hash = 2166136261; // FNV-1a offset basis
  for (let i = 0; i < endpoint.length; i++) {
    hash ^= endpoint.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return `sub_${(hash >>> 0).toString(36)}`;
}

function toResponse(row: Row): S["PushSubscriptionResponse"] {
  return {
    subscriptionId: row.subscriptionId,
    endpoint: row.endpoint,
    userAgent: row.userAgent,
    createdAt: row.createdAt,
    lastSuccessAt: row.lastSuccessAt,
    failureCount: row.failureCount,
  };
}

function handleSubscribe(body: unknown): S["PushSubscribeResponse"] {
  // Trusted internal shape — the only caller is our own typed `api.push.subscribe`.
  const request = (body ?? {}) as Partial<S["PushSubscribeRequest"]>;
  if (!request.endpoint) validationFailed("endpoint", "endpoint is required");
  if (!request.keys?.p256dh) validationFailed("keys.p256dh", "p256dh is required");
  if (!request.keys?.auth) validationFailed("keys.auth", "auth is required");

  const subscriptionId = subscriptionIdFor(request.endpoint);
  const now = new Date().toISOString();
  const existingIndex = subscriptions.findIndex((s) => s.subscriptionId === subscriptionId);

  if (existingIndex >= 0) {
    const existing = subscriptions[existingIndex]!;
    subscriptions = subscriptions.map((s, i) =>
      i === existingIndex
        ? { ...existing, userAgent: request.userAgent ?? existing.userAgent, failureCount: 0 }
        : s,
    );
    return { subscriptionId };
  }

  let next = subscriptions;
  if (next.length >= MAX_SUBSCRIPTIONS_PER_USER) {
    // Evict the row least recently successful — null counts oldest, tie -> oldest createdAt (D8).
    const [evict] = [...next].sort((a, b) => {
      const aKey = a.lastSuccessAt ?? "";
      const bKey = b.lastSuccessAt ?? "";
      return aKey !== bKey ? aKey.localeCompare(bKey) : a.createdAt.localeCompare(b.createdAt);
    });
    next = next.filter((s) => s.subscriptionId !== evict!.subscriptionId);
  }
  subscriptions = [
    ...next,
    {
      subscriptionId,
      endpoint: request.endpoint,
      userAgent: request.userAgent ?? "",
      createdAt: now,
      lastSuccessAt: null,
      failureCount: 0,
    },
  ];
  return { subscriptionId };
}

function handleUnsubscribe(body: unknown): undefined {
  // Idempotent per the contract — no error when the endpoint isn't found.
  const request = (body ?? {}) as Partial<S["PushUnsubscribeRequest"]>;
  if (request.endpoint) {
    subscriptions = subscriptions.filter((s) => s.endpoint !== request.endpoint);
  }
  return undefined;
}

function handleListSubscriptions(): S["PushSubscriptionListResponse"] {
  const items = [...subscriptions]
    .sort((a, b) => b.createdAt.localeCompare(a.createdAt))
    .map(toResponse);
  return { items };
}

function handleSendTest(): S["PushTestResponse"] {
  const results = subscriptions.map((row) => {
    row.lastSuccessAt = new Date().toISOString();
    row.failureCount = 0;
    return {
      subscriptionId: row.subscriptionId,
      userAgent: row.userAgent,
      outcome: "delivered" as const,
      statusCode: 201,
    };
  });
  return { results };
}

function handleListPreferences(): S["PushPreferenceListResponse"] {
  const items = GROUP_IDS.map((groupId) => ({
    groupId,
    cycleOpen: preferences[groupId]?.cycleOpen ?? true,
    deadlineReminders: preferences[groupId]?.deadlineReminders ?? true,
  }));
  return { items };
}

function handlePutPreference(groupId: string, body: unknown): S["PushPreferenceResponse"] {
  if (!GROUP_IDS.includes(groupId)) fail(403, "FORBIDDEN", `not a member of ${groupId}`);

  const request = (body ?? {}) as Record<string, unknown>;
  const allowedKeys = new Set(["cycleOpen", "deadlineReminders"]);
  for (const key of Object.keys(request)) {
    if (!allowedKeys.has(key)) validationFailed(key, `unknown field ${key}`);
  }
  if (typeof request.cycleOpen !== "boolean")
    validationFailed("cycleOpen", "cycleOpen is required");
  if (typeof request.deadlineReminders !== "boolean") {
    validationFailed("deadlineReminders", "deadlineReminders is required");
  }

  const pref = { cycleOpen: request.cycleOpen, deadlineReminders: request.deadlineReminders };
  preferences = { ...preferences, [groupId]: pref };
  return { groupId, ...pref };
}

export function mockPushRoute(request: MockRequest): unknown {
  const { method, segments, body } = request;
  if (segments[0] !== "push") return NO_MOCK_ROUTE;

  if (segments.length === 2 && segments[1] === "subscribe" && method === "POST") {
    return handleSubscribe(body);
  }
  if (segments.length === 2 && segments[1] === "unsubscribe" && method === "POST") {
    return handleUnsubscribe(body);
  }
  if (segments.length === 2 && segments[1] === "subscriptions" && method === "GET") {
    return handleListSubscriptions();
  }
  if (segments.length === 2 && segments[1] === "test" && method === "POST") {
    return handleSendTest();
  }
  if (segments.length === 2 && segments[1] === "preferences" && method === "GET") {
    return handleListPreferences();
  }
  if (segments.length === 3 && segments[1] === "preferences" && method === "PUT") {
    return handlePutPreference(decodeURIComponent(segments[2]!), body);
  }

  return NO_MOCK_ROUTE;
}
