import { useQueries, useQuery, type UseQueryResult } from "@tanstack/react-query";
import { api } from "./client";
import { useAuth } from "../auth/useAuth";
import { MEDIA_URLS_META } from "../utils/media";
import type { components } from "../types/api";

type S = components["schemas"];

export const queryKeys = {
  config: ["config"] as const,
  me: ["me"] as const,
  group: (groupId: string) => ["group", groupId] as const,
  newsletters: (groupId: string) => ["newsletters", groupId] as const,
  newsletter: (groupId: string, cycleId: string) => ["newsletter", groupId, cycleId] as const,
  /** Prefix for every sort order of a group's candidate pool — invalidate this. */
  candidatesAll: (groupId: string) => ["candidates", groupId] as const,
  candidates: (groupId: string, sort: CandidateSort) => ["candidates", groupId, sort] as const,
  myResponse: (groupId: string, cycleId: string, questionId: string) =>
    ["myResponse", groupId, cycleId, questionId] as const,
  invites: (groupId: string) => ["invites", groupId] as const,
  /** One uploaded image's `GET /uploads/{id}` row (`docs/design/media.md` §3.3, §4.4). */
  upload: (groupId: string, cycleId: string, imageId: string) =>
    ["upload", groupId, cycleId, imageId] as const,
  pushSubscriptions: ["pushSubscriptions"] as const,
  pushPreferences: ["pushPreferences"] as const,
};

export type CandidateSort = "top" | "recent";

export function useConfig() {
  const { status } = useAuth();
  return useQuery({
    queryKey: queryKeys.config,
    queryFn: api.getConfig,
    staleTime: Infinity,
    enabled: status === "authenticated",
  });
}

export function useMe() {
  // `GET /me` 401s until the caller redeems an invite; `/config` says whether they have.
  const { data: config } = useConfig();
  return useQuery({
    queryKey: queryKeys.me,
    queryFn: api.getMe,
    meta: MEDIA_URLS_META,
    enabled: !!config?.userId,
  });
}

export function useGroup(groupId: string | undefined) {
  return useQuery({
    queryKey: queryKeys.group(groupId ?? ""),
    queryFn: () => api.getGroup(groupId!),
    enabled: !!groupId,
    staleTime: 5 * 60_000,
    meta: MEDIA_URLS_META,
  });
}

export function useNewsletters(groupId: string | undefined) {
  return useQuery({
    queryKey: queryKeys.newsletters(groupId ?? ""),
    queryFn: () => api.listNewsletters(groupId!),
    enabled: !!groupId,
    staleTime: 30_000,
  });
}

/** The caller's membership in `groupId` (from `/config`), e.g. for the group's timezone. */
export function useMembership(groupId: string | undefined) {
  const { data } = useConfig();
  return data?.memberships.find((m) => m.groupId === groupId);
}

export function useNewsletter(groupId: string | undefined, cycleId: string | undefined) {
  return useQuery({
    queryKey: queryKeys.newsletter(groupId ?? "", cycleId ?? ""),
    queryFn: () => api.getNewsletter(groupId!, cycleId!),
    enabled: !!groupId && !!cycleId,
    staleTime: 30_000,
    meta: MEDIA_URLS_META,
  });
}

export function useCandidates(groupId: string | undefined, sort: CandidateSort = "top") {
  return useQuery({
    queryKey: queryKeys.candidates(groupId ?? "", sort),
    queryFn: () => api.listCandidates(groupId!, { sort }),
    enabled: !!groupId,
    staleTime: 30_000,
    meta: MEDIA_URLS_META,
  });
}

/**
 * One upload's row, the one place its presigned URLs live (`docs/design/media.md` §4.4): the
 * uploader's tiles, the editor's preview and the read-only answer all read this entry, so the
 * global `refreshMediaUrls` re-signs them together. Never stale on its own; the uploader's poll
 * writes the `ready` row in with `setQueryData`.
 */
export function uploadQuery(groupId: string, cycleId: string, imageId: string) {
  return {
    queryKey: queryKeys.upload(groupId, cycleId, imageId),
    queryFn: () => api.media.getUpload(imageId, groupId, cycleId),
    staleTime: Infinity,
    meta: MEDIA_URLS_META,
  };
}

type UploadRow = S["ImageMediaResponse"];

/** Module-level, so `useQueries` can keep the combined array's identity while nothing changed. */
function loadedRows(results: UseQueryResult<UploadRow>[]): UploadRow[] {
  return results.flatMap((result) => (result.data ? [result.data] : []));
}

/**
 * The cached rows for `imageIds`, in order. One that hasn't loaded, or failed to, is left out.
 * `initialRows` seeds an entry that isn't cached yet, so a row the caller already has costs no
 * request.
 */
export function useUploads(
  groupId: string,
  cycleId: string,
  imageIds: string[],
  initialRows?: ReadonlyMap<string, UploadRow>,
): UploadRow[] {
  return useQueries({
    queries: imageIds.map((imageId) => ({
      ...uploadQuery(groupId, cycleId, imageId),
      initialData: initialRows?.get(imageId),
    })),
    combine: loadedRows,
  });
}

/** A group's invites (Admin → Invites tab). Caller must be a group admin. */
export function useInvites(groupId: string | undefined) {
  return useQuery({
    queryKey: queryKeys.invites(groupId ?? ""),
    queryFn: () => api.listInvites(groupId!),
    enabled: !!groupId,
    staleTime: 30_000,
  });
}

/**
 * The caller's push subscriptions ("My devices", `docs/design/notifications.md` §11.1).
 * Only ever rendered inside `SettingsPage` (behind `RequireAuth`), so — like
 * `useGroup`/`useCandidates` — no separate auth-status gate.
 */
export function usePushSubscriptions() {
  return useQuery({
    queryKey: queryKeys.pushSubscriptions,
    queryFn: api.push.listSubscriptions,
    staleTime: 30_000,
  });
}

/** The caller's per-group notification preferences (cycle-open / deadline-reminders). */
export function usePushPreferences() {
  return useQuery({
    queryKey: queryKeys.pushPreferences,
    queryFn: api.push.listPreferences,
    staleTime: 30_000,
  });
}
