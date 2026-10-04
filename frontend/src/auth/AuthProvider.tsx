import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useQueryClient } from "@tanstack/react-query";
import type { User } from "oidc-client-ts";
import { env } from "../env";
import { useCurrentGroup } from "../state/currentGroup";
import {
  AuthContext,
  type AuthStatus,
  type AuthUser,
  type AuthValue,
  type LoginOptions,
} from "./useAuth";
import { renewSession } from "./renewSession";
import { userManager } from "./userManager";
import { stashInvite } from "./inviteStash";

const MOCK_USER: AuthUser = { sub: "mock-sub", email: "quinn@example.com", name: "Quinn" };

function toAuthUser(user: User): AuthUser {
  return {
    sub: user.profile.sub,
    email: user.profile.email ?? null,
    name: user.profile.name ?? null,
  };
}

export function AuthProvider({ children }: { children: ReactNode }): JSX.Element {
  const queryClient = useQueryClient();
  const [status, setStatus] = useState<AuthStatus>(env.useMocks ? "authenticated" : "loading");
  const [user, setUser] = useState<AuthUser | null>(env.useMocks ? MOCK_USER : null);
  // Set while logout() runs. Removing the user fires `userUnloaded`, and on a
  // protected route RequireAuth would answer with `signinRedirect()`, whose
  // redirect can land after (and override) the Cognito `/logout` redirect.
  const loggingOut = useRef(false);

  useEffect(() => {
    if (env.useMocks || !userManager) return;
    const manager = userManager;
    let cancelled = false;

    const handleUserLoaded = (loaded: User): void => {
      setUser(toAuthUser(loaded));
      setStatus("authenticated");
    };
    const handleUserUnloaded = (): void => {
      if (loggingOut.current) return;
      setUser(null);
      setStatus("unauthenticated");
    };
    // Fires shortly before the tokens expire (`automaticSilentRenew` is off;
    // see `renewSession`). A failed renewal signs the user out.
    const handleTokenExpiring = (): void => {
      void renewSession().then((renewed) => {
        if (!renewed) return manager.removeUser();
      });
    };

    manager.events.addUserLoaded(handleUserLoaded);
    manager.events.addUserUnloaded(handleUserUnloaded);
    manager.events.addAccessTokenExpiring(handleTokenExpiring);

    manager
      .getUser()
      .then((existing) => {
        if (cancelled) return;
        if (existing && !existing.expired) {
          setUser(toAuthUser(existing));
          setStatus("authenticated");
          return;
        }
        if (existing?.refresh_token) {
          // The ID token lapsed while the tab was idle, but the 30-day refresh
          // token can still renew it; success fires `userLoaded` above.
          return renewSession().then((renewed) => {
            if (!renewed) return manager.removeUser();
          });
        }
        setStatus("unauthenticated");
      })
      .catch(() => {
        if (!cancelled) setStatus("unauthenticated");
      });

    return () => {
      cancelled = true;
      manager.events.removeUserLoaded(handleUserLoaded);
      manager.events.removeUserUnloaded(handleUserUnloaded);
      manager.events.removeAccessTokenExpiring(handleTokenExpiring);
    };
  }, []);

  const login = useCallback(async (opts?: LoginOptions): Promise<void> => {
    if (env.useMocks || !userManager) return;
    if (opts?.invite) stashInvite(opts.invite);
    await userManager.signinRedirect({ state: { invite: opts?.invite, returnTo: opts?.returnTo } });
  }, []);

  const logout = useCallback(async (): Promise<void> => {
    if (env.useMocks) {
      queryClient.clear();
      useCurrentGroup.getState().clear();
      window.location.assign("/");
      return;
    }
    if (!userManager) return;
    loggingOut.current = true;
    await userManager.removeUser();
    queryClient.clear();
    useCurrentGroup.getState().clear();
    const logoutUri = encodeURIComponent(`${window.location.origin}/`);
    window.location.assign(
      `https://${env.cognitoHostedDomain}/logout?client_id=${env.cognitoClientId}&logout_uri=${logoutUri}`,
    );
  }, [queryClient]);

  const value = useMemo<AuthValue>(
    () => ({ status, user, login, logout }),
    [status, user, login, logout],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}
