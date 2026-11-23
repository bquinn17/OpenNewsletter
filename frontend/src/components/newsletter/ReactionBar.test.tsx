import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { api, ApiError } from "../../api/client";
import { queryKeys } from "../../api/queries";
import type { components } from "../../types/api";
import { ReactionBar } from "./ReactionBar";

type S = components["schemas"];

vi.mock("../../api/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../api/client")>();
  return {
    ...actual,
    api: {
      engagement: {
        putReaction: vi.fn(),
        deleteReaction: vi.fn(),
      },
    },
  };
});

function publishedWith(
  reactionGroups: S["ReactionGroupResponse"][],
): S["NewsletterDetailResponse"] {
  return {
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
            userId: "u1",
            displayName: "Sam",
            body: "Hello",
            images: [],
            publishedAt: "2026-06-05T00:00:01Z",
            comments: [],
            reactionGroups,
          },
        ],
      },
    ],
  };
}

function renderBar(reactionGroups: S["ReactionGroupResponse"][]) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  queryClient.setQueryData(queryKeys.newsletter("g1", "c1"), publishedWith(reactionGroups));

  render(
    <QueryClientProvider client={queryClient}>
      <ReactionBar
        groupId="g1"
        cycleId="c1"
        questionId="q1"
        responseId="r1"
        reactionGroups={reactionGroups}
      />
    </QueryClientProvider>,
  );
  return queryClient;
}

function cachedGroups(qc: QueryClient): S["ReactionGroupResponse"][] {
  const data = qc.getQueryData<S["NewsletterDetailResponse"]>(queryKeys.newsletter("g1", "c1"));
  if (!data || data.status !== "published") throw new Error("expected published cache");
  return data.questions[0]!.answers![0]!.reactionGroups;
}

describe("ReactionBar", () => {
  it("shows only the emoji already used, plus one add-reaction button", () => {
    renderBar([{ emoji: "🔥", count: 4, reactedByMe: true }]);

    const firePill = screen.getByRole("button", { name: /React with 🔥, 4 reactions/ });
    expect(firePill).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Add reaction" })).toBeInTheDocument();
    // Nothing else not already used shows directly on the bar.
    expect(screen.queryByRole("button", { name: "React with 🤣" })).not.toBeInTheDocument();
  });

  it("with no reactions yet, shows just the add-reaction button", () => {
    renderBar([]);

    expect(screen.getByRole("button", { name: "Add reaction" })).toBeInTheDocument();
    expect(screen.queryAllByRole("button", { name: /^React with/ })).toHaveLength(0);
  });

  it("toggles an existing pill off, rolling back on a server error", async () => {
    const user = userEvent.setup();
    let rejectDelete!: (error: unknown) => void;
    vi.mocked(api.engagement.deleteReaction).mockImplementation(
      () =>
        new Promise((_resolve, reject) => {
          rejectDelete = reject;
        }),
    );

    const qc = renderBar([{ emoji: "🔥", count: 4, reactedByMe: true }]);

    await user.click(screen.getByRole("button", { name: /React with 🔥, 4 reactions/ }));

    // Optimistic: count drops immediately.
    expect(cachedGroups(qc)).toEqual([{ emoji: "🔥", count: 3, reactedByMe: false }]);

    rejectDelete(new ApiError(500, "INTERNAL", null));
    await waitFor(() =>
      expect(cachedGroups(qc)).toEqual([{ emoji: "🔥", count: 4, reactedByMe: true }]),
    );
  });

  it("opens the picker with the quick picks in their own top row, and adds an emoji from the grid", async () => {
    const user = userEvent.setup();
    vi.mocked(api.engagement.putReaction).mockResolvedValue({
      reactionGroups: [{ emoji: "🚀", count: 1, reactedByMe: true }],
      myReactions: ["🚀"],
    });

    renderBar([]);

    await user.click(screen.getByRole("button", { name: "Add reaction" }));
    const dialog = await screen.findByRole("dialog", { name: "Choose an emoji" });
    expect(within(dialog).getByRole("group", { name: "Quick picks" })).toBeInTheDocument();

    await user.click(within(dialog).getByRole("button", { name: "React with 🚀" }));

    await waitFor(() =>
      expect(api.engagement.putReaction).toHaveBeenCalledWith("g1", "c1", "q1", "r1", "🚀"),
    );
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("adds a reaction from the picker's quick-pick row", async () => {
    const user = userEvent.setup();
    vi.mocked(api.engagement.putReaction).mockResolvedValue({
      reactionGroups: [{ emoji: "🔥", count: 1, reactedByMe: true }],
      myReactions: ["🔥"],
    });

    renderBar([]);

    await user.click(screen.getByRole("button", { name: "Add reaction" }));
    const dialog = await screen.findByRole("dialog");
    const quickPicks = within(dialog).getByRole("group", { name: "Quick picks" });

    await user.click(within(quickPicks).getByRole("button", { name: "React with 🔥" }));

    await waitFor(() =>
      expect(api.engagement.putReaction).toHaveBeenCalledWith("g1", "c1", "q1", "r1", "🔥"),
    );
  });

  it("skips an already-used emoji in the picker's quick-pick row", async () => {
    const user = userEvent.setup();
    renderBar([{ emoji: "🔥", count: 1, reactedByMe: true }]);

    await user.click(screen.getByRole("button", { name: "Add reaction" }));
    const dialog = await screen.findByRole("dialog");
    const quickPicks = within(dialog).getByRole("group", { name: "Quick picks" });

    expect(
      within(quickPicks).queryByRole("button", { name: "React with 🔥" }),
    ).not.toBeInTheDocument();
  });

  it("closes the picker on Escape and returns focus to the + button", async () => {
    const user = userEvent.setup();
    renderBar([]);

    const addButton = screen.getByRole("button", { name: "Add reaction" });
    await user.click(addButton);
    expect(await screen.findByRole("dialog")).toBeInTheDocument();

    await user.keyboard("{Escape}");

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(addButton).toHaveFocus();
  });
});
