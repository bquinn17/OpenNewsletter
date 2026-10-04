import type { components } from "../types/api";
import { ApiError, type ProblemDetails } from "./client";

type S = components["schemas"];
type Method = "GET" | "POST" | "PUT" | "PATCH" | "DELETE";

const LATENCY_MS = 250;
const CALLER_ID = "u_quinn";
const NEW_GROUP_ID = "g_newgroup";
const DEMO_INVITE_CODE = "DEMO-JOIN-CODE";

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

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

function fail(status: number, code: string, detail: string): never {
  throw new ApiError(status, code, problemFor(status, code, detail));
}

let me: S["UserResponse"] = {
  userId: CALLER_ID,
  email: "quinn@example.com",
  displayName: "Quinn",
  avatarColor: "teal",
  avatarMediaId: null,
  avatarUrl: null,
  createdAt: "2025-01-12T00:00:00Z",
};

let memberships: S["MembershipSummary"][] = [
  {
    groupId: "g_trail",
    role: "admin",
    groupName: "Trail Crew",
    timezone: "America/New_York",
    gradient: "grape-sky",
  },
  {
    groupId: "g_game",
    role: "member",
    groupName: "Game Night Gang",
    timezone: "America/New_York",
    gradient: "ocean-dusk",
  },
  {
    groupId: "g_meeple",
    role: "member",
    groupName: "Meeple Mailbox",
    timezone: "America/New_York",
    gradient: "forest-mint",
  },
];

const groups: Record<string, S["GroupResponse"]> = {
  g_trail: {
    groupId: "g_trail",
    name: "Trail Crew",
    timezone: "America/New_York",
    gradient: "grape-sky",
    cycleSettings: { questionsPerCycle: 5, votesPerUserPerCycle: 3, responseWindowDays: 4 },
    notificationSettings: { offsetsHoursBeforeClose: [96, 48, 24], onCycleOpen: true },
    memberCount: 2,
    memberSoftCap: 50,
    createdAt: "2025-01-12T00:00:00Z",
    members: [
      {
        userId: CALLER_ID,
        displayName: "Quinn",
        role: "admin",
        avatarColor: "teal",
        avatarUrl: null,
        joinedAt: "2025-01-12T00:00:00Z",
        editionsAnswered: 5,
      },
      {
        userId: "u_sam",
        displayName: "Sam",
        role: "admin",
        avatarColor: "red",
        avatarUrl: null,
        joinedAt: "2025-01-12T00:00:00Z",
        editionsAnswered: 5,
      },
    ],
  },
  g_game: {
    groupId: "g_game",
    name: "Game Night Gang",
    timezone: "America/New_York",
    gradient: "ocean-dusk",
    cycleSettings: { questionsPerCycle: 4, votesPerUserPerCycle: 3, responseWindowDays: 4 },
    notificationSettings: { offsetsHoursBeforeClose: [48, 24], onCycleOpen: true },
    memberCount: 1,
    memberSoftCap: 50,
    createdAt: "2025-08-01T00:00:00Z",
    members: [
      {
        userId: CALLER_ID,
        displayName: "Quinn",
        role: "member",
        avatarColor: "teal",
        avatarUrl: null,
        joinedAt: "2025-08-01T00:00:00Z",
        editionsAnswered: 2,
      },
    ],
  },
  g_meeple: {
    groupId: "g_meeple",
    name: "Meeple Mailbox",
    timezone: "America/New_York",
    gradient: "forest-mint",
    cycleSettings: { questionsPerCycle: 8, votesPerUserPerCycle: 4, responseWindowDays: 7 },
    notificationSettings: { offsetsHoursBeforeClose: [72, 24], onCycleOpen: true },
    memberCount: 2,
    memberSoftCap: 50,
    createdAt: "2024-08-01T00:00:00Z",
    members: [
      {
        userId: CALLER_ID,
        displayName: "Quinn",
        role: "member",
        avatarColor: "teal",
        avatarUrl: null,
        joinedAt: "2024-08-01T00:00:00Z",
        editionsAnswered: 9,
      },
      {
        userId: "u_m_tara",
        displayName: "Tara",
        role: "admin",
        avatarColor: "orange",
        avatarUrl: null,
        joinedAt: "2024-08-01T00:00:00Z",
        editionsAnswered: 9,
      },
    ],
  },
};

