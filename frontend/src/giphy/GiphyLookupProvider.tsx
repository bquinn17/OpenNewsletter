import { createContext, useContext, useMemo, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { getGifsByIds, GiphyRateLimitedError, type GiphyGif } from "./giphyClient";

export type GiphyLookupEntry =
  | { status: "loading" }
  | { status: "rate-limited" }
  | { status: "error" }
  /** `gif: null` means GIPHY no longer returns this id — "GIF unavailable" (`docs/design/frontend.md` §18.3). */
  | { status: "ready"; gif: GiphyGif | null };

type ContextValue = {
  get: (id: string) => GiphyLookupEntry;
  reload: () => void;
};

const GiphyLookupContext = createContext<ContextValue | null>(null);

/** The resolved GIF for one `giphy:{id}` token, from the nearest `GiphyLookupProvider`. */
export function useGiphyGif(id: string): GiphyLookupEntry {
  const ctx = useContext(GiphyLookupContext);
  return ctx ? ctx.get(id) : { status: "loading" };
}

type Props = { ids: string[]; children: ReactNode };

/**
 * One batched `GET https://api.giphy.com/v1/gifs?ids=…` per page (`docs/design/frontend.md`
 * §18.3): `ids` is every `giphy:` id the page renders, across every answer and comment, collected
 * once by the caller (e.g. `NewsletterPage`). A page with no GIFs makes no call. Cached with
 * `staleTime: Infinity` in React Query memory only — never persisted, so re-opening the page in a
 * new session re-fetches, but returning to it in the same session costs nothing.
 */
export function GiphyLookupProvider({ ids, children }: Props) {
  const sortedIds = useMemo(() => [...new Set(ids)].sort(), [ids]);
  const {
    data: gifs,
    isLoading,
    error,
    refetch,
  } = useQuery({
    queryKey: ["giphyLookup", sortedIds],
    queryFn: () => getGifsByIds(sortedIds),
    enabled: sortedIds.length > 0,
    staleTime: Infinity,
    retry: false,
  });

  const byId = useMemo(() => new Map((gifs ?? []).map((g) => [g.id, g])), [gifs]);

  const value = useMemo<ContextValue>(
    () => ({
      get(id) {
        if (!sortedIds.includes(id)) return { status: "ready", gif: null };
        if (error instanceof GiphyRateLimitedError) return { status: "rate-limited" };
        if (error) return { status: "error" };
        if (isLoading) return { status: "loading" };
        return { status: "ready", gif: byId.get(id) ?? null };
      },
      reload: () => void refetch(),
    }),
    [sortedIds, error, isLoading, byId, refetch],
  );

  return <GiphyLookupContext.Provider value={value}>{children}</GiphyLookupContext.Provider>;
}
