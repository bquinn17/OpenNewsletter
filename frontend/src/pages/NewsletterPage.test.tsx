import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import { api, ApiError } from "../api/client";
import { AuthContext, type AuthValue } from "../auth/useAuth";
import type { components } from "../types/api";
import { NewsletterPage } from "./NewsletterPage";

type S = components["schemas"];

vi.mock("../api/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api/client")>();
  return {
    ...actual,
    api: {
      getNewsletter: vi.fn(),
      listNewsletters: vi.fn(),
      getConfig: vi.fn(),
      getGroup: vi.fn(),
    },
  };
});

vi.mock("../utils/media", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../utils/media")>();
  return { ...actual, ensureCookie: vi.fn().mockResolvedValue(undefined) };
});

function baseConfig(): S["ConfigResponse"] {
  return {
    userId: "u_quinn",
    email: "quinn@example.com",
    displayName: "Quinn",
    vapidPublicKey: "",
    groupDefaults: {},
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

function baseGroup(): S["GroupResponse"] {
  return {
    groupId: "g1",
    name: "Trail Crew",
    timezone: "UTC",
    gradient: "grape-sky",
    cycleSettings: { questionsPerCycle: 2, votesPerUserPerCycle: 3, responseWindowDays: 4 },
    notificationSettings: { offsetsHoursBeforeClose: [48, 24], onCycleOpen: true },
    memberCount: 2,
    memberSoftCap: 50,
    createdAt: "2026-01-01T00:00:00Z",
    members: [
      {
        userId: "u_quinn",
        displayName: "Quinn",
        role: "member",
        avatarColor: "teal",
        avatarUrl: null,
        joinedAt: "2026-01-01T00:00:00Z",
        editionsAnswered: 1,
      },
    ],
  };
}

const signedIn: AuthValue = {
  status: "authenticated",
  user: { sub: "sub-1", email: "quinn@example.com", name: "Quinn" },
  login: vi.fn(),
  logout: vi.fn(),
};

function render_(initialPath: string) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <AuthContext.Provider value={signedIn}>
        <MemoryRouter initialEntries={[initialPath]}>
          <Routes>
            <Route path="/g/:groupId/n/:cycleId" element={<NewsletterPage />} />
            <Route path="/g/:groupId/upcoming" element={<div>Upcoming page</div>} />
          </Routes>
        </MemoryRouter>
      </AuthContext.Provider>
    </QueryClientProvider>,
  );
}

describe("NewsletterPage", () => {
  it("redirects a voting cycle to the candidates page", async () => {
    vi.mocked(api.getConfig).mockResolvedValue(baseConfig());
    vi.mocked(api.getNewsletter).mockResolvedValue({ status: "voting", candidates: [] });

    render_("/g/g1/n/c1");

    expect(await screen.findByText("Upcoming page")).toBeInTheDocument();
  });

  it("shows open questions sorted by displayOrder with status pills and a respond link", async () => {
    vi.mocked(api.getConfig).mockResolvedValue(baseConfig());
    vi.mocked(api.getNewsletter).mockResolvedValue({
      status: "open",
      cycleId: "c1",
      responseOpenAt: "2026-06-01T00:00:00Z",
      responseCloseAt: "2026-06-05T00:00:00Z",
      questions: [
        {
          questionId: "q2",
          kind: "text",
          prompt: "Second question",
          displayOrder: 1,
          askedBy: null,
          isAnonymous: false,
          myResponse: null,
        },
        {
          questionId: "q1",
          kind: "text",
          prompt: "First question",
          displayOrder: 0,
          askedBy: null,
          isAnonymous: false,
          myResponse: {
            responseId: "r1",
            status: "published",
            body: "My answer",
            imageMediaIds: [],
            updatedAt: "2026-06-02T00:00:00Z",
            publishedAt: "2026-06-02T00:00:00Z",
          },
        },
      ],
    });

    render_("/g/g1/n/c1");

    const rows = await screen.findAllByRole("link");
    expect(rows).toHaveLength(2);
    expect(rows[0]).toHaveTextContent("First question");
    expect(rows[0]).toHaveTextContent("Published");
    expect(rows[0]).toHaveAttribute("href", "/g/g1/n/c1/respond/q1");
    expect(rows[1]).toHaveTextContent("Second question");
    expect(rows[1]).toHaveTextContent("Not started");
  });

  it("shows published answers and highlights the caller's poll vote", async () => {
    vi.mocked(api.getConfig).mockResolvedValue(baseConfig());
    vi.mocked(api.getGroup).mockResolvedValue(baseGroup());
    vi.mocked(api.listNewsletters).mockResolvedValue({
      items: [
        {
          cycleId: "c1",
          status: "published",
          responseOpenAt: "2026-06-01T00:00:00Z",
          responseCloseAt: "2026-06-05T00:00:00Z",
          publishedAt: "2026-06-05T00:00:01Z",
          questionCount: 2,
          myDraftCount: 0,
          myPublishedCount: 1,
        },
      ],
      nextCursor: null,
    });
    vi.mocked(api.getNewsletter).mockResolvedValue({
      status: "published",
      cycleId: "c1",
      responseCloseAt: "2026-06-05T00:00:00Z",
      publishedAt: "2026-06-05T00:00:01Z",
      questions: [
        {
          questionId: "q1",
          kind: "text",
          prompt: "What's new?",
          displayOrder: 0,
          askedBy: null,
          isAnonymous: false,
          answers: [
            {
              responseId: "r1",
              userId: "u_quinn",
              displayName: "Quinn",
              body: "All good here.",
              images: [],
              publishedAt: "2026-06-05T00:00:01Z",
              comments: [],
              reactionGroups: [],
            },
          ],
        },
        {
          questionId: "q2",
          kind: "poll",
          prompt: "Best moment?",
          displayOrder: 1,
          askedBy: null,
          isAnonymous: true,
          options: [
            { optionId: "a", label: "Option A", voteCount: 3 },
            { optionId: "b", label: "Option B", voteCount: 1 },
          ],
          myVoteOptionId: "a",
        },
      ],
    });

    render_("/g/g1/n/c1");

    expect(await screen.findByText("All good here.")).toBeInTheDocument();
    expect(screen.getByText("Your vote")).toBeInTheDocument();
  });

  it("shows a friendly archived message on a 410", async () => {
    vi.mocked(api.getConfig).mockResolvedValue(baseConfig());
    vi.mocked(api.getNewsletter).mockRejectedValue(new ApiError(410, "ARCHIVED", null));

    render_("/g/g1/n/c1");

    expect(await screen.findByText("This edition has been archived.")).toBeInTheDocument();
  });
});
