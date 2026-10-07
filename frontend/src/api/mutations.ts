import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { api } from "./client";
import { queryKeys } from "./queries";
import { useCurrentGroup } from "../state/currentGroup";
import type { components } from "../types/api";

type S = components["schemas"];
type NewsletterDetail = S["NewsletterDetailResponse"];
type PublishedAnswer = S["PublishedAnswerResponse"];
type ReactionGroup = S["ReactionGroupResponse"];

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

// ---- Engagement (comments + reactions, M10 — `09-engagement.md`) ----------
//
// Comments and reactions come inline on the published newsletter (no
// separate per-answer query). Every mutation below patches the one answer
// it affects inside the cached `queryKeys.newsletter(groupId, cycleId)`
// response rather than refetching the whole edition.

/** Patches the one answer matching `responseId` inside the cached newsletter, if present. */
function patchAnswer(
  qc: QueryClient,
  groupId: string,
  cycleId: string,
  responseId: string,
  updater: (answer: PublishedAnswer) => PublishedAnswer,
): void {
  qc.setQueryData<NewsletterDetail>(queryKeys.newsletter(groupId, cycleId), (data) => {
    if (!data || data.status !== "published") return data;
    return {
      ...data,
      questions: data.questions.map((q) =>
        q.kind === "text" && q.answers
          ? { ...q, answers: q.answers.map((a) => (a.responseId === responseId ? updater(a) : a)) }
          : q,
      ),
    };
  });
}

/**
 * Optimistic local toggle, mirroring the server's ordering (count desc, then
 * earliest reaction, then emoji — `03-api-contract.md` §8.5): a new emoji is
 * appended at the end, an existing group's position is otherwise kept, and a
 * group that hits zero is dropped. The server's response (applied in
 * `onSuccess` below) always wins over this guess.
 */
function applyReactionToggle(
  groups: ReactionGroup[],
  emoji: string,
  active: boolean,
): ReactionGroup[] {
  const index = groups.findIndex((g) => g.emoji === emoji);
  if (active) {
    if (index === -1) return [...groups, { emoji, count: 1, reactedByMe: true }];
    const group = groups[index]!;
    if (group.reactedByMe) return groups;
    const next = [...groups];
    next[index] = { ...group, count: group.count + 1, reactedByMe: true };
    return next;
  }
  if (index === -1) return groups;
  const group = groups[index]!;
  if (!group.reactedByMe) return groups;
  if (group.count - 1 <= 0) return groups.filter((_, i) => i !== index);
  const next = [...groups];
  next[index] = { ...group, count: group.count - 1, reactedByMe: false };
  return next;
}

/**
 * Toggles the caller's reaction on one published answer. Optimistically
 * patches `reactionGroups`, then replaces them with the server's authoritative
 * `ReactionsResponse` on success; rolls back the optimistic patch on error
 * (the global mutation-error toast in `main.tsx` surfaces the failure).
 */
export function useToggleReaction(groupId: string, cycleId: string, questionId: string) {
  const qc = useQueryClient();
  const key = queryKeys.newsletter(groupId, cycleId);

  return useMutation({
    mutationFn: ({
      responseId,
      emoji,
      active,
    }: {
      responseId: string;
      emoji: string;
      active: boolean;
    }) =>
      active
        ? api.engagement.putReaction(groupId, cycleId, questionId, responseId, emoji)
        : api.engagement.deleteReaction(groupId, cycleId, questionId, responseId, emoji),
    onMutate: async ({ responseId, emoji, active }) => {
      // An in-flight refetch would otherwise land after, and erase, the optimistic patch.
      await qc.cancelQueries({ queryKey: key });
      const previous = qc.getQueryData<NewsletterDetail>(key);
      patchAnswer(qc, groupId, cycleId, responseId, (answer) => ({
        ...answer,
        reactionGroups: applyReactionToggle(answer.reactionGroups, emoji, active),
      }));
      return { previous };
    },
    onError: (_error, _vars, context) => {
      if (context?.previous) qc.setQueryData(key, context.previous);
    },
    onSuccess: (data, { responseId }) => {
      patchAnswer(qc, groupId, cycleId, responseId, (answer) => ({
        ...answer,
        reactionGroups: data.reactionGroups,
      }));
    },
  });
}

/**
 * Posts a new comment on a published answer. On success, appends the
 * server's returned comment into the cached answer, then invalidates the
 * newsletter query so a background refetch catches up. The composer renders
 * the server's `detail` inline on error (`meta.silent` suppresses the
 * generic global toast to avoid showing both).
 */
export function useAddComment(groupId: string, cycleId: string, questionId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ responseId, body }: { responseId: string; body: S["CreateCommentRequest"] }) =>
      api.engagement.createComment(groupId, cycleId, questionId, responseId, body),
    meta: { silent: true },
    onSuccess: (comment, { responseId }) => {
      patchAnswer(qc, groupId, cycleId, responseId, (answer) => ({
        ...answer,
        comments: [...answer.comments, comment],
      }));
      void qc.invalidateQueries({ queryKey: queryKeys.newsletter(groupId, cycleId) });
    },
  });
}

/** Edits the caller's own comment. Same inline-error convention as `useAddComment`. */
export function useEditComment(groupId: string, cycleId: string, questionId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({
      responseId,
      commentId,
      body,
    }: {
      responseId: string;
      commentId: string;
      body: S["PatchCommentRequest"];
    }) => api.engagement.patchComment(groupId, cycleId, questionId, responseId, commentId, body),
    meta: { silent: true },
    onSuccess: (comment, { responseId }) => {
      patchAnswer(qc, groupId, cycleId, responseId, (answer) => ({
        ...answer,
        comments: answer.comments.map((c) => (c.commentId === comment.commentId ? comment : c)),
      }));
      void qc.invalidateQueries({ queryKey: queryKeys.newsletter(groupId, cycleId) });
    },
  });
}

