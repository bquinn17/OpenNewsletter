import type { components } from "../types/api";
import {
  CALLER_ID,
  MOCK_IMAGE_DATA_URL,
  NO_MOCK_ROUTE,
  OPEN_CYCLE_ID,
  PUBLISHED_CYCLE_ID,
  PUBLISHED_CYCLE_ID_2,
  fail,
  newsletters,
  type MockRequest,
} from "./mockShared";

type S = components["schemas"];
type AskedBy = S["AskedBy"];
type OpenQuestion = S["OpenQuestionResponse"];
type PublishedQuestion = S["PublishedQuestionResponse"];
type PublishedAnswer = S["PublishedAnswerResponse"];
type MyResponseSummary = S["MyResponseSummary"];
type ResponseDto = S["ResponseDto"];
type SaveResponseRequest = S["SaveResponseRequest"];

const SAM: AskedBy = { userId: "u_sam", displayName: "Sam", avatarColor: "red", avatarUrl: null };
const TARA: AskedBy = {
  userId: "u_m_tara",
  displayName: "Tara",
  avatarColor: "orange",
  avatarUrl: null,
};

// ---- §6.2 open-cycle question banks (metadata only; `myResponse` is looked
// up live from the `responses` store below so a PUT is reflected on the next
// GET). ----

type OpenQuestionSeed = Omit<OpenQuestion, "myResponse">;

const OPEN_QUESTIONS: Record<string, OpenQuestionSeed[]> = {
  g_trail: [
    {
      questionId: "q_trail_hike",
      kind: "text",
      prompt: "What's your favorite hike of the year?",
      displayOrder: 0,
      askedBy: SAM,
      isAnonymous: false,
    },
    {
      questionId: "q_trail_photo",
      kind: "text",
      prompt: "📷 Photo Wall — share a shot from this month",
      displayOrder: 1,
      askedBy: null,
      isAnonymous: false,
    },
    {
      questionId: "q_trail_poll",
      kind: "poll",
      prompt: "Which trail should we do next?",
      displayOrder: 2,
      askedBy: null,
      isAnonymous: true,
      pollOptions: [
        { optionId: "opt_misi", label: "Mt Si" },
        { optionId: "opt_lake22", label: "Lake 22" },
        { optionId: "opt_rattlesnake", label: "Rattlesnake Ledge" },
      ],
    },
    {
      questionId: "q_trail_mind",
      kind: "text",
      prompt: "💬 On Your Mind — what's been on your mind lately?",
      displayOrder: 3,
      askedBy: null,
      isAnonymous: false,
    },
    {
      questionId: "q_trail_checkout",
      kind: "text",
      prompt: "👀 Check It Out — recommend something",
      displayOrder: 4,
      askedBy: null,
      isAnonymous: false,
    },
  ],
  g_meeple: [
    {
      questionId: "q_meeple_surprise",
      kind: "text",
      prompt: "What board game surprised you this month?",
      displayOrder: 0,
      askedBy: TARA,
      isAnonymous: false,
    },
    {
      questionId: "q_meeple_poll",
      kind: "poll",
      prompt: "Pick next game night's theme",
      displayOrder: 1,
      askedBy: null,
      isAnonymous: true,
      pollOptions: [
        { optionId: "opt_coop", label: "Co-op" },
        { optionId: "opt_party", label: "Party games" },
        { optionId: "opt_strategy", label: "Strategy" },
      ],
    },
    {
      questionId: "q_meeple_mvp",
      kind: "text",
      prompt: "Who was this month's MVP player?",
      displayOrder: 2,
      askedBy: null,
      isAnonymous: false,
    },
    {
      questionId: "q_meeple_checkout",
      kind: "text",
      prompt: "👀 Check It Out — a game you're eyeing",
      displayOrder: 3,
      askedBy: null,
      isAnonymous: false,
    },
  ],
};

// ---- §7 response store. Seeded with each group's initial drafts/publishes
// (kept consistent with `newsletters[groupId]`'s `myDraftCount`/
// `myPublishedCount`), then mutated by `PUT .../my-response`. ----

const responses = new Map<string, ResponseDto>();
let nextResponseId = 1;

