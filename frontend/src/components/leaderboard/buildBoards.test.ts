import { describe, expect, it } from "vitest";
import type { components } from "../../types/api";
import { buildBoards } from "./buildBoards";

type S = components["schemas"];

function member(overrides: Partial<S["MemberResponse"]>): S["MemberResponse"] {
  return {
    userId: "u_1",
    displayName: "Member",
    role: "member",
    avatarColor: "teal",
    avatarUrl: null,
    joinedAt: "2026-01-01T00:00:00Z",
    editionsAnswered: 0,
    ...overrides,
  };
}

function stats(overrides: Partial<S["MemberStatsResponse"]>): S["MemberStatsResponse"] {
  return {
    userId: "u_1",
    editionsAnswered: 0,
    reactionsReceived: 0,
    reactionsGiven: 0,
    commentsWritten: 0,
    questionsAsked: 0,
    currentStreak: 0,
    longestStreak: 0,
    ...overrides,
  };
}

function topAnswer(overrides: Partial<S["TopAnswerResponse"]>): S["TopAnswerResponse"] {
  return {
    responseId: "r_1",
    userId: "u_1",
    cycleId: "202610",
    questionId: "q_1",
    questionPrompt: "What made you laugh?",
    reactionCount: 1,
    ...overrides,
  };
}

const members = [
  member({ userId: "u_a", displayName: "Alex" }),
  member({ userId: "u_b", displayName: "Bailey" }),
  member({ userId: "u_c", displayName: "Casey" }),
  member({ userId: "u_d", displayName: "Dana" }),
];

describe("buildBoards", () => {
  it("caps a stat board at the top 3 distinct values", () => {
    const boards = buildBoards(
      {
        members: [
          stats({ userId: "u_a", reactionsReceived: 10 }),
          stats({ userId: "u_b", reactionsReceived: 8 }),
          stats({ userId: "u_c", reactionsReceived: 6 }),
          stats({ userId: "u_d", reactionsReceived: 4 }),
        ],
        topAnswers: [],
      },
      members,
    );
    const board = boards.find((b) => b.key === "reactionsReceived");
    expect(board).toBeDefined();
    expect(board!.entries.map((e) => e.userId)).toEqual(["u_a", "u_b", "u_c"]);
    expect(board!.entries.map((e) => e.rank)).toEqual([1, 2, 3]);
  });

  it("lets ties share a rank, even past 3 members, while staying within the top 3 ranks", () => {
    const boards = buildBoards(
      {
        members: [
          stats({ userId: "u_a", commentsWritten: 5 }),
          stats({ userId: "u_b", commentsWritten: 5 }),
          stats({ userId: "u_c", commentsWritten: 5 }),
          stats({ userId: "u_d", commentsWritten: 2 }),
        ],
        topAnswers: [],
      },
      members,
    );
    const board = boards.find((b) => b.key === "commentsWritten")!;
    // All three tied-for-first members show, all at rank 1; the 4th (lower, distinct) value
    // would be rank 2, which is still within the top-3-ranks cap.
    expect(board.entries.map((e) => e.userId).sort()).toEqual(["u_a", "u_b", "u_c", "u_d"]);
    expect(board.entries.find((e) => e.userId === "u_a")!.rank).toBe(1);
    expect(board.entries.find((e) => e.userId === "u_d")!.rank).toBe(2);
  });

  it("excludes members at zero", () => {
    const boards = buildBoards(
      {
        members: [
          stats({ userId: "u_a", questionsAsked: 3 }),
          stats({ userId: "u_b", questionsAsked: 0 }),
        ],
        topAnswers: [],
      },
      members,
    );
    const board = boards.find((b) => b.key === "questionsAsked")!;
    expect(board.entries.map((e) => e.userId)).toEqual(["u_a"]);
  });

  it("hides a board with nobody above zero", () => {
    const boards = buildBoards(
      {
        members: [stats({ userId: "u_a", longestStreak: 0 })],
        topAnswers: [],
      },
      members,
    );
    expect(boards.find((b) => b.key === "longestStreak")).toBeUndefined();
  });

  it("returns no boards at all when every stat is zero and there are no top answers", () => {
    const boards = buildBoards(
      {
        members: [stats({ userId: "u_a" })],
        topAnswers: [],
      },
      members,
    );
    expect(boards).toEqual([]);
  });

  it("builds the top-answer board from topAnswers, joined with member display info", () => {
    const boards = buildBoards(
      {
        members: [],
        topAnswers: [
          topAnswer({ responseId: "r_1", userId: "u_a", reactionCount: 5 }),
          topAnswer({ responseId: "r_2", userId: "u_b", reactionCount: 3 }),
        ],
      },
      members,
    );
    const board = boards.find((b) => b.key === "topAnswer");
    expect(board).toBeDefined();
    expect(board!.kind).toBe("topAnswer");
    expect(board!.entries).toMatchObject([
      { userId: "u_a", displayName: "Alex", rank: 1, reactionCount: 5 },
      { userId: "u_b", displayName: "Bailey", rank: 2, reactionCount: 3 },
    ]);
  });

  it("drops a top answer whose author isn't a current member", () => {
    const boards = buildBoards(
      {
        members: [],
        topAnswers: [topAnswer({ userId: "u_departed", reactionCount: 5 })],
      },
      members,
    );
    expect(boards.find((b) => b.key === "topAnswer")).toBeUndefined();
  });

  it("orders boards as spec'd: top answer, reactions received, editions answered, streak, questions asked, comments", () => {
    const boards = buildBoards(
      {
        members: [
          stats({
            userId: "u_a",
            reactionsReceived: 1,
            editionsAnswered: 1,
            longestStreak: 1,
            questionsAsked: 1,
            commentsWritten: 1,
          }),
        ],
        topAnswers: [topAnswer({ userId: "u_a" })],
      },
      members,
    );
    expect(boards.map((b) => b.key)).toEqual([
      "topAnswer",
      "reactionsReceived",
      "editionsAnswered",
      "longestStreak",
      "questionsAsked",
      "commentsWritten",
    ]);
  });

  it("carries currentStreak on the longest-streak board for the current-streak aside", () => {
    const boards = buildBoards(
      {
        members: [stats({ userId: "u_a", longestStreak: 4, currentStreak: 3 })],
        topAnswers: [],
      },
      members,
    );
    const board = boards.find((b) => b.key === "longestStreak")!;
    if (board.kind !== "member") throw new Error("expected a member board");
    expect(board.entries[0]!.currentStreak).toBe(3);
  });
});
