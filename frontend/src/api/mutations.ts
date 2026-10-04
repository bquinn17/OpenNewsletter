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
