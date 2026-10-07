import type { components } from "../types/api";
import { env } from "../env";
import { newCorrelationId } from "../utils/correlation";
import { renewSession } from "../auth/renewSession";
import { userManager } from "../auth/userManager";
import { mockFetch } from "./mockTransport";

type S = components["schemas"];
export type ProblemDetails = S["ProblemDetails"];

type Method = "GET" | "POST" | "PUT" | "PATCH" | "DELETE";

export class ApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly problem: ProblemDetails | null;

  constructor(status: number, code: string, problem: ProblemDetails | null) {
    super(problem?.detail ?? code);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
    this.problem = problem;
  }
}

async function parseProblemBody(response: Response): Promise<ProblemDetails | null> {
  const contentType = response.headers.get("content-type") ?? "";
  if (!contentType.includes("json")) return null;
  try {
    return (await response.json()) as ProblemDetails;
  } catch {
    return null;
  }
}

async function rawFetch<T>(
  path: string,
  method: Method,
  body: unknown,
  idToken: string | null,
  signal: AbortSignal | undefined,
  credentials: RequestCredentials | undefined,
): Promise<T> {
  const headers: Record<string, string> = { "x-correlation-id": newCorrelationId() };
  if (idToken) headers.Authorization = `Bearer ${idToken}`;
  if (body !== undefined) headers["Content-Type"] = "application/json";

  let response: Response;
  try {
    response = await fetch(`${env.apiBaseUrl}${path}`, {
      method,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
      signal,
      credentials,
    });
  } catch (cause) {
    if (cause instanceof DOMException && cause.name === "AbortError") throw cause;
    throw new ApiError(0, "NETWORK_ERROR", null);
  }

  if (response.status === 204) return undefined as T;

  if (!response.ok) {
    const problem = await parseProblemBody(response);
    throw new ApiError(response.status, problem?.code ?? "UNKNOWN", problem);
  }

  return (await response.json()) as T;
}

export async function apiFetch<T>(
  path: string,
  init?: {
    method?: Method;
    body?: unknown;
    signal?: AbortSignal;
    credentials?: RequestCredentials;
  },
): Promise<T> {
  const method = init?.method ?? "GET";

  if (env.useMocks) {
    return (await mockFetch(method, path, init?.body)) as T;
  }

  const manager = userManager;
  const user = manager ? await manager.getUser() : null;

  try {
    return await rawFetch<T>(
      path,
      method,
      init?.body,
      user?.id_token ?? null,
      init?.signal,
      init?.credentials,
    );
  } catch (error) {
    if (!(error instanceof ApiError) || error.status !== 401 || !manager) throw error;

    const renewed = await renewSession();
    if (!renewed?.id_token) {
      await manager.removeUser();
      throw error;
    }
    return await rawFetch<T>(
      path,
      method,
      init?.body,
      renewed.id_token,
      init?.signal,
      init?.credentials,
    );
  }
}

function groupPath(groupId: string): string {
  return `/groups/${encodeURIComponent(groupId)}`;
}

function myResponsePath(groupId: string, cycleId: string, questionId: string): string {
  return `${groupPath(groupId)}/newsletters/${encodeURIComponent(cycleId)}/questions/${encodeURIComponent(questionId)}/my-response`;
}

/** `groupId`/`cycleId` query string shared by every `/uploads/{imageId}*` route. */
function mediaQuery(groupId: string, cycleId: string): string {
  return new URLSearchParams({ groupId, cycleId }).toString();
}

/** Shared prefix for every engagement route (`03-api-contract.md` §8). */
function engagementBasePath(
  groupId: string,
  cycleId: string,
  questionId: string,
  responseId: string,
): string {
  return (
    `${groupPath(groupId)}/newsletters/${encodeURIComponent(cycleId)}` +
    `/questions/${encodeURIComponent(questionId)}/responses/${encodeURIComponent(responseId)}`
  );
}

