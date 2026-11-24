import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AuthContext, type AuthValue } from "../auth/useAuth";
import { api } from "../api/client";
import type { components } from "../types/api";
import { HomePage } from "./HomePage";

type S = components["schemas"];

vi.mock("../api/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api/client")>();
  return {
    ...actual,
    api: {
      getConfig: vi.fn(),
      listNewsletters: vi.fn(),
      getGroup: vi.fn(),
      getLeaderboard: vi.fn(),
    },
  };
});

const signedIn: AuthValue = {
  status: "authenticated",
  user: { sub: "sub-1", email: "sam@example.com", name: "Sam" },
  login: vi.fn(),
  logout: vi.fn(),
};

function config(memberships: S["MembershipSummary"][]): S["ConfigResponse"] {
  return {
    userId: "u_sam",
    email: "sam@example.com",
    displayName: "Sam",
    vapidPublicKey: "",
    groupDefaults: {},
    cycleCadence: { frequency: "monthly", maxResponseWindowDays: 28 },
    memberships,
  };
}

function member(overrides: Partial<S["MemberResponse"]>): S["MemberResponse"] {
  return {
    userId: "u_sam",
    displayName: "Sam",
    role: "member",
    avatarColor: "teal",
    avatarUrl: null,
    joinedAt: "2026-01-01T00:00:00Z",
    editionsAnswered: 0,
    ...overrides,
  };
}

function group(members: S["MemberResponse"][]): S["GroupResponse"] {
  return {
    groupId: "g_trail",
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

const MEMBER: S["MembershipSummary"] = {
  groupId: "g_trail",
  role: "member",
  groupName: "Trail Crew",
  timezone: "UTC",
  gradient: "grape-sky",
};

function renderHome(auth: AuthValue = signedIn, path = "/") {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <AuthContext.Provider value={auth}>
        <MemoryRouter initialEntries={[path]}>
          <HomePage />
        </MemoryRouter>
      </AuthContext.Provider>
    </QueryClientProvider>,
  );
}

afterEach(() => vi.clearAllMocks());

describe("HomePage", () => {
  it("offers the demo to a signed-out visitor as a full page load of /demo", () => {
    renderHome({ ...signedIn, status: "unauthenticated", user: null });

    expect(screen.getByRole("link", { name: "See a demo" })).toHaveAttribute("href", "/demo");
  });

  it("offers a signed-out visitor both signing up and signing in", () => {
    renderHome({ ...signedIn, status: "unauthenticated", user: null });

    expect(screen.getByRole("link", { name: "Sign up" })).toHaveAttribute("href", "/join");
    expect(screen.getByRole("link", { name: "Sign in" })).toHaveAttribute("href", "/signin");
  });

  it("shows a spinner, not the join form, while /config is loading", () => {
    vi.mocked(api.getConfig).mockReturnValue(new Promise(() => {}));

    renderHome();

    expect(screen.getByRole("status", { name: "Loading" })).toBeInTheDocument();
    expect(screen.queryByLabelText("Invite code")).not.toBeInTheDocument();
  });

  it("offers a retry, not the join form, when /config fails", async () => {
    const user = userEvent.setup();
    vi.mocked(api.getConfig)
      .mockRejectedValueOnce(new TypeError("Failed to fetch"))
      .mockResolvedValueOnce(config([MEMBER]));
    vi.mocked(api.listNewsletters).mockResolvedValue({ items: [], nextCursor: null });

    renderHome();

    expect(await screen.findByText("Couldn't load your groups.")).toBeInTheDocument();
    expect(screen.queryByLabelText("Invite code")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Try again" }));

    expect(await screen.findByText("No editions yet.")).toBeInTheDocument();
    expect(screen.queryByLabelText("Invite code")).not.toBeInTheDocument();
  });

  it("shows the join form once /config has loaded with no memberships", async () => {
    vi.mocked(api.getConfig).mockResolvedValue(config([]));

    renderHome();

    expect(await screen.findByLabelText("Invite code")).toBeInTheDocument();
    expect(
      screen.getByText(/You haven't joined a group yet\. Groups are invite-only/),
    ).toBeInTheDocument();
  });

  describe("group tabs", () => {
    it("opens on Editions, and the Members tab lists the group with no admin actions", async () => {
      const user = userEvent.setup();
      vi.mocked(api.getConfig).mockResolvedValue(config([MEMBER]));
      vi.mocked(api.listNewsletters).mockResolvedValue({ items: [], nextCursor: null });
      vi.mocked(api.getGroup).mockResolvedValue(
        group([
          member({ userId: "u_sam", displayName: "Sam", editionsAnswered: 3 }),
          member({ userId: "u_ava", displayName: "Ava", role: "admin" }),
        ]),
      );

      renderHome();

      expect(await screen.findByText("No editions yet.")).toBeInTheDocument();
      expect(screen.getByRole("tab", { name: "Editions" })).toHaveAttribute(
        "aria-selected",
        "true",
      );

      await user.click(screen.getByRole("tab", { name: "Members" }));

      expect(await screen.findByText("2 members")).toBeInTheDocument();
      expect(screen.getByText("(you)")).toBeInTheDocument();
      expect(screen.getByText("3 editions answered")).toBeInTheDocument();
      expect(screen.getByText("admin")).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: /Options for/ })).not.toBeInTheDocument();
      expect(screen.queryByText("No editions yet.")).not.toBeInTheDocument();
    });

    it("opens on the tab named in ?tab=", async () => {
      vi.mocked(api.getConfig).mockResolvedValue(config([MEMBER]));
      vi.mocked(api.listNewsletters).mockResolvedValue({ items: [], nextCursor: null });
      vi.mocked(api.getGroup).mockResolvedValue(group([member({})]));
      vi.mocked(api.getLeaderboard).mockResolvedValue({ members: [], topAnswers: [] });

      renderHome(signedIn, "/?tab=leaderboard");

      expect(
        await screen.findByText("Superlatives show up once your group's first edition is out."),
      ).toBeInTheDocument();
      expect(screen.getByRole("tab", { name: "Leaderboard" })).toHaveAttribute(
        "aria-selected",
        "true",
      );
    });
  });
});
