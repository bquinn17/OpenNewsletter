import { UserManager, WebStorageStateStore } from "oidc-client-ts";
import { env } from "../env";

/**
 * Single shared `UserManager` instance. `null` in mock mode — nothing in that
 * mode talks to Cognito, so constructing it would just throw on missing env vars.
 */
export const userManager: UserManager | null = env.useMocks
  ? null
  : new UserManager({
      authority: `https://cognito-idp.us-east-1.amazonaws.com/${env.cognitoUserPoolId}`,
      client_id: env.cognitoClientId,
      redirect_uri: env.redirectUri || `${window.location.origin}/auth/callback`,
      scope: "openid email profile",
      response_type: "code",
      userStore: new WebStorageStateStore({ store: window.sessionStorage }),
      // Off: AuthProvider renews on `accessTokenExpiring` via `renewSession`,
      // which also handles bootstrap-login sessions (a different client).
      automaticSilentRenew: false,
    });
