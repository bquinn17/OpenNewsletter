import type { User } from "oidc-client-ts";
import { env } from "../env";
import { isBootstrapSession } from "./cognitoPasswordAuth";

/** Logout never waits longer than this on Cognito. */
const REVOKE_TIMEOUT_MS = 3000;

/**
 * Best-effort revocation of `user`'s refresh token via Cognito
 * `POST /oauth2/revoke` (`docs/design/auth.md` §8 step 4, D1).
 *
 * Cognito only revokes a token when `client_id` is the client that issued it,
 * so a bootstrap-login session (`docs/design/auth.md` §9.2) revokes against the
 * `admin-bootstrap` client and everything else against the frontend client.
 * Never throws and never waits more than `REVOKE_TIMEOUT_MS`: the caller has
 * already removed the stored user, so a failure here doesn't leave the session
 * usable on this device.
 */
export async function revokeRefreshToken(user: User | null): Promise<void> {
  const token = user?.refresh_token;
  if (!user || !token || !env.cognitoHostedDomain) return;

  // `isBootstrapSession` is always false when there's no bootstrap client
  // (prod), so prod only ever revokes against the frontend client.
  const bootstrapClientId = isBootstrapSession(user) ? env.cognitoBootstrapClientId : null;
  const clientId = bootstrapClientId ?? env.cognitoClientId;
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), REVOKE_TIMEOUT_MS);
  try {
    await fetch(`https://${env.cognitoHostedDomain}/oauth2/revoke`, {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body: new URLSearchParams({ token, client_id: clientId }).toString(),
      signal: controller.signal,
    });
  } catch {
    // Network failure, abort or CSP block: logout carries on regardless.
  } finally {
    clearTimeout(timer);
  }
}
