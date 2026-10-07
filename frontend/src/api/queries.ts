import { useQuery } from "@tanstack/react-query";
import { api } from "./client";
import { useAuth } from "../auth/useAuth";

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
  return useQuery({ queryKey: queryKeys.me, queryFn: api.getMe });
}

export function useGroup(groupId: string | undefined) {
  return useQuery({
    queryKey: queryKeys.group(groupId ?? ""),
    queryFn: () => api.getGroup(groupId!),
    enabled: !!groupId,
    staleTime: 5 * 60_000,
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
  });
}

export function useCandidates(groupId: string | undefined, sort: CandidateSort = "top") {
  return useQuery({
    queryKey: queryKeys.candidates(groupId ?? "", sort),
    queryFn: () => api.listCandidates(groupId!, { sort }),
    enabled: !!groupId,
    staleTime: 30_000,
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
 * The caller's push subscriptions ("My devices", `07-notifications.md` §11.1).
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