function responseKey(groupId: string, cycleId: string, questionId: string): string {
  return `${groupId}|${cycleId}|${questionId}`;
}

function seedResponse(
  groupId: string,
  cycleId: string,
  questionId: string,
  partial: Omit<ResponseDto, "responseId" | "userId" | "groupId" | "cycleId" | "questionId">,
): void {
  responses.set(responseKey(groupId, cycleId, questionId), {
    responseId: `r_${nextResponseId++}`,
    userId: CALLER_ID,
    groupId,
    cycleId,
    questionId,
    ...partial,
  });
}

seedResponse("g_trail", OPEN_CYCLE_ID, "q_trail_hike", {
  kind: "text",
  status: "published",
  body: "Mailbox Peak kicked my butt but the view at the top was unreal.",
  imageMediaIds: [],
  updatedAt: new Date(Date.now() - 24 * 60 * 60_000).toISOString(),
  publishedAt: new Date(Date.now() - 24 * 60 * 60_000).toISOString(),
});
seedResponse("g_trail", OPEN_CYCLE_ID, "q_trail_photo", {
  kind: "text",
  status: "draft",
  body: "Still picking my favorite shot from the ridge walk...",
  imageMediaIds: [],
  updatedAt: new Date(Date.now() - 12 * 60 * 60_000).toISOString(),
  publishedAt: null,
});
seedResponse("g_meeple", OPEN_CYCLE_ID, "q_meeple_surprise", {
  kind: "text",
  status: "draft",
  body: "Wingspan, surprisingly — didn't expect to love the engine building.",
  imageMediaIds: [],
  updatedAt: new Date(Date.now() - 10 * 60 * 60_000).toISOString(),
  publishedAt: null,
});
seedResponse("g_meeple", OPEN_CYCLE_ID, "q_meeple_mvp", {
  kind: "text",
  status: "draft",
  body: "Tara, obviously. She swept us in Wavelength.",
  imageMediaIds: [],
  updatedAt: new Date(Date.now() - 9 * 60 * 60_000).toISOString(),
  publishedAt: null,
});

function toMyResponseSummary(dto: ResponseDto): MyResponseSummary {
  return {
    responseId: dto.responseId,
    status: dto.status,
    body: dto.body,
    imageMediaIds: dto.imageMediaIds,
    pollOptionId: dto.pollOptionId,
    updatedAt: dto.updatedAt,
    publishedAt: dto.publishedAt,
  };
}

function hydrateOpenQuestions(groupId: string, cycleId: string): OpenQuestion[] {
  const seeds = OPEN_QUESTIONS[groupId] ?? [];
  return seeds.map((seed) => {
    const saved = responses.get(responseKey(groupId, cycleId, seed.questionId));
    return { ...seed, myResponse: saved ? toMyResponseSummary(saved) : null };
  });
}

// ---- §5.2 published-cycle fixtures. Static — nothing writes to a published
// cycle (`PUT` always refuses once a cycle leaves `open`). ----

function imageRef(imageId: string, width: number, height: number): S["PublishedImageResponse"] {
  return { imageId, displayUrl: MOCK_IMAGE_DATA_URL, thumbUrl: MOCK_IMAGE_DATA_URL, width, height };
}

