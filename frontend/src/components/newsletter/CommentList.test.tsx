import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { AuthContext, type AuthValue } from "../../auth/useAuth";
import { api } from "../../api/client";
import type { components } from "../../types/api";
import { CommentList } from "./CommentList";

type S = components["schemas"];

vi.mock("../../api/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../api/client")>();
  return {
    ...actual,
    api: {
      getConfig: vi.fn(),
      getGroup: vi.fn(),
      engagement: {
        createComment: vi.fn(),
        patchComment: vi.fn(),
        deleteComment: vi.fn(),
      },
    },
  };
});

vi.mock("../../utils/media", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../utils/media")>();
  return { ...actual, ensureCookie: vi.fn().mockResolvedValue(undefined) };
});

const signedIn: AuthValue = {
  status: "authenticated",
  user: { sub: "sub-1", email: "quinn@example.com", name: "Quinn" },
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
        groupId: "g1",
        role: "member",
        groupName: "Test Group",
        timezone: "UTC",
        gradient: "grape-sky",
      },
    ],
  };
}

function adminConfig(): S["ConfigResponse"] {
  const config = baseConfig();
  config.memberships[0]!.role = "admin";
  return config;
}

function baseGroup(): S["GroupResponse"] {
  return {
    groupId: "g1",
    name: "Test Group",
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
      {
        userId: "u_sam",
        displayName: "Sam",
        role: "member",
        avatarColor: "red",
        avatarUrl: null,
        joinedAt: "2026-01-01T00:00:00Z",
        editionsAnswered: 1,
      },
    ],
  };
}

function comment(overrides: Partial<S["CommentResponse"]>): S["CommentResponse"] {
  return {
    commentId: "c1",
    authorUserId: "u_sam",
    displayName: "Sam",
    body: "A comment",
    image: null,
    createdAt: "2026-06-05T09:00:00Z",
    editedAt: null,
    deletedAt: null,
    ...overrides,
  };
}

function renderList(comments: S["CommentResponse"][], config: S["ConfigResponse"] = baseConfig()) {
  vi.mocked(api.getConfig).mockResolvedValue(config);
  vi.mocked(api.getGroup).mockResolvedValue(baseGroup());
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={queryClient}>
      <AuthContext.Provider value={signedIn}>
        <CommentList
          comments={comments}
          groupId="g1"
          cycleId="c1"
          questionId="q1"
          responseId="r1"
          timezone="UTC"
        />
      </AuthContext.Provider>
    </QueryClientProvider>,
  );
}

describe("CommentList", () => {
  it("collapses to the last 3 comments with a Show all button, which expands them all", async () => {
    const comments = [1, 2, 3, 4, 5].map((n) =>
      comment({ commentId: `c${n}`, body: `Comment ${n}`, createdAt: `2026-06-0${n}T00:00:00Z` }),
    );
    renderList(comments);

    expect(screen.getByRole("button", { name: "Show all 5 comments" })).toBeInTheDocument();
    expect(screen.queryByText("Comment 1")).not.toBeInTheDocument();
    expect(screen.queryByText("Comment 2")).not.toBeInTheDocument();
    expect(screen.getByText("Comment 3")).toBeInTheDocument();
    expect(screen.getByText("Comment 4")).toBeInTheDocument();
    expect(screen.getByText("Comment 5")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Show all 5 comments" }));

    expect(screen.getByText("Comment 1")).toBeInTheDocument();
    expect(screen.getByText("Comment 2")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Show all/ })).not.toBeInTheDocument();
  });

  it("disables Post for a whitespace-only body and enables it once there's text", async () => {
    const user = userEvent.setup();
    renderList([]);

    const postButton = await screen.findByRole("button", { name: "Post" });
    const textarea = screen.getByPlaceholderText("Add a comment…");

    expect(postButton).toBeDisabled();

    await user.type(textarea, "   ");
    expect(postButton).toBeDisabled();

    await user.type(textarea, "Hello there");
    expect(postButton).toBeEnabled();
  });

  it("posts a new comment with the typed body and clears the composer", async () => {
    const user = userEvent.setup();
    vi.mocked(api.engagement.createComment).mockResolvedValue(
      comment({ commentId: "new1", authorUserId: "u_quinn", displayName: "Quinn", body: "Nice!" }),
    );
    renderList([]);

    const textarea = await screen.findByPlaceholderText("Add a comment…");
    await user.type(textarea, "Nice!");
    await user.click(screen.getByRole("button", { name: "Post" }));

    await waitFor(() =>
      expect(api.engagement.createComment).toHaveBeenCalledWith("g1", "c1", "q1", "r1", {
        body: "Nice!",
      }),
    );
    await waitFor(() => expect(textarea).toHaveValue(""));
  });

  it("lets the author edit their own comment but not someone else's", async () => {
    const user = userEvent.setup();
    vi.mocked(api.engagement.patchComment).mockResolvedValue(
      comment({
        commentId: "mine",
        authorUserId: "u_quinn",
        displayName: "Quinn",
        body: "Updated body",
        editedAt: "2026-06-05T10:00:00Z",
      }),
    );
    const mine = comment({
      commentId: "mine",
      authorUserId: "u_quinn",
      displayName: "Quinn",
      body: "Original",
    });
    const theirs = comment({
      commentId: "theirs",
      authorUserId: "u_sam",
      displayName: "Sam",
      body: "Their text",
    });
    renderList([mine, theirs]);

    await screen.findByText("Original");
    await screen.findByText("Their text");

    // Own comment gets both Edit and Delete; a plain member gets neither on
    // someone else's comment — so exactly one of each should exist.
    const editButton = await screen.findByRole("button", { name: "Edit" });
    expect(screen.getAllByRole("button", { name: "Delete" })).toHaveLength(1);

    await user.click(editButton);
    const editTextarea = screen.getByLabelText("Edit comment");
    await user.clear(editTextarea);
    await user.type(editTextarea, "Updated body");
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(api.engagement.patchComment).toHaveBeenCalledWith("g1", "c1", "q1", "r1", "mine", {
        body: "Updated body",
      }),
    );
  });

  it("lets a group admin delete another member's comment after confirming", async () => {
    const user = userEvent.setup();
    vi.mocked(api.engagement.deleteComment).mockResolvedValue(undefined);
    const theirs = comment({
      commentId: "theirs",
      authorUserId: "u_sam",
      displayName: "Sam",
      body: "Their text",
    });
    renderList([theirs], adminConfig());

    await screen.findByText("Their text");
    await user.click(await screen.findByRole("button", { name: "Delete" }));

    const dialog = await screen.findByRole("dialog", { name: "Delete this comment?" });
    await user.click(within(dialog).getByRole("button", { name: "Delete" }));

    await waitFor(() =>
      expect(api.engagement.deleteComment).toHaveBeenCalledWith("g1", "c1", "q1", "r1", "theirs"),
    );
  });

  it("renders a soft-deleted comment as a muted placeholder with no actions", async () => {
    const deleted = comment({
      commentId: "gone",
      authorUserId: "u_quinn",
      displayName: "Quinn",
      body: "",
      deletedAt: "2026-06-06T00:00:00Z",
    });
    renderList([deleted], adminConfig());

    await screen.findByText("[deleted]");
    expect(screen.queryByRole("button", { name: "Edit" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Delete" })).not.toBeInTheDocument();
  });
});
