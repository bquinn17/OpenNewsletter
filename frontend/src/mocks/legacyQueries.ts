// Pre-M7 mock query/mutation layer. Every hook here reads and writes the
// in-memory store in `./data` via `./api`. Only `GroupAdminPage` (M12) still
// imports these hooks; it's deleted when that milestone rebuilds the admin UI
// against the live API. (M10 rebuilt comments/reactions against the live API
// and removed its hooks from here — `src/components/newsletter/{CommentList,ReactionBar}.tsx`.)

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { mockApi as api } from "./api";
import type { Group, Role } from "./types";

const keys = {
  config: ["config"] as const,
  group: (groupId: string) => ["group", groupId] as const,
  newsletters: (groupId: string) => ["newsletters", groupId] as const,
  candidates: (groupId: string) => ["candidates", groupId] as const,
  invites: (groupId: string) => ["invites", groupId] as const,
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
