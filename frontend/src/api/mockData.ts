import type {
  AppConfig,
  CandidateQuestion,
  Group,
  Invite,
  LockedQuestion,
  MyResponse,
  NewsletterDetail,
  NewsletterSummary,
  NotificationPref,
  OpenNewsletter,
  PublishedNewsletter,
  PushDevice,
  VotingNewsletter,
} from "./types";

// In-memory store. Mutations write back here so the UI feels live across navigations.

export const me = {
  userId: "u_quinn",
  email: "bryan.quinn17@gmail.com",
  displayName: "Quinn",
  avatarColor: "grape",
};

export const trailCrewGradient = { from: "#FF5A6B", to: "#FFB178", className: "hero-april" };
export const gameNightGradient = { from: "#7B5BFF", to: "#5AC8FF", className: "hero-may" };

const trailCrewMembers = [
  {
    userId: "u_quinn",
    displayName: "Quinn",
    role: "admin" as const,
    nickname: null,
    avatarColor: "grape",
    editionsAnswered: 5,
    joinedAt: "2025-01-12T00:00:00Z",
  },
  {
    userId: "u_sam",
    displayName: "Sam",
    role: "admin" as const,
    nickname: null,
    avatarColor: "coral",
    editionsAnswered: 5,
    joinedAt: "2025-01-12T00:00:00Z",
  },
  {
    userId: "u_riley",
    displayName: "Riley",
    role: "member" as const,
    nickname: null,
    avatarColor: "mint",
    editionsAnswered: 5,
    joinedAt: "2025-01-12T00:00:00Z",
  },
  {
    userId: "u_alex",
    displayName: "Alex",
    role: "member" as const,
    nickname: null,
    avatarColor: "grape",
    editionsAnswered: 5,
    joinedAt: "2025-02-04T00:00:00Z",
  },
  {
    userId: "u_jordan",
    displayName: "Jordan",
    role: "member" as const,
    nickname: null,
    avatarColor: "sky",
    editionsAnswered: 3,
    joinedAt: "2025-04-14T00:00:00Z",
  },
  {
    userId: "u_morgan",
    displayName: "Morgan",
    role: "member" as const,
    nickname: "Mo",
    avatarColor: "sun",
    editionsAnswered: 2,
    joinedAt: "2025-09-02T00:00:00Z",
  },
  {
    userId: "u_casey",
    displayName: "Casey",
    role: "member" as const,
    nickname: null,
    avatarColor: "peach",
    editionsAnswered: 0,
    joinedAt: "2026-04-22T00:00:00Z",
  },
];

const memberByName = (name: string) => trailCrewMembers.find((m) => m.displayName === name)!;

export const trailCrew: Group = {
  groupId: "g_trail",
  name: "Trail Crew",
  timezone: "America/New_York",
  cycleSettings: { questionsPerCycle: 5, votesPerUserPerCycle: 3, responseWindowDays: 4 },
  notificationSettings: { offsetsHoursBeforeClose: [96, 48, 24], onCycleOpen: true },
  memberSoftCap: 50,
  members: trailCrewMembers,
  gradient: trailCrewGradient,
};

export const gameNight: Group = {
  groupId: "g_game",
  name: "Game Night Gang",
  timezone: "America/New_York",
  cycleSettings: { questionsPerCycle: 4, votesPerUserPerCycle: 3, responseWindowDays: 4 },
  notificationSettings: { offsetsHoursBeforeClose: [48, 24], onCycleOpen: true },
  memberSoftCap: 50,
  members: [
    {
      userId: "u_quinn",
      displayName: "Quinn",
      role: "member" as const,
      nickname: null,
      avatarColor: "grape",
      editionsAnswered: 2,
      joinedAt: "2025-08-01T00:00:00Z",
    },
  ],
  gradient: gameNightGradient,
};

export const groups: Record<string, Group> = {
  [trailCrew.groupId]: trailCrew,
  [gameNight.groupId]: gameNight,
};

