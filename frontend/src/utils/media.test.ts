import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { invalidateQueries } = vi.hoisted(() => ({ invalidateQueries: vi.fn() }));

vi.mock("../api/queryClient", () => ({
  queryClient: { invalidateQueries },
}));

/**
 * `refreshMediaUrls` throttles itself via module-level state, so each test gets a fresh module
 * instance (`vi.resetModules()` + a dynamic re-import) rather than sharing that state across
 * cases (`coding-standards.md` §1.8: no shared mutable state between tests).
 */
async function freshRefreshMediaUrls() {
  vi.resetModules();
  const mod = await import("./media");
  return mod.refreshMediaUrls;
}

describe("refreshMediaUrls", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    invalidateQueries.mockReset();
    invalidateQueries.mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("refetches only the active queries flagged as carrying media URLs", async () => {
    const refreshMediaUrls = await freshRefreshMediaUrls();
    const { MEDIA_URLS_META } = await import("./media");

    await refreshMediaUrls();

    expect(invalidateQueries).toHaveBeenCalledTimes(1);
    const [{ predicate, refetchType }] = invalidateQueries.mock.calls[0];
    expect(refetchType).toBe("active");
    expect(predicate({ meta: MEDIA_URLS_META })).toBe(true);
    expect(predicate({ meta: undefined })).toBe(false);
  });

  it("does nothing while the browser is offline", async () => {
    const refreshMediaUrls = await freshRefreshMediaUrls();
    const onLine = vi.spyOn(navigator, "onLine", "get").mockReturnValue(false);

    await refreshMediaUrls();

    expect(invalidateQueries).not.toHaveBeenCalled();
    onLine.mockRestore();
  });

  it("shares one in-flight refresh across concurrent callers", async () => {
    const refreshMediaUrls = await freshRefreshMediaUrls();
    let resolveInvalidate!: () => void;
    invalidateQueries.mockReturnValue(
      new Promise<void>((resolve) => {
        resolveInvalidate = resolve;
      }),
    );

    const first = refreshMediaUrls();
    const second = refreshMediaUrls();
    resolveInvalidate();
    await Promise.all([first, second]);

    expect(invalidateQueries).toHaveBeenCalledTimes(1);
  });

  it("throttles to at most once per 10s, then refreshes again after", async () => {
    const refreshMediaUrls = await freshRefreshMediaUrls();

    await refreshMediaUrls();
    expect(invalidateQueries).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(9_999);
    await refreshMediaUrls();
    expect(invalidateQueries).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(1);
    await refreshMediaUrls();
    expect(invalidateQueries).toHaveBeenCalledTimes(2);
  });

  it("resolves without invalidating when a call lands inside the throttle window", async () => {
    const refreshMediaUrls = await freshRefreshMediaUrls();

    await refreshMediaUrls();
    await expect(refreshMediaUrls()).resolves.toBeUndefined();

    expect(invalidateQueries).toHaveBeenCalledTimes(1);
  });
});

describe("thumbSize", () => {
  it("scales a landscape original's long edge down to 400px", async () => {
    const { thumbSize } = await import("./media");

    expect(thumbSize(4032, 3024)).toEqual({ width: 400, height: 300 });
  });

  it("scales a portrait original by its height", async () => {
    const { thumbSize } = await import("./media");

    expect(thumbSize(1000, 2000)).toEqual({ width: 200, height: 400 });
  });

  it("keeps an original already under the thumb size, since variants are never upscaled", async () => {
    const { thumbSize } = await import("./media");

    expect(thumbSize(300, 200)).toEqual({ width: 300, height: 200 });
  });

  it("returns no size when the dimensions aren't known yet", async () => {
    const { thumbSize } = await import("./media");

    expect(thumbSize(null, undefined)).toEqual({});
  });
});
