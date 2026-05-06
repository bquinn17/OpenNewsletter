import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type NewCandidatePayload, type SaveResponseBody } from "./client";
import type { Role } from "./types";

const keys = {
  config: ["config"] as const,
  group: (groupId: string) => ["group", groupId] as const,
  newsletters: (groupId: string) => ["newsletters", groupId] as const,
  newsletter: (groupId: string, cycleId: string) => ["newsletter", groupId, cycleId] as const,
  candidates: (groupId: string) => ["candidates", groupId] as const,
  invites: (groupId: string) => ["invites", groupId] as const,
  pushDevices: ["pushDevices"] as const,
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

export const useCastPollVote = (groupId: string, cycleId: string) => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ questionId, optionId }: { questionId: string; optionId: string }) =>
      api.castPollVote(groupId, cycleId, questionId, optionId),
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
      patch: Partial<{ cycleOpen: boolean; deadlineReminders: boolean }>;
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