// April 2026 — published, the rich edition
const aprilQuestions: NonNullable<PublishedNewsletter["questions"]> = [
  {
    questionId: "q_apr_1",
    kind: "text",
    prompt: "What was the highlight of your April?",
    displayOrder: 0,
    answers: [
      {
        responseId: "r_apr_1_sam",
        userId: "u_sam",
        displayName: "Sam",
        avatarColor: "coral",
        body:
          "We finally hit the **Lake 22** trail. Five miles in, snow up to our knees, and a thermos of cocoa that didn't last past mile 2. Worth every step. I think Riley fell over six times — she's keeping score.",
        images: [
          { imageId: "i1", status: "ready", alt: "snowy trail" },
          { imageId: "i2", status: "ready", alt: "thermos of cocoa" },
          { imageId: "i3", status: "ready", alt: "summit selfie" },
        ],
        publishedAt: "2026-04-30T16:12:00Z",
        comments: [
          {
            commentId: "c1",
            authorUserId: "u_riley",
            authorDisplayName: "Riley",
            authorAvatarColor: "mint",
            body: "For the record it was four times. Snow doesn't count.",
            createdAt: "2026-05-01T15:02:00Z",
            editedAt: null,
          },
          {
            commentId: "c2",
            authorUserId: "u_alex",
            authorDisplayName: "Alex",
            authorAvatarColor: "grape",
            body: "need that cocoa recipe immediately",
            createdAt: "2026-05-01T17:48:00Z",
            editedAt: null,
          },
        ],
        reactionGroups: [
          { emoji: "🔥", count: 4, reactedByMe: true },
          { emoji: "😍", count: 2, reactedByMe: false },
          { emoji: "⛰️", count: 3, reactedByMe: true },
        ],
      },
      {
        responseId: "r_apr_1_alex",
        userId: "u_alex",
        displayName: "Alex",
        avatarColor: "grape",
        body:
          "Got the keys to the new place. It's tiny, it's drafty, and there's a window that won't close. I love it. First thing I did was hang the print Quinn made me three years ago.",
        images: [{ imageId: "i4", status: "ready", alt: "new apartment window" }],
        publishedAt: "2026-04-28T22:31:00Z",
        comments: [
          {
            commentId: "c3",
            authorUserId: "u_quinn",
            authorDisplayName: "Quinn",
            authorAvatarColor: "grape",
            body: "no way it's still around 🥹",
            createdAt: "2026-04-29T01:15:00Z",
            editedAt: null,
          },
        ],
        reactionGroups: [
          { emoji: "❤️", count: 5, reactedByMe: true },
          { emoji: "🎉", count: 3, reactedByMe: false },
        ],
      },
      {
        responseId: "r_apr_1_riley",
        userId: "u_riley",
        displayName: "Riley",
        avatarColor: "mint",
        body:
          "Adopted a cat. Her name is Casserole. She knocked over a glass of water within four minutes of arriving home and has not yet apologized.",
        images: [
          { imageId: "i5", status: "ready", alt: "cat on a windowsill" },
          { imageId: "i6", status: "ready", alt: "cat asleep" },
        ],
        publishedAt: "2026-04-30T20:01:00Z",
        comments: [],
        reactionGroups: [
          { emoji: "😺", count: 6, reactedByMe: true },
          { emoji: "💧", count: 2, reactedByMe: false },
        ],
      },
    ],
  },
  {
    questionId: "q_apr_2",
    kind: "text",
    prompt: "Best meal you ate or cooked this month",
    displayOrder: 1,
    answers: [
      {
        responseId: "r_apr_2_quinn",
        userId: "u_quinn",
        displayName: "Quinn",
        avatarColor: "grape",
        body:
          "Did a Sunday slow-cook pork shoulder, then turned the leftovers into tacos for lunch all week. Tuesday's tacos > Sunday's roast, fight me.",
        images: [{ imageId: "i7", status: "ready", alt: "pork tacos" }],
        publishedAt: "2026-04-29T18:00:00Z",
        comments: [],
        reactionGroups: [
          { emoji: "🌮", count: 4, reactedByMe: false },
          { emoji: "🤤", count: 2, reactedByMe: true },
        ],
      },
    ],
  },
  {
    questionId: "q_apr_3",
    kind: "poll",
    prompt: "Best trail this month",
    displayOrder: 2,
    options: [
      { optionId: "o1", label: "Lake 22", voteCount: 7 },
      { optionId: "o2", label: "Mt Si", voteCount: 4 },
      { optionId: "o3", label: "Mailbox Peak", voteCount: 1 },
      { optionId: "o4", label: "Wallace Falls", voteCount: 0 },
    ],
    totalVotes: 12,
    myVoteOptionId: "o1",
  },
  {
    questionId: "q_apr_4",
    kind: "text",
    prompt: "Something small that made you laugh",
    displayOrder: 3,
    answers: [
      {
        responseId: "r_apr_4_morgan",
        userId: "u_morgan",
        displayName: "Mo",
        avatarColor: "sun",
        body:
          "My toddler asked, \"is the moon a friend?\" — and I had to admit I don't know what to do with that question.",
        images: [],
        publishedAt: "2026-04-30T03:18:00Z",
        comments: [
          {
            commentId: "c4",
            authorUserId: "u_alex",
            authorDisplayName: "Alex",
            authorAvatarColor: "grape",
            body: "the moon is absolutely a friend",
            createdAt: "2026-05-01T14:00:00Z",
            editedAt: null,
          },
        ],
        reactionGroups: [
          { emoji: "🌙", count: 4, reactedByMe: true },
          { emoji: "🥹", count: 3, reactedByMe: false },
        ],
      },
    ],
  },
  {
    questionId: "q_apr_5",
    kind: "text",
    prompt: "What are you looking forward to in May?",
    displayOrder: 4,
    answers: [
      {
        responseId: "r_apr_5_jordan",
        userId: "u_jordan",
        displayName: "Jordan",
        avatarColor: "sky",
        body: "Sleeping outside on purpose for the first time this year. Tent pitched, fingers crossed for clear skies.",
        images: [],
        publishedAt: "2026-04-30T23:45:00Z",
        comments: [],
        reactionGroups: [{ emoji: "⛺️", count: 3, reactedByMe: true }],
      },
    ],
  },
];