export const api = {
  getConfig: (): Promise<S["ConfigResponse"]> => apiFetch("/config"),

  getMe: (): Promise<S["UserResponse"]> => apiFetch("/me"),

  patchMe: (body: S["PatchMeRequest"]): Promise<S["UserResponse"]> =>
    apiFetch("/me", { method: "PATCH", body }),

  listGroups: (): Promise<S["MembershipListResponse"]> => apiFetch("/groups"),

  getGroup: (groupId: string): Promise<S["GroupResponse"]> =>
    apiFetch(`/groups/${encodeURIComponent(groupId)}`),

  removeMember: (groupId: string, userId: string): Promise<void> =>
    apiFetch(`/groups/${encodeURIComponent(groupId)}/members/${encodeURIComponent(userId)}`, {
      method: "DELETE",
    }),

  patchMember: (
    groupId: string,
    userId: string,
    body: S["PatchMemberRequest"],
  ): Promise<S["MemberResponse"]> =>
    apiFetch(`/groups/${encodeURIComponent(groupId)}/members/${encodeURIComponent(userId)}`, {
      method: "PATCH",
      body,
    }),

  patchGroup: (groupId: string, body: S["PatchGroupRequest"]): Promise<S["GroupResponse"]> =>
    apiFetch(groupPath(groupId), { method: "PATCH", body }),

  listNewsletters: (
    groupId: string,
    params?: { status?: S["NewsletterStatus"]; limit?: number; cursor?: string },
  ): Promise<S["NewsletterListResponse"]> => {
    const query = new URLSearchParams();
    if (params?.status) query.set("status", params.status);
    if (params?.limit !== undefined) query.set("limit", String(params.limit));
    if (params?.cursor) query.set("cursor", params.cursor);
    const queryString = query.toString();
    return apiFetch(
      `/groups/${encodeURIComponent(groupId)}/newsletters${queryString ? `?${queryString}` : ""}`,
    );
  },

  getNewsletter: (groupId: string, cycleId: string): Promise<S["NewsletterDetailResponse"]> =>
    apiFetch(`${groupPath(groupId)}/newsletters/${encodeURIComponent(cycleId)}`),

  listCandidates: (
    groupId: string,
    params?: { sort?: "top" | "recent"; limit?: number; cursor?: string },
  ): Promise<S["CandidateListResponse"]> => {
    const query = new URLSearchParams();
    if (params?.sort) query.set("sort", params.sort);
    if (params?.limit !== undefined) query.set("limit", String(params.limit));
    if (params?.cursor) query.set("cursor", params.cursor);
    const queryString = query.toString();
    return apiFetch(
      `${groupPath(groupId)}/candidate-questions${queryString ? `?${queryString}` : ""}`,
    );
  },

  createCandidate: (
    groupId: string,
    body: S["CreateCandidateRequest"],
  ): Promise<S["CandidateItemResponse"]> =>
    apiFetch(`${groupPath(groupId)}/candidate-questions`, { method: "POST", body }),

  castVote: (groupId: string, questionId: string): Promise<S["CandidateVoteResponse"]> =>
    apiFetch(`${groupPath(groupId)}/candidate-questions/${encodeURIComponent(questionId)}/votes`, {
      method: "POST",
    }),

  withdrawVote: (groupId: string, questionId: string): Promise<S["CandidateVoteResponse"]> =>
    apiFetch(`${groupPath(groupId)}/candidate-questions/${encodeURIComponent(questionId)}/votes`, {
      method: "DELETE",
    }),

  listMyResponses: (groupId: string, cycleId: string): Promise<S["MyResponsesList"]> =>
    apiFetch(`${groupPath(groupId)}/newsletters/${encodeURIComponent(cycleId)}/my-responses`),

  getMyResponse: (
    groupId: string,
    cycleId: string,
    questionId: string,
  ): Promise<S["ResponseDto"]> => apiFetch(myResponsePath(groupId, cycleId, questionId)),

  saveMyResponse: (
    groupId: string,
    cycleId: string,
    questionId: string,
    body: S["SaveResponseRequest"],
  ): Promise<S["ResponseDto"]> =>
    apiFetch(myResponsePath(groupId, cycleId, questionId), { method: "PUT", body }),

  redeemInvite: (code: string): Promise<S["RedeemResponse"]> =>
    apiFetch("/invites/redeem", { method: "POST", body: { code } }),

  createInvite: (body: S["CreateInviteRequest"]): Promise<S["CreateInviteResponse"]> =>
    apiFetch("/admin/invites", { method: "POST", body }),

  listInvites: (groupId: string): Promise<S["InviteListResponse"]> =>
    apiFetch(`/admin/groups/${encodeURIComponent(groupId)}/invites`),

  revokeInvite: (code: string): Promise<void> =>
    apiFetch(`/admin/invites/${encodeURIComponent(code)}/revoke`, { method: "POST" }),

  media: {
    createUpload: (body: S["CreateUploadRequest"]): Promise<S["CreateUploadResponse"]> =>
      apiFetch("/uploads", { method: "POST", body }),

    getUpload: (
      imageId: string,
      groupId: string,
      cycleId: string,
    ): Promise<S["ImageMediaResponse"]> =>
      apiFetch(`/uploads/${encodeURIComponent(imageId)}?${mediaQuery(groupId, cycleId)}`),

    completeUpload: (
      imageId: string,
      groupId: string,
      cycleId: string,
    ): Promise<S["ImageMediaResponse"]> =>
      apiFetch(`/uploads/${encodeURIComponent(imageId)}/complete?${mediaQuery(groupId, cycleId)}`, {
        method: "POST",
      }),

    patchUpload: (
      imageId: string,
      groupId: string,
      cycleId: string,
      body: S["PatchUploadRequest"],
    ): Promise<S["ImageMediaResponse"]> =>
      apiFetch(`/uploads/${encodeURIComponent(imageId)}?${mediaQuery(groupId, cycleId)}`, {
        method: "PATCH",
        body,
      }),

    deleteUpload: (imageId: string, groupId: string, cycleId: string): Promise<void> =>
      apiFetch(`/uploads/${encodeURIComponent(imageId)}?${mediaQuery(groupId, cycleId)}`, {
        method: "DELETE",
      }),

    getMediaCookie: (groupId: string): Promise<S["MediaCookieResponse"]> =>
      apiFetch(`/media-cookie?groupId=${encodeURIComponent(groupId)}`, {
        credentials: "include",
      }),

    createAvatar: (body: S["CreateAvatarRequest"]): Promise<S["CreateAvatarResponse"]> =>
      apiFetch("/avatars", { method: "POST", body }),

    getAvatar: (avatarId: string): Promise<S["AvatarMediaResponse"]> =>
      apiFetch(`/avatars/${encodeURIComponent(avatarId)}`),

    deleteAvatar: (avatarId: string): Promise<void> =>
      apiFetch(`/avatars/${encodeURIComponent(avatarId)}`, { method: "DELETE" }),
  },

  engagement: {
    listComments: (
      groupId: string,
      cycleId: string,
      questionId: string,
      responseId: string,
      params?: { limit?: number; cursor?: string },
    ): Promise<S["CommentListResponse"]> => {
      const query = new URLSearchParams();
      if (params?.limit !== undefined) query.set("limit", String(params.limit));
      if (params?.cursor) query.set("cursor", params.cursor);
      const queryString = query.toString();
      return apiFetch(
        `${engagementBasePath(groupId, cycleId, questionId, responseId)}/comments${
          queryString ? `?${queryString}` : ""
        }`,
      );
    },

    createComment: (
      groupId: string,
      cycleId: string,
      questionId: string,
      responseId: string,
      body: S["CreateCommentRequest"],
    ): Promise<S["CommentResponse"]> =>
      apiFetch(`${engagementBasePath(groupId, cycleId, questionId, responseId)}/comments`, {
        method: "POST",
        body,
      }),

    patchComment: (
      groupId: string,
      cycleId: string,
      questionId: string,
      responseId: string,
      commentId: string,
      body: S["PatchCommentRequest"],
    ): Promise<S["CommentResponse"]> =>
      apiFetch(
        `${engagementBasePath(groupId, cycleId, questionId, responseId)}/comments/${encodeURIComponent(commentId)}`,
        { method: "PATCH", body },
      ),

    deleteComment: (
      groupId: string,
      cycleId: string,
      questionId: string,
      responseId: string,
      commentId: string,
    ): Promise<void> =>
      apiFetch(
        `${engagementBasePath(groupId, cycleId, questionId, responseId)}/comments/${encodeURIComponent(commentId)}`,
        { method: "DELETE" },
      ),

    listReactions: (
      groupId: string,
      cycleId: string,
      questionId: string,
      responseId: string,
    ): Promise<S["ReactionsResponse"]> =>
      apiFetch(`${engagementBasePath(groupId, cycleId, questionId, responseId)}/reactions`),

    putReaction: (
      groupId: string,
      cycleId: string,
      questionId: string,
      responseId: string,
      emoji: string,
    ): Promise<S["ReactionsResponse"]> =>
      apiFetch(
        `${engagementBasePath(groupId, cycleId, questionId, responseId)}/reactions/${encodeURIComponent(emoji)}`,
        { method: "PUT" },
      ),

    deleteReaction: (
      groupId: string,
      cycleId: string,
      questionId: string,
      responseId: string,
      emoji: string,
    ): Promise<S["ReactionsResponse"]> =>
      apiFetch(
        `${engagementBasePath(groupId, cycleId, questionId, responseId)}/reactions/${encodeURIComponent(emoji)}`,
        { method: "DELETE" },
      ),
  },

  push: {
    subscribe: (body: S["PushSubscribeRequest"]): Promise<S["PushSubscribeResponse"]> =>
      apiFetch("/push/subscribe", { method: "POST", body }),

    unsubscribe: (body: S["PushUnsubscribeRequest"]): Promise<void> =>
      apiFetch("/push/unsubscribe", { method: "POST", body }),

    listSubscriptions: (): Promise<S["PushSubscriptionListResponse"]> =>
      apiFetch("/push/subscriptions"),

    sendTest: (): Promise<S["PushTestResponse"]> => apiFetch("/push/test", { method: "POST" }),

    listPreferences: (): Promise<S["PushPreferenceListResponse"]> => apiFetch("/push/preferences"),

    putPreference: (
      groupId: string,
      body: S["PutPushPreferenceRequest"],
    ): Promise<S["PushPreferenceResponse"]> =>
      apiFetch(`/push/preferences/${encodeURIComponent(groupId)}`, { method: "PUT", body }),
  },
};
