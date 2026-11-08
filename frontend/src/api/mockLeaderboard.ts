import type { components } from "../types/api";
import { OPEN_QUESTIONS, PUBLISHED_QUESTIONS } from "./mockNewsletters";
import { CALLER_ID, fail, groups, NO_MOCK_ROUTE, type MockRequest } from "./mockShared";

type S = components["schemas"];
type MemberResponse = S["MemberResponse"];

/**
 * Mock route for `GET /groups/{groupId}/leaderboard` (`docs/design/frontend.md` §7.9,
 * `docs/design/api.md` §4.4). Derives `commentsWritten`, `reactionsReceived`, `questionsAsked`
 * and `topAnswers` from the group's existing published-cycle fixtures in `mockNewsletters.ts`
 * (so the demo's leaderboard stays consistent with what the edition itself shows), and seeds
 * `reactionsGiven`/streaks — this mock has no per-reactor identity or cross-cycle history to
 * derive those from.
 */

/** Plausible, group-specific `reactionsGiven` — not derivable from the reaction-group fixtures,
 * which only know the caller's own reaction, not who else reacted. */
const SEED_REACTIONS_GIVEN: Record<string, Record<string, number>> = {
  g_trail: { [CALLER_ID]: 4, u_sam: 7 },
  g_meeple: { [CALLER_ID]: 3, u_m_tara: 5 },
  g_game: { [CALLER_ID]: 2 },
};

/** Plausible, group-specific streaks — chosen so `g_meeple` demonstrates a tie at the top. */
const SEED_STREAKS: Record<string, Record<string, { current: number; longest: number }>> = {
  g_trail: { [CALLER_ID]: { current: 2, longest: 5 }, u_sam: { current: 0, longest: 3 } },
  g_meeple: { [CALLER_ID]: { current: 4, longest: 9 }, u_m_tara: { current: 4, longest: 9 } },
  g_game: { [CALLER_ID]: { current: 1, longest: 2 } },
};

function emptyCounts(): Map<string, number> {
  return new Map();
}

function bump(counts: Map<string, number>, userId: string, by = 1): void {
  counts.set(userId, (counts.get(userId) ?? 0) + by);
}

/** Every `(groupId, cycleId)` entry of `PUBLISHED_QUESTIONS` belonging to `groupId`. */
function publishedEntriesFor(
  groupId: string,
): { cycleId: string; questions: (typeof PUBLISHED_QUESTIONS)[string] }[] {
  const entries: { cycleId: string; questions: (typeof PUBLISHED_QUESTIONS)[string] }[] = [];
  for (const [key, questions] of Object.entries(PUBLISHED_QUESTIONS)) {
    const [gid, cycleId] = key.split("|");
    if (gid === groupId && cycleId) entries.push({ cycleId, questions });
  }
  return entries;
}

function computeCommentsWritten(groupId: string): Map<string, number> {
  const counts = emptyCounts();
  for (const { questions } of publishedEntriesFor(groupId)) {
    for (const q of questions) {
      if (q.kind !== "text") continue;
      for (const answer of q.answers ?? []) {
        for (const comment of answer.comments) {
          if (!comment.deletedAt) bump(counts, comment.authorUserId);
        }
      }
    }
  }
  return counts;
}

function computeReactionsReceived(groupId: string): Map<string, number> {
  const counts = emptyCounts();
  for (const { questions } of publishedEntriesFor(groupId)) {
    for (const q of questions) {
      if (q.kind !== "text") continue;
      for (const answer of q.answers ?? []) {
        const total = answer.reactionGroups.reduce((sum, g) => sum + g.count, 0);
        if (total > 0) bump(counts, answer.userId, total);
      }
    }
  }
  return counts;
}

/**
 * Non-anonymous `askedBy` occurrences, deduped by `questionId` so a question that appears in
 * both the live open-cycle fixtures and a past published-cycle fixture (the mock reuses ids
 * across the two) isn't double-counted.
 */
function computeQuestionsAsked(groupId: string): Map<string, number> {
  const askerByQuestion = new Map<string, string>();
  const consider = (q: {
    questionId: string;
    askedBy: S["AskedBy"] | null;
    isAnonymous: boolean;
  }) => {
    if (q.isAnonymous || !q.askedBy) return;
    askerByQuestion.set(q.questionId, q.askedBy.userId);
  };
  for (const q of OPEN_QUESTIONS[groupId] ?? []) consider(q);
  for (const { questions } of publishedEntriesFor(groupId)) {
    for (const q of questions) consider(q);
  }
  const counts = emptyCounts();
  for (const userId of askerByQuestion.values()) bump(counts, userId);
  return counts;
}

function computeTopAnswers(groupId: string, members: MemberResponse[]): S["TopAnswerResponse"][] {
  const memberIds = new Set(members.map((m) => m.userId));
  const candidates: S["TopAnswerResponse"][] = [];
  for (const { cycleId, questions } of publishedEntriesFor(groupId)) {
    for (const q of questions) {
      if (q.kind !== "text") continue;
      for (const answer of q.answers ?? []) {
        if (!memberIds.has(answer.userId)) continue;
        const reactionCount = answer.reactionGroups.reduce((sum, g) => sum + g.count, 0);
        if (reactionCount <= 0) continue;
        candidates.push({
          responseId: answer.responseId,
          userId: answer.userId,
          cycleId,
          questionId: q.questionId,
          questionPrompt: q.prompt,
          reactionCount,
        });
      }
    }
  }
  // Mirrors the real GSI2 order (`docs/design/api.md` §4.4): count desc, then response id desc.
  candidates.sort(
    (a, b) => b.reactionCount - a.reactionCount || b.responseId.localeCompare(a.responseId),
  );
  return candidates.slice(0, 3);
}

function handleGetLeaderboard(groupId: string): S["LeaderboardResponse"] {
  const group = groups[groupId];
  if (!group) fail(403, "FORBIDDEN", `not a member of ${groupId}`);

  const commentsWritten = computeCommentsWritten(groupId);
  const reactionsReceived = computeReactionsReceived(groupId);
  const questionsAsked = computeQuestionsAsked(groupId);
  const reactionsGivenSeed = SEED_REACTIONS_GIVEN[groupId] ?? {};
  const streaksSeed = SEED_STREAKS[groupId] ?? {};

  const members: S["MemberStatsResponse"][] = group.members.map((m) => {
    const streak = streaksSeed[m.userId] ?? { current: 0, longest: 0 };
    return {
      userId: m.userId,
      editionsAnswered: m.editionsAnswered,
      reactionsReceived: reactionsReceived.get(m.userId) ?? 0,
      reactionsGiven: reactionsGivenSeed[m.userId] ?? 0,
      commentsWritten: commentsWritten.get(m.userId) ?? 0,
      questionsAsked: questionsAsked.get(m.userId) ?? 0,
      currentStreak: streak.current,
      longestStreak: streak.longest,
    };
  });

  return { members, topAnswers: computeTopAnswers(groupId, group.members) };
}

/** Mock route for `GET /groups/{groupId}/leaderboard`. */
export function mockLeaderboardRoute(request: MockRequest): unknown {
  const { method, segments } = request;
  if (
    method !== "GET" ||
    segments.length !== 3 ||
    segments[0] !== "groups" ||
    segments[2] !== "leaderboard"
  ) {
    return NO_MOCK_ROUTE;
  }
  return handleGetLeaderboard(decodeURIComponent(segments[1]!));
}
