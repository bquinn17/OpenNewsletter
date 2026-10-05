export type AppEnv = "dev" | "prod";

type Env = {
  apiBaseUrl: string;
  cdnBaseUrl: string;
  cognitoUserPoolId: string;
  cognitoClientId: string;
  cognitoBootstrapClientId: string;
  cognitoHostedDomain: string;
  redirectUri: string;
  appEnv: AppEnv;
  buildSha: string;
  useMocks: boolean;
  /** Quiet period after the last keystroke before a draft autosaves (`04` §7.3, §8.2). */
  autosaveDebounceMs: number;
};

const raw = import.meta.env;
const useMocks = raw.VITE_USE_MOCKS === "true";

function required(value: string | undefined, name: string): string {
  if (!useMocks && !value) {
    throw new Error(`Missing required environment variable ${name} (set it in your .env file)`);
  }
  return value ?? "";
}

export const env: Env = {
  apiBaseUrl: required(raw.VITE_API_BASE_URL, "VITE_API_BASE_URL"),
  cdnBaseUrl: required(raw.VITE_CDN_BASE_URL, "VITE_CDN_BASE_URL"),
  cognitoUserPoolId: required(raw.VITE_COGNITO_USER_POOL_ID, "VITE_COGNITO_USER_POOL_ID"),
  cognitoClientId: required(raw.VITE_COGNITO_CLIENT_ID, "VITE_COGNITO_CLIENT_ID"),
  cognitoBootstrapClientId: required(
    raw.VITE_COGNITO_BOOTSTRAP_CLIENT_ID,
    "VITE_COGNITO_BOOTSTRAP_CLIENT_ID",
  ),
  cognitoHostedDomain: required(raw.VITE_COGNITO_HOSTED_DOMAIN, "VITE_COGNITO_HOSTED_DOMAIN"),
  // Falls back to same-origin `/auth/callback` at the call site (`auth/userManager.ts`),
  // so an empty string here is a valid "unset" value, not an error.
  redirectUri: raw.VITE_REDIRECT_URI ?? "",
  appEnv: raw.VITE_ENV === "prod" ? "prod" : "dev",
  buildSha: raw.VITE_BUILD_SHA ?? "dev",
  useMocks,
  autosaveDebounceMs: Math.max(1500, Number(raw.VITE_AUTOSAVE_DEBOUNCE_MS) || 1500),
};