const aprilNewsletter: PublishedNewsletter = {
  groupId: "g_trail",
  groupName: "Trail Crew",
  cycleId: "202604",
  status: "published",
  responseCloseAt: "2026-05-01T04:00:00Z",
  publishedAt: "2026-05-01T04:00:00Z",
  questions: aprilQuestions,
  gradientClass: "hero-april",
  monthLabel: "April",
  yearLabel: "2026",
  reactionTotal: 33,
};

const mayQuestions: LockedQuestion[] = [
  { questionId: "q_may_1", kind: "text", prompt: "What was the highlight of your May?", displayOrder: 0 },
  { questionId: "q_may_2", kind: "text", prompt: "Best meal you ate or cooked this month", displayOrder: 1 },
  {
    questionId: "q_may_3",
    kind: "poll",
    prompt: "Pick your favorite photo from this month",
    displayOrder: 2,
    pollOptions: [
      { optionId: "p1", label: "Option A" },
      { optionId: "p2", label: "Option B" },
      { optionId: "p3", label: "Option C" },
      { optionId: "p4", label: "Option D" },
    ],
    helperText: "Vote for someone else's photo. No looking at the tally until publish.",
  },
  { questionId: "q_may_4", kind: "text", prompt: "Something small that made you laugh", displayOrder: 3 },
  { questionId: "q_may_5", kind: "text", prompt: "What are you looking forward to in June?", displayOrder: 4 },
];

const myMayResponses: MyResponse[] = [
  {
    responseId: "r_may_1_q",
    questionId: "q_may_1",
    status: "published",
    kind: "text",
    body:
      "Took my first solo trip in years — three nights on the Olympic coast. Walked until my feet hurt, slept in the back of the truck, and didn't open my laptop once.",
    imageMediaIds: ["i_may_1", "i_may_2", "i_may_3"],
    version: 7,
    updatedAt: new Date(Date.now() - 60_000).toISOString(),
    publishedAt: new Date(Date.now() - 60_000).toISOString(),
    wordCount: 248,
  },
  {
    responseId: "r_may_2_q",
    questionId: "q_may_2",
    status: "draft",
    kind: "text",
    body:
      "Sam came over Sunday and we tried to make handmade ravioli. We did not succeed. The filling was incredible — brown butter, sage, ricotta, lemon zest — and we ended up just spooning it on top of pappardelle. 10/10, would fail again.",
    imageMediaIds: ["i_may_4", "i_may_5"],
    version: 3,
    updatedAt: new Date(Date.now() - 3 * 60_000).toISOString(),
    wordCount: 52,
  },
];