const RICH_PUBLISHED_QUESTIONS: PublishedQuestion[] = [
  {
    questionId: "q_trail_hike",
    kind: "text",
    prompt: "What's your favorite hike of the year?",
    displayOrder: 0,
    askedBy: SAM,
    isAnonymous: false,
    answers: [
      {
        responseId: "pub_r1",
        userId: "u_sam",
        displayName: "Sam",
        body: "Can't beat the **ridge walk at sunset** — golden light the whole way down.",
        images: [],
        publishedAt: new Date(Date.now() - 28 * 24 * 60 * 60_000 + 1000).toISOString(),
        // Seeded for M10 (`09-engagement.md` §1) — one edited, one soft-deleted,
        // so VITE_USE_MOCKS=true demonstrates both without any manual steps.
        comments: [
          {
            commentId: "cmt_seed_1",
            authorUserId: CALLER_ID,
            displayName: "Quinn",
            body: "Same — the switchbacks are brutal.",
            image: null,
            createdAt: new Date(Date.now() - 27 * 24 * 60 * 60_000).toISOString(),
            editedAt: new Date(Date.now() - 27 * 24 * 60 * 60_000 + 60_000).toISOString(),
            deletedAt: null,
          },
          {
            commentId: "cmt_seed_2",
            authorUserId: "u_alex",
            displayName: "Alex",
            body: "",
            image: null,
            createdAt: new Date(Date.now() - 26 * 24 * 60 * 60_000).toISOString(),
            editedAt: null,
            deletedAt: new Date(Date.now() - 25 * 24 * 60 * 60_000).toISOString(),
          },
        ],
        reactionGroups: [{ emoji: "🔥", count: 2, reactedByMe: false }],
      },
      {
        responseId: "pub_r2",
        userId: CALLER_ID,
        displayName: "Quinn",
        body: "Mailbox Peak nearly killed me but so worth it.\n\n![](image:img_pub_hike)",
        images: [imageRef("img_pub_hike", 1200, 800)],
        publishedAt: new Date(Date.now() - 28 * 24 * 60 * 60_000 + 2000).toISOString(),
        comments: [],
        reactionGroups: [],
      },
    ],
  },
  {
    questionId: "q_trail_photo",
    kind: "text",
    prompt: "📷 Photo Wall — share a shot from this month",
    displayOrder: 1,
    askedBy: null,
    isAnonymous: false,
    answers: [
      {
        responseId: "pub_r3",
        userId: CALLER_ID,
        displayName: "Quinn",
        body: "From the summit.",
        images: [imageRef("img_pub_photo1", 1200, 900)],
        publishedAt: new Date(Date.now() - 28 * 24 * 60 * 60_000 + 3000).toISOString(),
        comments: [],
        reactionGroups: [],
      },
    ],
  },
  {
    questionId: "q_trail_poll",
    kind: "poll",
    prompt: "Which trail should we do next?",
    displayOrder: 2,
    askedBy: null,
    isAnonymous: true,
    options: [
      { optionId: "opt_misi", label: "Mt Si", voteCount: 3 },
      { optionId: "opt_lake22", label: "Lake 22", voteCount: 1 },
      { optionId: "opt_rattlesnake", label: "Rattlesnake Ledge", voteCount: 0 },
    ],
    myVoteOptionId: "opt_misi",
  },
  {
    questionId: "q_trail_mind",
    kind: "text",
    prompt: "💬 On Your Mind — what's been on your mind lately?",
    displayOrder: 3,
    askedBy: null,
    isAnonymous: false,
    answers: [
      {
        responseId: "pub_r4",
        userId: CALLER_ID,
        displayName: "Quinn",
        body: "Planning next year's trip already.",
        images: [],
        publishedAt: new Date(Date.now() - 28 * 24 * 60 * 60_000 + 4000).toISOString(),
        comments: [],
        reactionGroups: [],
      },
    ],
  },
  {
    questionId: "q_trail_checkout",
    kind: "text",
    prompt: "👀 Check It Out — recommend something",
    displayOrder: 4,
    askedBy: null,
    isAnonymous: false,
    answers: [
      {
        responseId: "pub_r5",
        userId: "u_sam",
        displayName: "Sam",
        body: "A podcast about long-distance trail running.",
        images: [],
        publishedAt: new Date(Date.now() - 28 * 24 * 60 * 60_000 + 5000).toISOString(),
        comments: [],
        reactionGroups: [],
      },
    ],
  },
];

function simplePublishedQuestions(
  groupId: string,
  publishedAtIso: string,
  otherMember: AskedBy | null,
): PublishedQuestion[] {
  const other = otherMember
    ? [
        {
          responseId: `pub_${groupId}_other`,
          userId: otherMember.userId,
          displayName: otherMember.displayName,
          body: "Loved this one — great question.",
          images: [],
          publishedAt: publishedAtIso,
          comments: [],
          reactionGroups: [],
        },
      ]
    : [];
  return [
    {
      questionId: `q_${groupId}_archive_1`,
      kind: "text",
      prompt: "What made you laugh this month?",
      displayOrder: 0,
      askedBy: null,
      isAnonymous: false,
      answers: [
        {
          responseId: `pub_${groupId}_mine`,
          userId: CALLER_ID,
          displayName: "Quinn",
          body: "Too many inside jokes to pick just one.",
          images: [],
          publishedAt: publishedAtIso,
          comments: [],
          reactionGroups: [],
        },
        ...other,
      ],
    },
    {
      questionId: `q_${groupId}_archive_poll`,
      kind: "poll",
      prompt: "Best moment of the month?",
      displayOrder: 1,
      askedBy: null,
      isAnonymous: true,
      options: [
        { optionId: "opt_a", label: "The comeback", voteCount: 2 },
        { optionId: "opt_b", label: "The group photo", voteCount: 1 },
      ],
      myVoteOptionId: "opt_a",
    },
  ];
}

