import type { components } from "../../types/api";
import type { AvatarColorSlug } from "../../utils/avatarColor";

type S = components["schemas"];
type LeaderboardResponse = S["LeaderboardResponse"];
type MemberStatsResponse = S["MemberStatsResponse"];
type MemberResponse = S["MemberResponse"];

/** A board entry backed by a current member (every board except the top-answer one). */
export interface MemberBoardEntry {
  rank: number;
  userId: string;
  displayName: string;
  avatarColor: AvatarColorSlug;
  avatarUrl: string | null;
  value: number;
  /** Only meaningful on the longest-streak board; shown beside `value` when > 0. */
  currentStreak: number;
}

/** One of the (up to 3) most-reacted answers, joined with its author's display info. */
export interface TopAnswerEntry {
  rank: number;
  userId: string;
  displayName: string;
  avatarColor: AvatarColorSlug;
  avatarUrl: string | null;
  questionPrompt: string;
  cycleId: string;
  questionId: string;
  responseId: string;
  reactionCount: number;
}

export type Board =
  | { key: "topAnswer"; title: string; icon: string; kind: "topAnswer"; entries: TopAnswerEntry[] }
  | { key: string; title: string; icon: string; kind: "member"; entries: MemberBoardEntry[] };

/**
 * Dense ranking (1, 1, 1, 2, …) over the positive values of `items`, capped to the top 3
 * *distinct* values (`docs/design/frontend.md` §7.9): ties share a rank and all stay on the
 * board even if that's more than 3 members, and a board is "for fun" — it never shows a full
 * ranking or anyone at zero. Ties within a rank are ordered by `tieBreak` for a deterministic,
 * testable order.
 */
function rankTop3<T>(
  items: T[],
  valueOf: (item: T) => number,
  tieBreak: (a: T, b: T) => number,
): { item: T; rank: number; value: number }[] {
  const positive = items.filter((item) => valueOf(item) > 0);
  const sorted = [...positive].sort((a, b) => valueOf(b) - valueOf(a) || tieBreak(a, b));
  const topValues = [...new Set(sorted.map(valueOf))].slice(0, 3);
  const minValue = topValues.at(-1);
  if (minValue === undefined) return [];
  return sorted
    .filter((item) => valueOf(item) >= minValue)
    .map((item) => {
      const value = valueOf(item);
      return { item, rank: topValues.indexOf(value) + 1, value };
    });
}

function statBoard(
  key: string,
  title: string,
  icon: string,
  stats: MemberStatsResponse[],
  memberMap: Map<string, MemberResponse>,
  valueOf: (stat: MemberStatsResponse) => number,
): Board | null {
  const known = stats.filter((stat) => memberMap.has(stat.userId));
  const ranked = rankTop3(known, valueOf, (a, b) =>
    memberMap.get(a.userId)!.displayName.localeCompare(memberMap.get(b.userId)!.displayName),
  );
  if (ranked.length === 0) return null;
  return {
    key,
    title,
    icon,
    kind: "member",
    entries: ranked.map(({ item, rank, value }) => {
      const member = memberMap.get(item.userId)!;
      return {
        rank,
        userId: item.userId,
        displayName: member.displayName,
        avatarColor: member.avatarColor,
        avatarUrl: member.avatarUrl,
        value,
        currentStreak: item.currentStreak,
      };
    }),
  };
}

/**
 * Turns raw leaderboard stats into the ordered boards `LeaderboardPage` renders
 * (`docs/design/frontend.md` §7.9). Pure, so a new board using an existing stat is a
 * frontend-only change with its own unit test, not a page rewrite.
 */
export function buildBoards(data: LeaderboardResponse, members: MemberResponse[]): Board[] {
  const memberMap = new Map(members.map((member) => [member.userId, member]));
  const boards: Board[] = [];

  const topAnswers = data.topAnswers.filter((answer) => memberMap.has(answer.userId));
  const rankedAnswers = rankTop3(
    topAnswers,
    (answer) => answer.reactionCount,
    () => 0, // the API already orders ties (count desc, then response id desc)
  );
  if (rankedAnswers.length > 0) {
    boards.push({
      key: "topAnswer",
      title: "Most-reacted answer",
      icon: "🏆",
      kind: "topAnswer",
      entries: rankedAnswers.map(({ item, rank, value }) => {
        const member = memberMap.get(item.userId)!;
        return {
          rank,
          userId: item.userId,
          displayName: member.displayName,
          avatarColor: member.avatarColor,
          avatarUrl: member.avatarUrl,
          questionPrompt: item.questionPrompt,
          cycleId: item.cycleId,
          questionId: item.questionId,
          responseId: item.responseId,
          reactionCount: value,
        };
      }),
    });
  }

  const statBoards: [string, string, string, (stat: MemberStatsResponse) => number][] = [
    ["reactionsReceived", "Most reactions received", "❤️", (s) => s.reactionsReceived],
    ["editionsAnswered", "Most editions answered", "📬", (s) => s.editionsAnswered],
    ["longestStreak", "Longest streak", "⚡", (s) => s.longestStreak],
    ["questionsAsked", "Most questions asked", "💡", (s) => s.questionsAsked],
    ["commentsWritten", "Most comments written", "💬", (s) => s.commentsWritten],
  ];

  for (const [key, title, icon, valueOf] of statBoards) {
    const board = statBoard(key, title, icon, data.members, memberMap, valueOf);
    if (board) boards.push(board);
  }

  return boards;
}
