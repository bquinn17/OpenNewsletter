import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "../api/client";
import { AuthContext, type AuthValue } from "../auth/useAuth";
import type { components } from "../types/api";
import { RequireGroupAdmin } from "./RequireGroupAdmin";

vi.mock("../api/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api/client")>();
  return { ...actual, api: { getConfig: vi.fn() } };
});

type Role = components["schemas"]["Role"];

const signedIn: AuthValue = {
  status: "authenticated",
  user: { sub: "s", email: null, name: null },
  login: vi.fn(),
  logout: vi.fn(),
};

function config(role: Role) {
  return {
    userId: "u1",
    email: null,
    displayName: null,
    vapidPublicKey: "",
    groupDefaults: {},
    memberships: [
      { groupId: "g1", role, groupName: "G", timezone: "UTC", gradient: "grape-sky" as const },
    ],
  };
}

async function renderAdminRoute(cached: Role) {
  const queryClient = new QueryClient();
  vi.mocked(api.getConfig).mockResolvedValueOnce(config(cached));
  await queryClient.prefetchQuery({ queryKey: ["config"], queryFn: api.getConfig });
  render(
    <QueryClientProvider client={queryClient}>
      <AuthContext.Provider value={signedIn}>
        <MemoryRouter initialEntries={["/g/g1/admin"]}>
          <Routes>
            <Route path="/g/:groupId" element={<div>Group home</div>} />
            <Route path="/g/:groupId" element={<RequireGroupAdmin />}>
              <Route path="admin" element={<div>Admin page</div>} />
            </Route>
          </Routes>
        </MemoryRouter>
      </AuthContext.Provider>
    </QueryClientProvider>,
  );
}

afterEach(() => vi.clearAllMocks());

describe("RequireGroupAdmin", () => {
  it("rechecks a stale cached role before redirecting a newly promoted admin", async () => {
    vi.mocked(api.getConfig).mockResolvedValue(config("admin"));
    await renderAdminRoute("member");

    expect(await screen.findByText("Admin page")).toBeInTheDocument();
  });

  it("redirects a member to the group home once the recheck confirms the role", async () => {
    vi.mocked(api.getConfig).mockResolvedValue(config("member"));
    await renderAdminRoute("member");

    expect(await screen.findByText("Group home")).toBeInTheDocument();
    expect(api.getConfig).toHaveBeenCalledTimes(2);
  });
});
