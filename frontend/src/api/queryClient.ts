import { MutationCache, QueryCache, QueryClient } from "@tanstack/react-query";
import { apiErrorMessage } from "./client";
import { useToasts } from "../state/toast";

function messageFor(error: unknown): string {
  return apiErrorMessage(
    error,
    error instanceof Error && error.message ? error.message : "Something went wrong.",
  );
}

/**
 * The app's one TanStack Query client. Pulled out of `main.tsx` so
 * `utils/media.ts#refreshMediaUrls` can invalidate its queries without
 * importing a React entry point (`docs/design/media.md` §4.4).
 */
export const queryClient = new QueryClient({
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
