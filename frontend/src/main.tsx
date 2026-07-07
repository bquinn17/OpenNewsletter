import React from "react";
import ReactDOM from "react-dom/client";
import { MutationCache, QueryCache, QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { App } from "./App";
import { AuthProvider } from "./auth/AuthProvider";
import { useToasts } from "./state/toast";
import "./styles/tailwind.css";

function messageFor(error: unknown): string {
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
      <ReactQueryDevtools initialIsOpen={false} buttonPosition="bottom-left" />
    </QueryClientProvider>
  </React.StrictMode>,
);