const PUBLISHED_QUESTIONS: Record<string, PublishedQuestion[]> = {
  [`g_trail|${PUBLISHED_CYCLE_ID}`]: RICH_PUBLISHED_QUESTIONS,
  [`g_trail|${PUBLISHED_CYCLE_ID_2}`]: simplePublishedQuestions(
    "g_trail",
    new Date(Date.now() - 58 * 24 * 60 * 60_000 + 1000).toISOString(),
    SAM,
  ),
  [`g_game|${PUBLISHED_CYCLE_ID}`]: simplePublishedQuestions(
    "g_game",
    new Date(Date.now() - 28 * 24 * 60 * 60_000 + 1000).toISOString(),
    null,
  ),
  [`g_meeple|${PUBLISHED_CYCLE_ID}`]: simplePublishedQuestions(
    "g_meeple",
    new Date(Date.now() - 29 * 24 * 60 * 60_000 + 1000).toISOString(),
    TARA,
  ),
};

// ---- route handlers ----

function findSummary(groupId: string, cycleId: string): S["NewsletterSummary"] | undefined {
  return newsletters[groupId]?.find((n) => n.cycleId === cycleId);
}

/**
 * The live `PublishedAnswerResponse` object backing one published text
 * answer — a direct reference, not a copy, so `mockEngagement.ts` can mutate
 * its `comments`/`reactionGroups` in place and have the next `GET` newsletter
 * pick up the change without a second source of truth.
 */
export function findPublishedAnswer(
  groupId: string,
  cycleId: string,
  questionId: string,
  responseId: string,
): PublishedAnswer | undefined {
  const questions = PUBLISHED_QUESTIONS[`${groupId}|${cycleId}`] ?? [];
  const question = questions.find((q) => q.questionId === questionId && q.kind === "text");
  return question?.answers?.find((a) => a.responseId === responseId);
}

function handleGetDetail(groupId: string, cycleId: string): S["NewsletterDetailResponse"] {
  if (!newsletters[groupId]) fail(403, "FORBIDDEN", `not a member of ${groupId}`);
  const summary = findSummary(groupId, cycleId);
  if (!summary) fail(404, "NOT_FOUND", `cycle ${cycleId} not found for group ${groupId}`);

  if (summary.status === "voting") return { status: "voting", candidates: [] };

  if (summary.status === "open") {
    return {
      status: "open",
      cycleId,
      responseOpenAt: summary.responseOpenAt,
      responseCloseAt: summary.responseCloseAt,
      questions: hydrateOpenQuestions(groupId, cycleId),
    };
  }

  if (summary.status === "published") {
    const questions = PUBLISHED_QUESTIONS[`${groupId}|${cycleId}`] ?? [];
    return {
      status: "published",
      cycleId,
      responseCloseAt: summary.responseCloseAt,
      publishedAt: summary.publishedAt ?? summary.responseCloseAt,
      questions,
    };
  }

  fail(410, "ARCHIVED", `cycle ${cycleId} has been archived`);
}

function handleGetMyResponses(groupId: string, cycleId: string): S["MyResponsesList"] {
  if (!newsletters[groupId]) fail(403, "FORBIDDEN", `not a member of ${groupId}`);
  if (!findSummary(groupId, cycleId)) fail(404, "NOT_FOUND", `cycle ${cycleId} not found`);
  const items = (OPEN_QUESTIONS[groupId] ?? [])
    .map((seed) => responses.get(responseKey(groupId, cycleId, seed.questionId)))
    .filter((dto): dto is ResponseDto => !!dto);
  return { items };
}

