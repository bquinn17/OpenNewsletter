import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";
import { api, ApiError } from "../../api/client";
import { useResponseEditor } from "./useResponseEditor";

vi.mock("../../api/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../api/client")>();
  return { ...actual, api: { saveMyResponse: vi.fn() } };
});

const saveMyResponse = api.saveMyResponse as ReturnType<typeof vi.fn>;

let client: QueryClient;

function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

function responseDto(overrides: Partial<ReturnType<typeof baseResponse>> = {}) {
  return { ...baseResponse(), ...overrides };
}

function baseResponse() {
  return {
    responseId: "r1",
    userId: "u1",
    questionId: "q1",
    groupId: "g1",
    cycleId: "202606",
    kind: "text" as const,
    status: "draft" as const,
    body: "abc",
    imageMediaIds: [],
    updatedAt: "2026-06-01T00:00:00Z",
    publishedAt: null,
  };
}

async function flushMicrotasks(times = 5) {
  for (let i = 0; i < times; i++) await Promise.resolve();
}

describe("useResponseEditor", () => {
  beforeEach(() => {
    client = new QueryClient();
    saveMyResponse.mockReset();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("debounces and saves once after the quiet period", async () => {
    saveMyResponse.mockResolvedValue(responseDto());
    const { result } = renderHook(
      () =>
        useResponseEditor({
          groupId: "g1",
          cycleId: "202606",
          questionId: "q1",
          userId: "u1",
          initial: null,
        }),
      { wrapper },
    );

    act(() => result.current.setBody("a"));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(500);
    });
    act(() => result.current.setBody("ab"));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(500);
    });
    act(() => result.current.setBody("abc"));

    expect(saveMyResponse).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1500);
    });

    expect(saveMyResponse).toHaveBeenCalledTimes(1);
    expect(saveMyResponse).toHaveBeenCalledWith("g1", "202606", "q1", {
      kind: "text",
      body: "abc",
      imageMediaIds: [],
      publish: false,
    });
  });

  it("flushes immediately on blur without waiting for the debounce", async () => {
    saveMyResponse.mockResolvedValue(responseDto());
    const { result } = renderHook(
      () =>
        useResponseEditor({
          groupId: "g1",
          cycleId: "202606",
          questionId: "q1",
          userId: "u1",
          initial: null,
        }),
      { wrapper },
    );

    act(() => result.current.setBody("typed"));
    await act(async () => {
      result.current.onBlur();
      await flushMicrotasks();
    });

    expect(saveMyResponse).toHaveBeenCalledTimes(1);
    expect(saveMyResponse).toHaveBeenCalledWith(
      "g1",
      "202606",
      "q1",
      expect.objectContaining({ body: "typed" }),
    );
  });

  it("does not save on blur, tab-hide or unmount when nothing changed", async () => {
    const { result, unmount } = renderHook(
      () =>
        useResponseEditor({
          groupId: "g1",
          cycleId: "202606",
          questionId: "q1",
          userId: "u1",
          initial: null,
        }),
      { wrapper },
    );

    await act(async () => {
      result.current.onBlur();
      Object.defineProperty(document, "visibilityState", { value: "hidden", configurable: true });
      document.dispatchEvent(new Event("visibilitychange"));
      await flushMicrotasks();
    });
    Object.defineProperty(document, "visibilityState", { value: "visible", configurable: true });
    unmount();
    await flushMicrotasks();

    expect(saveMyResponse).not.toHaveBeenCalled();
  });

  it("coalesces an edit made while a save is in flight into exactly one follow-up save", async () => {
    let resolveFirst!: (value: ReturnType<typeof responseDto>) => void;
    saveMyResponse.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveFirst = resolve;
        }),
    );
    saveMyResponse.mockResolvedValueOnce(responseDto({ body: "second" }));

    const { result } = renderHook(
      () =>
        useResponseEditor({
          groupId: "g1",
          cycleId: "202606",
          questionId: "q1",
          userId: "u1",
          initial: null,
        }),
      { wrapper },
    );

    act(() => result.current.setBody("first"));
    await act(async () => {
      result.current.onBlur();
      await flushMicrotasks();
    });
    expect(saveMyResponse).toHaveBeenCalledTimes(1);

    // Edit arrives while the first save is still in flight.
    act(() => result.current.setBody("second"));
    await act(async () => {
      result.current.onBlur();
      await flushMicrotasks();
    });
    // Still just the one in-flight request — the edit was queued, not fired.
    expect(saveMyResponse).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveFirst(responseDto({ body: "first" }));
      await flushMicrotasks();
    });

    expect(saveMyResponse).toHaveBeenCalledTimes(2);
    expect(saveMyResponse).toHaveBeenLastCalledWith(
      "g1",
      "202606",
      "q1",
      expect.objectContaining({ body: "second" }),
    );
  });

  it("retries with exponential backoff on a 5xx and eventually succeeds", async () => {
    saveMyResponse
      .mockRejectedValueOnce(new ApiError(500, "INTERNAL", null))
      .mockResolvedValueOnce(responseDto({ body: "x" }));

    const { result } = renderHook(
      () =>
        useResponseEditor({
          groupId: "g1",
          cycleId: "202606",
          questionId: "q1",
          userId: "u1",
          initial: null,
        }),
      { wrapper },
    );

    act(() => result.current.setBody("x"));
    await act(async () => {
      result.current.onBlur();
      await flushMicrotasks();
    });
    expect(saveMyResponse).toHaveBeenCalledTimes(1);
    expect(result.current.status.kind).toBe("retrying");

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
      await flushMicrotasks();
    });

    expect(saveMyResponse).toHaveBeenCalledTimes(2);
    expect(result.current.status.kind).toBe("saved");
  });

  it("stops retrying and goes read-only on CYCLE_NOT_OPEN", async () => {
    saveMyResponse.mockRejectedValue(new ApiError(409, "CYCLE_NOT_OPEN", null));

    const { result } = renderHook(
      () =>
        useResponseEditor({
          groupId: "g1",
          cycleId: "202606",
          questionId: "q1",
          userId: "u1",
          initial: null,
        }),
      { wrapper },
    );

    act(() => result.current.setBody("x"));
    await act(async () => {
      result.current.onBlur();
      await flushMicrotasks();
    });

    expect(result.current.status.kind).toBe("closed");
    expect(result.current.readOnly).toBe(true);

    saveMyResponse.mockClear();
    act(() => result.current.setBody("y"));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
      await flushMicrotasks();
    });
    expect(saveMyResponse).not.toHaveBeenCalled();
  });

  it("mirrors local edits into localStorage and clears them after a successful save", async () => {
    saveMyResponse.mockResolvedValue(responseDto());
    const key = "draft:g1/202606/q1/u1";

    const { result } = renderHook(
      () =>
        useResponseEditor({
          groupId: "g1",
          cycleId: "202606",
          questionId: "q1",
          userId: "u1",
          initial: null,
        }),
      { wrapper },
    );

    act(() => result.current.setBody("draft text"));
    await flushMicrotasks();
    expect(JSON.parse(localStorage.getItem(key)!)).toEqual({
      body: "draft text",
      imageMediaIds: [],
    });

    await act(async () => {
      result.current.onBlur();
      await flushMicrotasks();
    });

    expect(localStorage.getItem(key)).toBeNull();
  });

  it("restores a localStorage draft that differs from the server on mount", async () => {
    const key = "draft:g1/202606/q1/u1";
    localStorage.setItem(key, JSON.stringify({ body: "recovered", imageMediaIds: [] }));

    const { result } = renderHook(
      () =>
        useResponseEditor({
          groupId: "g1",
          cycleId: "202606",
          questionId: "q1",
          userId: "u1",
          initial: {
            responseId: "r1",
            status: "draft",
            body: "server body",
            imageMediaIds: [],
            updatedAt: "2026-06-01T00:00:00Z",
            publishedAt: null,
          },
        }),
      { wrapper },
    );

    expect(result.current.body).toBe("recovered");
    expect(result.current.status.kind).toBe("dirty");
  });
});
