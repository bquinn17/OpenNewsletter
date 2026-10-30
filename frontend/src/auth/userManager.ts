import { UserManager, WebStorageStateStore } from "oidc-client-ts";
import { env } from "../env";

/**
 * Single shared `UserManager` instance. `null` in a mock build — nothing in that
 * build talks to Cognito, so constructing it would just throw on missing env vars.
 * The demo still has one: its "Sign in" starts a real sign-in (`docs/design/frontend.md` §17).
 */
export const userManager: UserManager | null = env.useMocks
  ? null
  : new UserManager({
      authority: `https://cognito-idp.us-east-1.amazonaws.com/${env.cognitoUserPoolId}`,
      client_id: env.cognitoClientId,
      redirect_uri: env.redirectUri || `${window.location.origin}/auth/callback`,
      scope: "openid email profile",
      response_type: "code",
      // `localStorage` so an installed PWA stays signed in across launches
      // (`docs/design/auth.md` §7, D1). Logout removes it and revokes the refresh
      // token (`AuthProvider`). The sign-in state store (PKCE verifier +
      // invite data) keeps the library default; the invite stash itself stays
      // in `sessionStorage` (`inviteStash.ts`).
      userStore: new WebStorageStateStore({ store: window.localStorage }),
      // Off: AuthProvider renews on `accessTokenExpiring` via `renewSession`,
      // which also handles bootstrap-login sessions (a different client).
      automaticSilentRenew: false,
    });
