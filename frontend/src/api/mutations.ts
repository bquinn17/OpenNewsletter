import { useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "./client";
import { queryKeys } from "./queries";
import { useCurrentGroup } from "../state/currentGroup";
import type { components } from "../types/api";

type S = components["schemas"];

export function useRedeemInvite() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (code: string) => api.redeemInvite(code),
    // JoinPage renders its own friendly, code-specific error message —
    // suppress the generic global toast to avoid showing both.
    meta: { silent: true },
    // Returned so TanStack awaits the refetch before the caller's `onSuccess`
    // runs: JoinPage navigates into the new group straight away, and
    // RequireMembership would bounce a stale membership list back to `/`.
    onSuccess: () =>
      Promise.all([
        qc.invalidateQueries({ queryKey: queryKeys.config, refetchType: "all" }),
        qc.invalidateQueries({ queryKey: ["groups"] }),
      ]),
  });
}

export function useUpdateProfile() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: S["PatchMeRequest"]) => api.patchMe(body),
    onSuccess: (data) => {
      qc.setQueryData(queryKeys.me, data);
      qc.invalidateQueries({ queryKey: queryKeys.config });
    },
  });
}

export function useLeaveGroup() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (groupId: string) => {
      const config = qc.getQueryData<S["ConfigResponse"]>(queryKeys.config);
      const userId = config?.userId;
      if (!userId) throw new Error("You must be signed in to leave a group.");
      await api.removeMember(groupId, userId);
      return groupId;
    },
    // SettingsPage surfaces LAST_ADMIN (and other failures) with its own
    // copy — suppress the generic global toast to avoid showing both.
    meta: { silent: true },
    onSuccess: (groupId) => {
      qc.invalidateQueries({ queryKey: queryKeys.config });
      qc.removeQueries({ queryKey: queryKeys.group(groupId) });
      qc.removeQueries({ queryKey: queryKeys.newsletters(groupId) });
      if (useCurrentGroup.getState().currentGroupId === groupId) {
        useCurrentGroup.getState().clear();
      }
    },
  });
}

export function useCreateCandidate(groupId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: S["CreateCandidateRequest"]) => api.createCandidate(groupId, body),
    // SuggestPage renders its own field-level and form-level error messages —
    // suppress the generic global toast to avoid showing both.
    meta: { silent: true },
    onSuccess: () => qc.invalidateQueries({ queryKey: queryKeys.candidatesAll(groupId) }),
  });
}

/**
 * Casts (`voted: true`) or withdraws (`voted: false`) the caller's upvote. The
 * server's tally is patched into every cached sort order of the pool, then the
 * pool is refetched so ordering under `sort=top` catches up.
 */
export function useToggleVote(groupId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ questionId, voted }: { questionId: string; voted: boolean }) =>
      voted ? api.castVote(groupId, questionId) : api.withdrawVote(groupId, questionId),
    // CandidatesPage renders a friendly, cap-specific message (the server's
    // VOTE_CAP_REACHED `detail` is a raw list of question IDs, not something
    // to show as-is) — suppress the generic global toast to avoid showing both.
    meta: { silent: true },
    onSuccess: (result) => {
      qc.setQueriesData<S["CandidateListResponse"]>(
        { queryKey: queryKeys.candidatesAll(groupId) },
        (list) =>
          list && {
            ...list,
            myVoteCount: result.myVoteCount,
            items: list.items.map((item) =>
              item.questionId === result.questionId
                ? { ...item, voteCount: result.voteCount, votedByMe: result.votedByMe }
                : item,
            ),
          },
      );
      return qc.invalidateQueries({ queryKey: queryKeys.candidatesAll(groupId) });
    },
    // A rejected vote (e.g. the cap was reached by a concurrent request since
    // our last fetch) can leave the cache stale — refetch so the UI catches up.
    onError: () => qc.invalidateQueries({ queryKey: queryKeys.candidatesAll(groupId) }),
  });
}
