// Pre-M7 mock query/mutation layer. Every hook here reads and writes the
// in-memory store in `./data` via `./api`. Only `GroupAdminPage` (M12) and
// the comment/reaction components (M10) still import these hooks; each is
// deleted when its milestone rebuilds that UI against the live API.

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { mockApi as api } from "./api";
import type { Group, Role } from "./types";

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

export const useInvites = (groupId: string | undefined) =>
  useQuery({
    queryKey: keys.invites(groupId ?? ""),
    queryFn: () => api.listInvites(groupId!),
    enabled: !!groupId,
  });

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