const mayNewsletter: OpenNewsletter = {
  groupId: "g_trail",
  groupName: "Trail Crew",
  cycleId: "202605",
  status: "open",
  responseOpenAt: "2026-05-01T04:00:00Z",
  responseCloseAt: new Date(Date.now() + 3 * 24 * 60 * 60 * 1000 + 4 * 60 * 60 * 1000).toISOString(),
  questions: mayQuestions,
  myResponses: myMayResponses,
  gradientClass: "hero-may",
  monthLabel: "May",
  yearLabel: "2026",
  hypeMessage: "Sam, Riley, and Alex have all started writing. Casey hasn't started yet — maybe say hi 👋",
};

const juneCandidates: CandidateQuestion[] = [
  {
    questionId: "qc_1",
    kind: "text",
    prompt: "If you could relive one day from this past month, which one and why?",
    voteCount: 7,
    votedByMe: true,
    submittedAt: "2026-05-01T10:00:00Z",
  },
  {
    questionId: "qc_2",
    kind: "text",
    prompt: "A song you put on repeat this month",
    voteCount: 5,
    votedByMe: false,
    submittedAt: "2026-05-03T12:00:00Z",
  },
  {
    questionId: "qc_3",
    kind: "poll",
    prompt: "Which of these should we do as a group this summer?",
    pollOptions: [
      { optionId: "po1", label: "camping" },
      { optionId: "po2", label: "backyard movie" },
      { optionId: "po3", label: "road trip" },
      { optionId: "po4", label: "cooking class" },
    ],
    voteCount: 5,
    votedByMe: true,
    submittedAt: "2026-04-30T08:00:00Z",
  },
  {
    questionId: "qc_4",
    kind: "text",
    prompt: 'What\'s the most "you" thing you did this month?',
    voteCount: 3,
    votedByMe: false,
    submittedAt: "2026-04-29T22:00:00Z",
  },
  {
    questionId: "qc_5",
    kind: "text",
    prompt: "Show us your most chaotic camera roll photo",
    voteCount: 2,
    votedByMe: false,
    submittedAt: "2026-04-28T19:00:00Z",
  },
  {
    questionId: "qc_6",
    kind: "text",
    prompt: "Tell us about a tiny act of kindness — given or received",
    voteCount: 2,
    votedByMe: false,
    submittedAt: "2026-04-25T11:00:00Z",
  },
  {
    questionId: "qc_7",
    kind: "text",
    prompt: "What's the closest you've come to a true adventure this year?",
    voteCount: 1,
    votedByMe: false,
    submittedAt: "2026-04-22T16:00:00Z",
  },
];

const juneNewsletter: VotingNewsletter = {
  groupId: "g_trail",
  groupName: "Trail Crew",
  cycleId: "202606",
  status: "voting",
  responseOpenAt: new Date(Date.now() + 12 * 24 * 60 * 60 * 1000).toISOString(),
  candidates: juneCandidates,
  myVoteCount: 2,
  votesPerUserPerCycle: 3,
  gradientClass: "hero-june",
  monthLabel: "June",
  yearLabel: "2026",
};

