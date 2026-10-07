import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AuthContext, type AuthValue } from "../auth/useAuth";
import { api, ApiError } from "../api/client";
import { useToasts } from "../state/toast";
import type { components } from "../types/api";
import { GroupAdminPage } from "./GroupAdminPage";

type S = components["schemas"];

vi.mock("../api/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api/client")>();
  return {
    ...actual,
    api: {
      getGroup: vi.fn(),
      getMe: vi.fn(),
      createInvite: vi.fn(),
      listInvites: vi.fn(),
      revokeInvite: vi.fn(),
      patchMember: vi.fn(),
      removeMember: vi.fn(),
      patchGroup: vi.fn(),
    },
  };
});

const signedIn: AuthValue = {
  status: "authenticated",
  user: { sub: "sub-1", email: "quinn@example.com", name: "Quinn" },
  login: vi.fn(),
  logout: vi.fn(),
};

const ME: S["UserResponse"] = {
  userId: "u_me",
  email: "quinn@example.com",
  displayName: "Quinn",
  avatarColor: "teal",
  avatarMediaId: null,
  avatarUrl: null,
  createdAt: "2026-01-01T00:00:00Z",
};

function baseGroup(overrides: Partial<S["GroupResponse"]> = {}): S["GroupResponse"] {
  return {
    groupId: "g1",
    name: "Trail Crew",
    timezone: "America/New_York",
    gradient: "grape-sky",
    cycleSettings: { questionsPerCycle: 5, votesPerUserPerCycle: 3, responseWindowDays: 4 },
    notificationSettings: { offsetsHoursBeforeClose: [96, 48, 24], onCycleOpen: true },
    memberCount: 3,
    memberSoftCap: 50,
    createdAt: "2026-01-01T00:00:00Z",
    members: [
      {
        userId: "u_me",
        displayName: "Quinn",
        role: "admin",
        avatarColor: "teal",
        avatarUrl: null,
        joinedAt: "2026-01-01T00:00:00Z",
        editionsAnswered: 5,
      },
      {
        userId: "u_sam",
        displayName: "Sam",
        role: "admin",
        avatarColor: "red",
        avatarUrl: null,
        joinedAt: "2026-01-01T00:00:00Z",
        editionsAnswered: 0,
      },
      {
        userId: "u_jordan",
        displayName: "Jordan",
        role: "member",
        avatarColor: "violet",
        avatarUrl: null,
        joinedAt: "2026-01-02T00:00:00Z",
        editionsAnswered: 1,
      },
    ],
    ...overrides,
  };
}

function renderPage(
  queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } }),
) {
  return {
    queryClient,
    ...render(
      <QueryClientProvider client={queryClient}>
        <AuthContext.Provider value={signedIn}>
          <MemoryRouter initialEntries={["/g/g1/admin"]}>
            <Routes>
              <Route path="/g/:groupId/admin" element={<GroupAdminPage />} />
            </Routes>
          </MemoryRouter>
        </AuthContext.Provider>
      </QueryClientProvider>,
    ),
  };
}

beforeEach(() => {
  vi.mocked(api.getMe).mockResolvedValue(ME);
  vi.mocked(api.getGroup).mockResolvedValue(baseGroup());
  vi.mocked(api.listInvites).mockResolvedValue({ items: [], nextCursor: null });
});

afterEach(() => {
  vi.clearAllMocks();
  useToasts.setState({ toasts: [] });
});

