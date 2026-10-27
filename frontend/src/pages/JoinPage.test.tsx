import { StrictMode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AuthContext, type AuthValue } from "../auth/useAuth";
import { RequireMembership } from "../routes/RequireMembership";
import { api, ApiError } from "../api/client";
import { useCurrentGroup } from "../state/currentGroup";
import { useToasts } from "../state/toast";
import { JoinPage } from "./JoinPage";

vi.mock("../api/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api/client")>();
  return { ...actual, api: { getConfig: vi.fn(), redeemInvite: vi.fn() } };
});

const signedIn: AuthValue = {
  status: "authenticated",
  user: { sub: "sub-1", email: "sam@example.com", name: "Sam" },
  login: vi.fn(),
  logout: vi.fn(),
};

// StrictMode, as in `main.tsx`: its simulated unmount/remount is what once
// left the page spinning after a failed redeem.
function renderJoin(auth: AuthValue, queryClient = new QueryClient()) {
  return render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <AuthContext.Provider value={auth}>
          <MemoryRouter initialEntries={["/join?code=ABCD-EFGH"]}>
            <Routes>
              <Route path="/" element={<div>Home page</div>} />
              <Route path="/join" element={<JoinPage />} />
              <Route path="/g/:groupId" element={<RequireMembership />}>
                <Route path="upcoming" element={<div>Upcoming for new group</div>} />
              </Route>
            </Routes>
          </MemoryRouter>
        </AuthContext.Provider>
      </QueryClientProvider>
    </StrictMode>,
  );
}

const tzGradient = { timezone: "UTC", gradient: "grape-sky" } as const;

afterEach(() => vi.clearAllMocks());

describe("JoinPage", () => {
  it("lands in the new group even when /config was cached before redeeming", async () => {
    const queryClient = new QueryClient();
    // A cached config without the new group, as a signed-in user joining a
    // second group would have. Navigating before the refetch lands would
    // make RequireMembership bounce to "/".
    const before = {
      userId: "u1",
      email: "sam@example.com",
      displayName: "Sam",
      vapidPublicKey: "",
      groupDefaults: {},
      memberships: [{ groupId: "g_old", role: "member" as const, groupName: "Old", ...tzGradient }],
    };
    vi.mocked(api.getConfig).mockResolvedValueOnce(before);
    await queryClient.prefetchQuery({ queryKey: ["config"], queryFn: api.getConfig });
    vi.mocked(api.redeemInvite).mockResolvedValue({
      groupId: "g_new",
      groupName: "New",
      role: "member",
    });
    vi.mocked(api.getConfig).mockResolvedValue({
      userId: "u1",
      email: "sam@example.com",
      displayName: "Sam",
      vapidPublicKey: "",
      groupDefaults: {},
      memberships: [
        { groupId: "g_old", role: "member", groupName: "Old", ...tzGradient },
        { groupId: "g_new", role: "member", groupName: "New", ...tzGradient },
      ],
    });

    renderJoin(signedIn, queryClient);

    expect(await screen.findByText("Upcoming for new group")).toBeInTheDocument();
    expect(api.redeemInvite).toHaveBeenCalledTimes(1);
    expect(api.redeemInvite).toHaveBeenCalledWith("ABCD-EFGH");
  });

  it("explains an expired invite", async () => {
    vi.mocked(api.redeemInvite).mockRejectedValue(new ApiError(410, "INVITE_EXPIRED", null));

    renderJoin(signedIn);

    expect(await screen.findByText(/That invite has expired/)).toBeInTheDocument();
  });

  it("explains an already-used invite", async () => {
    vi.mocked(api.redeemInvite).mockRejectedValue(new ApiError(409, "INVITE_CONSUMED", null));

    renderJoin(signedIn);

    expect(await screen.findByText("That invite has already been used.")).toBeInTheDocument();
    expect(api.redeemInvite).toHaveBeenCalledTimes(1);
  });

  it("explains an invite bound to a different email (D5)", async () => {
    vi.mocked(api.redeemInvite).mockRejectedValue(new ApiError(403, "INVITE_EMAIL_MISMATCH", null));
    renderJoin(signedIn);

    expect(
      await screen.findByText(
        "This invite is for a different email address. Sign in with Google using the invited email.",
      ),
    ).toBeInTheDocument();
  });

  it("retries the same code when the form is submitted again after an error", async () => {
    const user = userEvent.setup();
    vi.mocked(api.redeemInvite)
      .mockRejectedValueOnce(new ApiError(409, "MEMBER_CAP_REACHED", null))
      .mockResolvedValueOnce({ groupId: "g_new", groupName: "New", role: "member" });
    vi.mocked(api.getConfig).mockResolvedValue({
      userId: "u1",
      email: "sam@example.com",
      displayName: "Sam",
      vapidPublicKey: "",
      groupDefaults: {},
      memberships: [{ groupId: "g_new", role: "member", groupName: "New", ...tzGradient }],
    });

    renderJoin(signedIn);
    expect(await screen.findByText("That group is full.")).toBeInTheDocument();

    await user.type(screen.getByLabelText("Invite code"), "ABCD-EFGH");
    await user.click(screen.getByRole("button", { name: "Join" }));

    expect(await screen.findByText("Upcoming for new group")).toBeInTheDocument();
    expect(api.redeemInvite).toHaveBeenCalledTimes(2);
  });

  it("still welcomes, but doesn't switch group or navigate, when the user left mid-redeem", async () => {
    let resolveRedeem!: (value: Awaited<ReturnType<typeof api.redeemInvite>>) => void;
    vi.mocked(api.redeemInvite).mockReturnValue(
      new Promise((resolve) => {
        resolveRedeem = resolve;
      }),
    );
    vi.mocked(api.getConfig).mockResolvedValue({
      userId: "u1",
      email: "sam@example.com",
      displayName: "Sam",
      vapidPublicKey: "",
      groupDefaults: {},
      memberships: [{ groupId: "g_new", role: "member", groupName: "New", ...tzGradient }],
    });
    useCurrentGroup.getState().setCurrentGroup("g_old");
    useToasts.setState({ toasts: [] });

    const { unmount } = renderJoin(signedIn);
    await waitFor(() => expect(api.redeemInvite).toHaveBeenCalledTimes(1));
    unmount();
    await act(async () => {
      resolveRedeem({ groupId: "g_new", groupName: "New", role: "member" });
    });

    await waitFor(() =>
      expect(useToasts.getState().toasts).toContainEqual(
        expect.objectContaining({ message: "Welcome to New" }),
      ),
    );
    expect(useCurrentGroup.getState().currentGroupId).toBe("g_old");
  });

  it("sends a signed-out visitor to login with the invite attached", () => {
    const login = vi.fn().mockResolvedValue(undefined);

    renderJoin({ ...signedIn, status: "unauthenticated", user: null, login });

    expect(login).toHaveBeenCalledWith({
      returnTo: "/join?code=ABCD-EFGH",
      invite: "ABCD-EFGH",
    });
    expect(api.redeemInvite).not.toHaveBeenCalled();
  });
});
