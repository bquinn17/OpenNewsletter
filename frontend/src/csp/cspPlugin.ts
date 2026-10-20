import type { Plugin } from "vite";
import { loadEnv } from "vite";
import { buildCspPolicy, cspMetaTag, CHARSET_META_RE } from "./csp";

/**
 * Injects the production CSP `<meta>` tag as the first child of `<head>`
 * after the charset meta. `apply: "build"` is what keeps this out of
 * `vite dev` entirely — under `vite dev`, `@vitejs/plugin-react`'s inline
 * preamble script would be blocked by `script-src 'self'`
 * (`docs/design/frontend.md` §13).
 *
 * Split out from `csp.ts` (the pure, Vitest-covered policy builder) because
 * importing the `vite` package in a file that a jsdom-environment test also
 * imports trips esbuild's startup self-check under jsdom's `TextEncoder`.
 */
export function cspPlugin(): Plugin {
  let policy = "";
  return {
    name: "opennewsletter-csp",
    apply: "build",
    configResolved(config) {
      const env = loadEnv(config.mode, config.envDir, "VITE_");
      policy = buildCspPolicy({
        mediaBaseUrl: env.VITE_MEDIA_BASE_URL,
        apiBaseUrl: env.VITE_API_BASE_URL,
        cognitoHostedDomain: env.VITE_COGNITO_HOSTED_DOMAIN,
        uploadOrigins: env.VITE_UPLOAD_ORIGINS,
      });
    },
    transformIndexHtml(html) {
      return html.replace(CHARSET_META_RE, `$1\n    ${cspMetaTag(policy)}`);
    },
  };
}
