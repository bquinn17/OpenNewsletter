import { env } from "../env";
import { isMockMode } from "../demo";
import { useGiphyLimit, isGiphyRateLimited } from "../state/giphyLimit";
import { mockGiphyGetByIds, mockGiphySearch } from "./mockGiphy";

/** A `giphy:{gifId}` token's id must match this before it's looked up (`docs/design/frontend.md` §18.3). */
export const GIPHY_ID_RE = /^[A-Za-z0-9]+$/;

/** Ratings at or below this are shown; anything above (or unrecognized) is hidden (`docs/design/frontend.md` §18.3). */
const ALLOWED_RATINGS = new Set(["g", "pg", "pg-13"]);

export function isAllowedGiphyRating(rating: string): boolean {
  return ALLOWED_RATINGS.has(rating.toLowerCase());
}

export type GiphyGif = {
  id: string;
  title: string;
  rating: string;
  width: number;
  height: number;
  /** The `fixed_width` mp4 rendition (or, in mock/demo mode, a GIF served from `public/demo-media/`). */
  videoUrl: string;
  /** The `fixed_width_still` rendition (or, in mock/demo mode, a local single-frame GIF). */
  stillUrl: string;
};

export class GiphyRateLimitedError extends Error {
  constructor() {
    super("GIPHY's shared hourly allowance is used up.");
    this.name = "GiphyRateLimitedError";
  }
}

export class GiphyOfflineError extends Error {
  constructor() {
    super("You're offline. GIF search needs a connection.");
    this.name = "GiphyOfflineError";
  }
}

export class GiphyError extends Error {
  constructor(message = "Couldn't reach GIPHY") {
    super(message);
    this.name = "GiphyError";
  }
}

const SEARCH_RATING = "pg-13";
export const SEARCH_PAGE_SIZE = 50;

type RawGiphyImage = { width?: string; height?: string; mp4?: string; webp?: string; url?: string };
type RawGiphyGif = {
  id: string;
  title?: string;
  rating?: string;
  images?: { fixed_width?: RawGiphyImage; fixed_width_still?: RawGiphyImage };
};
type RawSearchResponse = { data?: RawGiphyGif[]; pagination?: { total_count?: number } };
type RawGetResponse = { data?: RawGiphyGif[] | RawGiphyGif };

function toGif(raw: RawGiphyGif): GiphyGif {
  const fixed = raw.images?.fixed_width;
  const still = raw.images?.fixed_width_still;
  return {
    id: raw.id,
    title: raw.title ?? "",
    rating: raw.rating ?? "unrated",
    width: Number(fixed?.width ?? 0),
    height: Number(fixed?.height ?? 0),
    videoUrl: fixed?.mp4 ?? "",
    stillUrl: still?.webp ?? still?.url ?? fixed?.webp ?? "",
  };
}

/**
 * The one real-network entry point for every GIPHY call (`docs/design/frontend.md` §18.6):
 * no referrer, no cookies, and the shared rate-limit store checked before and updated after.
 */
async function giphyFetch(path: string, params: Record<string, string>): Promise<unknown> {
  if (isGiphyRateLimited()) throw new GiphyRateLimitedError();
  if (typeof navigator !== "undefined" && navigator.onLine === false) throw new GiphyOfflineError();
  const key = env.giphyApiKey;
  if (!key) throw new GiphyError("No GIPHY API key is configured.");

  const query = new URLSearchParams({ api_key: key, ...params });
  let response: Response;
  try {
    response = await fetch(`https://api.giphy.com/v1/gifs${path}?${query.toString()}`, {
      referrerPolicy: "no-referrer",
      credentials: "omit",
    });
  } catch {
    throw new GiphyError();
  }

  if (response.status === 429) {
    useGiphyLimit.getState().markRateLimited();
    throw new GiphyRateLimitedError();
  }
  if (!response.ok) throw new GiphyError();
  return response.json();
}

/** GIPHY's Search endpoint, sent only on an explicit action (`docs/design/frontend.md` §18.3). */
export async function searchGifs(
  query: string,
  offset = 0,
): Promise<{ gifs: GiphyGif[]; hasMore: boolean }> {
  if (isMockMode) return mockGiphySearch(query, offset);
  const data = (await giphyFetch("/search", {
    q: query,
    rating: SEARCH_RATING,
    limit: String(SEARCH_PAGE_SIZE),
    offset: String(offset),
  })) as RawSearchResponse;
  const gifs = (data.data ?? []).map(toGif);
  const totalCount = data.pagination?.total_count ?? offset + gifs.length;
  return { gifs, hasMore: offset + gifs.length < totalCount };
}

/** GIPHY's Get GIFs by ID endpoint, one batched call per page (`docs/design/frontend.md` §18.3). */
export async function getGifsByIds(ids: string[]): Promise<GiphyGif[]> {
  if (ids.length === 0) return [];
  if (isMockMode) return mockGiphyGetByIds(ids);
  const data = (await giphyFetch("", { ids: ids.join(",") })) as RawGetResponse;
  const raw = Array.isArray(data.data) ? data.data : data.data ? [data.data] : [];
  return raw.map(toGif);
}
