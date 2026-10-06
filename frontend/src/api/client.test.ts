import { beforeEach, describe, expect, it, vi } from "vitest";
import type { User } from "oidc-client-ts";

vi.mock("../auth/userManager", () => ({
  userManager: {
    getUser: vi.fn(),
    signinSilent: vi.fn(),
    removeUser: vi.fn(),
  },
}));

import { userManager } from "../auth/userManager";
import { api, ApiError, apiFetch } from "./client";

// The real module is mocked above; this narrows the import back to the shape
// client.ts actually calls, so tests can drive it without a real UserManager.
const mockedManager = userManager as unknown as {
  getUser: ReturnType<typeof vi.fn>;
  signinSilent: ReturnType<typeof vi.fn>;
  removeUser: ReturnType<typeof vi.fn>;
};

function fakeUser(idToken: string): User {
  return { id_token: idToken } as unknown as User;
}

function jsonResponse(status: number, body: unknown, contentType = "application/json"): Response {
  return new Response(JSON.stringify(body), { status, headers: { "content-type": contentType } });
}

describe("apiFetch", () => {
  beforeEach(() => {
    vi.stubGlobal("fetch", vi.fn());
    mockedManager.getUser.mockReset();
    mockedManager.signinSilent.mockReset();
    mockedManager.removeUser.mockReset();
  });

  it("sends the id token as a Bearer header and a fresh correlation id", async () => {
    mockedManager.getUser.mockResolvedValue(fakeUser("id-token-abc"));
    const fetchMock = fetch as unknown as ReturnType<typeof vi.fn>;
    fetchMock.mockResolvedValue(jsonResponse(200, { ok: true }));

    await apiFetch("/me");

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const requestInit = fetchMock.mock.calls[0][1] as { headers: Record<string, string> };
    expect(requestInit.headers.Authorization).toBe("Bearer id-token-abc");
    expect(requestInit.headers["x-correlation-id"]).toMatch(/^[0-9A-HJKMNP-TV-Z]{26}$/);
  });

  it("rejects non-2xx responses with an ApiError carrying the problem code", async () => {
    mockedManager.getUser.mockResolvedValue(null);
    const fetchMock = fetch as unknown as ReturnType<typeof vi.fn>;
    fetchMock.mockResolvedValue(
      jsonResponse(
        422,
        {
          type: "https://api.opennewsletter.example.com/errors/validation-failed",
          title: "Validation failed",
          status: 422,
          detail: "displayName is required",
          code: "VALIDATION_FAILED",
          correlationId: "c1",
        },
        "application/problem+json",
      ),
    );

    const error = await apiFetch("/me", { method: "PATCH", body: {} }).catch((e: unknown) => e);

    expect(error).toBeInstanceOf(ApiError);
    expect(error).toMatchObject({ status: 422, code: "VALIDATION_FAILED" });
  });

  it("resolves 204 responses as undefined", async () => {
    mockedManager.getUser.mockResolvedValue(null);
    const fetchMock = fetch as unknown as ReturnType<typeof vi.fn>;
    fetchMock.mockResolvedValue(new Response(null, { status: 204 }));

    await expect(apiFetch("/groups/g1/members/u1", { method: "DELETE" })).resolves.toBeUndefined();
  });

  it("retries once via signinSilent after a 401, then succeeds with the renewed token", async () => {
    mockedManager.getUser.mockResolvedValue(fakeUser("stale-token"));
    mockedManager.signinSilent.mockResolvedValue(fakeUser("renewed-token"));
    const fetchMock = fetch as unknown as ReturnType<typeof vi.fn>;
    fetchMock
      .mockResolvedValueOnce(
        jsonResponse(
          401,
          {
            type: "t",
            title: "Unauthenticated",
            status: 401,
            detail: "expired",
            code: "UNAUTHENTICATED",
            correlationId: "c1",
          },
          "application/problem+json",
        ),
      )
      .mockResolvedValueOnce(jsonResponse(200, { userId: "u1" }));

    const result = await apiFetch("/me");

    expect(result).toEqual({ userId: "u1" });
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(mockedManager.signinSilent).toHaveBeenCalledTimes(1);
    const retryInit = fetchMock.mock.calls[1][1] as { headers: Record<string, string> };
    expect(retryInit.headers.Authorization).toBe("Bearer renewed-token");
  });

  it("drops the user when silent renew also fails after a 401", async () => {
    mockedManager.getUser.mockResolvedValue(fakeUser("stale-token"));
    mockedManager.signinSilent.mockResolvedValue(null);
    const fetchMock = fetch as unknown as ReturnType<typeof vi.fn>;
    fetchMock.mockResolvedValue(
      jsonResponse(
        401,
        {
          type: "t",
          title: "Unauthenticated",
          status: 401,
          detail: "expired",
          code: "UNAUTHENTICATED",
          correlationId: "c1",
        },
        "application/problem+json",
      ),
    );

    await expect(apiFetch("/me")).rejects.toMatchObject({ status: 401, code: "UNAUTHENTICATED" });
    expect(mockedManager.removeUser).toHaveBeenCalledTimes(1);
  });
});

