import type {
  AnswerView,
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
} from "../api/types";

// MOCK in-memory store. Mutations write back here so the UI feels live across
// navigations within a session. Reload to reset. Replace with a real backend
// by swapping out `src/mocks/api.ts`.

export const me = {
  userId: "u_quinn",
  email: "bryan.quinn17@gmail.com",
  displayName: "Quinn",
  avatarColor: "grape",
};

export const trailCrewGradient = { from: "#FF5A6B", to: "#FFB178", className: "hero-april" };
export const gameNightGradient = { from: "#7B5BFF", to: "#5AC8FF", className: "hero-may" };
export const meepleMailboxGradient = { from: "#2E7D5B", to: "#F2C94C", className: "hero-meeple" };

const trailCrewMembers = [
  {
    userId: "u_quinn",
    displayName: "Quinn",
    role: "admin" as const,
    avatarColor: "grape",
    editionsAnswered: 5,
    joinedAt: "2025-01-12T00:00:00Z",
  },
  {
    userId: "u_sam",
    displayName: "Sam",
    role: "admin" as const,
    avatarColor: "coral",
    editionsAnswered: 5,
    joinedAt: "2025-01-12T00:00:00Z",
  },
  {
    userId: "u_riley",
    displayName: "Riley",
    role: "member" as const,
    avatarColor: "mint",
    editionsAnswered: 5,
    joinedAt: "2025-01-12T00:00:00Z",
  },
  {
    userId: "u_alex",
    displayName: "Alex",
    role: "member" as const,
    avatarColor: "grape",
    editionsAnswered: 5,
    joinedAt: "2025-02-04T00:00:00Z",
  },
  {
    userId: "u_jordan",
    displayName: "Jordan",
    role: "member" as const,
    avatarColor: "sky",
    editionsAnswered: 3,
    joinedAt: "2025-04-14T00:00:00Z",
  },
  {
    userId: "u_morgan",
    displayName: "Morgan",
    role: "member" as const,
    avatarColor: "sun",
    editionsAnswered: 2,
    joinedAt: "2025-09-02T00:00:00Z",
  },
  {
    userId: "u_casey",
    displayName: "Casey",
    role: "member" as const,
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
      avatarColor: "grape",
      editionsAnswered: 2,
      joinedAt: "2025-08-01T00:00:00Z",
    },
  ],
  gradient: gameNightGradient,
};

// Meeple Mailbox — a published-only sample group based on the imported Issue No.9
const meepleMembers = [
  {
    userId: "u_quinn",
    displayName: "Quinn",
    role: "member" as const,
    avatarColor: "grape",
    editionsAnswered: 9,
    joinedAt: "2024-08-01T00:00:00Z",
  },
  { userId: "u_m_tara", displayName: "Tara", role: "admin" as const, avatarColor: "coral", editionsAnswered: 9, joinedAt: "2024-08-01T00:00:00Z" },
  { userId: "u_m_kaavya", displayName: "Kaavya", role: "member" as const, avatarColor: "sun", editionsAnswered: 9, joinedAt: "2024-08-01T00:00:00Z" },
  { userId: "u_m_maria", displayName: "Maria", role: "member" as const, avatarColor: "mint", editionsAnswered: 9, joinedAt: "2024-08-01T00:00:00Z" },
  { userId: "u_m_christian", displayName: "Christian", role: "member" as const, avatarColor: "sky", editionsAnswered: 9, joinedAt: "2024-08-01T00:00:00Z" },
  { userId: "u_m_meha", displayName: "Meha", role: "admin" as const, avatarColor: "peach", editionsAnswered: 9, joinedAt: "2024-08-01T00:00:00Z" },
  { userId: "u_m_rohan", displayName: "Rohan", role: "member" as const, avatarColor: "grape", editionsAnswered: 9, joinedAt: "2024-08-01T00:00:00Z" },
  { userId: "u_m_brigid", displayName: "Brigid", role: "member" as const, avatarColor: "mint", editionsAnswered: 7, joinedAt: "2024-09-15T00:00:00Z" },
  { userId: "u_m_gordon", displayName: "Gordon", role: "member" as const, avatarColor: "sky", editionsAnswered: 9, joinedAt: "2024-08-01T00:00:00Z" },
  { userId: "u_m_walt", displayName: "Walt", role: "member" as const, avatarColor: "coral", editionsAnswered: 9, joinedAt: "2024-08-01T00:00:00Z" },
  { userId: "u_m_calvin", displayName: "Calvin", role: "member" as const, avatarColor: "sun", editionsAnswered: 9, joinedAt: "2024-08-01T00:00:00Z" },
  { userId: "u_m_kari", displayName: "Kari", role: "member" as const, avatarColor: "peach", editionsAnswered: 6, joinedAt: "2024-11-01T00:00:00Z" },
];

export const meepleMailbox: Group = {
  groupId: "g_meeple",
  name: "Meeple Mailbox",
  timezone: "America/New_York",
  cycleSettings: { questionsPerCycle: 8, votesPerUserPerCycle: 4, responseWindowDays: 7 },
  notificationSettings: { offsetsHoursBeforeClose: [72, 24], onCycleOpen: true },
  memberSoftCap: 50,
  members: meepleMembers,
  gradient: meepleMailboxGradient,
};

const meepleMember = (name: string) => meepleMembers.find((m) => m.displayName === name)!;

export const groups: Record<string, Group> = {
  [trailCrew.groupId]: trailCrew,
  [gameNight.groupId]: gameNight,
  [meepleMailbox.groupId]: meepleMailbox,
};

// April 2026 — published, the rich edition
//
// Authorship convention for sample data: questions submitted by a specific
// member carry an `askedBy` object with that member's id/name (matching the
// "Kari asked: …" rendering in the UI). Questions that represent a recurring
// "house" prompt are marked `isAnonymous: true` so they render as
// "Asked anonymously" rather than attributed to any one person.
const askedByMember = (m: { userId: string; displayName: string; avatarColor?: string }) => ({
  userId: m.userId,
  displayName: m.displayName,
  avatarColor: m.avatarColor,
});

