import { act, render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import { CandidateList } from "./CandidateList";
import type { components } from "../../types/api";

type CandidateItemResponse = components["schemas"]["CandidateItemResponse"];

function item(id: string, voteCount: number): CandidateItemResponse {
  return {
    questionId: id,
    kind: "text",
    prompt: `Prompt ${id}`,
    askedBy: null,
    isAnonymous: false,
    voteCount,
    votedByMe: false,
    submittedAt: "2026-09-20T00:00:00Z",
  };
}

function renderList(items: CandidateItemResponse[], sort: "top" | "recent" = "top") {
  return render(
    <MemoryRouter>
      <CandidateList
        groupId="g1"
        items={items}
        timezone="UTC"
        myVoteCount={0}
        votesPerUserPerCycle={3}
        pendingQuestionId={null}
        sort={sort}
        onToggleVote={() => {}}
      />
    </MemoryRouter>,
  );
}

function promptOrder() {
  return screen.getAllByRole("article").map((el) => within(el).getByText(/^Prompt /).textContent);
}

describe("CandidateList", () => {
  it("renders in the given order", () => {
    renderList([item("a", 2), item("b", 1)]);
    expect(promptOrder()).toEqual(["Prompt a", "Prompt b"]);
  });

  it("reorders to match new items once the Top sort's order changes (jsdom has no startViewTransition, so this applies immediately)", async () => {
    const { rerender } = renderList([item("a", 1), item("b", 2)]);
    expect(promptOrder()).toEqual(["Prompt a", "Prompt b"]);

    rerender(
      <MemoryRouter>
        <CandidateList
          groupId="g1"
          items={[item("b", 2), item("a", 1)]}
          timezone="UTC"
          myVoteCount={0}
          votesPerUserPerCycle={3}
          pendingQuestionId={null}
          sort="top"
          onToggleVote={() => {}}
        />
      </MemoryRouter>,
    );
    // The reorder is deferred a tick so `flushSync` never nests inside this render's own commit.
    await act(async () => {
      await Promise.resolve();
    });

    expect(promptOrder()).toEqual(["Prompt b", "Prompt a"]);
  });

  it("shows the empty state when there are no candidates", () => {
    renderList([]);
    expect(screen.getByText("No one's suggested a question yet.")).toBeInTheDocument();
  });
});
