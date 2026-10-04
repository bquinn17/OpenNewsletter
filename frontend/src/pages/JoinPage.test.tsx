import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AuthContext, type AuthValue } from "../auth/useAuth";
import { RequireMembership } from "../routes/RequireMembership";
import { api, ApiError } from "../api/client";
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

function renderJoin(auth: AuthValue, queryClient = new QueryClient()) {
  return render(
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
    </QueryClientProvider>,
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
