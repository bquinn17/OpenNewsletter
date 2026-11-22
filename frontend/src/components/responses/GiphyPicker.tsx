import { useEffect, useRef, useState, type KeyboardEvent, type RefObject } from "react";
import { createPortal } from "react-dom";
import {
  GiphyOfflineError,
  GiphyRateLimitedError,
  isAllowedGiphyRating,
  searchGifs,
  SEARCH_PAGE_SIZE,
  type GiphyGif,
} from "../../giphy/giphyClient";
import { giphyLimitRemainingMs, useGiphyLimit } from "../../state/giphyLimit";
import { Spinner } from "../ui/Spinner";

/** Hard-coded so opening the picker costs no API call (`docs/design/frontend.md` §18.3/§9.3). */
const SUGGESTION_CHIPS = ["lol", "yes", "wow", "thank you", "celebrate"];
const SM_BREAKPOINT_PX = 640;

type SearchState =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "results"; query: string; gifs: GiphyGif[]; offset: number; hasMore: boolean }
  | { kind: "empty"; query: string }
  | { kind: "offline" }
  | { kind: "error" };

type Props = {
  anchorRef: RefObject<HTMLElement | null>;
  onPick: (gif: GiphyGif) => void;
  onClose: () => void;
};

function minutesLeft(ms: number): number {
  return Math.max(1, Math.ceil(ms / 60_000));
}

function mergeResults(
  prev: SearchState,
  query: string,
  gifs: GiphyGif[],
  offset: number,
  hasMore: boolean,
): SearchState {
  if (offset === 0 || prev.kind !== "results" || prev.query !== query) {
    return { kind: "results", query, gifs, offset, hasMore };
  }
  return { kind: "results", query, gifs: [...prev.gifs, ...gifs], offset, hasMore };
}

/**
 * The embedded GIPHY search (`docs/design/frontend.md` §18.3): a popover on desktop, a bottom
 * sheet on mobile, with no search-as-you-type — a search fires only on the button, Enter, a
 * chip, or More GIFs. A query already searched while the picker is open is served from an
 * in-memory cache rather than a repeat request.
 */
