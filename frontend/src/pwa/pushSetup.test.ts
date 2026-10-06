import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api/client", () => ({
  api: { push: { subscribe: vi.fn(), unsubscribe: vi.fn() } },
}));

import { api } from "../api/client";
import {
  disablePush,
  ensurePushSubscription,
  isPushSupported,
  urlB64ToUint8Array,
} from "./pushSetup";

const mockedApi = vi.mocked(api, { deep: true });

function fakeSubscription(
  applicationServerKey: Uint8Array | null,
  endpoint = "https://push.example/abc",
) {
  return {
    endpoint,
    options: { applicationServerKey },
    unsubscribe: vi.fn().mockResolvedValue(true),
    toJSON: () => ({
      endpoint,
      expirationTime: null,
      keys: { p256dh: "p256dh-value", auth: "auth-value" },
    }),
  };
}

/** Stubs `navigator.serviceWorker`/`PushManager`/`Notification` for one test. */
function stubPushEnvironment(opts: {
  permission?: NotificationPermission;
  requestPermission?: ReturnType<typeof vi.fn>;
  getSubscription?: ReturnType<typeof vi.fn>;
  subscribe?: ReturnType<typeof vi.fn>;
}) {
  const requestPermission = opts.requestPermission ?? vi.fn().mockResolvedValue("granted");
  const getSubscription = opts.getSubscription ?? vi.fn().mockResolvedValue(null);
  const subscribe = opts.subscribe ?? vi.fn();

  class FakeNotification {
    static permission: NotificationPermission = opts.permission ?? "default";
    static requestPermission = requestPermission;
  }
  vi.stubGlobal("Notification", FakeNotification);
  vi.stubGlobal("PushManager", class {});

  const registration = { pushManager: { getSubscription, subscribe } };
  vi.stubGlobal("navigator", {
    ...globalThis.navigator,
    serviceWorker: { ready: Promise.resolve(registration) },
    userAgent: "test-agent",
  });

  return { requestPermission, getSubscription, subscribe, registration };
}

const VAPID_KEY = "AAEC"; // decodes to [0, 1, 2]

