import type { components } from "../types/api";
import { ApiError } from "./client";
import { CALLER_ID, fail, newsletters, NO_MOCK_ROUTE, type MockRequest } from "./mockShared";

type S = components["schemas"];
type AskedBy = S["AskedBy"];
type QuestionKind = S["QuestionKind"];
type PollOptionResponse = S["PollOptionResponse"];
type CandidateItemResponse = S["CandidateItemResponse"];
type CreateCandidateRequest = S["CreateCandidateRequest"];

/**
 * Mock routes for `/groups/{g}/candidate-questions*` (`03-api-contract.md`
 * §6). Mirrors `backend/crates/lambda-questions`: prompt/poll-option
 * validation (`validation.rs`), the vote cap transaction semantics
 * (`handlers.rs`), and anonymity redaction.
 */

const VOTES_PER_USER_PER_CYCLE = 3;
const MIN_PROMPT_CHARS = 5;
const MAX_PROMPT_CHARS = 500;
const MIN_POLL_OPTIONS = 2;
const MAX_POLL_OPTIONS = 6;
const MAX_POLL_OPTION_LABEL_CHARS = 80;

/** `CALLER_ID` is a group admin only in `g_trail` (mirrors the memberships fixture in `mockTransport.ts`). */
const ADMIN_GROUP_IDS = new Set(["g_trail"]);

type StoredCandidate = {
  questionId: string;
  kind: QuestionKind;
  prompt: string;
  pollOptions?: PollOptionResponse[];
  isAnonymous: boolean;
  submitter: AskedBy;
  voteCount: number;
  submittedAt: string;
};

function daysAgo(days: number): string {
  return new Date(Date.now() - days * 24 * 60 * 60 * 1000).toISOString();
}

const SAM: AskedBy = {
  userId: "u_sam",
  displayName: "Sam",
  avatarColor: "orange",
  avatarUrl: null,
};
const QUINN: AskedBy = {
  userId: CALLER_ID,
  displayName: "Quinn",
  avatarColor: "teal",
  avatarUrl: null,
};
const JORDAN: AskedBy = {
  userId: "u_jordan",
  displayName: "Jordan",
  avatarColor: "violet",
  avatarUrl: null,
};
const RILEY: AskedBy = {
  userId: "u_riley",
  displayName: "Riley",
  avatarColor: "pink",
  avatarUrl: null,
};

function seedFor(groupId: string): StoredCandidate[] {
  if (groupId === "g_trail") {
    return [
      {
        questionId: "cq_trail_1",
        kind: "text",
        prompt: "What's the best trail you hiked this summer?",
        isAnonymous: false,
        submitter: SAM,
        voteCount: 5,
        submittedAt: daysAgo(6),
      },
      {
        questionId: "cq_trail_2",
        kind: "poll",
        prompt: "Which trailhead should we carpool from?",
        pollOptions: [
          { optionId: "cq_trail_2_opt_1", label: "North lot" },
          { optionId: "cq_trail_2_opt_2", label: "South lot" },
        ],
        isAnonymous: true,
        submitter: SAM,
        voteCount: 3,
        submittedAt: daysAgo(5),
      },
      {
        questionId: "cq_trail_3",
        kind: "text",
        prompt: "Favorite post-hike meal?",
        isAnonymous: false,
        submitter: QUINN,
        voteCount: 2,
        submittedAt: daysAgo(4),
      },
      {
        questionId: "cq_trail_4",
        kind: "poll",
        prompt: "Should we try a sunrise hike next month?",
        pollOptions: [
          { optionId: "cq_trail_4_opt_1", label: "Yes" },
          { optionId: "cq_trail_4_opt_2", label: "No" },
          { optionId: "cq_trail_4_opt_3", label: "Maybe" },
        ],
        isAnonymous: false,
        submitter: SAM,
        voteCount: 7,
        submittedAt: daysAgo(3),
      },
      {
        questionId: "cq_trail_5",
        kind: "text",
        prompt: "What gear do you swear by?",
        isAnonymous: true,
        submitter: SAM,
        voteCount: 1,
        submittedAt: daysAgo(1),
      },
    ];
  }
  if (groupId === "g_game") {
    return [
      {
        questionId: "cq_game_1",
        kind: "text",
        prompt: "What's your go-to game night snack?",
        isAnonymous: false,
        submitter: JORDAN,
        voteCount: 4,
        submittedAt: daysAgo(5),
      },
      {
        questionId: "cq_game_2",
        kind: "poll",
        prompt: "Which game should we add to the rotation?",
        pollOptions: [
          { optionId: "cq_game_2_opt_1", label: "Wingspan" },
          { optionId: "cq_game_2_opt_2", label: "Codenames" },
          { optionId: "cq_game_2_opt_3", label: "Azul" },
        ],
        isAnonymous: false,
        submitter: QUINN,
        voteCount: 6,
        submittedAt: daysAgo(4),
      },
      {
        questionId: "cq_game_3",
        kind: "text",
        prompt: "Most memorable game night moment this year?",
        isAnonymous: true,
        submitter: RILEY,
        voteCount: 2,
        submittedAt: daysAgo(2),
      },
      {
        questionId: "cq_game_4",
        kind: "text",
        prompt: "Who's hosting next month?",
        isAnonymous: false,
        submitter: JORDAN,
        voteCount: 1,
        submittedAt: daysAgo(1),
      },
    ];
  }
  // g_meeple and any other group start with an empty pool.
  return [];
}