const newsletters: Record<string, S["NewsletterSummary"][]> = {
  g_trail: [
    {
      cycleId: "202610",
      status: "voting",
      responseOpenAt: "2026-10-01T04:00:00Z",
      responseCloseAt: "2026-10-01T04:00:00Z",
      publishedAt: null,
      questionCount: 0,
      myDraftCount: 0,
      myPublishedCount: 0,
    },
    {
      cycleId: "202609",
      status: "open",
      responseOpenAt: "2026-09-01T04:00:00Z",
      responseCloseAt: "2026-09-05T04:00:00Z",
      publishedAt: null,
      questionCount: 5,
      myDraftCount: 1,
      myPublishedCount: 1,
    },
    {
      cycleId: "202608",
      status: "published",
      responseOpenAt: "2026-08-01T04:00:00Z",
      responseCloseAt: "2026-08-05T04:00:00Z",
      publishedAt: "2026-08-05T04:00:01Z",
      questionCount: 5,
      myDraftCount: 0,
      myPublishedCount: 5,
    },
    {
      cycleId: "202607",
      status: "published",
      responseOpenAt: "2026-07-01T04:00:00Z",
      responseCloseAt: "2026-07-05T04:00:00Z",
      publishedAt: "2026-07-05T04:00:01Z",
      questionCount: 5,
      myDraftCount: 0,
      myPublishedCount: 5,
    },
  ],
  g_game: [
    {
      cycleId: "202610",
      status: "voting",
      responseOpenAt: "2026-10-01T04:00:00Z",
      responseCloseAt: "2026-10-01T04:00:00Z",
      publishedAt: null,
      questionCount: 0,
      myDraftCount: 0,
      myPublishedCount: 0,
    },
    {
      cycleId: "202609",
      status: "published",
      responseOpenAt: "2026-09-01T04:00:00Z",
      responseCloseAt: "2026-09-05T04:00:00Z",
      publishedAt: "2026-09-05T04:00:01Z",
      questionCount: 4,
      myDraftCount: 0,
      myPublishedCount: 4,
    },
  ],
  g_meeple: [
    {
      cycleId: "202609",
      status: "open",
      responseOpenAt: "2026-09-03T04:00:00Z",
      responseCloseAt: "2026-09-10T04:00:00Z",
      publishedAt: null,
      questionCount: 8,
      myDraftCount: 2,
      myPublishedCount: 0,
    },
    {
      cycleId: "202608",
      status: "published",
      responseOpenAt: "2026-08-01T04:00:00Z",
      responseCloseAt: "2026-08-08T04:00:00Z",
      publishedAt: "2026-08-08T04:00:01Z",
      questionCount: 8,
      myDraftCount: 0,
      myPublishedCount: 8,
    },
  ],
};

function handleGetConfig(): S["ConfigResponse"] {
  return {
    userId: me.userId,
    email: me.email,
    displayName: me.displayName,
    vapidPublicKey: "BPV_mock_vapid_public_key",
    groupDefaults: {},
    memberships: [...memberships],
  };
}

function handlePatchMe(body: unknown): S["UserResponse"] {
  // Trusted internal shape — the only caller is our own typed `api.patchMe`.
  const patch = (body ?? {}) as S["PatchMeRequest"];
  const next = { ...me };
  if (patch.displayName !== undefined) next.displayName = patch.displayName;
  if (patch.avatarColor !== undefined) next.avatarColor = patch.avatarColor;
  if ("avatarMediaId" in patch) next.avatarMediaId = patch.avatarMediaId ?? null;
  me = next;
  return { ...me };
}

function handleGetGroup(groupId: string): S["GroupResponse"] {
  const group = groups[groupId];
  if (!group) fail(404, "NOT_FOUND", `group ${groupId} not found`);
  return { ...group, members: [...group.members] };
}

function handleRemoveMember(groupId: string, userId: string): undefined {
  const group = groups[groupId];
  if (!group) fail(404, "NOT_FOUND", `group ${groupId} not found`);
  // Simplified fixture rule: g_trail is always "the caller's only-admin group".
  if (groupId === "g_trail") fail(409, "LAST_ADMIN", "the last admin can't leave their group");

  const memberIndex = group.members.findIndex((m) => m.userId === userId);
  if (memberIndex === -1) fail(404, "NOT_FOUND", `member ${userId} not found in group ${groupId}`);

  group.members.splice(memberIndex, 1);
  group.memberCount = group.members.length;
  if (userId === CALLER_ID) memberships = memberships.filter((m) => m.groupId !== groupId);
  return undefined;
}

