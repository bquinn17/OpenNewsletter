/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import basicSsl from "@vitejs/plugin-basic-ssl";
import { VitePWA } from "vite-plugin-pwa";

// `npm run dev:https` (`vite --mode https`) is the only mode that gets the
// self-signed cert — Web Push's `PushManager.subscribe`/`Notification.requestPermission`
// need a secure context, but plain `npm run dev` (http, mocks) shouldn't pay
// the cert-trust tax for everyday UI work (`04-frontend-architecture.md` §14.6).
export default defineConfig(({ mode }) => ({
  plugins: [
    react(),
    ...(mode === "https" ? [basicSsl()] : []),
    VitePWA({
      strategies: "injectManifest",
      srcDir: "src/pwa",
      filename: "sw.ts",
      injectRegister: false, // `main.tsx` registers via `virtual:pwa-register` itself (§14.2).
      manifest: {
        name: "OpenNewsletter",
        short_name: "Newsletter",
        description: "Monthly newsletters with friends",
        start_url: "/",
        display: "standalone",
        background_color: "#FBF6EE",
        theme_color: "#FBF6EE",
        icons: [
          { src: "/icon-192.png", sizes: "192x192", type: "image/png" },
          { src: "/icon-512.png", sizes: "512x512", type: "image/png" },
          {
            src: "/icon-maskable-512.png",
            sizes: "512x512",
            type: "image/png",
            purpose: "maskable",
          },
        ],
      },
      injectManifest: {
        // The precache manifest only needs the build's own hashed assets —
        // CDN images and API responses are handled (or deliberately not
        // handled, D11) by the SW's own runtime routes.
        globPatterns: ["**/*.{js,css,html,svg}"],
      },
      devOptions: {
        // Keep the SW out of the way during `vite dev`/mock mode; it's only
        // meaningful against a built, served app (`npm run build && npm run preview`).
        enabled: false,
      },
    }),
  ],
  server: { port: 5173 },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    // `virtual:pwa-register` only resolves under `vite dev`/`vite build`; in
    // tests, `main.tsx`'s import is redirected to a no-op stand-in so
    // registering the real SW/Workbox machinery never runs under Vitest.
    // Relative to this config file's directory — no `node:url` needed.
    alias: {
      "virtual:pwa-register": "./src/test/virtualPwaRegisterMock.ts",
    },
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
}));
