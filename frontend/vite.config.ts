/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  server: { port: 5173 },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    env: {
      VITE_USE_MOCKS: "false",
      VITE_API_BASE_URL: "https://api.test.invalid",
      VITE_CDN_BASE_URL: "https://cdn.test.invalid",
      VITE_COGNITO_USER_POOL_ID: "us-east-1_TEST",
      VITE_COGNITO_CLIENT_ID: "test-client-id",
      VITE_COGNITO_BOOTSTRAP_CLIENT_ID: "test-bootstrap-client-id",
      VITE_COGNITO_HOSTED_DOMAIN: "test.auth.us-east-1.amazoncognito.com",
    },
  },
});