const aprilQuestions: NonNullable<PublishedNewsletter["questions"]> = [
  {
    questionId: "q_apr_1",
    kind: "text",
    prompt: "What was the highlight of your April?",
    displayOrder: 0,
    isAnonymous: true,
    askedBy: null,
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
    isAnonymous: false,
    askedBy: askedByMember(memberByName("Sam")),
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
    isAnonymous: false,
    askedBy: askedByMember(memberByName("Riley")),
    options: [
      { optionId: "o1", label: "Lake 22", voteCount: 7 },
      { optionId: "o2", label: "Mt Si", voteCount: 4 },
      { optionId: "o3", label: "Mailbox Peak", voteCount: 1 },
      { optionId: "o4", label: "Wallace Falls", voteCount: 0 },
    ],
    totalVotes: 12,
    myVoteOptionId: "o1",
    comments: [],
  },
  {
    questionId: "q_apr_4",
    kind: "text",
    prompt: "Something small that made you laugh",
    displayOrder: 3,
    isAnonymous: false,
    askedBy: askedByMember(memberByName("Morgan")),
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
    isAnonymous: true,
    askedBy: null,
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
  {
    questionId: "q_may_1",
    kind: "text",
    prompt: "What was the highlight of your May?",
    displayOrder: 0,
    isAnonymous: true,
    askedBy: null,
  },
  {
    questionId: "q_may_2",
    kind: "text",
    prompt: "Best meal you ate or cooked this month",
    displayOrder: 1,
    isAnonymous: false,
    askedBy: askedByMember(memberByName("Sam")),
  },
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
    isAnonymous: false,
    askedBy: askedByMember(memberByName("Quinn")),
  },
  {
    questionId: "q_may_4",
    kind: "text",
    prompt: "Something small that made you laugh",
    displayOrder: 3,
    isAnonymous: true,
    askedBy: null,
  },
  {
    questionId: "q_may_5",
    kind: "text",
    prompt: "What are you looking forward to in June?",
    displayOrder: 4,
    isAnonymous: false,
    askedBy: askedByMember(memberByName("Alex")),
  },
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
    isAnonymous: false,
    askedBy: askedByMember(memberByName("Sam")),
  },
  {
    questionId: "qc_2",
    kind: "text",
    prompt: "A song you put on repeat this month",
    voteCount: 5,
    votedByMe: false,
    submittedAt: "2026-05-03T12:00:00Z",
    isAnonymous: false,
    askedBy: askedByMember(memberByName("Riley")),
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
    isAnonymous: false,
    askedBy: askedByMember(memberByName("Quinn")),
  },
  {
    questionId: "qc_4",
    kind: "text",
    prompt: 'What\'s the most "you" thing you did this month?',
    voteCount: 3,
    votedByMe: false,
    submittedAt: "2026-04-29T22:00:00Z",
    // Submitted anonymously — `askedBy` is null for non-admin callers.
    isAnonymous: true,
    askedBy: null,
  },
  {
    questionId: "qc_5",
    kind: "text",
    prompt: "Show us your most chaotic camera roll photo",
    voteCount: 2,
    votedByMe: false,
    submittedAt: "2026-04-28T19:00:00Z",
    isAnonymous: false,
    askedBy: askedByMember(memberByName("Alex")),
  },
  {
    questionId: "qc_6",
    kind: "text",
    prompt: "Tell us about a tiny act of kindness — given or received",
    voteCount: 2,
    votedByMe: false,
    submittedAt: "2026-04-25T11:00:00Z",
    isAnonymous: true,
    askedBy: null,
  },
  {
    questionId: "qc_7",
    kind: "text",
    prompt: "What's the closest you've come to a true adventure this year?",
    voteCount: 1,
    votedByMe: false,
    submittedAt: "2026-04-22T16:00:00Z",
    isAnonymous: false,
    askedBy: askedByMember(memberByName("Casey")),
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

// Meeple Mailbox — Issue No.9 · May 3rd, 2026 (published)
//
// Real images live in `frontend/public/sample/` (copied verbatim from the
// imported sample newsletter). Each id below is also the filename stem.
const meepleImg = (stem: string): string => `/sample/${stem}.jpg`;

const meepleAnswer = (
  responseId: string,
  memberName: string,
  body: string,
  opts: {
    publishedAt?: string;
    images?: { imageId: string; alt: string; displayUrl?: string; caption?: string }[];
    reactions?: { emoji: string; count: number; reactedByMe?: boolean }[];
    comments?: {
      author: string;
      body: string;
      createdAt?: string;
      image?: { imageId: string; alt?: string; displayUrl: string };
    }[];
    displayNameOverride?: string;
  } = {},
): AnswerView => {
  const member = meepleMember(memberName);
  return {
    responseId,
    userId: member.userId,
    displayName: opts.displayNameOverride ?? member.displayName,
    avatarColor: member.avatarColor,
    body,
    images: (opts.images ?? []).map((i) => ({
      imageId: i.imageId,
      status: "ready" as const,
      alt: i.alt,
      displayUrl: i.displayUrl ?? null,
      thumbUrl: i.displayUrl ?? null,
      caption: i.caption ?? null,
    })),
    publishedAt: opts.publishedAt ?? "2026-05-03T16:00:00Z",
    comments: (opts.comments ?? []).map((c, idx) => {
      const cm = meepleMember(c.author);
      return {
        commentId: `c_${responseId}_${idx}`,
        authorUserId: cm.userId,
        authorDisplayName: cm.displayName,
        authorAvatarColor: cm.avatarColor,
        body: c.body,
        image: c.image
          ? { imageId: c.image.imageId, status: "ready" as const, alt: c.image.alt, displayUrl: c.image.displayUrl, thumbUrl: c.image.displayUrl }
          : null,
        createdAt: c.createdAt ?? "2026-05-04T18:00:00Z",
        editedAt: null,
      };
    }),
    reactionGroups: (opts.reactions ?? []).map((r) => ({ emoji: r.emoji, count: r.count, reactedByMe: r.reactedByMe ?? false })),
  };
};

const meepleQuestions: NonNullable<PublishedNewsletter["questions"]> = [
  {
    questionId: "q_meeple_1",
    kind: "poll",
    // Authorship is now carried in `askedBy`; the UI prefixes "Kari asked:"
    // automatically. The prompt itself is the bare question.
    prompt:
      "Hypothetical — you're staying with two friends in another city. Days before arrival, friend A tells you friend B suddenly has to leave town for the duration. On arrival, friend A also has to flee under mysterious circumstances. Both are unreachable for ~a week. You let yourself in and discover a large stove pot of unidentified pasta dish taking up half the fridge. What do you do?",
    displayOrder: 0,
    isAnonymous: false,
    askedBy: askedByMember(meepleMember("Kari")),
    options: [
      { optionId: "o_pasta_a", label: "A: Throw out the pasta — you don't want them coming back to a rotten fridge", voteCount: 0 },
      { optionId: "o_pasta_b", label: "B: Leave it alone — you don't know its plans for it", voteCount: 8 },
      { optionId: "o_pasta_c", label: "C: Eat it — you're hungry, despite living in a major metro", voteCount: 3 },
    ],
    totalVotes: 11,
    myVoteOptionId: "o_pasta_b",
    comments: [
      {
        commentId: "c_meeple_pasta_christian",
        authorUserId: "u_m_christian",
        authorDisplayName: "Christian",
        authorAvatarColor: "sky",
        body: "Damn I really erased that explicit line completely. I should have known this question had covered all its bases. I am also very curious what the origins are for this question",
        createdAt: "2026-05-04T18:00:00Z",
        editedAt: null,
      },
    ],
  },
  {
    questionId: "q_meeple_2",
    kind: "text",
    prompt: "Has there been a celebrity death that impacted you more than you might have expected?",
    displayOrder: 1,
    isAnonymous: false,
    askedBy: askedByMember(meepleMember("Meha")),
    answers: [
      meepleAnswer(
        "r_meeple_2_tara",
        "Tara",
        "I don't really think so, not more than I would expect. There have definitely been several that made me sad and felt significant. I think the first celebrity death I really perceived was Alan Rickman, partly because I don't really know that many celebrities (especially when I was younger) but y'know I did know Harry Potter as a kid; so that one I think has stuck in my memory a bit.",
      ),
      meepleAnswer(
        "r_meeple_2_kaavya",
        "Kaavya",
        "I was moved by Matthew Perry's death more than I expected. I think it was because he spoke out so much about his struggles with addiction and depression, the fact that he died from K (I think, I might have the details wrong) felt like insult to the injury of losing such a great actor and advocate for other people.",
      ),
      meepleAnswer(
        "r_meeple_2_maria",
        "Maria",
        "Steve Prefontaine's death in 1975 affected me more than one might think, since people in the running world never let you forget about him. Also looking him up reminded me that he died on my birthday 🎂🥳",
      ),
      meepleAnswer(
        "r_meeple_2_bryan",
        "Quinn",
        "Mac Miller. I'm a big fan of so much of his music, and I still listen to it multiple times a week (he died in 2018). I wish he were still around and making music.\n\n*Please skip if you do not want to be super bummed out.*\n\nAlso, not to get too dark in the meeple newsletter, but I had far too many friends from High School and College lose their lives to either drugs or suicide. I wasn't as close to any of them when it happened, but I guess that's how those things tend to go. People get isolated, and you don't know they're struggling until it's too late. A lot of his music towards the end of his life seemed like an obvious cry for help, and it reminds me of those former friends and what they might have been going through.",
        { displayNameOverride: "Bryan", reactions: [{ emoji: "❤️", count: 3, reactedByMe: false }] },
      ),
      meepleAnswer(
        "r_meeple_2_meha",
        "Meha",
        "Andre Braugher (Captain Holt from Brooklyn Nine-Nine) because I loved his character and also his castmates spoke/speak so highly of him as a person and actor. It sounds like he was very dedicated to his family so that just makes me especially sad for them. Also I have a parasocial relationship and love for Andy Samberg (so anything that affects him affects me).",
      ),
      meepleAnswer(
        "r_meeple_2_rohan",
        "Rohan",
        "For me it was Matthew Perry's death. Friends has always been one of those comfort shows for me, and it felt really out of nowhere on Halloween a few years ago.",
      ),
      meepleAnswer(
        "r_meeple_2_calvin",
        "Calvin",
        "I tried to think hard about this and I guess none really since I don't consider myself a fan of any celebrity really (at least in the normal sense).\n\nAnthony Bourdain is one I think about.",
      ),
      meepleAnswer(
        "r_meeple_2_gordon",
        "Gordon",
        "I felt kind of sad when Pope Francis died despite not being Catholic or religious at all, and despite disagreeing with some of his less-progressive viewpoints. But I think seeing what he meant to a lot of people and watching others mourn for him got to me too.",
        { reactions: [{ emoji: "⛪", count: 1 }] },
      ),
      meepleAnswer(
        "r_meeple_2_walt",
        "Walt",
        "Probably Kelvin Kiptum. It was pretty crazy that he came out of nowhere and set the marathon world record as a 21-year-old. Then he gets in a car crash like 3 months later. Can't take time for granted. Edit: world record just got broken in London. By over a minute. Also by 3 people in the same race. It's partly the shoes but probably even more because of the high-carb and bicarb fueling nowadays.",
      ),
    ],
  },
  {
    questionId: "q_meeple_3",
    kind: "text",
    prompt: "Do you have ideas for healthy, easy, filling snacks for the work week? I'm okay with some prep/assembly!",
    displayOrder: 2,
    isAnonymous: false,
    askedBy: askedByMember(meepleMember("Meha")),
    answers: [
      meepleAnswer("r_meeple_3_tara", "Tara", "Unfortunately I don't think most of my regular snacks fall into the particularly healthy category. I almost exclusively source my office snacks from Trader Joe's lol. But I feel like nuts are a classic."),
      meepleAnswer(
        "r_meeple_3_kaavya",
        "Kaavya",
        "I am a big fan of making a dip and eating it during the week w baby carrots! I made [this fava bean dip](https://thewanderlustkitchen.com/easy-fava-bean-dip/) recently — super easy and less basic than ur typical hummus!",
        { reactions: [{ emoji: "❤️", count: 3, reactedByMe: true }] },
      ),
      meepleAnswer("r_meeple_3_maria", "Maria", "i usually just go for cheese and crackers"),
      meepleAnswer(
        "r_meeple_3_bryan",
        "Quinn",
        "I like Barbells protein bars, but if you don't like the artificial sweetener taste, then I wouldn't recommend them. Another go-to for me is Babybel cheese.",
        { displayNameOverride: "Bryan" },
      ),
      meepleAnswer(
        "r_meeple_3_christian",
        "Christian",
        "Not original, but I am a fan of carrots and hummus, or celery and peanut butter. Both are relatively minimal to prep and will get me through the work week.\n\nAlso I like to have mixed nuts or almonds or something at my desk as well.",
      ),
      meepleAnswer("r_meeple_3_meha", "Meha", "My mom roasts batches of raw cashews and walnuts to just have on hand as a snack for the week and I did really enjoy that the one time I did it at home."),
      meepleAnswer(
        "r_meeple_3_rohan",
        "Rohan",
        "IDK if this is a faux pas to bring to work but tuna salad and crackers is healthy/high protein and really easy to make. I've been really enjoying tuna salad sandwiches as a lunch because I can meal prep some for a few days but it still feels homemade each day.",
        { reactions: [{ emoji: "❤️", count: 3, reactedByMe: true }] },
      ),
      meepleAnswer("r_meeple_3_calvin", "Calvin", "I wish I had an answer to this but I currently do not. Also going to be looking at other peoples' responses."),
      meepleAnswer("r_meeple_3_brigid", "Brigid", "I mostly just have bell peppers and carrots with hummus."),
      meepleAnswer("r_meeple_3_gordon", "Gordon", "I like a bag of almonds with raisins (about a 50/50 split)."),
      meepleAnswer(
        "r_meeple_3_walt",
        "Walt",
        "I had to use up my sesame seeds the other day so I made pasteli. It's literally just sesame seeds and honey in energy bar form. You have to get the ratio right — it's supposed to be 1:1. It also helps the higher quality honey you have. I used a little too much honey and probably could've cooked it a little longer caused mine turned out a lil too sticky.",
        { reactions: [{ emoji: "❤️", count: 3 }] },
      ),
    ],
  },
  {
    questionId: "q_meeple_4",
    kind: "poll",
    prompt: "pick a tiny home (all have leaky roofs though)",
    displayOrder: 3,
    isAnonymous: false,
    askedBy: askedByMember(meepleMember("Maria")),
    options: [
      { optionId: "o_home_beach", label: "beach hut", voteCount: 5 },
      { optionId: "o_home_tree", label: "tree house", voteCount: 5 },
      { optionId: "o_home_cliff", label: "cliffside cave home", voteCount: 0 },
      { optionId: "o_home_roof", label: "rooftop studio apt in city skyscraper", voteCount: 1 },
    ],
    totalVotes: 11,
    myVoteOptionId: "o_home_tree",
    comments: [],
  },
  {
    questionId: "q_meeple_5",
    kind: "text",
    prompt: "name something happening in your neighborhood!",
    displayOrder: 4,
    isAnonymous: false,
    askedBy: askedByMember(meepleMember("Maria")),
    answers: [
      meepleAnswer("r_meeple_5_tara", "Tara", "Is Porchfest a cop-out?\n\nThere was also Somerville Open Studios this weekend but I didn't get a chance to check it out."),
      meepleAnswer(
        "r_meeple_5_kaavya",
        "Kaavya",
        "This weekend was flower mart in Baltimore! It's surprisingly over a 100 years old and was originally started by the women's civic league to promote better living conditions throughout Baltimore. There's a bunch of local vendors who sell plants/art, but most importantly it is the home of the famous Baltimore lemon stick!",
        {
          images: [{ imageId: "meep_kaavya_XweoS4YqsAoLqfaVt6KI_0", alt: "Baltimore lemon stick", displayUrl: meepleImg("meep_kaavya_XweoS4YqsAoLqfaVt6KI_0") }],
          reactions: [{ emoji: "🍋", count: 2 }, { emoji: "❤️", count: 1 }, { emoji: "🤔", count: 1 }],
        },
      ),
      meepleAnswer("r_meeple_5_bryan", "Quinn", "This is probably cheating, but Porchfest is next weekend, and I'm very excited!", { displayNameOverride: "Bryan" }),
      meepleAnswer(
        "r_meeple_5_meha",
        "Meha",
        "SPRING!",
        {
          images: [
            { imageId: "meep_meha_XweoS4YqsAoLqfaVt6KI_0", alt: "spring blooms", displayUrl: meepleImg("meep_meha_XweoS4YqsAoLqfaVt6KI_0") },
            { imageId: "meep_meha_XweoS4YqsAoLqfaVt6KI_1", alt: "spring blooms", displayUrl: meepleImg("meep_meha_XweoS4YqsAoLqfaVt6KI_1") },
            { imageId: "meep_meha_XweoS4YqsAoLqfaVt6KI_2", alt: "spring blooms", displayUrl: meepleImg("meep_meha_XweoS4YqsAoLqfaVt6KI_2") },
            { imageId: "meep_meha_XweoS4YqsAoLqfaVt6KI_3", alt: "spring blooms", displayUrl: meepleImg("meep_meha_XweoS4YqsAoLqfaVt6KI_3") },
            { imageId: "meep_meha_XweoS4YqsAoLqfaVt6KI_4", alt: "spring blooms", displayUrl: meepleImg("meep_meha_XweoS4YqsAoLqfaVt6KI_4") },
            { imageId: "meep_meha_XweoS4YqsAoLqfaVt6KI_5", alt: "spring blooms", displayUrl: meepleImg("meep_meha_XweoS4YqsAoLqfaVt6KI_5") },
          ],
          reactions: [{ emoji: "❤️", count: 2 }],
        },
      ),
      meepleAnswer(
        "r_meeple_5_rohan",
        "Rohan",
        "It is spring in Chicago! Still persistently cold, but the flowers and greenery have made for exceptional walks.",
        {
          images: [
            { imageId: "meep_rohan_XweoS4YqsAoLqfaVt6KI_0", alt: "Chicago spring", displayUrl: meepleImg("meep_rohan_XweoS4YqsAoLqfaVt6KI_0") },
            { imageId: "meep_rohan_XweoS4YqsAoLqfaVt6KI_1", alt: "Chicago spring", displayUrl: meepleImg("meep_rohan_XweoS4YqsAoLqfaVt6KI_1") },
            { imageId: "meep_rohan_XweoS4YqsAoLqfaVt6KI_2", alt: "Chicago spring", displayUrl: meepleImg("meep_rohan_XweoS4YqsAoLqfaVt6KI_2") },
          ],
          reactions: [{ emoji: "❤️", count: 4, reactedByMe: true }],
          comments: [{ author: "Rohan", body: "The creature? I think a groundhog but who knows" }],
        },
      ),
      meepleAnswer(
        "r_meeple_5_calvin",
        "Calvin",
        "I guess the big general Cornell news is we got Chainsmokers and Daya for slope day (our spring concert).\n\nAnd some other crazy heated news that made it to the big leagues (NY Times): Cornell's president Michael Kotlikoff('s driver) is caught on video backing a car and hitting a student behind them and running over a student's foot after being confronted about freedom of speech on campus. This came in the aftermath of also after a hosted debate with Norman Finkelstein, who is famous for many reasons. The debate was titled \"Israel Was Not Justified in its Response to Oct. 7\". So yeah, one can imagine how heated this was. The students followed the president and kept pressing him until he got in his car and then the car backing into student happened. Kind of crazy news tbh.\n\n[NYT story](https://www.nytimes.com/2026/05/01/us/cornell-president-student-protester.html)\n\nThere is seemingly a general frustration with Kotlikoff with the suppression of student protests and voices (related to pro-palestine) while other student orgs have literally invited the likes of Ann Coulter, an actual IDF soldier, the federalist society inviting a law professor who has basically said black people aren't smart enough to be lawyers, etc — while revoking Kehlani's slope day performance because people are too uncomfortable with her expression for pro palestine. And also Cornell folding to the Trump administration. (We're no better than Columbia.)",
        { reactions: [{ emoji: "😮", count: 1 }] },
      ),
      meepleAnswer("r_meeple_5_brigid", "Brigid", "Went to Harvard art festival this weekend, which had some music and dance and was fun in a student project type way but am also excited for porchfest next week."),
      meepleAnswer("r_meeple_5_gordon", "Gordon", "Medford Porchfest is June 6th! Mark your calendars."),
      meepleAnswer("r_meeple_5_walt", "Walt", "Porchfest May 9! I am cutting my Hawaii vacation a day shorter than I could've just so that I can be back in time for the best day of the year in Somerville!", { reactions: [{ emoji: "❤️", count: 2 }] }),
    ],
  },
  {
    questionId: "q_meeple_6",
    kind: "text",
    prompt: "📸 Photo Wall — share what you've been seeing this month",
    displayOrder: 5,
    // Recurring "house" prompt — no specific submitter to attribute it to.
    isAnonymous: true,
    askedBy: null,
    answers: [
      meepleAnswer(
        "r_meeple_6_tara",
        "Tara",
        "Idk how many of you know Sophia Yan, but I went to her wedding! Also some cool fog from a 17th floor downtown.",
        {
          images: [
            { imageId: "meep_tara_photowall_0", alt: "Sophia's wedding", displayUrl: meepleImg("meep_tara_photowall_0") },
            { imageId: "meep_tara_photowall_1", alt: "downtown fog from the 17th floor", displayUrl: meepleImg("meep_tara_photowall_1") },
          ],
          reactions: [{ emoji: "❤️", count: 3 }],
        },
      ),
      meepleAnswer(
        "r_meeple_6_maria",
        "Maria",
        "Pics from my brother's wedding! It was the best wedding I've ever been to. Being the sister of the groom was fun and because they had been dating so long, I felt like I knew everyone there. I love how my dress came out so that also felt good. And my speech went well! Highlights include joking around with my cousins (cousin table at a wedding is top tier good times) and seeing my 92 y/o great uncle Eugene on the dance floor the whole time (pictured in conga line).",
        {
          images: [
            { imageId: "meep_maria_photowall_0", alt: "brother's wedding", displayUrl: meepleImg("meep_maria_photowall_0") },
            { imageId: "meep_maria_photowall_1", alt: "brother's wedding", displayUrl: meepleImg("meep_maria_photowall_1") },
            { imageId: "meep_maria_photowall_2", alt: "brother's wedding", displayUrl: meepleImg("meep_maria_photowall_2") },
            { imageId: "meep_maria_photowall_3", alt: "brother's wedding", displayUrl: meepleImg("meep_maria_photowall_3") },
            { imageId: "meep_maria_photowall_4", alt: "uncle Eugene on the dance floor", displayUrl: meepleImg("meep_maria_photowall_4"), caption: "Uncle Eugene (92) running the conga line" },
            { imageId: "meep_maria_photowall_5", alt: "cousin table at the wedding", displayUrl: meepleImg("meep_maria_photowall_5") },
          ],
          reactions: [{ emoji: "❤️", count: 4 }],
          comments: [{ author: "Maria", body: "thank you!!" }],
        },
      ),
      meepleAnswer(
        "r_meeple_6_bryan",
        "Quinn",
        "Marathon pics + random friendly cat that came up to Leah and me at Prospect Hill.",
        {
          displayNameOverride: "Bryan",
          images: [
            { imageId: "meep_bryan_photowall_0", alt: "marathon", displayUrl: meepleImg("meep_bryan_photowall_0"), caption: "Mile 18 — still smiling somehow" },
            { imageId: "meep_bryan_photowall_1", alt: "marathon", displayUrl: meepleImg("meep_bryan_photowall_1") },
            { imageId: "meep_bryan_photowall_2", alt: "friendly cat at Prospect Hill", displayUrl: meepleImg("meep_bryan_photowall_2"), caption: "Prospect Hill's unofficial mayor" },
          ],
          reactions: [{ emoji: "❤️", count: 2 }],
        },
      ),
      meepleAnswer(
        "r_meeple_6_meha",
        "Meha",
        "Hello from day ~1.5 in Japan with Gordon and my parents!! We're in Tokyo for the first 5 days and I'm so excited to explore more of this city and beyond!!!",
        {
          images: [
            { imageId: "meep_meha_photowall_0", alt: "Tokyo", displayUrl: meepleImg("meep_meha_photowall_0") },
            { imageId: "meep_meha_photowall_1", alt: "Tokyo", displayUrl: meepleImg("meep_meha_photowall_1") },
            { imageId: "meep_meha_photowall_2", alt: "Tokyo", displayUrl: meepleImg("meep_meha_photowall_2") },
            { imageId: "meep_meha_photowall_3", alt: "Tokyo", displayUrl: meepleImg("meep_meha_photowall_3") },
            { imageId: "meep_meha_photowall_4", alt: "Tokyo", displayUrl: meepleImg("meep_meha_photowall_4") },
            { imageId: "meep_meha_photowall_5", alt: "Tokyo", displayUrl: meepleImg("meep_meha_photowall_5") },
          ],
          reactions: [{ emoji: "❤️", count: 5, reactedByMe: true }, { emoji: "🗾", count: 1 }],
        },
      ),
      meepleAnswer(
        "r_meeple_6_rohan",
        "Rohan",
        "We had some crazy tornado warnings, and a tornado touched down about 60ft from my parent's house last weekend. All-in-all lucky that nothing serious was destroyed, but we lost an apple tree, peach tree and the neighbor's roof got pulled up.",
        {
          images: [
            { imageId: "meep_rohan_photowall_0", alt: "tornado damage", displayUrl: meepleImg("meep_rohan_photowall_0") },
            { imageId: "meep_rohan_photowall_1", alt: "tornado damage", displayUrl: meepleImg("meep_rohan_photowall_1") },
          ],
          reactions: [{ emoji: "😮", count: 3 }],
        },
      ),
      meepleAnswer(
        "r_meeple_6_brigid",
        "Brigid",
        "Trip to AZ.",
        {
          images: [
            { imageId: "meep_brigid_photowall_0", alt: "Arizona", displayUrl: meepleImg("meep_brigid_photowall_0") },
            { imageId: "meep_brigid_photowall_1", alt: "Arizona", displayUrl: meepleImg("meep_brigid_photowall_1") },
            { imageId: "meep_brigid_photowall_2", alt: "Arizona", displayUrl: meepleImg("meep_brigid_photowall_2") },
            { imageId: "meep_brigid_photowall_3", alt: "Arizona", displayUrl: meepleImg("meep_brigid_photowall_3") },
            { imageId: "meep_brigid_photowall_4", alt: "Arizona", displayUrl: meepleImg("meep_brigid_photowall_4") },
            { imageId: "meep_brigid_photowall_5", alt: "Arizona", displayUrl: meepleImg("meep_brigid_photowall_5") },
            { imageId: "meep_brigid_photowall_6", alt: "Arizona", displayUrl: meepleImg("meep_brigid_photowall_6") },
          ],
          reactions: [{ emoji: "🌵", count: 3 }],
        },
      ),
      meepleAnswer(
        "r_meeple_6_gordon",
        "Gordon",
        "Sign-making for Walt running the Boston Marathon, lilac and pear tree in bloom at 19 Vassar St, and first photos from Tokyo where Meha and I now are!",
        {
          images: [
            { imageId: "meep_gordon_photowall_0", alt: "marathon sign", displayUrl: meepleImg("meep_gordon_photowall_0"), caption: "Walt's official cheer squad signage" },
            { imageId: "meep_gordon_photowall_1", alt: "lilac in bloom at 19 Vassar", displayUrl: meepleImg("meep_gordon_photowall_1"), caption: "19 Vassar lilac, finally" },
            { imageId: "meep_gordon_photowall_2", alt: "Tokyo", displayUrl: meepleImg("meep_gordon_photowall_2") },
            { imageId: "meep_gordon_photowall_3", alt: "Tokyo", displayUrl: meepleImg("meep_gordon_photowall_3"), caption: "First night out in Shinjuku" },
          ],
          reactions: [{ emoji: "❤️", count: 4 }],
        },
      ),
      meepleAnswer(
        "r_meeple_6_walt",
        "Walt",
        "Thanks for all the support during the marathon!",
        {
          images: [
            { imageId: "meep_walt_photowall_0", alt: "Boston Marathon", displayUrl: meepleImg("meep_walt_photowall_0") },
            { imageId: "meep_walt_photowall_1", alt: "Boston Marathon", displayUrl: meepleImg("meep_walt_photowall_1") },
            { imageId: "meep_walt_photowall_2", alt: "Boston Marathon", displayUrl: meepleImg("meep_walt_photowall_2") },
            { imageId: "meep_walt_photowall_3", alt: "Boston Marathon", displayUrl: meepleImg("meep_walt_photowall_3") },
            { imageId: "meep_walt_photowall_4", alt: "Boston Marathon", displayUrl: meepleImg("meep_walt_photowall_4") },
            { imageId: "meep_walt_photowall_5", alt: "Boston Marathon", displayUrl: meepleImg("meep_walt_photowall_5") },
          ],
          reactions: [{ emoji: "❤️", count: 4, reactedByMe: true }],
        },
      ),
    ],
  },
  {
    questionId: "q_meeple_7",
    kind: "text",
    prompt: "💭 On Your Mind — what's been on yours this month?",
    displayOrder: 6,
    isAnonymous: true,
    askedBy: null,
    answers: [
      meepleAnswer(
        "r_meeple_7_tara",
        "Tara",
        "I stopped by Porter Square Books on Indie Bookstore Day and it was so much fun! I rolled a d20 for this mystery book.\n\nLow key I feel like I've had a lot of things piling up for me recently so I'm feeling kinda stressed :(",
        {
          images: [
            { imageId: "meep_tara_onyourmind_0", alt: "mystery book from Porter Square Books", displayUrl: meepleImg("meep_tara_onyourmind_0") },
          ],
        },
      ),
      meepleAnswer(
        "r_meeple_7_kaavya",
        "Kaavya",
        "I've been thinking a lot about conditional friendships! With most of the friends that I've had for 4+ years, those relationships are unconditional. And by that I mean that I'm not friends with those people because i think they're funny or smart or cool or accomplished, but because something in me just loves the other person, even through the instances where they've not been funny or intelligent or suave or whatever. I think (hope) all of the people I feel this way about feel the same about me. At the same time, when we initially became friends it absolutely was conditional and because I thought they were funny or cool or something. I wonder when that shift towards unconditionality happens, and if it can be brought on by anything other than sheer time together.",
        { reactions: [{ emoji: "❤️", count: 1 }] },
      ),
      meepleAnswer(
        "r_meeple_7_maria",
        "Maria",
        "Using this to put more photos. me and becca ran a 50k! here are some pics of me before and after. including the cakes that our friend jane made us post race and me double fisting water afterwards.",
        {
          images: [
            { imageId: "meep_maria_onyourmind_0", alt: "pre-race", displayUrl: meepleImg("meep_maria_onyourmind_0") },
            { imageId: "meep_maria_onyourmind_1", alt: "post-race cakes", displayUrl: meepleImg("meep_maria_onyourmind_1") },
            { imageId: "meep_maria_onyourmind_2", alt: "double-fisting water", displayUrl: meepleImg("meep_maria_onyourmind_2") },
            { imageId: "meep_maria_onyourmind_3", alt: "50k", displayUrl: meepleImg("meep_maria_onyourmind_3") },
            { imageId: "meep_maria_onyourmind_4", alt: "50k", displayUrl: meepleImg("meep_maria_onyourmind_4") },
          ],
          reactions: [{ emoji: "❤️", count: 3 }, { emoji: "😮", count: 1 }],
          comments: [
            {
              author: "Maria",
              body:
                "honestly i've been feeling lately that my perception of difficulty of tasks has gotten out of hand. like if i am capable of something then that means it's actually not that difficult. so things that seem super impressive are reduced to mundane once i've managed to complete them. i dont think this has affected me negatively, but i realized that it's a bit ridiculous when i finished a 50k and was like well it's obviously possible, so anyone can do it. and then retroactively being like well i did it so it must not have been that hard (even though i wanted to drop out at mile 12 lol)",
            },
          ],
        },
      ),
      meepleAnswer(
        "r_meeple_7_bryan",
        "Quinn",
        "On May 11th, I start my new job doing cybersecurity at Boston Dynamics! I've been trying to make the most of my time off before I start the new job.",
        {
          displayNameOverride: "Bryan",
          images: [
            { imageId: "meep_bryan_onyourmind_0", alt: "time off", displayUrl: meepleImg("meep_bryan_onyourmind_0") },
            { imageId: "meep_bryan_onyourmind_1", alt: "time off", displayUrl: meepleImg("meep_bryan_onyourmind_1") },
            { imageId: "meep_bryan_onyourmind_2", alt: "time off", displayUrl: meepleImg("meep_bryan_onyourmind_2") },
            { imageId: "meep_bryan_onyourmind_3", alt: "time off", displayUrl: meepleImg("meep_bryan_onyourmind_3") },
            { imageId: "meep_bryan_onyourmind_4", alt: "time off", displayUrl: meepleImg("meep_bryan_onyourmind_4") },
          ],
          reactions: [{ emoji: "❤️", count: 2, reactedByMe: false }],
          comments: [{ author: "Meha", body: "congrats Bryan!!! so glad you have some time off to recharge before you start!" }],
        },
      ),
      meepleAnswer(
        "r_meeple_7_christian",
        "Christian",
        "I think my photos are scrambled, so description in no particular order:\n\n- some teishoku style breakfast w/ Rheya\n- some spring pics in GR, the weather is either beautiful or freakin cold again\n- a terrifying figure Rheya and I found on a walk, it looked kinda cute from behind and turned out to very much not be\n- the legendary Alaska license plate I saw on my way to work one day shout out Maria\n- the best cheesecake I've ever had that Rheya mostly made and I somewhat helped\n- Rheya's last performance of the season is today 5/3\n- photos with Wade, another company dancer, in our long coats",
        {
          images: [
            { imageId: "meep_christian_onyourmind_0", alt: "Christian's photo dump", displayUrl: meepleImg("meep_christian_onyourmind_0") },
            { imageId: "meep_christian_onyourmind_1", alt: "Christian's photo dump", displayUrl: meepleImg("meep_christian_onyourmind_1") },
            { imageId: "meep_christian_onyourmind_2", alt: "Christian's photo dump", displayUrl: meepleImg("meep_christian_onyourmind_2") },
            { imageId: "meep_christian_onyourmind_3", alt: "Christian's photo dump", displayUrl: meepleImg("meep_christian_onyourmind_3") },
            { imageId: "meep_christian_onyourmind_4", alt: "Christian's photo dump", displayUrl: meepleImg("meep_christian_onyourmind_4") },
            { imageId: "meep_christian_onyourmind_5", alt: "Christian's photo dump", displayUrl: meepleImg("meep_christian_onyourmind_5") },
            { imageId: "meep_christian_onyourmind_6", alt: "Christian's photo dump", displayUrl: meepleImg("meep_christian_onyourmind_6") },
            { imageId: "meep_christian_onyourmind_7", alt: "Christian's photo dump", displayUrl: meepleImg("meep_christian_onyourmind_7") },
            { imageId: "meep_christian_onyourmind_8", alt: "Christian's photo dump", displayUrl: meepleImg("meep_christian_onyourmind_8") },
            { imageId: "meep_christian_onyourmind_9", alt: "Christian's photo dump", displayUrl: meepleImg("meep_christian_onyourmind_9") },
          ],
          reactions: [{ emoji: "❤️", count: 3 }],
          comments: [{ author: "Christian", body: "I was so hyped! I must know, where are you in your license plate journey?" }],
        },
      ),
      meepleAnswer(
        "r_meeple_7_meha",
        "Meha",
        "For all of you on the edge of your seats wondering how my work retreat in San Diego went last month — it was actually very fun and lovely and beachy!!!!\n\nI was reminded that I love 98% of my coworkers — they're literally all so smart and funny and personable and I sometimes forget that working for an impact-oriented org in the progressive political space can mean you have a self-selecting group of kind empathetic people who more or less share your values (not always though, like actual dem campaign workers sound evil). So jumping into the ocean and taking walks on the beach with my coworker friends was nice co-relaxation time. I also gave a (comedic) presentation about the competitive collegiate raas circuit during our all-staff open mic/talent show and I think it's safe to say it was a hit.\n\nThe company is still toxic as hell and Maxtt suck and I don't think I'm growing the skills I want to be growing but I'm glad that that week in San Diego wasn't as miserable as I thought it'd be.",
        {
          images: [
            { imageId: "meep_meha_onyourmind_0", alt: "San Diego retreat", displayUrl: meepleImg("meep_meha_onyourmind_0") },
            { imageId: "meep_meha_onyourmind_1", alt: "San Diego retreat", displayUrl: meepleImg("meep_meha_onyourmind_1") },
            { imageId: "meep_meha_onyourmind_2", alt: "San Diego retreat", displayUrl: meepleImg("meep_meha_onyourmind_2") },
            { imageId: "meep_meha_onyourmind_3", alt: "San Diego retreat", displayUrl: meepleImg("meep_meha_onyourmind_3") },
            { imageId: "meep_meha_onyourmind_4", alt: "San Diego retreat", displayUrl: meepleImg("meep_meha_onyourmind_4") },
          ],
        },
      ),
      meepleAnswer(
        "r_meeple_7_rohan",
        "Rohan",
        "Mini rant bc this is on my mind today: I am studying for a final in Linear Algebra in my online masters program this weekend, and I am reinforced on my opinion that much of higher education is a scam. The class itself is the same content I took my senior year of high school — I'd love to skip the class but it is required to graduate. So now it's nearly $2k to take a class of basic math. Even more fun, every lecture is prerecorded from a different semester, entirely online and 0 professor interaction. So I am paying for a set of videos that are worse quality than the free MIT lectures or 3 Blue 1 Brown. Still I have weekly homework and 2 hour exams.\n\nNow I am traveling to Austin next week. 2 weeks ago they scheduled the final exam that day, so I email the professor and ask if I can take it another time. (Keep in mind this is an online remote exam that requires no interaction). He says \"24 hours is plenty of time to take the exam, I hope you can find time.\" Even though my whole email is that actually I am traveling that day so I don't have time. Otherwise I wouldn't have emailed. Not to mention they scheduled the final exam on the last possible day of finals, after I already planned the trip.\n\nBasically I need my professor to click a button to enable my online exam a few hours early. For this I am paying $2000. But nope, that kind of flexibility wouldn't prepare me for the real world, where clearly this professor has never stepped foot.",
        {
          comments: [
            {
              author: "Calvin",
              body: "",
              image: {
                imageId: "meep_comment_hjWvhzgK",
                alt: "meme",
                displayUrl: meepleImg("meep_comment_hjWvhzgK"),
              },
            },
          ],
        },
      ),
    ],
  },
  {
    questionId: "q_meeple_8",
    kind: "text",
    prompt: "👀 Check It Out — drop a link, video, or recommendation",
    displayOrder: 7,
    isAnonymous: true,
    askedBy: null,
    answers: [
      meepleAnswer(
        "r_meeple_8_bryan",
        "Quinn",
        "As part of my new job announcement, I'll include a fun video of the time Boston Dynamics went on America's Got Talent.\n\nhttps://www.youtube.com/watch?v=ptYDWP9uTis",
        {
          displayNameOverride: "Bryan",
          comments: [{ author: "Kaavya", body: "Omg who knew Bryan was a small business owner in 8th grade" }],
        },
      ),
      meepleAnswer(
        "r_meeple_8_christian",
        "Christian",
        "A cool website my coworker showed me this week:\n\nhttps://rotatingsandwiches.com/",
        { reactions: [{ emoji: "😂", count: 2 }] },
      ),
    ],
  },
];

const meepleNewsletter: PublishedNewsletter = {
  groupId: "g_meeple",
  groupName: "Meeple Mailbox",
  cycleId: "202605",
  status: "published",
  responseCloseAt: "2026-05-03T04:00:00Z",
  publishedAt: "2026-05-03T13:00:00Z",
  questions: meepleQuestions,
  gradientClass: "hero-meeple",
  monthLabel: "May",
  yearLabel: "2026",
  reactionTotal: 51,
};

const meepleSummaries: NewsletterSummary[] = [
  {
    cycleId: "202605",
    status: "published",
    responseOpenAt: "2026-04-26T04:00:00Z",
    responseCloseAt: "2026-05-03T04:00:00Z",
    publishedAt: "2026-05-03T13:00:00Z",
    questionCount: meepleQuestions.length,
    myDraftCount: 0,
    myPublishedCount: 6,
    gradientClass: "hero-meeple",
    monthLabel: "May",
    yearLabel: "2026",
  },
];

void meepleMember;

export const newsletters: Record<string, NewsletterDetail> = {
  "g_trail/202604": aprilNewsletter,
  "g_trail/202605": mayNewsletter,
  "g_trail/202606": juneNewsletter,
  "g_meeple/202605": meepleNewsletter,
};

export const newsletterSummariesByGroup: Record<string, NewsletterSummary[]> = {
  g_trail: newsletterSummaries,
  g_game: [],
  g_meeple: meepleSummaries,
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
  { groupId: "g_trail", cycleOpen: true, deadlineReminders: true, publication: true },
  { groupId: "g_game", cycleOpen: false, deadlineReminders: false, publication: false },
  { groupId: "g_meeple", cycleOpen: true, deadlineReminders: true, publication: true },
];

export const config: AppConfig = {
  user: me,
  memberships: [
    { groupId: "g_trail", role: "admin", groupName: "Trail Crew", gradient: trailCrewGradient },
    { groupId: "g_game", role: "member", groupName: "Game Night Gang", gradient: gameNightGradient },
    { groupId: "g_meeple", role: "member", groupName: "Meeple Mailbox", gradient: meepleMailboxGradient },
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
