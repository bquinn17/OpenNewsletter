import type { components } from "../types/api";
import { ApiError, type ProblemDetails } from "./client";

type S = components["schemas"];
export type Method = "GET" | "POST" | "PUT" | "PATCH" | "DELETE";

/**
 * State and helpers shared by the mock transport's per-feature route modules
 * (`mockCandidates.ts`, `mockNewsletters.ts`).
 */

export const CALLER_ID = "u_quinn";

// 1x1 transparent PNG, duplicated from `mockTransport.ts`'s own copy (not
// exported there) — stands in for a real CDN asset so published/draft images
// render without any network request in mock mode.
export const MOCK_IMAGE_DATA_URL =
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUAAWJDR3EAAAAASUVORK5CYII=";

/** Returned by a per-feature mock route that doesn't handle the request. */
export const NO_MOCK_ROUTE = Symbol("NO_MOCK_ROUTE");

export type MockRequest = {
  method: Method;
  rawPath: string;
  /** Path segments, still URL-encoded. */
  segments: string[];
  query: URLSearchParams;
  body: unknown;
};

export type MockRoute = (request: MockRequest) => unknown;

function problemFor(status: number, code: string, detail: string): ProblemDetails {
  return {
    type: `https://api.opennewsletter.example.com/errors/${code.toLowerCase().replace(/_/g, "-")}`,
    title: code,
    status,
    detail,
    code,
    correlationId: "mock-correlation-id",
  };
}

export function fail(status: number, code: string, detail: string): never {
  throw new ApiError(status, code, problemFor(status, code, detail));
}

const DAY_MS = 24 * 60 * 60 * 1000;
// Computed once at module load so every fixture date stays relative to "now"
// instead of drifting into the past as the real calendar moves on (M9 — the
// `open` cycle below must actually be open, and `voting` must actually be
// upcoming, whenever the mock app happens to run).
const loadedAt = Date.now();

function iso(offsetMs: number): string {
  return new Date(loadedAt + offsetMs).toISOString();
}

/** A `yyyymm`-shaped cycle id for the calendar month `offsetMonths` away from now. */
function monthCycleId(offsetMonths: number): string {
  const d = new Date(loadedAt);
  d.setUTCDate(1);
  d.setUTCMonth(d.getUTCMonth() + offsetMonths);
  return `${d.getUTCFullYear()}${String(d.getUTCMonth() + 1).padStart(2, "0")}`;
}

/** The cycle currently in its 4-day response window across every fixture group. */
export const OPEN_CYCLE_ID = monthCycleId(0);
/** The next cycle, still in its pre-open voting period. */
export const VOTING_CYCLE_ID = monthCycleId(1);
/** The most recently published cycle across every fixture group. */
export const PUBLISHED_CYCLE_ID = monthCycleId(-1);
/** An older published cycle (only `g_trail` has two). */
export const PUBLISHED_CYCLE_ID_2 = monthCycleId(-2);

export const newsletters: Record<string, S["NewsletterSummary"][]> = {
  g_trail: [
    {
      cycleId: VOTING_CYCLE_ID,
      status: "voting",
      responseOpenAt: iso(27 * DAY_MS),
      responseCloseAt: iso(27 * DAY_MS),
      publishedAt: null,
      questionCount: 0,
      myDraftCount: 0,
      myPublishedCount: 0,
    },
    {
      cycleId: OPEN_CYCLE_ID,
      status: "open",
      responseOpenAt: iso(-2 * DAY_MS),
      responseCloseAt: iso(2 * DAY_MS),
      publishedAt: null,
      questionCount: 5,
      myDraftCount: 1,
      myPublishedCount: 1,
    },
    {
      cycleId: PUBLISHED_CYCLE_ID,
      status: "published",
      responseOpenAt: iso(-32 * DAY_MS),
      responseCloseAt: iso(-28 * DAY_MS),
      publishedAt: iso(-28 * DAY_MS + 1000),
      questionCount: 5,
      myDraftCount: 0,
      myPublishedCount: 5,
    },
    {
      cycleId: PUBLISHED_CYCLE_ID_2,
      status: "published",
      responseOpenAt: iso(-62 * DAY_MS),
      responseCloseAt: iso(-58 * DAY_MS),
      publishedAt: iso(-58 * DAY_MS + 1000),
      questionCount: 2,
      myDraftCount: 0,
      myPublishedCount: 2,
    },
  ],
  g_game: [
    {
      cycleId: VOTING_CYCLE_ID,
      status: "voting",
      responseOpenAt: iso(27 * DAY_MS),
      responseCloseAt: iso(27 * DAY_MS),
      publishedAt: null,
      questionCount: 0,
      myDraftCount: 0,
      myPublishedCount: 0,
    },
    {
      cycleId: PUBLISHED_CYCLE_ID,
      status: "published",
      responseOpenAt: iso(-32 * DAY_MS),
      responseCloseAt: iso(-28 * DAY_MS),
      publishedAt: iso(-28 * DAY_MS + 1000),
      questionCount: 2,
      myDraftCount: 0,
      myPublishedCount: 2,
    },
  ],
  g_meeple: [
    {
      cycleId: OPEN_CYCLE_ID,
      status: "open",
      responseOpenAt: iso(-2 * DAY_MS),
      responseCloseAt: iso(2 * DAY_MS),
      publishedAt: null,
      questionCount: 4,
      myDraftCount: 2,
      myPublishedCount: 0,
    },
    {
      cycleId: PUBLISHED_CYCLE_ID,
      status: "published",
      responseOpenAt: iso(-33 * DAY_MS),
      responseCloseAt: iso(-29 * DAY_MS),
      publishedAt: iso(-29 * DAY_MS + 1000),
      questionCount: 2,
      myDraftCount: 0,
      myPublishedCount: 2,
    },
  ],
};