const newsletterSummaries: NewsletterSummary[] = [
  {
    cycleId: "202606",
    status: "voting",
    responseOpenAt: juneNewsletter.responseOpenAt,
    responseCloseAt: juneNewsletter.responseOpenAt,
    publishedAt: null,
    questionCount: 0,
    myDraftCount: 0,
    myPublishedCount: 0,
    gradientClass: "hero-june",
    monthLabel: "Jun",
    yearLabel: "2026",
  },
  {
    cycleId: "202605",
    status: "open",
    responseOpenAt: mayNewsletter.responseOpenAt,
    responseCloseAt: mayNewsletter.responseCloseAt,
    publishedAt: null,
    questionCount: mayQuestions.length,
    myDraftCount: myMayResponses.filter((r) => r.status === "draft").length,
    myPublishedCount: myMayResponses.filter((r) => r.status === "published").length,
    gradientClass: "hero-may",
    monthLabel: "May",
    yearLabel: "2026",
  },
  {
    cycleId: "202604",
    status: "published",
    responseOpenAt: "2026-04-01T04:00:00Z",
    responseCloseAt: "2026-05-01T04:00:00Z",
    publishedAt: "2026-05-01T04:00:00Z",
    questionCount: 5,
    myDraftCount: 0,
    myPublishedCount: 5,
    gradientClass: "hero-april",
    monthLabel: "Apr",
    yearLabel: "2026",
  },
  {
    cycleId: "202603",
    status: "published",
    responseOpenAt: "2026-03-01T05:00:00Z",
    responseCloseAt: "2026-04-01T04:00:00Z",
    publishedAt: "2026-04-01T04:00:00Z",
    questionCount: 5,
    myDraftCount: 0,
    myPublishedCount: 5,
    gradientClass: "hero-march",
    monthLabel: "Mar",
    yearLabel: "2026",
  },
  {
    cycleId: "202602",
    status: "published",
    responseOpenAt: "2026-02-01T05:00:00Z",
    responseCloseAt: "2026-03-01T05:00:00Z",
    publishedAt: "2026-03-01T05:00:00Z",
    questionCount: 4,
    myDraftCount: 0,
    myPublishedCount: 4,
    gradientClass: "hero-feb",
    monthLabel: "Feb",
    yearLabel: "2026",
  },
];

export const newsletters: Record<string, NewsletterDetail> = {
  "g_trail/202604": aprilNewsletter,
  "g_trail/202605": mayNewsletter,
  "g_trail/202606": juneNewsletter,
};

export const newsletterSummariesByGroup: Record<string, NewsletterSummary[]> = {
  g_trail: newsletterSummaries,
  g_game: [],
};

export const invites: Invite[] = [
  {
    code: "VHQM-2K8R-PX3F-T9JN",
    groupId: "g_trail",
    groupName: "Trail Crew",
    status: "pending",
    expiresAt: new Date(Date.now() + 5 * 24 * 60 * 60 * 1000).toISOString(),
    roleOnRedeem: "member",
  },
  {
    code: "JM7P-RTNQ-K42X-WB8Y",
    groupId: "g_trail",
    groupName: "Trail Crew",
    status: "consumed",
    expiresAt: "2026-04-30T00:00:00Z",
    consumedBy: "u_casey",
    consumedByName: "Casey",
    consumedAt: "2026-04-22T11:00:00Z",
    roleOnRedeem: "member",
  },
];

export const pushDevices: PushDevice[] = [
  {
    subscriptionId: "ps_pixel",
    userAgent: "Pixel 8 · Chrome",
    icon: "📱",
    lastSuccessAt: new Date(Date.now() - 2 * 60 * 60 * 1000).toISOString(),
  },
  {
    subscriptionId: "ps_mac",
    userAgent: "MacBook · Safari",
    icon: "💻",
    lastSuccessAt: new Date(Date.now() - 5 * 24 * 60 * 60 * 1000).toISOString(),
  },
];

export const notificationPrefs: NotificationPref[] = [
  { groupId: "g_trail", cycleOpen: true, deadlineReminders: true },
  { groupId: "g_game", cycleOpen: false, deadlineReminders: false },
];

export const config: AppConfig = {
  user: me,
  memberships: [
    { groupId: "g_trail", role: "admin", groupName: "Trail Crew", gradient: trailCrewGradient },
    { groupId: "g_game", role: "member", groupName: "Game Night Gang", gradient: gameNightGradient },
  ],
  vapidPublicKey: "MOCK_PUBLIC_KEY",
  groupDefaults: {
    questionsPerCycle: 5,
    votesPerUserPerCycle: 3,
    responseWindowDays: 4,
    timezone: "America/New_York",
    autosaveDebounceMs: 1500,
  },
};

// Convenience getter for components that need the friendly name of an answer's question
export function getQuestionPrompt(cycleKey: string, questionId: string): string | undefined {
  const nl = newsletters[cycleKey];
  if (!nl) return undefined;
  if (nl.status === "published") return nl.questions.find((q) => q.questionId === questionId)?.prompt;
  if (nl.status === "open") return nl.questions.find((q) => q.questionId === questionId)?.prompt;
  return undefined;
}

export function memberDisplay(groupId: string, userId: string) {
  const g = groups[groupId];
  return g?.members.find((m) => m.userId === userId);
}

void memberByName;
