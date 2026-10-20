type CspEnvVars = {
  mediaBaseUrl?: string;
  apiBaseUrl?: string;
  cognitoHostedDomain?: string;
  /** Space-separated list of upload origins (`VITE_UPLOAD_ORIGINS`) — the two originals buckets. */
  uploadOrigins?: string;
};

/** The origin of `url`, or `null` when it's unset, empty or unparseable. */
function originOf(url: string | undefined): string | null {
  if (!url) return null;
  try {
    return new URL(url).origin;
  } catch {
    return null;
  }
}

/** Each whitespace-separated URL in `value`, reduced to its origin; invalid/empty entries dropped. */
function originsOf(value: string | undefined): string[] {
  if (!value) return [];
  return value
    .split(/\s+/)
    .map((url) => originOf(url))
    .filter((origin): origin is string => origin !== null);
}

/**
 * Builds the `Content-Security-Policy` directive string for `index.html`
 * (`docs/design/frontend.md` §12.1). A pure function of the env vars so it's unit-testable without
 * spinning up Vite — `cspPlugin` below is the only caller in the real build.
 *
 * `worker-src`/`manifest-src` aren't in the `docs/design/frontend.md` §12.1 policy text; added
 * here because the service worker and PWA manifest need them and a `meta`
 * CSP with neither falls back to `default-src`, which would also work, but
 * being explicit documents the requirement.
 */
export function buildCspPolicy(vars: CspEnvVars): string {
  const mediaOrigin = originOf(vars.mediaBaseUrl);
  const apiOrigin = originOf(vars.apiBaseUrl);
  const hostedOrigin = vars.cognitoHostedDomain ? `https://${vars.cognitoHostedDomain}` : null;
  const uploadOrigins = originsOf(vars.uploadOrigins);

  const directives: Array<[string, Array<string | null>]> = [
    ["default-src", ["'self'"]],
    ["script-src", ["'self'"]],
    // Google Fonts: `index.html` links the stylesheet, which pulls the font
    // files from gstatic (decided M13, `docs/design/frontend.md` §12.1).
    ["style-src", ["'self'", "'unsafe-inline'", "https://fonts.googleapis.com"]],
    ["img-src", ["'self'", "data:", "blob:", mediaOrigin]],
    [
      "connect-src",
      [
        "'self'",
        apiOrigin,
        ...uploadOrigins,
        "https://cognito-idp.us-east-1.amazonaws.com",
        hostedOrigin,
      ],
    ],
    ["font-src", ["'self'", "https://fonts.gstatic.com"]],
    ["base-uri", ["'self'"]],
    ["form-action", ["'self'", hostedOrigin]],
    ["worker-src", ["'self'"]],
    ["manifest-src", ["'self'"]],
  ];

  return directives
    .map(([name, values]) => `${name} ${values.filter((v): v is string => !!v).join(" ")}`)
    .join("; ");
}

/**
 * Matches the charset `<meta>` so the CSP tag can be inserted right after it
 * — kept here (not in `cspPlugin.ts`) so it's exercised by the same
 * Vitest-only module as `buildCspPolicy`, with no `vite` import.
 */
export const CHARSET_META_RE = /(<meta\s+charset=["'][^"']*["']\s*\/?>)/i;

/** The `<meta>` tag `cspPlugin.ts` splices into `index.html` at build time. */
export function cspMetaTag(policy: string): string {
  return `<meta http-equiv="Content-Security-Policy" content="${policy}" />`;
}
