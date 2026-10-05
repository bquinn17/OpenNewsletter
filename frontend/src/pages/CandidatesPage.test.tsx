import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AuthContext, type AuthValue } from "../auth/useAuth";
import { api, ApiError } from "../api/client";
import { useToasts } from "../state/toast";
import type { components } from "../types/api";
import { CandidatesPage } from "./CandidatesPage";

type S = components["schemas"];

vi.mock("../api/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api/client")>();
  return {
    ...actual,
    api: {
      getConfig: vi.fn(),
      listNewsletters: vi.fn(),
      listCandidates: vi.fn(),
      castVote: vi.fn(),
      withdrawVote: vi.fn(),
    },
  };
});

const signedIn: AuthValue = {
  status: "authenticated",
  user: { sub: "sub-1", email: "sam@example.com", name: "Sam" },
  login: vi.fn(),
  logout: vi.fn(),
};

function baseConfig(): S["ConfigResponse"] {
  return {
    userId: "u_quinn",
    email: "quinn@example.com",
    displayName: "Quinn",
    vapidPublicKey: "",
    groupDefaults: {},
    memberships: [
      {
        groupId: "g_test",
        role: "member",
        groupName: "Test Group",
        timezone: "UTC",
        gradient: "grape-sky",
      },
    ],
  };
}

function newsletters(items: S["NewsletterSummary"][]): S["NewsletterListResponse"] {
  return { items, nextCursor: null };
}

function askedBy(displayName: string): S["AskedBy"] {
  return {
    userId: `u_${displayName.toLowerCase()}`,
    displayName,
    avatarColor: "teal",
    avatarUrl: null,
  };
}

function item(overrides: Partial<S["CandidateItemResponse"]>): S["CandidateItemResponse"] {
  return {
    questionId: "cq_1",
    kind: "text",
    prompt: "What's the best trail you hiked?",
    askedBy: askedBy("Sam"),
    isAnonymous: false,
    voteCount: 2,
    votedByMe: false,
    submittedAt: "2026-09-20T00:00:00Z",
    ...overrides,
  };
}

function candidateList(
  items: S["CandidateItemResponse"][],
  overrides: Partial<S["CandidateListResponse"]> = {},
): S["CandidateListResponse"] {
  return {
    nextCycleId: "202610",
    votesPerUserPerCycle: 3,
    myVoteCount: items.filter((i) => i.votedByMe).length,
    items,
    nextCursor: null,
    ...overrides,
  };
}

function renderPage() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <AuthContext.Provider value={signedIn}>
        <MemoryRouter initialEntries={["/g/g_test/upcoming"]}>
          <Routes>
            <Route path="/g/:groupId/upcoming" element={<CandidatesPage />} />
          </Routes>
        </MemoryRouter>
      </AuthContext.Provider>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  vi.mocked(api.getConfig).mockResolvedValue(baseConfig());
});

afterEach(() => {
  vi.clearAllMocks();
  useToasts.setState({ toasts: [] });
});

