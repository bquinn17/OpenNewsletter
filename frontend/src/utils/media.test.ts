import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api/client", () => ({
  api: { media: { getMediaCookie: vi.fn() } },
}));

import { api } from "../api/client";
import { env } from "../env";
import { ensureCookie, refreshMediaAuth, resetMediaAuthCache, withMediaAuth } from "./media";

const getMediaCookie = api.media.getMediaCookie as ReturnType<typeof vi.fn>;

function cookie(
  overrides: Partial<{ policy: string; signature: string; keyPairId: string }> = {},
  expiresInMs = 60 * 60_000,
) {
  return {
    policy: overrides.policy ?? "p",
    signature: overrides.signature ?? "s",
    keyPairId: overrides.keyPairId ?? "k",
    expiresAt: new Date(Date.now() + expiresInMs).toISOString(),
  };
}

describe("ensureCookie / withMediaAuth", () => {
  const originalAppEnv = env.appEnv;
  const originalUseMocks = env.useMocks;

  beforeEach(() => {
    resetMediaAuthCache();
    getMediaCookie.mockReset();
    env.appEnv = "dev";
    env.useMocks = false;
  });

  afterEach(() => {
    env.appEnv = originalAppEnv;
    env.useMocks = originalUseMocks;
  });

  it("reuses the cached cookie until 5 minutes before expiry", async () => {
    getMediaCookie.mockResolvedValue(cookie());

    await ensureCookie("g1");
    await ensureCookie("g1");

    expect(getMediaCookie).toHaveBeenCalledTimes(1);
  });

  it("refetches once the cached cookie is within 5 minutes of expiry", async () => {
    getMediaCookie.mockResolvedValueOnce(cookie({}, 4 * 60_000));
    getMediaCookie.mockResolvedValueOnce(cookie({}, 60 * 60_000));

    await ensureCookie("g1");
    await ensureCookie("g1");

    expect(getMediaCookie).toHaveBeenCalledTimes(2);
  });

  it("shares one in-flight request across concurrent callers for the same group", async () => {
    let resolveFetch!: (value: ReturnType<typeof cookie>) => void;
    getMediaCookie.mockReturnValue(
      new Promise((resolve) => {
        resolveFetch = resolve;
      }),
    );

    const first = ensureCookie("g1");
    const second = ensureCookie("g1");
    resolveFetch(cookie());
    await Promise.all([first, second]);

    expect(getMediaCookie).toHaveBeenCalledTimes(1);
  });

  it("fetches separately per group", async () => {
    getMediaCookie.mockResolvedValue(cookie());

    await ensureCookie("g1");
    await ensureCookie("g2");

    expect(getMediaCookie).toHaveBeenCalledTimes(2);
  });

  it("is a no-op in mock mode", async () => {
    env.useMocks = true;

    await ensureCookie("g1");

    expect(getMediaCookie).not.toHaveBeenCalled();
  });

  it("appends CloudFront signed-URL query params in dev for a cached group", async () => {
    getMediaCookie.mockResolvedValue(cookie({ policy: "POL", signature: "SIG", keyPairId: "KEY" }));
    await ensureCookie("g1");

    const url = withMediaAuth(`${env.cdnBaseUrl}/img/g1/202606/q1/u1/img1/thumb.webp`);

    expect(url).toContain("Policy=POL");
    expect(url).toContain("Signature=SIG");
    expect(url).toContain("Key-Pair-Id=KEY");
  });

  it("passes through unchanged in prod even with a cached cookie", async () => {
    getMediaCookie.mockResolvedValue(cookie());
    await ensureCookie("g1");
    env.appEnv = "prod";

    const url = `${env.cdnBaseUrl}/img/g1/202606/q1/u1/img1/thumb.webp`;

    expect(withMediaAuth(url)).toBe(url);
  });

  it("passes through a group with no cached cookie", () => {
    const url = `${env.cdnBaseUrl}/img/unknown-group/202606/q1/u1/img1/thumb.webp`;

    expect(withMediaAuth(url)).toBe(url);
  });

  it("passes through unsigned avatar URLs regardless of cache state", async () => {
    getMediaCookie.mockResolvedValue(cookie());
    await ensureCookie("g1");
    const url = `${env.cdnBaseUrl}/avatar/a1/display.webp`;

    expect(withMediaAuth(url)).toBe(url);
  });

  it("refreshMediaAuth refetches even when the cached cookie is still fresh", async () => {
    getMediaCookie.mockResolvedValue(cookie());
    await ensureCookie("g1");

    await refreshMediaAuth("g1");

    expect(getMediaCookie).toHaveBeenCalledTimes(2);
  });
});
