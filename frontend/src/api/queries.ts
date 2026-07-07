import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type NewCandidatePayload, type SaveResponseBody } from "./client";
import type { Group, Role, User } from "./types";

const keys = {
  config: ["config"] as const,
  group: (groupId: string) => ["group", groupId] as const,
  newsletters: (groupId: string) => ["newsletters", groupId] as const,
  newsletter: (groupId: string, cycleId: string) => ["newsletter", groupId, cycleId] as const,
  candidates: (groupId: string) => ["candidates", groupId] as const,
  invites: (groupId: string) => ["invites", groupId] as const,
  pushDevices: ["pushDevices"] as const,
  pushEnabled: ["pushEnabled"] as const,
  notificationPrefs: ["notificationPrefs"] as const,
};

export const useConfig = () =>
  useQuery({ queryKey: keys.config, queryFn: api.getConfig, staleTime: Infinity });

export const useGroup = (groupId: string | undefined) =>
  useQuery({
    queryKey: keys.group(groupId ?? ""),
    queryFn: () => api.getGroup(groupId!),
    enabled: !!groupId,
  });

export const useNewsletters = (groupId: string | undefined) =>
  useQuery({
    queryKey: keys.newsletters(groupId ?? ""),
    queryFn: () => api.listNewsletters(groupId!),
    enabled: !!groupId,
  });

export const useNewsletter = (groupId: string | undefined, cycleId: string | undefined) =>
  useQuery({
    queryKey: keys.newsletter(groupId ?? "", cycleId ?? ""),
    queryFn: () => api.getNewsletter(groupId!, cycleId!),
    enabled: !!groupId && !!cycleId,
  });

export const useCandidates = (groupId: string | undefined) =>
  useQuery({
    queryKey: keys.candidates(groupId ?? ""),
    queryFn: () => api.listCandidates(groupId!),
    enabled: !!groupId,
  });

export const useInvites = (groupId: string | undefined) =>
  useQuery({
    queryKey: keys.invites(groupId ?? ""),
    queryFn: () => api.listInvites(groupId!),
    enabled: !!groupId,
  });

export const usePushDevices = () =>
  useQuery({ queryKey: keys.pushDevices, queryFn: api.listPushDevices });

export const usePushEnabled = () =>
  useQuery({ queryKey: keys.pushEnabled, queryFn: api.getPushEnabled });

export const useNotificationPrefs = () =>
  useQuery({ queryKey: keys.notificationPrefs, queryFn: api.listNotificationPrefs });

// --- Mutations -----------------------------------------------------------

export const useCastVote = (groupId: string) => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (questionId: string) => api.castVote(groupId, questionId),
    onSuccess: (data) => qc.setQueryData(keys.candidates(groupId), data),
  });
};

export const useWithdrawVote = (groupId: string) => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (questionId: string) => api.withdrawVote(groupId, questionId),
    onSuccess: (data) => qc.setQueryData(keys.candidates(groupId), data),
  });
};

export const useSuggestCandidate = (groupId: string) => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (payload: NewCandidatePayload) => api.suggestCandidate(groupId, payload),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.candidates(groupId) }),
  });
};

export const useSaveResponse = (groupId: string, cycleId: string) => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ questionId, body }: { questionId: string; body: SaveResponseBody }) =>
      api.saveResponse(groupId, cycleId, questionId, body),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.newsletter(groupId, cycleId) }),
  });
};

export const useToggleReaction = (groupId: string, cycleId: string) => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({
      questionId,
      responseId,
      emoji,
    }: {
      questionId: string;
      responseId: string;
      emoji: string;
    }) => api.toggleReaction(groupId, cycleId, questionId, responseId, emoji),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.newsletter(groupId, cycleId) }),
  });
};

export const useAddComment = (groupId: string, cycleId: string) => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({
      questionId,
      responseId,
      body,
    }: {
      questionId: string;
      responseId: string;
      body: string;
    }) => api.addComment(groupId, cycleId, questionId, responseId, body),
    onSuccess: (data) => qc.setQueryData(keys.newsletter(groupId, cycleId), data),
  });
};

export const useEditComment = (groupId: string, cycleId: string) => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({
      questionId,
      responseId,
      commentId,
      body,
    }: {
      questionId: string;
      responseId: string;
      commentId: string;
      body: string;
    }) => api.editComment(groupId, cycleId, questionId, responseId, commentId, body),
    onSuccess: (data) => qc.setQueryData(keys.newsletter(groupId, cycleId), data),
  });
};

export const useDeleteComment = (groupId: string, cycleId: string) => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({
      questionId,
      responseId,
      commentId,
    }: {
      questionId: string;
      responseId: string;
      commentId: string;
    }) => api.deleteComment(groupId, cycleId, questionId, responseId, commentId),
    onSuccess: (data) => qc.setQueryData(keys.newsletter(groupId, cycleId), data),
  });
};

export const useCreateInvite = (groupId: string) => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (role: Role) => api.createInvite(groupId, role),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.invites(groupId) }),
  });
};

export const useRevokeInvite = (groupId: string) => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (code: string) => api.revokeInvite(code),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.invites(groupId) }),
  });
};

export const useRemovePushDevice = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.removePushDevice(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.pushDevices }),
  });
};

export const useSendTestPush = () =>
  useMutation({ mutationFn: () => api.sendTestPush() });

export const useUpdateNotificationPref = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({
      groupId,
      patch,
    }: {
      groupId: string;
      patch: Partial<{ cycleOpen: boolean; deadlineReminders: boolean; publication: boolean }>;
    }) => api.updateNotificationPref(groupId, patch),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.notificationPrefs }),
  });
};

export const useRedeemInvite = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (code: string) => api.redeemInvite(code),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.config }),
  });
};

export const usePatchUser = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (patch: Partial<Pick<User, "displayName" | "avatarColor">>) => api.patchUser(patch),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: keys.config });
      // Member rows in groups also reflect display name changes.
      qc.invalidateQueries({ queryKey: ["group"] });
    },
  });
};

export const useSetPushEnabled = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (enabled: boolean) => api.setPushEnabled(enabled),
    onSuccess: (data) => qc.setQueryData(keys.pushEnabled, data),
  });
};

export const useUpdateGroup = (groupId: string) => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (patch: Partial<Group>) => api.patchGroup(groupId, patch),
    onSuccess: (data) => qc.setQueryData(keys.group(groupId), data),
  });
};

export const useUpdateMemberRole = (groupId: string) => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ userId, role }: { userId: string; role: Role }) =>
      api.updateMemberRole(groupId, userId, role),
    onSuccess: (data) => qc.setQueryData(keys.group(groupId), data),
  });
};

export const useRemoveMember = (groupId: string) => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (userId: string) => api.removeMember(groupId, userId),
    onSuccess: (data) => qc.setQueryData(keys.group(groupId), data),
  });
};

export const useLeaveGroup = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (groupId: string) => api.leaveGroup(groupId),
    onSuccess: (_data, groupId) => {
      qc.invalidateQueries({ queryKey: keys.config });
      qc.removeQueries({ queryKey: keys.group(groupId) });
      qc.removeQueries({ queryKey: keys.newsletters(groupId) });
    },
  });
};

export const useUploadAvatar = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (dataUrl: string) => api.uploadUserAvatar(dataUrl),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: keys.config });
      qc.invalidateQueries({ queryKey: ["group"] });
    },
  });
};

export const useRemoveAvatar = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => api.removeUserAvatar(),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: keys.config });
      qc.invalidateQueries({ queryKey: ["group"] });
    },
  });
};
