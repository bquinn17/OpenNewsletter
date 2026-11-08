import { describe, expect, it } from "vitest";
import type { components } from "../types/api";
import { ApiError } from "./client";
import { mockLeaderboardRoute } from "./mockLeaderboard";
import { CALLER_ID, NO_MOCK_ROUTE, PUBLISHED_CYCLE_ID } from "./mockShared";

type S = components["schemas"];

function req(rawPath: string): {
  method: "GET";
  rawPath: string;
  segments: string[];
  query: URLSearchParams;
  body: unknown;
} {
  return {
    method: "GET",
    rawPath,
    segments: rawPath.split("/").filter(Boolean),
    query: new URLSearchParams(),
    body: undefined,
  };
}

describe("mockLeaderboardRoute", () => {
  it("returns NO_MOCK_ROUTE for an unrelated path", () => {
    expect(mockLeaderboardRoute(req("/groups/g_trail/newsletters"))).toBe(NO_MOCK_ROUTE);
  });

  it("403s for an unknown group", () => {
    expect(() => mockLeaderboardRoute(req("/groups/g_nope/leaderboard"))).toThrow(ApiError);
  });

  it("returns one MemberStatsResponse per current member, clamped shapes and non-negative", () => {
    const result = mockLeaderboardRoute(
      req("/groups/g_trail/leaderboard"),
    ) as S["LeaderboardResponse"];
    expect(result.members.map((m) => m.userId).sort()).toEqual([CALLER_ID, "u_sam"].sort());
    for (const member of result.members) {
      expect(member.editionsAnswered).toBeGreaterThanOrEqual(0);
      expect(member.reactionsReceived).toBeGreaterThanOrEqual(0);
      expect(member.reactionsGiven).toBeGreaterThanOrEqual(0);
      expect(member.commentsWritten).toBeGreaterThanOrEqual(0);
      expect(member.questionsAsked).toBeGreaterThanOrEqual(0);
      expect(member.currentStreak).toBeGreaterThanOrEqual(0);
      expect(member.longestStreak).toBeGreaterThanOrEqual(0);
    }
  });

  it("derives commentsWritten from the published cycle's live (non-deleted) comments", () => {
    const result = mockLeaderboardRoute(
      req("/groups/g_trail/leaderboard"),
    ) as S["LeaderboardResponse"];
    // `g_trail`'s rich published cycle seeds one live comment from the caller
    // (`cmt_seed_1`) and one soft-deleted comment from a non-member (`cmt_seed_2`, Alex).
    const me = result.members.find((m) => m.userId === CALLER_ID);
    expect(me?.commentsWritten).toBeGreaterThanOrEqual(1);
  });

  it("derives topAnswers from answers with at least one reaction, most reactions first, capped at 3", () => {
    const result = mockLeaderboardRoute(
      req("/groups/g_trail/leaderboard"),
    ) as S["LeaderboardResponse"];
    expect(result.topAnswers.length).toBeGreaterThan(0);
    expect(result.topAnswers.length).toBeLessThanOrEqual(3);
    for (const answer of result.topAnswers) {
      expect(answer.reactionCount).toBeGreaterThan(0);
      expect(answer.cycleId).toBe(PUBLISHED_CYCLE_ID);
    }
    const counts = result.topAnswers.map((a) => a.reactionCount);
    expect(counts).toEqual([...counts].sort((a, b) => b - a));
  });

  it("a single-member group (g_game) still returns a populated member row", () => {
    const result = mockLeaderboardRoute(
      req("/groups/g_game/leaderboard"),
    ) as S["LeaderboardResponse"];
    expect(result.members).toHaveLength(1);
    expect(result.members[0]!.userId).toBe(CALLER_ID);
  });
});
