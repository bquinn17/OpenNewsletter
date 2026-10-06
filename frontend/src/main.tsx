import React from "react";
import ReactDOM from "react-dom/client";
import { MutationCache, QueryCache, QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { registerSW } from "virtual:pwa-register";
import { App } from "./App";
import { AuthProvider } from "./auth/AuthProvider";
import { ApiError } from "./api/client";
import { useToasts } from "./state/toast";
import "./styles/tailwind.css";

// `vite-plugin-pwa` (`injectManifest`, §14.2) — registers `src/pwa/sw.ts`.
// Aliased to a no-op in Vitest (`vite.config.ts`'s `test.alias`).
registerSW({ immediate: true });

function messageFor(error: unknown): string {
  if (error instanceof ApiError) return error.problem?.detail ?? error.message;
  if (error instanceof Error && error.message) return error.message;
  return "Something went wrong.";
}

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      refetchOnWindowFocus: false,
      retry: 1,
    },
  },
  queryCache: new QueryCache({
    onError: (error, query) => {
      // Only toast background refetch errors; the page itself renders an error
      // state for the initial load via React Query's error fields.
      if (query.state.data !== undefined) {
        useToasts.getState().push(messageFor(error), "error");
      }
    },
  }),
  mutationCache: new MutationCache({
    onError: (error, _vars, _ctx, mutation) => {
      // Per-mutation onError can opt out by setting `meta: { silent: true }`.
      if (mutation.meta && (mutation.meta as { silent?: boolean }).silent) return;
      useToasts.getState().push(messageFor(error), "error");
    },
  }),
});

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