const pools = new Map<string, StoredCandidate[]>();
const myVotes = new Map<string, Set<string>>();
let nextQuestionId = 1;

function poolKey(groupId: string, cycleId: string): string {
  return `${groupId}:${cycleId}`;
}

function ensurePool(groupId: string, cycleId: string): StoredCandidate[] {
  const key = poolKey(groupId, cycleId);
  let pool = pools.get(key);
  if (!pool) {
    pool = seedFor(groupId);
    pools.set(key, pool);
  }
  return pool;
}

function votesFor(groupId: string, cycleId: string): Set<string> {
  const key = poolKey(groupId, cycleId);
  let set = myVotes.get(key);
  if (!set) {
    set = new Set();
    myVotes.set(key, set);
  }
  return set;
}

function requireVotingCycle(groupId: string): S["NewsletterSummary"] {
  const all = newsletters[groupId];
  if (!all) fail(403, "FORBIDDEN", `not a member of ${groupId}`);
  const voting = all.find((n) => n.status === "voting");
  if (!voting) {
    fail(
      409,
      "CYCLE_NOT_VOTING",
      "this group has no cycle currently accepting candidate questions",
    );
  }
  return voting;
}

function toItem(
  candidate: StoredCandidate,
  isAdmin: boolean,
  votedByMe: boolean,
): CandidateItemResponse {
  const visible = !candidate.isAnonymous || isAdmin || candidate.submitter.userId === CALLER_ID;
  return {
    questionId: candidate.questionId,
    kind: candidate.kind,
    prompt: candidate.prompt,
    ...(candidate.pollOptions ? { pollOptions: candidate.pollOptions } : {}),
    askedBy: visible ? candidate.submitter : null,
    isAnonymous: candidate.isAnonymous,
    voteCount: candidate.voteCount,
    votedByMe,
    submittedAt: candidate.submittedAt,
  };
}

function listCandidates(groupId: string, query: URLSearchParams): S["CandidateListResponse"] {
  const voting = requireVotingCycle(groupId);
  const pool = ensurePool(groupId, voting.cycleId);
  const sort = query.get("sort") === "recent" ? "recent" : "top";
  const sorted = [...pool].sort((a, b) =>
    sort === "top"
      ? b.voteCount - a.voteCount ||
        new Date(a.submittedAt).getTime() - new Date(b.submittedAt).getTime()
      : new Date(b.submittedAt).getTime() - new Date(a.submittedAt).getTime(),
  );
  const votedSet = votesFor(groupId, voting.cycleId);
  const isAdmin = ADMIN_GROUP_IDS.has(groupId);

  return {
    nextCycleId: voting.cycleId,
    votesPerUserPerCycle: VOTES_PER_USER_PER_CYCLE,
    myVoteCount: votedSet.size,
    items: sorted.map((c) => toItem(c, isAdmin, votedSet.has(c.questionId))),
    nextCursor: null,
  };
}

type FieldError = { field: string; code: string; message: string };

function validationFailed(fieldErrors: FieldError[]): never {
  const problem: S["ProblemDetails"] = {
    type: "https://api.opennewsletter.example.com/errors/validation-failed",
    title: "Validation failed",
    status: 422,
    detail: fieldErrors[0]!.message,
    code: "VALIDATION_FAILED",
    correlationId: "mock-correlation-id",
    fieldErrors,
  };
  throw new ApiError(422, "VALIDATION_FAILED", problem);
}

function validatePrompt(raw: string): string {
  const trimmed = raw.trim();
  const len = trimmed.length;
  if (len < MIN_PROMPT_CHARS || len > MAX_PROMPT_CHARS) {
    validationFailed([
      {
        field: "prompt",
        code: "INVALID",
        message: `prompt must be between ${MIN_PROMPT_CHARS} and ${MAX_PROMPT_CHARS} characters`,
      },
    ]);
  }
  return trimmed;
}

