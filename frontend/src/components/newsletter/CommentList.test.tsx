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

// Stands in for the real uploader (S3/polling): keeps its own image list, as
// the real one does, so a test can see whether the parent remounted it.
vi.mock("../responses/ImageUploader", async () => {
  const { useState } = await import("react");
  return {
    ImageUploader: ({
      onChange,
    }: {
      onChange?: (images: { imageId: string; status: "ready" }[]) => void;
    }) => {
      const [attached, setAttached] = useState<string | null>(null);
      return attached ? (
        <span>attached {attached}</span>
      ) : (
        <button
          type="button"
          onClick={() => {
            setAttached("img-new");
            onChange?.([{ imageId: "img-new", status: "ready" }]);
          }}
        >
          Attach test image
        </button>
      );
    },
  };
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
    cycleCadence: { frequency: "monthly", maxResponseWindowDays: 28 },
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

/** Expands the collapsed thread summary row, for tests that need to see comment rows. */
async function expandThread(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole("button", { name: /^Show \d+ comments?$/ }));
}

/** Expands the collapsed "Add a comment…" composer button into its textarea + Post form. */
async function expandComposer(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole("button", { name: "Add a comment…" }));
}

describe("CommentList", () => {
  it("with no comments, shows just the composer button", () => {
    renderList([]);

    expect(screen.getByRole("button", { name: "Add a comment…" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Show \d+ comments?$/ })).not.toBeInTheDocument();
  });

  it("collapses to a summary row with commenter avatars, which expands into the full thread", async () => {
    const user = userEvent.setup();
    const comments = [1, 2, 3, 4, 5].map((n) =>
      comment({ commentId: `c${n}`, body: `Comment ${n}`, createdAt: `2026-06-0${n}T00:00:00Z` }),
    );
    renderList(comments);

    const summary = screen.getByRole("button", { name: "Show 5 comments" });
    expect(summary).toBeInTheDocument();
    expect(screen.queryByText("Comment 1")).not.toBeInTheDocument();

    await user.click(summary);

    expect(screen.getByText("Comment 1")).toBeInTheDocument();
    expect(screen.getByText("Comment 5")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Show \d+ comments?$/ })).not.toBeInTheDocument();
    // The full thread includes the composer too.
    expect(screen.getByRole("button", { name: "Add a comment…" })).toBeInTheDocument();
  });

  it("singularizes the summary for exactly one comment", () => {
    renderList([comment({})]);

    expect(screen.getByRole("button", { name: "Show 1 comment" })).toBeInTheDocument();
  });

  it("disables Post for a whitespace-only body and enables it once there's text", async () => {
    const user = userEvent.setup();
    renderList([]);
    await expandComposer(user);

    const postButton = screen.getByRole("button", { name: "Post" });
    const textarea = screen.getByLabelText("Add a comment");

    expect(postButton).toBeDisabled();

    await user.type(textarea, "   ");
    expect(postButton).toBeDisabled();

    await user.type(textarea, "Hello there");
    expect(postButton).toBeEnabled();
  });

  it("posts a new comment and collapses the composer back to its button", async () => {
    const user = userEvent.setup();
    vi.mocked(api.engagement.createComment).mockResolvedValue(
      comment({ commentId: "new1", authorUserId: "u_quinn", displayName: "Quinn", body: "Nice!" }),
    );
    renderList([]);
    await expandComposer(user);

    const textarea = screen.getByLabelText("Add a comment");
    await user.type(textarea, "Nice!");
    await user.click(screen.getByRole("button", { name: "Post" }));

    await waitFor(() =>
      expect(api.engagement.createComment).toHaveBeenCalledWith("g1", "c1", "q1", "r1", {
        body: "Nice!",
      }),
    );
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Add a comment…" })).toBeInTheDocument(),
    );
  });

  it("collapses the composer back to its button on Cancel", async () => {
    const user = userEvent.setup();
    renderList([]);
    await expandComposer(user);

    await user.type(screen.getByLabelText("Add a comment"), "Never mind");
    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect(screen.getByRole("button", { name: "Add a comment…" })).toBeInTheDocument();
    expect(screen.queryByLabelText("Add a comment")).not.toBeInTheDocument();
  });

  it("only shows the image dropzone after tapping the photo button", async () => {
    const user = userEvent.setup();
    renderList([]);
    await expandComposer(user);

    expect(screen.queryByRole("button", { name: "Attach test image" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Add a photo" }));

    expect(screen.getByRole("button", { name: "Attach test image" })).toBeInTheDocument();
  });

  it("resets the uploader after posting a comment with an image", async () => {
    const user = userEvent.setup();
    vi.mocked(api.engagement.createComment).mockResolvedValue(
      comment({ commentId: "new2", authorUserId: "u_quinn", displayName: "Quinn", body: "" }),
    );
    renderList([]);
    await expandComposer(user);
    await user.click(screen.getByRole("button", { name: "Add a photo" }));

    await user.click(await screen.findByRole("button", { name: "Attach test image" }));
    expect(screen.getByText("attached img-new")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Post" }));

    await waitFor(() =>
      expect(api.engagement.createComment).toHaveBeenCalledWith("g1", "c1", "q1", "r1", {
        body: "",
        imageMediaId: "img-new",
      }),
    );
    // Back to the collapsed button — the posted image doesn't linger in any slot.
    expect(await screen.findByRole("button", { name: "Add a comment…" })).toBeInTheDocument();
  });

  it("replaces a comment's image when the author removes it and uploads another", async () => {
    vi.mocked(api.engagement.patchComment).mockClear();
    const user = userEvent.setup();
    vi.mocked(api.engagement.patchComment).mockResolvedValue(
      comment({ commentId: "mine", authorUserId: "u_quinn", displayName: "Quinn", body: "" }),
    );
    const mine = comment({
      commentId: "mine",
      authorUserId: "u_quinn",
      displayName: "Quinn",
      body: "",
      image: {
        imageId: "img-old",
        thumbUrl: "https://media.example.com/thumb.webp",
        displayUrl: "https://media.example.com/display.webp",
        width: 800,
        height: 600,
        caption: null,
      },
    });
    renderList([mine]);
    await expandThread(user);

    await user.click(screen.getByRole("button", { name: "Edit" }));
    await user.click(screen.getByRole("button", { name: "Remove image" }));
    await user.click(screen.getByRole("button", { name: "Attach test image" }));
    await user.click(screen.getByRole("button", { name: "Save" }));

    // D8: replacing deletes the old image for real, so it's confirmed first.
    const dialog = await screen.findByRole("dialog", { name: "Delete the attached image?" });
    expect(dialog).toHaveTextContent("This permanently deletes the image. It can't be recovered.");
    expect(api.engagement.patchComment).not.toHaveBeenCalled();
    await user.click(within(dialog).getByRole("button", { name: "Delete image and save" }));

    await waitFor(() =>
      expect(api.engagement.patchComment).toHaveBeenCalledWith("g1", "c1", "q1", "r1", "mine", {
        body: "",
        imageMediaId: "img-new",
      }),
    );
  });

  it("sends an explicit null when the author removes the image without replacing it", async () => {
    vi.mocked(api.engagement.patchComment).mockClear();
    const user = userEvent.setup();
    vi.mocked(api.engagement.patchComment).mockResolvedValue(
      comment({ commentId: "mine", authorUserId: "u_quinn", displayName: "Quinn", body: "Text" }),
    );
    const mine = comment({
      commentId: "mine",
      authorUserId: "u_quinn",
      displayName: "Quinn",
      body: "Text",
      image: {
        imageId: "img-old",
        thumbUrl: "https://media.example.com/thumb.webp",
        displayUrl: "https://media.example.com/display.webp",
        width: 800,
        height: 600,
        caption: null,
      },
    });
    renderList([mine]);
    await expandThread(user);

    await user.click(screen.getByRole("button", { name: "Edit" }));
    await user.click(screen.getByRole("button", { name: "Remove image" }));
    await user.click(screen.getByRole("button", { name: "Save" }));

    const dialog = await screen.findByRole("dialog", { name: "Delete the attached image?" });
    expect(api.engagement.patchComment).not.toHaveBeenCalled();
    await user.click(within(dialog).getByRole("button", { name: "Delete image and save" }));

    await waitFor(() =>
      expect(api.engagement.patchComment).toHaveBeenCalledWith("g1", "c1", "q1", "r1", "mine", {
        body: "Text",
        imageMediaId: null,
      }),
    );
  });

  it("doesn't save an image removal when its permanent-delete confirm is cancelled", async () => {
    vi.mocked(api.engagement.patchComment).mockClear();
    const user = userEvent.setup();
    const mine = comment({
      commentId: "mine",
      authorUserId: "u_quinn",
      displayName: "Quinn",
      body: "Text",
      image: {
        imageId: "img-old",
        thumbUrl: "https://media.example.com/thumb.webp",
        displayUrl: "https://media.example.com/display.webp",
        width: 800,
        height: 600,
        caption: null,
      },
    });
    renderList([mine]);
    await expandThread(user);

    await user.click(screen.getByRole("button", { name: "Edit" }));
    await user.click(screen.getByRole("button", { name: "Remove image" }));
    await user.click(screen.getByRole("button", { name: "Save" }));
    const dialog = await screen.findByRole("dialog", { name: "Delete the attached image?" });
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(api.engagement.patchComment).not.toHaveBeenCalled();
  });

  it("saves a text-only edit without the image confirm", async () => {
    const user = userEvent.setup();
    vi.mocked(api.engagement.patchComment).mockResolvedValue(
      comment({ commentId: "mine", authorUserId: "u_quinn", displayName: "Quinn", body: "New" }),
    );
    const mine = comment({
      commentId: "mine",
      authorUserId: "u_quinn",
      displayName: "Quinn",
      body: "Old",
      image: {
        imageId: "img-old",
        thumbUrl: "https://media.example.com/thumb.webp",
        displayUrl: "https://media.example.com/display.webp",
        width: 800,
        height: 600,
        caption: null,
      },
    });
    renderList([mine]);
    await expandThread(user);

    await user.click(screen.getByRole("button", { name: "Edit" }));
    const box = screen.getByLabelText("Edit comment");
    await user.clear(box);
    await user.type(box, "New");
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(api.engagement.patchComment).toHaveBeenCalledWith("g1", "c1", "q1", "r1", "mine", {
        body: "New",
      }),
    );
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("warns that deleting a comment with an image deletes the image too", async () => {
    vi.mocked(api.engagement.deleteComment).mockClear();
    const user = userEvent.setup();
    vi.mocked(api.engagement.deleteComment).mockResolvedValue(undefined);
    const mine = comment({
      commentId: "mine",
      authorUserId: "u_quinn",
      displayName: "Quinn",
      body: "With a photo",
      image: {
        imageId: "img-old",
        thumbUrl: "https://media.example.com/thumb.webp",
        displayUrl: "https://media.example.com/display.webp",
        width: 800,
        height: 600,
        caption: null,
      },
    });
    renderList([mine]);
    await expandThread(user);

    await user.click(screen.getByRole("button", { name: "Delete" }));
    const dialog = await screen.findByRole("dialog", { name: "Delete this comment?" });
    expect(dialog).toHaveTextContent(
      "This also permanently deletes the attached image. It can't be recovered.",
    );
    expect(api.engagement.deleteComment).not.toHaveBeenCalled();
    await user.click(within(dialog).getByRole("button", { name: "Delete" }));

    await waitFor(() =>
      expect(api.engagement.deleteComment).toHaveBeenCalledWith("g1", "c1", "q1", "r1", "mine"),
    );
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
    await expandThread(user);

    await screen.findByText("Original");
    await screen.findByText("Their text");

    // Own comment gets both Edit and Delete; a plain member gets neither on
    // someone else's comment — so exactly one of each should exist.
    const editButton = screen.getByRole("button", { name: "Edit" });
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
    await expandThread(user);

    await screen.findByText("Their text");
    await user.click(screen.getByRole("button", { name: "Delete" }));

    const dialog = await screen.findByRole("dialog", { name: "Delete this comment?" });
    await user.click(within(dialog).getByRole("button", { name: "Delete" }));

    await waitFor(() =>
      expect(api.engagement.deleteComment).toHaveBeenCalledWith("g1", "c1", "q1", "r1", "theirs"),
    );
  });

  it("renders a soft-deleted comment as a muted placeholder with no actions", async () => {
    const user = userEvent.setup();
    const deleted = comment({
      commentId: "gone",
      authorUserId: "u_quinn",
      displayName: "Quinn",
      body: "",
      deletedAt: "2026-06-06T00:00:00Z",
    });
    renderList([deleted], adminConfig());
    await expandThread(user);

    await screen.findByText("[deleted]");
    expect(screen.queryByRole("button", { name: "Edit" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Delete" })).not.toBeInTheDocument();
  });
});
