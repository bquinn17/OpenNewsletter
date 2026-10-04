import { User, type UserProfile } from "oidc-client-ts";
import { env } from "../env";

// Direct Cognito `InitiateAuth` calls for the `admin-bootstrap` client
// (`05-auth-flow.md` §9.2). The hosted-UI/OIDC path never comes through here.

type AuthenticationResult = {
  AccessToken: string;
  IdToken: string;
  /** Absent on `REFRESH_TOKEN_AUTH` responses; the existing token stays valid. */
  RefreshToken?: string;
  TokenType: string;
  ExpiresIn: number;
};

type InitiateAuthResponse = {
  AuthenticationResult?: AuthenticationResult;
  message?: string;
};

export class CognitoAuthError extends Error {}

export type BootstrapAuthRequest =
  | { flow: "USER_PASSWORD_AUTH"; username: string; password: string }
  | { flow: "REFRESH_TOKEN_AUTH"; refreshToken: string };

/** Decode an ID token's claims. Base64url + UTF-8, so non-ASCII names survive. */
export function decodeIdTokenClaims(idToken: string): UserProfile {
  const payload = idToken.split(".")[1] ?? "";
  const binary = atob(payload.replace(/-/g, "+").replace(/_/g, "/"));
  const bytes = Uint8Array.from(binary, (c) => c.charCodeAt(0));
  const decoded: unknown = JSON.parse(new TextDecoder().decode(bytes));
  if (
    typeof decoded !== "object" ||
    decoded === null ||
    typeof (decoded as { sub?: unknown }).sub !== "string"
  ) {
    throw new Error("ID token is missing a `sub` claim");
  }
  // Narrowed via the `sub` check above; the remaining standard OIDC claims are
  // trusted as issued by Cognito.
  return decoded as UserProfile;
}

/** Whether a stored session came from the `admin-bootstrap` client (its ID token's `aud`). */
export function isBootstrapSession(user: User): boolean {
  const aud: unknown = user.profile?.aud;
  const audiences = Array.isArray(aud) ? aud : [aud];
  return audiences.includes(env.cognitoBootstrapClientId);
}

/**
 * Authenticate against the `admin-bootstrap` client and return an oidc-client-ts
 * `User` ready for `userManager.storeUser`. Throws `CognitoAuthError` with
 * Cognito's message on rejection.
 */
export async function bootstrapAuth(request: BootstrapAuthRequest): Promise<User> {
  const authParameters =
    request.flow === "USER_PASSWORD_AUTH"
      ? { USERNAME: request.username, PASSWORD: request.password }
      : { REFRESH_TOKEN: request.refreshToken };

  let response: Response;
  try {
    response = await fetch("https://cognito-idp.us-east-1.amazonaws.com/", {
      method: "POST",
      headers: {
        "Content-Type": "application/x-amz-json-1.1",
        "X-Amz-Target": "AWSCognitoIdentityProviderService.InitiateAuth",
      },
      body: JSON.stringify({
        AuthFlow: request.flow,
        ClientId: env.cognitoBootstrapClientId,
        AuthParameters: authParameters,
      }),
    });
  } catch {
    throw new CognitoAuthError("Network error reaching Cognito.");
  }

  // Cognito's JSON error/success bodies; checked field-by-field below.
  const payload = (await response.json().catch(() => ({}))) as InitiateAuthResponse;
  const result = payload.AuthenticationResult;
  if (!response.ok || !result) {
    throw new CognitoAuthError(payload.message ?? "Sign-in failed.");
  }

  return new User({
    id_token: result.IdToken,
    access_token: result.AccessToken,
    refresh_token:
      result.RefreshToken ??
      (request.flow === "REFRESH_TOKEN_AUTH" ? request.refreshToken : undefined),
    token_type: result.TokenType,
    expires_at: Math.floor(Date.now() / 1000) + result.ExpiresIn,
    profile: decodeIdTokenClaims(result.IdToken),
  });
}