function validatePollOptions(
  kind: QuestionKind,
  raw: CreateCandidateRequest["pollOptions"],
): PollOptionResponse[] | undefined {
  if (kind === "text") {
    if (raw && raw.length > 0) {
      validationFailed([
        {
          field: "pollOptions",
          code: "INVALID",
          message: "pollOptions must be omitted for kind=text",
        },
      ]);
    }
    return undefined;
  }

  const opts = raw && raw.length > 0 ? raw : undefined;
  if (!opts) {
    validationFailed([
      { field: "pollOptions", code: "INVALID", message: "pollOptions is required for kind=poll" },
    ]);
  }
  if (opts.length < MIN_POLL_OPTIONS || opts.length > MAX_POLL_OPTIONS) {
    validationFailed([
      {
        field: "pollOptions",
        code: "INVALID",
        message: `pollOptions must have between ${MIN_POLL_OPTIONS} and ${MAX_POLL_OPTIONS} options`,
      },
    ]);
  }

  const seen = new Set<string>();
  const result: PollOptionResponse[] = [];
  for (const opt of opts) {
    const label = opt.label.trim();
    if (label.length === 0 || label.length > MAX_POLL_OPTION_LABEL_CHARS) {
      validationFailed([
        {
          field: "pollOptions",
          code: "INVALID",
          message: `pollOptions labels must be 1 to ${MAX_POLL_OPTION_LABEL_CHARS} characters`,
        },
      ]);
    }
    if (seen.has(label)) {
      validationFailed([
        {
          field: "pollOptions",
          code: "INVALID",
          message: `pollOptions labels must be unique; "${label}" is duplicated`,
        },
      ]);
    }
    seen.add(label);
    result.push({ optionId: `cq_${nextQuestionId}_opt_${result.length + 1}`, label });
  }
  return result;
}

function createCandidate(groupId: string, body: unknown): CandidateItemResponse {
  const voting = requireVotingCycle(groupId);
  // Trusted internal shape — the only caller is our own typed `api.createCandidate`.
  const request = body as CreateCandidateRequest;

  const prompt = validatePrompt(request.prompt);
  const pollOptions = validatePollOptions(request.kind, request.pollOptions);

  const candidate: StoredCandidate = {
    questionId: `cq_${nextQuestionId++}`,
    kind: request.kind,
    prompt,
    ...(pollOptions ? { pollOptions } : {}),
    isAnonymous: request.isAnonymous ?? false,
    submitter: QUINN,
    voteCount: 0,
    submittedAt: new Date().toISOString(),
  };
  ensurePool(groupId, voting.cycleId).push(candidate);

  // The submitter always sees their own attribution, even when isAnonymous=true (§6.2).
  return toItem(candidate, ADMIN_GROUP_IDS.has(groupId), false);
}

function castVote(groupId: string, questionId: string): S["CandidateVoteResponse"] {
  const voting = requireVotingCycle(groupId);
  const pool = ensurePool(groupId, voting.cycleId);
  const candidate = pool.find((c) => c.questionId === questionId);
  if (!candidate) fail(404, "NOT_FOUND", `candidate question ${questionId} not found`);

  const votedSet = votesFor(groupId, voting.cycleId);
  if (votedSet.has(questionId)) {
    return {
      questionId,
      voteCount: candidate.voteCount,
      votedByMe: true,
      myVoteCount: votedSet.size,
    };
  }
  if (votedSet.size >= VOTES_PER_USER_PER_CYCLE) {
    fail(
      409,
      "VOTE_CAP_REACHED",
      `no votes remaining this cycle; already voted for: ${[...votedSet].join(", ")}`,
    );
  }

  votedSet.add(questionId);
  candidate.voteCount += 1;
  return {
    questionId,
    voteCount: candidate.voteCount,
    votedByMe: true,
    myVoteCount: votedSet.size,
  };
}

function withdrawVote(groupId: string, questionId: string): S["CandidateVoteResponse"] {
  const voting = requireVotingCycle(groupId);
  const pool = ensurePool(groupId, voting.cycleId);
  const candidate = pool.find((c) => c.questionId === questionId);
  if (!candidate) fail(404, "NOT_FOUND", `candidate question ${questionId} not found`);

  const votedSet = votesFor(groupId, voting.cycleId);
  if (!votedSet.has(questionId)) {
    return {
      questionId,
      voteCount: candidate.voteCount,
      votedByMe: false,
      myVoteCount: votedSet.size,
    };
  }

  votedSet.delete(questionId);
  candidate.voteCount = Math.max(0, candidate.voteCount - 1);
  return {
    questionId,
    voteCount: candidate.voteCount,
    votedByMe: false,
    myVoteCount: votedSet.size,
  };
}

export function mockCandidateRoute(request: MockRequest): unknown {
  const { method, segments, query, body } = request;
  if (segments[0] !== "groups" || segments[2] !== "candidate-questions") return NO_MOCK_ROUTE;

  const groupId = decodeURIComponent(segments[1]!);

  if (segments.length === 3) {
    if (method === "GET") return listCandidates(groupId, query);
    if (method === "POST") return createCandidate(groupId, body);
  }

  if (segments.length === 5 && segments[4] === "votes") {
    const questionId = decodeURIComponent(segments[3]!);
    if (method === "POST") return castVote(groupId, questionId);
    if (method === "DELETE") return withdrawVote(groupId, questionId);
  }

  return NO_MOCK_ROUTE;
}
