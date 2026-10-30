import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClientProvider } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { registerSW } from "virtual:pwa-register";
import { App } from "./App";
import { AuthProvider } from "./auth/AuthProvider";
import { queryClient } from "./api/queryClient";
import "./styles/tailwind.css";

// `vite-plugin-pwa` (`injectManifest`, §14.2) — registers `src/pwa/sw.ts`.
// Aliased to a no-op in Vitest (`vite.config.ts`'s `test.alias`).
registerSW({ immediate: true });

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <AuthProvider>
        <App />
      </AuthProvider>
      {import.meta.env.DEV && (
        <ReactQueryDevtools initialIsOpen={false} buttonPosition="bottom-left" />
      )}
    </QueryClientProvider>
  </React.StrictMode>,
);