function handleGetMyResponse(groupId: string, cycleId: string, questionId: string): ResponseDto {
  const dto = responses.get(responseKey(groupId, cycleId, questionId));
  if (!dto) fail(404, "NOT_FOUND", `no response for question ${questionId}`);
  return dto;
}

function adjustSummaryCounts(
  groupId: string,
  cycleId: string,
  delta: { draft: number; published: number },
): void {
  const summary = findSummary(groupId, cycleId);
  if (!summary) return;
  summary.myDraftCount = Math.max(0, summary.myDraftCount + delta.draft);
  summary.myPublishedCount = Math.max(0, summary.myPublishedCount + delta.published);
}

function handlePutMyResponse(
  groupId: string,
  cycleId: string,
  questionId: string,
  body: unknown,
): ResponseDto {
  if (!newsletters[groupId]) fail(403, "FORBIDDEN", `not a member of ${groupId}`);
  const summary = findSummary(groupId, cycleId);
  if (!summary) fail(404, "NOT_FOUND", `cycle ${cycleId} not found`);

  const now = Date.now();
  const stillOpen = summary.status === "open" && now < new Date(summary.responseCloseAt).getTime();
  if (!stillOpen) fail(409, "CYCLE_NOT_OPEN", `cycle ${cycleId} is not open for responses`);

  const seed = (OPEN_QUESTIONS[groupId] ?? []).find((q) => q.questionId === questionId);
  if (!seed) fail(404, "NOT_FOUND", `question ${questionId} not found in cycle ${cycleId}`);

  // Trusted internal shape — the only caller is our own typed `api.saveMyResponse`.
  const request = body as SaveResponseRequest;
  if (request.kind !== seed.kind) {
    fail(422, "VALIDATION_FAILED", `question ${questionId} expects kind ${seed.kind}`);
  }

  const key = responseKey(groupId, cycleId, questionId);
  const existing = responses.get(key);
  const wasPublished = existing?.status === "published";
  const nowPublishing = request.publish || wasPublished;
  const nowIso = new Date(now).toISOString();

  const dto: ResponseDto = {
    responseId: existing?.responseId ?? `r_${nextResponseId++}`,
    userId: CALLER_ID,
    groupId,
    cycleId,
    questionId,
    kind: request.kind,
    status: nowPublishing ? "published" : "draft",
    body: request.kind === "text" ? request.body : undefined,
    pollOptionId: request.kind === "poll" ? request.pollOptionId : undefined,
    imageMediaIds: request.kind === "text" ? [...new Set(request.imageMediaIds)].slice(0, 10) : [],
    updatedAt: nowIso,
    publishedAt: wasPublished ? existing.publishedAt : request.publish ? nowIso : null,
  };
  responses.set(key, dto);

  if (!existing) {
    adjustSummaryCounts(groupId, cycleId, {
      draft: dto.status === "draft" ? 1 : 0,
      published: dto.status === "published" ? 1 : 0,
    });
  } else if (!wasPublished && dto.status === "published") {
    adjustSummaryCounts(groupId, cycleId, { draft: -1, published: 1 });
  }

  return dto;
}

/** Mock routes for newsletter detail and `my-response(s)`. */
export function mockNewsletterRoute(request: MockRequest): unknown {
  const { method, segments, body } = request;
  if (segments[0] !== "groups" || segments[2] !== "newsletters") return NO_MOCK_ROUTE;

  const groupId = decodeURIComponent(segments[1]!);
  const cycleId = decodeURIComponent(segments[3]!);

  if (method === "GET" && segments.length === 4) {
    return handleGetDetail(groupId, cycleId);
  }
  if (method === "GET" && segments.length === 5 && segments[4] === "my-responses") {
    return handleGetMyResponses(groupId, cycleId);
  }
  if (segments.length === 7 && segments[4] === "questions" && segments[6] === "my-response") {
    const questionId = decodeURIComponent(segments[5]!);
    if (method === "GET") return handleGetMyResponse(groupId, cycleId, questionId);
    if (method === "PUT") return handlePutMyResponse(groupId, cycleId, questionId, body);
  }

  return NO_MOCK_ROUTE;
}