export function GiphyPicker({ anchorRef, onPick, onClose }: Props) {
  const [query, setQuery] = useState("");
  const [state, setState] = useState<SearchState>({ kind: "idle" });
  const cacheRef = useRef(new Map<string, { gifs: GiphyGif[]; hasMore: boolean }>());
  const popRef = useRef<HTMLDivElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const until = useGiphyLimit((s) => s.until);
  const [, forceTick] = useState(0);

  const rateLimited = until !== null && until > Date.now();

  // Live countdown while the shared cooldown is active, so the "try again in N minutes" copy
  // and the disabled controls count down rather than freezing.
  useEffect(() => {
    if (until === null) return;
    const id = setInterval(() => forceTick((t) => t + 1), 1000);
    return () => clearInterval(id);
  }, [until]);

  useEffect(() => {
    searchInputRef.current?.focus();
  }, []);

  useEffect(() => {
    function reposition() {
      const el = popRef.current;
      const anchor = anchorRef.current;
      if (!el || !anchor || window.innerWidth < SM_BREAKPOINT_PX) return;
      const rect = anchor.getBoundingClientRect();
      const popW = 360;
      const popH = 420;
      let left = rect.left;
      left = Math.max(8, Math.min(left, window.innerWidth - popW - 8));
      const top = Math.max(8, rect.top - popH - 8);
      el.style.left = `${left}px`;
      el.style.top = `${top}px`;
    }
    reposition();
    window.addEventListener("resize", reposition);
    return () => window.removeEventListener("resize", reposition);
  }, [anchorRef]);

  useEffect(() => {
    function onKeyDown(e: globalThis.KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    function onMouseDown(e: MouseEvent) {
      const anchor = anchorRef.current;
      const target = e.target as Node;
      if (
        popRef.current &&
        !popRef.current.contains(target) &&
        !(anchor && anchor.contains(target))
      ) {
        onClose();
      }
    }
    document.addEventListener("keydown", onKeyDown);
    document.addEventListener("mousedown", onMouseDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      document.removeEventListener("mousedown", onMouseDown);
    };
  }, [anchorRef, onClose]);

  async function runSearch(rawQuery: string, offset: number) {
    const trimmed = rawQuery.trim();
    if (!trimmed || rateLimited) return;
    const cacheKey = `${trimmed.toLowerCase()}:${offset}`;
    const cached = cacheRef.current.get(cacheKey);
    if (cached) {
      setState((prev) => mergeResults(prev, trimmed, cached.gifs, offset, cached.hasMore));
      return;
    }
    setState({ kind: "loading" });
    try {
      const result = await searchGifs(trimmed, offset);
      cacheRef.current.set(cacheKey, result);
      if (result.gifs.length === 0 && offset === 0) {
        setState({ kind: "empty", query: trimmed });
      } else {
        setState((prev) => mergeResults(prev, trimmed, result.gifs, offset, result.hasMore));
      }
    } catch (err) {
      if (err instanceof GiphyRateLimitedError) {
        setState({ kind: "idle" });
      } else if (err instanceof GiphyOfflineError) {
        setState({ kind: "offline" });
      } else {
        setState({ kind: "error" });
      }
    }
  }

  function submitSearch() {
    void runSearch(query, 0);
  }

  function pickChip(chip: string) {
    setQuery(chip);
    void runSearch(chip, 0);
  }

  function loadMore() {
    if (state.kind !== "results") return;
    void runSearch(state.query, state.offset + SEARCH_PAGE_SIZE);
  }

  function handleGridKeyDown(e: KeyboardEvent<HTMLUListElement>) {
    if (!["ArrowRight", "ArrowLeft", "ArrowDown", "ArrowUp"].includes(e.key)) return;
    e.preventDefault();
    const tiles = Array.from(
      popRef.current?.querySelectorAll<HTMLButtonElement>('[role="option"]') ?? [],
    );
    if (tiles.length === 0) return;
    const current = tiles.indexOf(document.activeElement as HTMLButtonElement);
    const delta = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : -1;
    const next = current === -1 ? 0 : (current + delta + tiles.length) % tiles.length;
    tiles[next]?.focus();
  }

  return createPortal(
    <div
      ref={popRef}
      role="dialog"
      aria-label="Search GIPHY"
      className="fixed inset-x-0 bottom-0 z-50 flex h-[70vh] flex-col rounded-t-3xl border border-line bg-white shadow-pop sm:inset-x-auto sm:bottom-auto sm:h-[420px] sm:w-[360px] sm:rounded-2xl"
    >
      <div className="flex items-center gap-2 border-b border-line p-3">
        <label htmlFor="giphy-search-input" className="sr-only">
          Search GIPHY
        </label>
        <input
          id="giphy-search-input"
          ref={searchInputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              submitSearch();
            }
          }}
          placeholder="Search GIPHY"
          disabled={rateLimited}
          className="min-w-0 flex-1 rounded-full border border-line bg-cream px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-coral/30"
        />
        <button
          type="button"
          aria-label="Search GIPHY"
          onClick={submitSearch}
          disabled={!query.trim() || rateLimited || state.kind === "loading"}
          className="grid h-9 w-9 shrink-0 place-items-center rounded-full bg-grape text-white disabled:opacity-40"
        >
          🔍
        </button>
      </div>

      {rateLimited ? (
        <div
          role="status"
          className="m-3 rounded-2xl border border-sun/60 bg-sun/20 p-3 text-sm text-ink"
        >
          <p className="font-semibold">GIF limit reached.</p>
          <p className="mt-1 text-xs">
            GIPHY lets this app make a limited number of requests each hour, shared by everyone
            using it. Try again in {minutesLeft(giphyLimitRemainingMs())} minutes.
          </p>
        </div>
      ) : (
        <div className="flex flex-wrap gap-1.5 px-3 pt-2">
          {SUGGESTION_CHIPS.map((chip) => (
            <button
              key={chip}
              type="button"
              onClick={() => pickChip(chip)}
              className="rounded-full border border-line px-2.5 py-1 text-xs font-semibold text-inkmuted hover:bg-cream"
            >
              {chip}
            </button>
          ))}
        </div>
      )}

      <div className="flex-1 overflow-y-auto px-3 py-2">
        {state.kind === "idle" && !rateLimited && (
          <p className="pt-10 text-center text-sm text-inkmuted">Search for a GIF</p>
        )}
        {state.kind === "loading" && (
          <div className="flex justify-center pt-10">
            <Spinner />
          </div>
        )}
        {state.kind === "empty" && (
          <p className="pt-10 text-center text-sm text-inkmuted">
            No GIFs for &quot;{state.query}&quot;
          </p>
        )}
        {state.kind === "offline" && (
          <p className="pt-10 text-center text-sm text-inkmuted">
            You&apos;re offline. GIF search needs a connection.
          </p>
        )}
        {state.kind === "error" && (
          <div className="pt-10 text-center text-sm text-inkmuted">
            <p>Couldn&apos;t reach GIPHY</p>
            <button type="button" onClick={submitSearch} className="mt-2 font-semibold text-grape">
              Retry
            </button>
          </div>
        )}
        {state.kind === "results" && (
          <>
            <ul
              role="listbox"
              aria-label="GIF results"
              tabIndex={-1}
              onKeyDown={handleGridKeyDown}
              className="grid grid-cols-2 gap-2 sm:grid-cols-3"
            >
              {state.gifs
                .filter((gif) => isAllowedGiphyRating(gif.rating))
                .map((gif) => (
                  <li key={gif.id}>
                    <button
                      type="button"
                      role="option"
                      aria-selected={false}
                      aria-label={gif.title || "GIF"}
                      onClick={() => onPick(gif)}
                      className="block aspect-square w-full overflow-hidden rounded-xl bg-cream"
                    >
                      <img
                        src={gif.stillUrl || gif.videoUrl}
                        alt={gif.title}
                        referrerPolicy="no-referrer"
                        crossOrigin="anonymous"
                        className="h-full w-full object-cover"
                      />
                    </button>
                  </li>
                ))}
            </ul>
            {state.hasMore && (
              <div className="flex justify-center py-3">
                <button
                  type="button"
                  onClick={loadMore}
                  disabled={rateLimited}
                  className="rounded-full bg-cream px-4 py-1.5 text-xs font-bold text-ink disabled:opacity-40"
                >
                  More GIFs
                </button>
              </div>
            )}
          </>
        )}
      </div>

      <div className="border-t border-line px-3 py-2 text-center text-[10px] font-bold uppercase tracking-wide text-inkmuted">
        Powered by GIPHY
      </div>
    </div>,
    document.body,
  );
}
