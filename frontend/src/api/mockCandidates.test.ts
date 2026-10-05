import { describe, expect, it } from "vitest";
import { ApiError } from "./client";
import { mockCandidateRoute } from "./mockCandidates";
import { NO_MOCK_ROUTE, VOTING_CYCLE_ID, type MockRequest } from "./mockShared";
import type { components } from "../types/api";

// This module's routes hold stateful, in-memory pools shared across the
// whole mock session (mirroring a real API's persisted state), so these
// tests share that module-level state too. Each mutating test is written to
// be symmetric (it restores what it changed) or additive-only in a way that
// doesn't affect other tests' assertions (which check relative properties —
// sort order, redaction, cap behavior — not exact counts).

type S = components["schemas"];

function req(
  method: MockRequest["method"],
  rawPath: string,
  body?: unknown,
  queryString = "",
): MockRequest {
  return {
    method,
    rawPath,
    segments: rawPath.split("/").filter(Boolean),
    query: new URLSearchParams(queryString),
    body,
  };
}

function listCandidates(groupId: string, queryString = ""): S["CandidateListResponse"] {
  return mockCandidateRoute(
    req("GET", `/groups/${groupId}/candidate-questions`, undefined, queryString),
  ) as S["CandidateListResponse"];
}

function createCandidate(groupId: string, body: unknown): S["CandidateItemResponse"] {
  return mockCandidateRoute(
    req("POST", `/groups/${groupId}/candidate-questions`, body),
  ) as S["CandidateItemResponse"];
}

function castVote(groupId: string, questionId: string): S["CandidateVoteResponse"] {
  return mockCandidateRoute(
    req("POST", `/groups/${groupId}/candidate-questions/${questionId}/votes`),
  ) as S["CandidateVoteResponse"];
}

function withdrawVote(groupId: string, questionId: string): S["CandidateVoteResponse"] {
  return mockCandidateRoute(
    req("DELETE", `/groups/${groupId}/candidate-questions/${questionId}/votes`),
  ) as S["CandidateVoteResponse"];
}

describe("mockCandidateRoute", () => {
  it("returns NO_MOCK_ROUTE for paths it doesn't own", () => {
    const result = mockCandidateRoute(req("GET", "/groups/g_trail/newsletters"));
    expect(result).toBe(NO_MOCK_ROUTE);
  });

  it("lists the seeded pool for g_trail sorted by votes by default", () => {
    const list = listCandidates("g_trail");
    expect(list.nextCycleId).toBe(VOTING_CYCLE_ID);
    expect(list.votesPerUserPerCycle).toBe(3);
    const voteCounts = list.items.map((i) => i.voteCount);
    expect(voteCounts).toEqual([...voteCounts].sort((a, b) => b - a));
  });

  it("sorts by recent when requested", () => {
    const list = listCandidates("g_trail", "sort=recent");
    const times = list.items.map((i) => new Date(i.submittedAt).getTime());
    expect(times).toEqual([...times].sort((a, b) => b - a));
  });

  it("redacts anonymous askedBy for a non-admin member of g_game", () => {
    const list = listCandidates("g_game");
    const anonymous = list.items.find((i) => i.isAnonymous);
    expect(anonymous).toBeDefined();
    expect(anonymous!.askedBy).toBeNull();
  });

  it("lets a group admin see askedBy on anonymous questions in g_trail", () => {
    const list = listCandidates("g_trail");
    const anonymous = list.items.find((i) => i.isAnonymous);
    expect(anonymous).toBeDefined();
    expect(anonymous!.askedBy).not.toBeNull();
  });

  it("rejects an unknown group with FORBIDDEN", () => {
    expect(() => listCandidates("g_does_not_exist")).toThrowError(
      expect.objectContaining({ status: 403, code: "FORBIDDEN" }),
    );
  });

  it("rejects a group with no voting cycle with CYCLE_NOT_VOTING", () => {
    expect(() => listCandidates("g_meeple")).toThrowError(
      expect.objectContaining({ status: 409, code: "CYCLE_NOT_VOTING" }),
    );
  });

  it("creates a text candidate and always shows the submitter their own attribution", () => {
    const created = createCandidate("g_game", {
      kind: "text",
      prompt: "What's a game you want to try next?",
      isAnonymous: true,
    });
    expect(created.askedBy).not.toBeNull();
    expect(created.isAnonymous).toBe(true);
    expect(created.voteCount).toBe(0);

    const list = listCandidates("g_game");
    expect(list.items.some((i) => i.questionId === created.questionId)).toBe(true);
  });

  it("rejects a prompt that's too short with VALIDATION_FAILED fieldErrors", () => {
    try {
      createCandidate("g_trail", { kind: "text", prompt: "hi", isAnonymous: false });
      expect.fail("expected a VALIDATION_FAILED error");
    } catch (error) {
      expect(error).toBeInstanceOf(ApiError);
      const apiError = error as ApiError;
      expect(apiError.code).toBe("VALIDATION_FAILED");
      expect(apiError.problem?.fieldErrors?.[0]?.field).toBe("prompt");
    }
  });

  it("rejects duplicate poll option labels", () => {
    try {
      createCandidate("g_trail", {
        kind: "poll",
        prompt: "Which spot should we go to next?",
        pollOptions: [{ label: "Mt Si" }, { label: "Mt Si" }],
        isAnonymous: false,
      });
      expect.fail("expected a VALIDATION_FAILED error");
    } catch (error) {
      expect(error).toBeInstanceOf(ApiError);
      expect((error as ApiError).problem?.fieldErrors?.[0]?.field).toBe("pollOptions");
    }
  });

  it("rejects a poll with only one option", () => {
    expect(() =>
      createCandidate("g_trail", {
        kind: "poll",
        prompt: "Which spot should we go to next?",
        pollOptions: [{ label: "Mt Si" }],
        isAnonymous: false,
      }),
    ).toThrowError(expect.objectContaining({ code: "VALIDATION_FAILED" }));
  });

  it("casts a vote, is idempotent on re-post, and withdraws cleanly", () => {
    const list = listCandidates("g_game");
    const target = list.items.find((i) => !i.votedByMe)!;
    const before = target.voteCount;

    const first = castVote("g_game", target.questionId);
    expect(first.voteCount).toBe(before + 1);
    expect(first.votedByMe).toBe(true);
    expect(first.myVoteCount).toBe(1);

    const second = castVote("g_game", target.questionId);
    expect(second.voteCount).toBe(before + 1);
    expect(second.myVoteCount).toBe(1);

    const withdrawn = withdrawVote("g_game", target.questionId);
    expect(withdrawn.voteCount).toBe(before);
    expect(withdrawn.votedByMe).toBe(false);
    expect(withdrawn.myVoteCount).toBe(0);

    const withdrawnAgain = withdrawVote("g_game", target.questionId);
    expect(withdrawnAgain.voteCount).toBe(before);
    expect(withdrawnAgain.myVoteCount).toBe(0);
  });

  it("enforces the per-cycle vote cap", () => {
    const list = listCandidates("g_trail");
    const unvoted = list.items.filter((i) => !i.votedByMe);
    for (const item of unvoted.slice(0, 3)) {
      castVote("g_trail", item.questionId);
    }
    const stillUnvoted = unvoted[3];
    expect(() => castVote("g_trail", stillUnvoted!.questionId)).toThrowError(
      expect.objectContaining({ status: 409, code: "VOTE_CAP_REACHED" }),
    );
    for (const item of unvoted.slice(0, 3)) {
      withdrawVote("g_trail", item.questionId);
    }
  });
});
