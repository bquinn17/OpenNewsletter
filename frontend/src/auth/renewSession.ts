import type { User } from "oidc-client-ts";
import { bootstrapAuth, isBootstrapSession } from "./cognitoPasswordAuth";
import { userManager } from "./userManager";

/**
 * Renew the stored session, or return `null` if it can't be renewed.
 *
 * Federated sessions use oidc-client-ts's refresh-token grant against the
 * frontend client. Bootstrap-login sessions hold a refresh token for the
 * `admin-bootstrap` client, which that grant would reject, so they go through
 * Cognito `REFRESH_TOKEN_AUTH` instead. Either way the renewed user is stored
 * and `userLoaded` fires.
 */
export async function renewSession(): Promise<User | null> {
  if (!userManager) return null;
  const manager = userManager;
  const current = await manager.getUser();
  try {
    if (current?.refresh_token && isBootstrapSession(current)) {
      const renewed = await bootstrapAuth({
        flow: "REFRESH_TOKEN_AUTH",
        refreshToken: current.refresh_token,
      });
      await manager.storeUser(renewed);
      await manager.events.load(renewed);
      return renewed;
    }
    return await manager.signinSilent();
  } catch {
    return null;
  }
}
