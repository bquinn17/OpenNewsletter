import { useQuery } from "@tanstack/react-query";
import { api } from "./client";
import { useAuth } from "../auth/useAuth";

export const queryKeys = {
  config: ["config"] as const,
  me: ["me"] as const,
  group: (groupId: string) => ["group", groupId] as const,
  newsletters: (groupId: string) => ["newsletters", groupId] as const,
};

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