function handleListNewsletters(
  groupId: string,
  query: URLSearchParams,
): S["NewsletterListResponse"] {
  const all = newsletters[groupId];
  if (!all) fail(403, "FORBIDDEN", `not a member of ${groupId}`);

  const status = query.get("status");
  const limit = query.get("limit");
  let items = status ? all.filter((n) => n.status === status) : all;
  if (limit) items = items.slice(0, Number(limit));
  return { items: items.map((n) => ({ ...n })), nextCursor: null };
}

function handleRedeemInvite(body: unknown): S["RedeemResponse"] {
  // Trusted internal shape — the only caller is our own typed `api.redeemInvite`.
  const request = body as S["RedeemRequest"] | undefined;
  if (request?.code !== DEMO_INVITE_CODE) fail(400, "INVITE_INVALID", "invite code is invalid");

  if (!memberships.some((m) => m.groupId === NEW_GROUP_ID)) {
    memberships = [
      ...memberships,
      {
        groupId: NEW_GROUP_ID,
        role: "member",
        groupName: "New Group",
        timezone: "America/New_York",
        gradient: "citrus-blush",
      },
    ];
    groups[NEW_GROUP_ID] = {
      groupId: NEW_GROUP_ID,
      name: "New Group",
      timezone: "America/New_York",
      gradient: "citrus-blush",
      cycleSettings: { questionsPerCycle: 5, votesPerUserPerCycle: 3, responseWindowDays: 4 },
      notificationSettings: { offsetsHoursBeforeClose: [48, 24], onCycleOpen: true },
      memberCount: 1,
      memberSoftCap: 50,
      createdAt: "2026-09-27T00:00:00Z",
      members: [
        {
          userId: CALLER_ID,
          displayName: me.displayName,
          role: "member",
          avatarColor: me.avatarColor,
          avatarUrl: me.avatarUrl,
          joinedAt: "2026-09-27T00:00:00Z",
          editionsAnswered: 0,
        },
      ],
    };
    newsletters[NEW_GROUP_ID] = [
      {
        cycleId: "202610",
        status: "voting",
        responseOpenAt: "2026-10-01T04:00:00Z",
        responseCloseAt: "2026-10-01T04:00:00Z",
        publishedAt: null,
        questionCount: 0,
        myDraftCount: 0,
        myPublishedCount: 0,
      },
    ];
  }
  return { groupId: NEW_GROUP_ID, groupName: "New Group", role: "member" };
}

/** Routes `apiFetch` calls to in-memory fixtures instead of the network (`env.useMocks`). */
export async function mockFetch(method: Method, path: string, body?: unknown): Promise<unknown> {
  await sleep(LATENCY_MS);

  const [rawPath, queryString] = path.split("?");
  const segments = rawPath.split("/").filter(Boolean);
  const query = new URLSearchParams(queryString ?? "");

  if (method === "GET" && rawPath === "/config") return handleGetConfig();
  if (method === "GET" && rawPath === "/me") return { ...me };
  if (method === "PATCH" && rawPath === "/me") return handlePatchMe(body);
  if (method === "GET" && rawPath === "/groups") return { memberships: [...memberships] };
  if (method === "GET" && segments.length === 2 && segments[0] === "groups") {
    return handleGetGroup(decodeURIComponent(segments[1]));
  }
  if (
    method === "DELETE" &&
    segments.length === 4 &&
    segments[0] === "groups" &&
    segments[2] === "members"
  ) {
    return handleRemoveMember(decodeURIComponent(segments[1]), decodeURIComponent(segments[3]));
  }
  if (
    method === "GET" &&
    segments.length === 3 &&
    segments[0] === "groups" &&
    segments[2] === "newsletters"
  ) {
    return handleListNewsletters(decodeURIComponent(segments[1]), query);
  }
  if (method === "POST" && rawPath === "/invites/redeem") return handleRedeemInvite(body);

  fail(404, "NOT_FOUND", `no mock route for ${method} ${rawPath}`);
}