afterEach(() => {
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

describe("urlB64ToUint8Array", () => {
  it("decodes a base64url string to its raw bytes", () => {
    expect(Array.from(urlB64ToUint8Array("AAEC"))).toEqual([0, 1, 2]);
  });

  it("round-trips unpadded base64url with - and _ substitutions", () => {
    // base64 "//4=" (bytes [0xff, 0xfe]) becomes unpadded base64url "__4".
    expect(Array.from(urlB64ToUint8Array("__4"))).toEqual([0xff, 0xfe]);
  });
});

describe("isPushSupported", () => {
  it("is false in a plain jsdom environment (no serviceWorker/PushManager/Notification)", () => {
    expect(isPushSupported()).toBe(false);
  });

  it("is true once all three are stubbed in", () => {
    stubPushEnvironment({});
    expect(isPushSupported()).toBe(true);
  });
});

describe("ensurePushSubscription", () => {
  beforeEach(() => {
    mockedApi.push.subscribe.mockResolvedValue({ subscriptionId: "sub1" });
    mockedApi.push.unsubscribe.mockResolvedValue(undefined);
  });

  it("returns unsupported when the browser lacks the Push APIs", async () => {
    const result = await ensurePushSubscription(VAPID_KEY, { prompt: true });
    expect(result).toEqual({ status: "unsupported" });
    expect(mockedApi.push.subscribe).not.toHaveBeenCalled();
  });

  it("denied: prompting when permission is already denied resolves denied without subscribing", async () => {
    const requestPermission = vi.fn().mockResolvedValue("denied");
    stubPushEnvironment({ permission: "denied", requestPermission });

    const result = await ensurePushSubscription(VAPID_KEY, { prompt: true });

    expect(result).toEqual({ status: "denied" });
    expect(requestPermission).toHaveBeenCalledTimes(1);
    expect(mockedApi.push.subscribe).not.toHaveBeenCalled();
  });

  it("prompt flow: requests permission, subscribes, and posts to the server", async () => {
    const sub = fakeSubscription(new Uint8Array([0, 1, 2]));
    const requestPermission = vi.fn().mockResolvedValue("granted");
    const getSubscription = vi.fn().mockResolvedValue(null);
    const subscribe = vi.fn().mockResolvedValue(sub);
    stubPushEnvironment({ permission: "default", requestPermission, getSubscription, subscribe });

    const result = await ensurePushSubscription(VAPID_KEY, { prompt: true });

    expect(requestPermission).toHaveBeenCalledTimes(1);
    expect(subscribe).toHaveBeenCalledWith({
      userVisibleOnly: true,
      applicationServerKey: new Uint8Array([0, 1, 2]),
    });
    expect(mockedApi.push.subscribe).toHaveBeenCalledWith({
      endpoint: "https://push.example/abc",
      expirationTime: null,
      keys: { p256dh: "p256dh-value", auth: "auth-value" },
      userAgent: "test-agent",
    });
    expect(result).toEqual({ status: "subscribed" });
  });

  it("refresh-only on load: prompt:false with permission granted and an existing subscription re-posts it", async () => {
    const sub = fakeSubscription(new Uint8Array([0, 1, 2]));
    const getSubscription = vi.fn().mockResolvedValue(sub);
    const requestPermission = vi.fn();
    stubPushEnvironment({ permission: "granted", requestPermission, getSubscription });

    const result = await ensurePushSubscription(VAPID_KEY, { prompt: false });

    expect(requestPermission).not.toHaveBeenCalled();
    expect(mockedApi.push.subscribe).toHaveBeenCalledTimes(1);
    expect(result).toEqual({ status: "subscribed" });
  });

  it("refresh-only on load: prompt:false with no existing subscription is skipped and never prompts", async () => {
    const requestPermission = vi.fn();
    const getSubscription = vi.fn().mockResolvedValue(null);
    stubPushEnvironment({ permission: "granted", requestPermission, getSubscription });

    const result = await ensurePushSubscription(VAPID_KEY, { prompt: false });

    expect(result).toEqual({ status: "skipped" });
    expect(requestPermission).not.toHaveBeenCalled();
    expect(mockedApi.push.subscribe).not.toHaveBeenCalled();
  });

  it("refresh-only on load: prompt:false with permission not yet granted is skipped without prompting", async () => {
    const requestPermission = vi.fn();
    stubPushEnvironment({ permission: "default", requestPermission });

    const result = await ensurePushSubscription(VAPID_KEY, { prompt: false });

    expect(result).toEqual({ status: "skipped" });
    expect(requestPermission).not.toHaveBeenCalled();
  });

  it("key mismatch: unsubscribes the stale subscription and creates a new one", async () => {
    const staleSub = fakeSubscription(new Uint8Array([9, 9, 9]), "https://push.example/stale");
    const freshSub = fakeSubscription(new Uint8Array([0, 1, 2]), "https://push.example/fresh");
    const getSubscription = vi.fn().mockResolvedValue(staleSub);
    const subscribe = vi.fn().mockResolvedValue(freshSub);
    stubPushEnvironment({ permission: "granted", getSubscription, subscribe });

    const result = await ensurePushSubscription(VAPID_KEY, { prompt: true });

    expect(staleSub.unsubscribe).toHaveBeenCalledTimes(1);
    expect(subscribe).toHaveBeenCalledWith({
      userVisibleOnly: true,
      applicationServerKey: new Uint8Array([0, 1, 2]),
    });
    expect(mockedApi.push.subscribe).toHaveBeenCalledWith(
      expect.objectContaining({ endpoint: "https://push.example/fresh" }),
    );
    expect(result).toEqual({ status: "subscribed" });
  });

  it("reuses a matching existing subscription without resubscribing", async () => {
    const sub = fakeSubscription(new Uint8Array([0, 1, 2]));
    const getSubscription = vi.fn().mockResolvedValue(sub);
    const subscribe = vi.fn();
    stubPushEnvironment({ permission: "granted", getSubscription, subscribe });

    const result = await ensurePushSubscription(VAPID_KEY, { prompt: true });

    expect(subscribe).not.toHaveBeenCalled();
    expect(sub.unsubscribe).not.toHaveBeenCalled();
    expect(result).toEqual({ status: "subscribed" });
  });
});

describe("disablePush", () => {
  it("is a no-op when push isn't supported", async () => {
    await expect(disablePush()).resolves.toBeUndefined();
  });

  it("is a no-op when there is no local subscription", async () => {
    const getSubscription = vi.fn().mockResolvedValue(null);
    stubPushEnvironment({ getSubscription });

    await disablePush();

    expect(mockedApi.push.unsubscribe).not.toHaveBeenCalled();
  });

  it("posts /push/unsubscribe then unsubscribes the local subscription", async () => {
    const sub = fakeSubscription(new Uint8Array([0, 1, 2]), "https://push.example/abc");
    const getSubscription = vi.fn().mockResolvedValue(sub);
    stubPushEnvironment({ getSubscription });
    mockedApi.push.unsubscribe.mockResolvedValue(undefined);

    await disablePush();

    expect(mockedApi.push.unsubscribe).toHaveBeenCalledWith({
      endpoint: "https://push.example/abc",
    });
    expect(sub.unsubscribe).toHaveBeenCalledTimes(1);
  });

  it("still unsubscribes locally even when the server call fails (best-effort)", async () => {
    const sub = fakeSubscription(new Uint8Array([0, 1, 2]));
    const getSubscription = vi.fn().mockResolvedValue(sub);
    stubPushEnvironment({ getSubscription });
    mockedApi.push.unsubscribe.mockRejectedValue(new Error("network error"));

    await expect(disablePush()).resolves.toBeUndefined();
    expect(sub.unsubscribe).toHaveBeenCalledTimes(1);
  });
});