describe("GroupAdminPage", () => {
  it("renders the Members tab by default and switches to Invites/Settings", async () => {
    const user = userEvent.setup();
    renderPage();

    const membersTab = await screen.findByRole("tab", { name: "Members" });
    expect(membersTab).toHaveAttribute("aria-selected", "true");
    expect(await screen.findByText("3 of 50 members")).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "Invites" }));
    expect(screen.getByRole("tab", { name: "Invites" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("button", { name: "Create invite" })).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "Settings" }));
    expect(screen.getByRole("tab", { name: "Settings" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByLabelText("Group name")).toBeInTheDocument();
  });

  it("shows the caller's own row with no options menu, but a menu for other members", async () => {
    renderPage();

    // Waits for both `useGroup` and `useMe` to settle — Quinn's own-row menu
    // only disappears once the caller's `userId` is known.
    await screen.findByRole("button", { name: "Options for Sam" });
    await waitFor(() =>
      expect(screen.queryByRole("button", { name: "Options for Quinn" })).not.toBeInTheDocument(),
    );
    expect(screen.getByRole("button", { name: "Options for Jordan" })).toBeInTheDocument();
  });

  it("promotes a member to admin via the row menu", async () => {
    const user = userEvent.setup();
    vi.mocked(api.patchMember).mockResolvedValue({
      userId: "u_jordan",
      displayName: "Jordan",
      role: "admin",
      avatarColor: "violet",
      avatarUrl: null,
      joinedAt: "2026-01-02T00:00:00Z",
      editionsAnswered: 1,
    });

    renderPage();
    await screen.findByText("Jordan");

    await user.click(screen.getByRole("button", { name: "Options for Jordan" }));
    await user.click(screen.getByRole("menuitem", { name: "Make admin" }));

    await waitFor(() =>
      expect(api.patchMember).toHaveBeenCalledWith("g1", "u_jordan", { role: "admin" }),
    );
    expect(useToasts.getState().toasts.some((t) => t.message.includes("now an admin"))).toBe(true);
  });

  it("removes a member only after confirming the dialog", async () => {
    const user = userEvent.setup();
    vi.mocked(api.removeMember).mockResolvedValue(undefined);

    renderPage();
    await screen.findByText("Jordan");

    await user.click(screen.getByRole("button", { name: "Options for Jordan" }));
    await user.click(screen.getByRole("menuitem", { name: "Remove from group" }));

    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText(/Remove Jordan from the group/)).toBeInTheDocument();
    expect(api.removeMember).not.toHaveBeenCalled();

    await user.click(within(dialog).getByRole("button", { name: "Remove" }));

    await waitFor(() => expect(api.removeMember).toHaveBeenCalledWith("g1", "u_jordan"));
    expect(useToasts.getState().toasts.some((t) => t.message.includes("Removed Jordan"))).toBe(
      true,
    );
  });

  it("shows a friendly LAST_ADMIN toast instead of a raw error", async () => {
    const user = userEvent.setup();
    vi.mocked(api.patchMember).mockRejectedValue(
      new ApiError(409, "LAST_ADMIN", {
        type: "https://api.opennewsletter.example.com/errors/last-admin",
        title: "Conflict",
        status: 409,
        detail: "a group must keep at least one admin",
        code: "LAST_ADMIN",
        correlationId: "corr-1",
      }),
    );

    renderPage();
    await screen.findByText("Sam");

    await user.click(screen.getByRole("button", { name: "Options for Sam" }));
    await user.click(screen.getByRole("menuitem", { name: "Make member" }));

    await waitFor(() => expect(api.patchMember).toHaveBeenCalled());
    expect(useToasts.getState().toasts.some((t) => t.message.includes("at least one admin"))).toBe(
      true,
    );
  });

  describe("Invites tab", () => {
    it("creates an invite with the chosen role/TTL and shows a copyable URL", async () => {
      const user = userEvent.setup();
      vi.mocked(api.createInvite).mockResolvedValue({
        code: "ABCD-EFGH-JKMN-PQRS",
        groupId: "g1",
        expiresAt: "2026-02-01T00:00:00Z",
        roleOnRedeem: "admin",
      });

      renderPage();
      await user.click(await screen.findByRole("tab", { name: "Invites" }));

      await user.selectOptions(screen.getByLabelText("Role"), "admin");
      await user.selectOptions(screen.getByLabelText("Expires in"), "30");
      await user.click(screen.getByRole("button", { name: "Create invite" }));

      await waitFor(() =>
        expect(api.createInvite).toHaveBeenCalledWith({
          groupId: "g1",
          roleOnRedeem: "admin",
          ttlDays: 30,
        }),
      );

      const urlInput = (await screen.findByLabelText("Invite link")) as HTMLInputElement;
      expect(urlInput.value).toContain("join?code=ABCD-EFGH-JKMN-PQRS");

      await user.click(screen.getByRole("button", { name: "Copy" }));
      // `userEvent.setup()` attaches its own clipboard stub, so the real
      // write is verified by reading it back rather than mocking `writeText`.
      await expect(navigator.clipboard.readText()).resolves.toContain(
        "join?code=ABCD-EFGH-JKMN-PQRS",
      );
    });

    it("renders active/expired/revoked/consumed invites, with a fallback for an unknown redeemer", async () => {
      const user = userEvent.setup();
      const now = Date.now();
      vi.mocked(api.listInvites).mockResolvedValue({
        items: [
          {
            code: "ACTV-ACTV-ACTV-ACTV",
            groupId: "g1",
            status: "pending",
            roleOnRedeem: "member",
            createdBy: "u_me",
            createdAt: new Date(now - 1000).toISOString(),
            expiresAt: new Date(now + 5 * 24 * 60 * 60 * 1000).toISOString(),
            consumedBy: null,
            consumedAt: null,
          },
          {
            code: "EXPD-EXPD-EXPD-EXPD",
            groupId: "g1",
            status: "pending",
            roleOnRedeem: "member",
            createdBy: "u_me",
            createdAt: new Date(now - 10 * 24 * 60 * 60 * 1000).toISOString(),
            expiresAt: new Date(now - 24 * 60 * 60 * 1000).toISOString(),
            consumedBy: null,
            consumedAt: null,
          },
          {
            code: "RVKD-RVKD-RVKD-RVKD",
            groupId: "g1",
            status: "revoked",
            roleOnRedeem: "member",
            createdBy: "u_me",
            createdAt: new Date(now - 2000).toISOString(),
            expiresAt: new Date(now + 5 * 24 * 60 * 60 * 1000).toISOString(),
            consumedBy: null,
            consumedAt: null,
          },
          {
            code: "SAMC-SAMC-SAMC-SAMC",
            groupId: "g1",
            status: "consumed",
            roleOnRedeem: "member",
            createdBy: "u_me",
            createdAt: new Date(now - 3000).toISOString(),
            expiresAt: new Date(now + 5 * 24 * 60 * 60 * 1000).toISOString(),
            consumedBy: "u_sam",
            consumedAt: new Date(now - 1 * 24 * 60 * 60 * 1000).toISOString(),
          },
          {
            code: "GONE-GONE-GONE-GONE",
            groupId: "g1",
            status: "consumed",
            roleOnRedeem: "member",
            createdBy: "u_me",
            createdAt: new Date(now - 4000).toISOString(),
            expiresAt: new Date(now + 5 * 24 * 60 * 60 * 1000).toISOString(),
            consumedBy: "u_departed",
            consumedAt: new Date(now - 2 * 24 * 60 * 60 * 1000).toISOString(),
          },
        ],
        nextCursor: null,
      });

      renderPage();
      await user.click(await screen.findByRole("tab", { name: "Invites" }));

      expect(await screen.findByText("ACTV-ACTV-ACTV-ACTV")).toBeInTheDocument();
      expect(screen.getByText("EXPD-EXPD-EXPD-EXPD")).toBeInTheDocument();
      expect(screen.getByText("expired")).toBeInTheDocument();
      expect(screen.getByText("RVKD-RVKD-RVKD-RVKD")).toBeInTheDocument();
      expect(screen.getByText("revoked")).toBeInTheDocument();
      expect(screen.getByText(/used by Sam/)).toBeInTheDocument();
      expect(screen.getByText(/used by a former member/)).toBeInTheDocument();

      // Only the active invite gets a Revoke button.
      expect(screen.getAllByRole("button", { name: "Revoke" })).toHaveLength(1);
    });

    it("revokes an invite", async () => {
      const user = userEvent.setup();
      vi.mocked(api.listInvites).mockResolvedValue({
        items: [
          {
            code: "ACTV-ACTV-ACTV-ACTV",
            groupId: "g1",
            status: "pending",
            roleOnRedeem: "member",
            createdBy: "u_me",
            createdAt: "2026-01-01T00:00:00Z",
            expiresAt: new Date(Date.now() + 5 * 24 * 60 * 60 * 1000).toISOString(),
            consumedBy: null,
            consumedAt: null,
          },
        ],
        nextCursor: null,
      });
      vi.mocked(api.revokeInvite).mockResolvedValue(undefined);

      renderPage();
      await user.click(await screen.findByRole("tab", { name: "Invites" }));
      await screen.findByText("ACTV-ACTV-ACTV-ACTV");

      await user.click(screen.getByRole("button", { name: "Revoke" }));

      await waitFor(() => expect(api.revokeInvite).toHaveBeenCalledWith("ACTV-ACTV-ACTV-ACTV"));
      expect(useToasts.getState().toasts.some((t) => t.message === "Invite revoked.")).toBe(true);
    });
  });

  describe("Settings tab", () => {
    async function openSettingsTab() {
      const user = userEvent.setup();
      const harness = renderPage();
      await user.click(await screen.findByRole("tab", { name: "Settings" }));
      await screen.findByLabelText("Group name");
      return { user, ...harness };
    }

    it("disables Save until a field actually differs from the loaded group", async () => {
      const { user } = await openSettingsTab();
      const saveButton = screen.getByRole("button", { name: "Save" });
      expect(saveButton).toBeDisabled();

      const nameInput = screen.getByLabelText("Group name");
      await user.clear(nameInput);
      await user.type(nameInput, "Trail Crew");
      expect(saveButton).toBeDisabled();

      await user.type(nameInput, "!");
      expect(saveButton).toBeEnabled();
    });

    it("sends only the changed top-level field", async () => {
      const { user } = await openSettingsTab();
      vi.mocked(api.patchGroup).mockResolvedValue(baseGroup({ name: "Trailblazers" }));

      await user.clear(screen.getByLabelText("Group name"));
      await user.type(screen.getByLabelText("Group name"), "Trailblazers");
      await user.click(screen.getByRole("button", { name: "Save" }));

      await waitFor(() =>
        expect(api.patchGroup).toHaveBeenCalledWith("g1", { name: "Trailblazers" }),
      );
      expect(useToasts.getState().toasts.some((t) => t.message === "Group settings saved.")).toBe(
        true,
      );
    });

    it("blocks save client-side when votes exceed questions per cycle", async () => {
      const { user } = await openSettingsTab();

      await user.clear(screen.getByLabelText("Votes per member per cycle"));
      await user.type(screen.getByLabelText("Votes per member per cycle"), "10");
      await user.click(screen.getByRole("button", { name: "Save" }));

      expect(await screen.findByText(/Can't exceed questions per cycle/)).toBeInTheDocument();
      expect(api.patchGroup).not.toHaveBeenCalled();
    });

    it("blocks save client-side on an out-of-range reminder offset", async () => {
      const { user } = await openSettingsTab();

      const firstReminder = screen.getByLabelText("Reminder 1 (hours before close)");
      await user.clear(firstReminder);
      await user.type(firstReminder, "999");
      await user.click(screen.getByRole("button", { name: "Save" }));

      expect(await screen.findByText(/hours max/)).toBeInTheDocument();
      expect(api.patchGroup).not.toHaveBeenCalled();
    });

    it("blocks save client-side when the member soft cap drops below the current member count", async () => {
      const { user } = await openSettingsTab();

      const cap = screen.getByLabelText("Member soft cap");
      await user.clear(cap);
      await user.type(cap, "1");
      await user.click(screen.getByRole("button", { name: "Save" }));

      expect(await screen.findByText(/Can't be below the current 3 members/)).toBeInTheDocument();
      expect(api.patchGroup).not.toHaveBeenCalled();
    });

    it("renders the server's field error inline on a 422", async () => {
      const { user } = await openSettingsTab();
      vi.mocked(api.patchGroup).mockRejectedValue(
        new ApiError(422, "VALIDATION_FAILED", {
          type: "https://api.opennewsletter.example.com/errors/validation-failed",
          title: "Validation failed",
          status: 422,
          detail: "name is already taken",
          code: "VALIDATION_FAILED",
          correlationId: "corr-1",
          fieldErrors: [{ field: "name", code: "INVALID", message: "That name is taken." }],
        }),
      );

      await user.clear(screen.getByLabelText("Group name"));
      await user.type(screen.getByLabelText("Group name"), "Taken Name");
      await user.click(screen.getByRole("button", { name: "Save" }));

      expect(await screen.findByText("That name is taken.")).toBeInTheDocument();
    });

    it("invalidates the config query on a successful save", async () => {
      const { user, queryClient } = await openSettingsTab();
      queryClient.setQueryData(["config"], { userId: "u_me" });
      vi.mocked(api.patchGroup).mockResolvedValue(baseGroup({ name: "Trailblazers" }));

      await user.clear(screen.getByLabelText("Group name"));
      await user.type(screen.getByLabelText("Group name"), "Trailblazers");
      await user.click(screen.getByRole("button", { name: "Save" }));

      await waitFor(() => expect(api.patchGroup).toHaveBeenCalled());
      await waitFor(() => expect(queryClient.getQueryState(["config"])?.isInvalidated).toBe(true));
    });
  });
});