/**
 * Soft-deletes a comment (author or group admin). Patches the cache to the
 * muted "[deleted]" placeholder locally, then invalidates. Uses the default
 * global error toast — there's no bespoke inline message for delete failures.
 */
export function useDeleteComment(groupId: string, cycleId: string, questionId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ responseId, commentId }: { responseId: string; commentId: string }) =>
      api.engagement.deleteComment(groupId, cycleId, questionId, responseId, commentId),
    onSuccess: (_void, { responseId, commentId }) => {
      patchAnswer(qc, groupId, cycleId, responseId, (answer) => ({
        ...answer,
        comments: answer.comments.map((c) =>
          c.commentId === commentId
            ? { ...c, body: "", image: null, deletedAt: new Date().toISOString() }
            : c,
        ),
      }));
      void qc.invalidateQueries({ queryKey: queryKeys.newsletter(groupId, cycleId) });
    },
  });
}

// ---- Push notifications (M11 — `07-notifications.md` §11, D11) -----------

/** Sends a `kind: test` push to every one of the caller's subscriptions. */
export function useSendTestPush() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => api.push.sendTest(),
    // SettingsPage renders per-device results inline — suppress the generic
    // global toast to avoid showing both.
    meta: { silent: true },
    // A failed/expired send can delete the subscription server-side.
    onSuccess: () => qc.invalidateQueries({ queryKey: queryKeys.pushSubscriptions }),
  });
}

/** Removes one push subscription by endpoint ("My devices" remove button). */
export function useRemovePushSubscription() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (endpoint: string) => api.push.unsubscribe({ endpoint }),
    onSuccess: () => qc.invalidateQueries({ queryKey: queryKeys.pushSubscriptions }),
  });
}

/**
 * Upserts the caller's per-group `cycleOpen`/`deadlineReminders` prefs.
 * Optimistically patches the cached preference list, rolling back on error.
 */
export function usePutPushPreference(groupId: string) {
  const qc = useQueryClient();
  const key = queryKeys.pushPreferences;

  return useMutation({
    mutationFn: (body: S["PutPushPreferenceRequest"]) => api.push.putPreference(groupId, body),
    onMutate: async (body) => {
      await qc.cancelQueries({ queryKey: key });
      const previous = qc.getQueryData<S["PushPreferenceListResponse"]>(key);
      qc.setQueryData<S["PushPreferenceListResponse"]>(key, (data) => {
        if (!data) return data;
        const exists = data.items.some((p) => p.groupId === groupId);
        const items = exists
          ? data.items.map((p) => (p.groupId === groupId ? { ...p, ...body } : p))
          : [...data.items, { groupId, ...body }];
        return { items };
      });
      return { previous };
    },
    onError: (_error, _vars, context) => {
      if (context?.previous) qc.setQueryData(key, context.previous);
    },
    onSuccess: (data) => {
      qc.setQueryData<S["PushPreferenceListResponse"]>(key, (cache) => {
        if (!cache) return cache;
        return { items: cache.items.map((p) => (p.groupId === data.groupId ? data : p)) };
      });
    },
  });
}

// ---- Group admin (M12 — `03-api-contract.md` §3/§4) -----------------------

/** Creates an invite for the given group (`GroupAdminPage`'s Invites tab). */
export function useCreateInvite(groupId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: S["CreateInviteRequest"]) => api.createInvite(body),
    onSuccess: () => qc.invalidateQueries({ queryKey: queryKeys.invites(groupId) }),
  });
}

/** Revokes a pending invite. Idempotent — revoking an already-revoked code still succeeds. */
export function useRevokeInvite(groupId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (code: string) => api.revokeInvite(code),
    onSuccess: () => qc.invalidateQueries({ queryKey: queryKeys.invites(groupId) }),
  });
}

/**
 * Changes a member's role. The page surfaces `LAST_ADMIN` with its own
 * friendly copy and toasts success/failure itself — suppress the generic
 * global toast to avoid showing both.
 */
export function usePatchMember(groupId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ userId, role }: { userId: string; role: S["Role"] }) =>
      api.patchMember(groupId, userId, { role }),
    meta: { silent: true },
    onSuccess: () => qc.invalidateQueries({ queryKey: queryKeys.group(groupId) }),
  });
}

/** Removes a member from the group (admin "Remove from group" action). Same toast convention as `usePatchMember`. */
export function useKickMember(groupId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (userId: string) => api.removeMember(groupId, userId),
    meta: { silent: true },
    onSuccess: () => qc.invalidateQueries({ queryKey: queryKeys.group(groupId) }),
  });
}

/**
 * Updates group settings (name/gradient/timezone/cycle/notification settings/
 * member cap). The form renders its own field and form-level errors —
 * suppress the generic global toast to avoid showing both.
 */
export function usePatchGroup(groupId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: S["PatchGroupRequest"]) => api.patchGroup(groupId, body),
    meta: { silent: true },
    onSuccess: (data) => {
      qc.setQueryData(queryKeys.group(groupId), data);
      // Memberships denormalize groupName/gradient/timezone; the server
      // reads the group row live, so a refetch picks up the new values.
      qc.invalidateQueries({ queryKey: queryKeys.config });
      // A timezone/responseWindowDays change can reschedule the voting
      // cycle's dates (`06-newsletter-lifecycle.md` §6).
      qc.invalidateQueries({ queryKey: queryKeys.newsletters(groupId) });
    },
  });
}
