/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_API_BASE_URL: string;
  readonly VITE_CDN_BASE_URL: string;
  readonly VITE_COGNITO_USER_POOL_ID: string;
  readonly VITE_COGNITO_CLIENT_ID: string;
  readonly VITE_COGNITO_BOOTSTRAP_CLIENT_ID: string;
  readonly VITE_COGNITO_HOSTED_DOMAIN: string;
  readonly VITE_REDIRECT_URI: string;
  readonly VITE_ENV: string;
  readonly VITE_BUILD_SHA: string;
  readonly VITE_USE_MOCKS: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
