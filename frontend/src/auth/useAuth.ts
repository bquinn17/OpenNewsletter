import { createContext, useContext } from "react";

export type AuthStatus = "loading" | "authenticated" | "unauthenticated";

export type AuthUser = {
  sub: string;
  email: string | null;
  name: string | null;
};

export type LoginOptions = {
  returnTo?: string;
  invite?: string;
};

export type AuthValue = {
  status: AuthStatus;
  user: AuthUser | null;
  login: (opts?: LoginOptions) => Promise<void>;
  logout: () => Promise<void>;
};

/** Provided by `AuthProvider`; not exported for direct use outside this module and `AuthProvider`. */
export const AuthContext = createContext<AuthValue | null>(null);

export function useAuth(): AuthValue {
  const value = useContext(AuthContext);
  if (!value) throw new Error("useAuth must be used inside AuthProvider");
  return value;
}
