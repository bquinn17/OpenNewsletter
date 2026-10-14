export type AppEnv = "dev" | "prod";

type Env = {
  apiBaseUrl: string;
  cdnBaseUrl: string;
  cognitoUserPoolId: string;
  cognitoClientId: string;
  /**
   * The `admin-bootstrap` client, or `null`. Dev only: prod has no such client
   * (D5, `docs/design/auth.md` §9.2) and its build doesn't pass the variable, so a
   * prod build ignores it even if it's set.
   */
  cognitoBootstrapClientId: string | null;
  /** Whether `/admin/bootstrap-login` (password sign-in) exists: dev, with a bootstrap client. */
  bootstrapLoginEnabled: boolean;
  cognitoHostedDomain: string;
  redirectUri: string;
  appEnv: AppEnv;
  buildSha: string;
  useMocks: boolean;
  /** Quiet period after the last keystroke before a draft autosaves (`docs/design/frontend.md` §7.3, §8.2). */
  autosaveDebounceMs: number;
};

/** The `VITE_*` variables `buildEnv` reads; every one may be missing. */
export type RawEnv = Partial<Record<keyof ImportMetaEnv, string | undefined>>;

/**
 * Validates and normalizes the build-time env. A pure function of `raw`
 * (rather than reading `import.meta.env` inline) so it's unit-testable.
 * Throws on a missing required variable outside mock mode.
 */
export function buildEnv(raw: RawEnv): Env {
  const useMocks = raw.VITE_USE_MOCKS === "true";
  function required(value: string | undefined, name: string): string {
    if (!useMocks && !value) {
      throw new Error(`Missing required environment variable ${name} (set it in your .env file)`);
    }
    return value ?? "";
  }

  const appEnv: AppEnv = raw.VITE_ENV === "prod" ? "prod" : "dev";
  const cognitoBootstrapClientId =
    appEnv === "dev" && raw.VITE_COGNITO_BOOTSTRAP_CLIENT_ID
      ? raw.VITE_COGNITO_BOOTSTRAP_CLIENT_ID
      : null;

  return {
    apiBaseUrl: required(raw.VITE_API_BASE_URL, "VITE_API_BASE_URL"),
    cdnBaseUrl: required(raw.VITE_CDN_BASE_URL, "VITE_CDN_BASE_URL"),
    cognitoUserPoolId: required(raw.VITE_COGNITO_USER_POOL_ID, "VITE_COGNITO_USER_POOL_ID"),
    cognitoClientId: required(raw.VITE_COGNITO_CLIENT_ID, "VITE_COGNITO_CLIENT_ID"),
    cognitoBootstrapClientId,
    bootstrapLoginEnabled: cognitoBootstrapClientId !== null,
    cognitoHostedDomain: required(raw.VITE_COGNITO_HOSTED_DOMAIN, "VITE_COGNITO_HOSTED_DOMAIN"),
    // Falls back to same-origin `/auth/callback` at the call site (`auth/userManager.ts`),
    // so an empty string here is a valid "unset" value, not an error.
    redirectUri: raw.VITE_REDIRECT_URI ?? "",
    appEnv,
    buildSha: raw.VITE_BUILD_SHA ?? "dev",
    useMocks,
    autosaveDebounceMs: Math.max(1500, Number(raw.VITE_AUTOSAVE_DEBOUNCE_MS) || 1500),
  };
}

export const env: Env = buildEnv(import.meta.env);