describe("api.media", () => {
  beforeEach(() => {
    vi.stubGlobal("fetch", vi.fn());
    mockedManager.getUser.mockResolvedValue(null);
  });

  it("sends groupId and cycleId as query params for /uploads/{imageId} routes", async () => {
    const fetchMock = fetch as unknown as ReturnType<typeof vi.fn>;
    fetchMock.mockResolvedValue(jsonResponse(200, {}));

    await api.media.getUpload("img1", "g1", "202606");

    const url = fetchMock.mock.calls[0][0] as string;
    expect(url).toContain("/uploads/img1?");
    expect(url).toContain("groupId=g1");
    expect(url).toContain("cycleId=202606");
  });

  it("sends a DELETE with groupId/cycleId query params for deleteUpload", async () => {
    const fetchMock = fetch as unknown as ReturnType<typeof vi.fn>;
    fetchMock.mockResolvedValue(new Response(null, { status: 204 }));

    await api.media.deleteUpload("img1", "g1", "202606");

    const [url, requestInit] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toContain("groupId=g1");
    expect(url).toContain("cycleId=202606");
    expect(requestInit.method).toBe("DELETE");
  });

  it("calls getMediaCookie with credentials: include", async () => {
    const fetchMock = fetch as unknown as ReturnType<typeof vi.fn>;
    fetchMock.mockResolvedValue(
      jsonResponse(200, {
        policy: "p",
        signature: "s",
        keyPairId: "k",
        expiresAt: "2026-01-01T00:00:00Z",
      }),
    );

    await api.media.getMediaCookie("g1");

    const [url, requestInit] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toContain("/media-cookie?groupId=g1");
    expect(requestInit.credentials).toBe("include");
  });
});

describe("api.engagement", () => {
  beforeEach(() => {
    vi.stubGlobal("fetch", vi.fn());
    mockedManager.getUser.mockResolvedValue(null);
  });

  function basePath(): string {
    return "/groups/g1/newsletters/202606/questions/q1/responses/r1";
  }

  it("builds the comments list URL with limit and cursor", async () => {
    const fetchMock = fetch as unknown as ReturnType<typeof vi.fn>;
    fetchMock.mockResolvedValue(jsonResponse(200, { items: [], nextCursor: null }));

    await api.engagement.listComments("g1", "202606", "q1", "r1", { limit: 10, cursor: "c1" });

    const url = fetchMock.mock.calls[0]![0] as string;
    expect(url).toContain(`${basePath()}/comments?`);
    expect(url).toContain("limit=10");
    expect(url).toContain("cursor=c1");
  });

  it("POSTs a new comment to the comments collection", async () => {
    const fetchMock = fetch as unknown as ReturnType<typeof vi.fn>;
    fetchMock.mockResolvedValue(jsonResponse(201, { commentId: "c1" }));

    await api.engagement.createComment("g1", "202606", "q1", "r1", { body: "hi" });

    const [url, requestInit] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toContain(`${basePath()}/comments`);
    expect(requestInit.method).toBe("POST");
    expect(requestInit.body).toBe(JSON.stringify({ body: "hi" }));
  });

  it("PATCHes a specific comment by id", async () => {
    const fetchMock = fetch as unknown as ReturnType<typeof vi.fn>;
    fetchMock.mockResolvedValue(jsonResponse(200, { commentId: "c1" }));

    await api.engagement.patchComment("g1", "202606", "q1", "r1", "c1", { body: "edited" });

    const [url, requestInit] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toContain(`${basePath()}/comments/c1`);
    expect(requestInit.method).toBe("PATCH");
  });

  it("DELETEs a specific comment by id", async () => {
    const fetchMock = fetch as unknown as ReturnType<typeof vi.fn>;
    fetchMock.mockResolvedValue(new Response(null, { status: 204 }));

    await api.engagement.deleteComment("g1", "202606", "q1", "r1", "c1");

    const [url, requestInit] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toContain(`${basePath()}/comments/c1`);
    expect(requestInit.method).toBe("DELETE");
  });

  it("GETs reactions for an answer", async () => {
    const fetchMock = fetch as unknown as ReturnType<typeof vi.fn>;
    fetchMock.mockResolvedValue(jsonResponse(200, { reactionGroups: [], myReactions: [] }));

    await api.engagement.listReactions("g1", "202606", "q1", "r1");

    const url = fetchMock.mock.calls[0]![0] as string;
    expect(url).toContain(`${basePath()}/reactions`);
  });

  it("PUTs a reaction with the emoji URL-encoded in the path", async () => {
    const fetchMock = fetch as unknown as ReturnType<typeof vi.fn>;
    fetchMock.mockResolvedValue(jsonResponse(200, { reactionGroups: [], myReactions: [] }));

    await api.engagement.putReaction("g1", "202606", "q1", "r1", "🔥");

    const [url, requestInit] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toContain(`${basePath()}/reactions/${encodeURIComponent("🔥")}`);
    expect(requestInit.method).toBe("PUT");
  });

  it("DELETEs a reaction with the emoji URL-encoded in the path", async () => {
    const fetchMock = fetch as unknown as ReturnType<typeof vi.fn>;
    fetchMock.mockResolvedValue(jsonResponse(200, { reactionGroups: [], myReactions: [] }));

    await api.engagement.deleteReaction("g1", "202606", "q1", "r1", "👨‍👩‍👧‍👦");

    const [url, requestInit] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toContain(`${basePath()}/reactions/${encodeURIComponent("👨‍👩‍👧‍👦")}`);
    expect(requestInit.method).toBe("DELETE");
  });
});
