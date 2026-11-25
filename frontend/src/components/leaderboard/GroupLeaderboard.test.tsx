import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api, ApiError } from "../../api/client";
import { AuthContext, type AuthValue } from "../../auth/useAuth";
import type { components } from "../../types/api";
import { GroupLeaderboard } from "./GroupLeaderboard";

type S = components["schemas"];

vi.mock("../../api/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../api/client")>();
  return {
    ...actual,
    api: {
      getConfig: vi.fn(),
      getGroup: vi.fn(),
      getLeaderboard: vi.fn(),
    },
  };
});

function baseConfig(): S["ConfigResponse"] {
  return {
    userId: "u_quinn",
    email: "quinn@example.com",
    displayName: "Quinn",
    vapidPublicKey: "",
    groupDefaults: {},
    cycleCadence: { frequency: "monthly", maxResponseWindowDays: 28 },
    memberships: [
      {
        groupId: "g1",
        role: "member",
        groupName: "Trail Crew",
        timezone: "UTC",
        gradient: "grape-sky",
      },
    ],
  };
}

function member(overrides: Partial<S["MemberResponse"]>): S["MemberResponse"] {
  return {
    userId: "u_quinn",
    displayName: "Quinn",
    role: "member",
    avatarColor: "teal",
    avatarUrl: null,
    joinedAt: "2026-01-01T00:00:00Z",
    editionsAnswered: 0,
    ...overrides,
  };
}

function baseGroup(members: S["MemberResponse"][]): S["GroupResponse"] {
  return {
    groupId: "g1",
    name: "Trail Crew",
    timezone: "UTC",
    gradient: "grape-sky",
    cycleSettings: { questionsPerCycle: 2, votesPerUserPerCycle: 3, responseWindowDays: 4 },
    notificationSettings: { offsetsHoursBeforeClose: [48, 24], onCycleOpen: true },
    memberCount: members.length,
    memberSoftCap: 50,
    createdAt: "2026-01-01T00:00:00Z",
    members,
  };
}

function emptyStats(userId: string): S["MemberStatsResponse"] {
  return {
    userId,
    editionsAnswered: 0,
    reactionsReceived: 0,
    reactionsGiven: 0,
    commentsWritten: 0,
    questionsAsked: 0,
    currentStreak: 0,
    longestStreak: 0,
  };
}

const signedIn: AuthValue = {
  status: "authenticated",
  user: { sub: "sub-1", email: "quinn@example.com", name: "Quinn" },
  login: vi.fn(),
  logout: vi.fn(),
};

function renderPage() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <AuthContext.Provider value={signedIn}>
        <MemoryRouter>
          <GroupLeaderboard groupId="g1" />
        </MemoryRouter>
      </AuthContext.Provider>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  vi.mocked(api.getConfig).mockResolvedValue(baseConfig());
});

describe("GroupLeaderboard", () => {
  it("shows a loading skeleton before data arrives", async () => {
    vi.mocked(api.getConfig).mockResolvedValue(baseConfig());
    vi.mocked(api.getGroup).mockReturnValue(new Promise(() => undefined));
    vi.mocked(api.getLeaderboard).mockReturnValue(new Promise(() => undefined));

    renderPage();

    expect(await screen.findByLabelText("Loading the leaderboard")).toBeInTheDocument();
  });

  it("shows an error state with a retry button", async () => {
    vi.mocked(api.getConfig).mockResolvedValue(baseConfig());
    vi.mocked(api.getGroup).mockResolvedValue(baseGroup([member({})]));
    vi.mocked(api.getLeaderboard).mockRejectedValue(new ApiError(500, "UNKNOWN", null));

    renderPage();

    expect(await screen.findByRole("button", { name: "Try again" })).toBeInTheDocument();
  });

  it("renders boards for a populated leaderboard, joined with member display info", async () => {
    vi.mocked(api.getConfig).mockResolvedValue(baseConfig());
    vi.mocked(api.getGroup).mockResolvedValue(
      baseGroup([
        member({ userId: "u_quinn", displayName: "Quinn" }),
        member({ userId: "u_sam", displayName: "Sam" }),
      ]),
    );
    vi.mocked(api.getLeaderboard).mockResolvedValue({
      members: [
        { ...emptyStats("u_quinn"), reactionsReceived: 5, commentsWritten: 2 },
        { ...emptyStats("u_sam"), reactionsReceived: 2, longestStreak: 4, currentStreak: 2 },
      ],
      topAnswers: [
        {
          responseId: "r1",
          userId: "u_sam",
          cycleId: "202609",
          questionId: "q1",
          questionPrompt: "What made you laugh?",
          reactionCount: 6,
        },
      ],
    });

    renderPage();

    expect(await screen.findByText("Most-reacted answer")).toBeInTheDocument();
    expect(screen.getByText("What made you laugh?")).toBeInTheDocument();
    expect(screen.getByText("Most reactions received")).toBeInTheDocument();
    expect(screen.getByText("Longest streak")).toBeInTheDocument();
    expect(screen.getByText("4 in a row")).toBeInTheDocument();
    expect(screen.getByText("current: 2")).toBeInTheDocument();
    // Nobody wrote a comment above zero except Quinn, nobody asked a question.
    expect(screen.queryByText("Most questions asked")).not.toBeInTheDocument();
  });

  it("shows the friendly empty state when every board is empty", async () => {
    vi.mocked(api.getConfig).mockResolvedValue(baseConfig());
    vi.mocked(api.getGroup).mockResolvedValue(baseGroup([member({})]));
    vi.mocked(api.getLeaderboard).mockResolvedValue({
      members: [emptyStats("u_quinn")],
      topAnswers: [],
    });

    renderPage();

    expect(
      await screen.findByText("Superlatives show up once your group's first edition is out."),
    ).toBeInTheDocument();
  });
});
