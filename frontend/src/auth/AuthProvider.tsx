// Mock auth context. The real implementation goes through Cognito hosted UI
// (see plans/05-auth-flow.md). For the prototype we always treat the user as
// signed in, since every API call is mocked.

import { createContext, useContext, type ReactNode } from "react";
import { useConfig } from "../api/queries";
import { useToasts } from "../state/toast";
import type { User } from "../api/types";

interface AuthValue {
  user: User | null;
  isLoading: boolean;
  signOut: () => void;
}

const AuthContext = createContext<AuthValue | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const { data: config, isLoading } = useConfig();
  const value: AuthValue = {
    user: config?.user ?? null,
    isLoading,
    signOut: () => {
      useToasts.getState().push("Signed out (mocked). Reloading…", "default");
      setTimeout(() => window.location.reload(), 800);
    },
  };
  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth(): AuthValue {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error("useAuth must be used inside AuthProvider");
  return ctx;
}
