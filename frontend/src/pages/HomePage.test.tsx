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
  return { ...actual, api: { getConfig: vi.fn(), listNewsletters: vi.fn() } };
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
    memberships,
  };
}

const MEMBER: S["MembershipSummary"] = {
  groupId: "g_trail",
  role: "member",
  groupName: "Trail Crew",
  timezone: "UTC",
  gradient: "grape-sky",
};

function renderHome(auth: AuthValue = signedIn) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <AuthContext.Provider value={auth}>
        <MemoryRouter>
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
      screen.getByText("You haven't joined a group yet — paste your invite code below."),
    ).toBeInTheDocument();
  });
});