describe("CandidatesPage", () => {
  it("renders the candidate pool and casts a vote on upvote", async () => {
    const user = userEvent.setup();
    vi.mocked(api.listNewsletters).mockResolvedValue(
      newsletters([
        {
          cycleId: "202610",
          status: "voting",
          responseOpenAt: new Date(Date.now() + 5 * 24 * 60 * 60 * 1000).toISOString(),
          responseCloseAt: new Date(Date.now() + 5 * 24 * 60 * 60 * 1000).toISOString(),
          publishedAt: null,
          questionCount: 0,
          myDraftCount: 0,
          myPublishedCount: 0,
        },
      ]),
    );
    const list = candidateList([item({ questionId: "cq_1", votedByMe: false })]);
    vi.mocked(api.listCandidates).mockResolvedValue(list);
    vi.mocked(api.castVote).mockResolvedValue({
      questionId: "cq_1",
      voteCount: 3,
      votedByMe: true,
      myVoteCount: 1,
    });

    renderPage();

    expect(await screen.findByText("What's the best trail you hiked?")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /Upvote/ }));

    await waitFor(() => expect(api.castVote).toHaveBeenCalledWith("g_test", "cq_1"));
  });

  it("disables un-voted buttons at the cap and lets the caller remove a vote from the panel", async () => {
    const user = userEvent.setup();
    vi.mocked(api.listNewsletters).mockResolvedValue(
      newsletters([
        {
          cycleId: "202610",
          status: "voting",
          responseOpenAt: new Date(Date.now() + 5 * 24 * 60 * 60 * 1000).toISOString(),
          responseCloseAt: new Date(Date.now() + 5 * 24 * 60 * 60 * 1000).toISOString(),
          publishedAt: null,
          questionCount: 0,
          myDraftCount: 0,
          myPublishedCount: 0,
        },
      ]),
    );
    const votedItems = [
      item({ questionId: "cq_1", votedByMe: true, prompt: "Voted one" }),
      item({ questionId: "cq_2", votedByMe: true, prompt: "Voted two" }),
      item({ questionId: "cq_3", votedByMe: true, prompt: "Voted three" }),
    ];
    const unvoted = item({ questionId: "cq_4", votedByMe: false, prompt: "Not voted" });
    vi.mocked(api.listCandidates).mockResolvedValue(
      candidateList([...votedItems, unvoted], { myVoteCount: 3, votesPerUserPerCycle: 3 }),
    );
    vi.mocked(api.withdrawVote).mockResolvedValue({
      questionId: "cq_1",
      voteCount: 1,
      votedByMe: false,
      myVoteCount: 2,
    });

    renderPage();

    await screen.findByText("Not voted");
    const unvotedButton = screen.getByRole("button", { name: /Upvote "Not voted"/ });
    expect(unvotedButton).toBeDisabled();

    await user.click(screen.getByRole("button", { name: "Manage votes" }));
    const removeButtons = screen.getAllByRole("button", { name: "Remove vote" });
    await user.click(removeButtons[0]!);

    await waitFor(() => expect(api.withdrawVote).toHaveBeenCalledWith("g_test", "cq_1"));
  });

  it("switches sort order between top and recent", async () => {
    const user = userEvent.setup();
    vi.mocked(api.listNewsletters).mockResolvedValue(newsletters([]));
    vi.mocked(api.listCandidates).mockResolvedValue(candidateList([item({})]));

    renderPage();
    await screen.findByText("What's the best trail you hiked?");

    await user.click(screen.getByRole("button", { name: "Recent" }));

    await waitFor(() =>
      expect(api.listCandidates).toHaveBeenCalledWith("g_test", { sort: "recent" }),
    );
  });

  it("shows a calm state when voting hasn't opened yet, instead of an error", async () => {
    vi.mocked(api.listNewsletters).mockResolvedValue(newsletters([]));
    vi.mocked(api.listCandidates).mockRejectedValue(
      new ApiError(409, "CYCLE_NOT_VOTING", {
        type: "https://api.opennewsletter.example.com/errors/cycle-not-voting",
        title: "Conflict",
        status: 409,
        detail: "this group has no cycle currently accepting candidate questions",
        code: "CYCLE_NOT_VOTING",
        correlationId: "corr-1",
      }),
    );

    renderPage();

    expect(await screen.findByText(/hasn.t opened yet/)).toBeInTheDocument();
    expect(screen.queryByText(/Couldn't load/)).not.toBeInTheDocument();
  });

  it("shows a friendly message (not the raw id list) and refetches on VOTE_CAP_REACHED", async () => {
    const user = userEvent.setup();
    vi.mocked(api.listNewsletters).mockResolvedValue(newsletters([]));
    const list = candidateList([
      item({ questionId: "cq_1", votedByMe: false }),
      item({ questionId: "cq_2", votedByMe: true, prompt: "Already voted" }),
    ]);
    vi.mocked(api.listCandidates).mockResolvedValue(list);
    vi.mocked(api.castVote).mockRejectedValue(
      new ApiError(409, "VOTE_CAP_REACHED", {
        type: "https://api.opennewsletter.example.com/errors/vote-cap-reached",
        title: "Conflict",
        status: 409,
        detail: "no votes remaining this cycle; already voted for: cq_2",
        code: "VOTE_CAP_REACHED",
        correlationId: "corr-2",
      }),
    );

    renderPage();
    await screen.findByText("What's the best trail you hiked?");

    await user.click(screen.getByRole("button", { name: /Upvote/ }));

    await waitFor(() => expect(api.castVote).toHaveBeenCalled());
    await waitFor(() => expect(api.listCandidates).toHaveBeenCalledTimes(2));

    const toasts = useToasts.getState().toasts;
    expect(toasts.some((t) => t.message.includes("votes this cycle"))).toBe(true);
    expect(toasts.some((t) => t.message.includes("cq_2"))).toBe(false);
  });
});
