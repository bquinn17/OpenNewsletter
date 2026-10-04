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
  init?: { method?: Method; body?: unknown; signal?: AbortSignal },
): Promise<T> {
  const method = init?.method ?? "GET";

  if (env.useMocks) {
    return (await mockFetch(method, path, init?.body)) as T;
  }

  const manager = userManager;
  const user = manager ? await manager.getUser() : null;

  try {
    return await rawFetch<T>(path, method, init?.body, user?.id_token ?? null, init?.signal);
  } catch (error) {
    if (!(error instanceof ApiError) || error.status !== 401 || !manager) throw error;

    const renewed = await renewSession();
    if (!renewed?.id_token) {
      await manager.removeUser();
      throw error;
    }
    return await rawFetch<T>(path, method, init?.body, renewed.id_token, init?.signal);
  }
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

  redeemInvite: (code: string): Promise<S["RedeemResponse"]> =>
    apiFetch("/invites/redeem", { method: "POST", body: { code } }),
};
