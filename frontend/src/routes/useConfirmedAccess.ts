import { useEffect, useState } from "react";
import { useConfig } from "../api/queries";
import type { components } from "../types/api";

type ConfigResponse = components["schemas"]["ConfigResponse"];

export type AccessState = "checking" | "allowed" | "denied";

/**
 * Decide a route guard against `/config`, refetching once before denying.
 *
 * `/config` has `staleTime: Infinity`, so a membership or role granted
 * elsewhere (another tab or device, or an invite just redeemed) isn't in the
 * cache yet. Bouncing on the cached answer would send the user away from a
 * page they can now see, so a denial is only final after a fresh fetch.
 */
export function useConfirmedAccess(isAllowed: (config: ConfigResponse) => boolean): AccessState {
  const { data: config, isLoading, isFetching, refetch } = useConfig();
  const allowed = config ? isAllowed(config) : false;
  const [rechecked, setRechecked] = useState(false);

  useEffect(() => {
    if (isLoading || allowed || rechecked || isFetching) return;
    void refetch().finally(() => setRechecked(true));
  }, [isLoading, allowed, rechecked, isFetching, refetch]);

  if (allowed) return "allowed";
  if (isLoading || isFetching || !rechecked) return "checking";
  return "denied";
}
