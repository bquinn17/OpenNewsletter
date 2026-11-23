import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { CandidateCard } from "./CandidateCard";
import type { components } from "../../types/api";

type CandidateItemResponse = components["schemas"]["CandidateItemResponse"];

function item(overrides: Partial<CandidateItemResponse> = {}): CandidateItemResponse {
  return {
    questionId: "cq_1",
    kind: "text",
    prompt: "What's the best trail?",
    askedBy: null,
    isAnonymous: false,
    voteCount: 2,
    votedByMe: false,
    submittedAt: "2026-09-20T00:00:00Z",
    ...overrides,
  };
}

describe("CandidateCard", () => {
  it("springs the vote tile on a fresh vote but not on mount with an existing vote", () => {
    // Mounting already-voted (e.g. after a sort change) must not replay the spring — only an
    // actual false→true transition does (`docs/design/frontend.md` §18).
    render(
      <CandidateCard
        item={item({ votedByMe: true })}
        timezone="UTC"
        disabled={false}
        pending={false}
        onToggleVote={() => {}}
      />,
    );
    const button = screen.getByRole("button", { name: /Remove your vote/ });
    expect(button.className).not.toContain("animate-pop-in");
  });

  it("springs the vote tile when the caller votes", () => {
    const { rerender } = render(
      <CandidateCard
        item={item({ votedByMe: false })}
        timezone="UTC"
        disabled={false}
        pending={false}
        onToggleVote={() => {}}
      />,
    );

    rerender(
      <CandidateCard
        item={item({ votedByMe: true, voteCount: 3 })}
        timezone="UTC"
        disabled={false}
        pending={false}
        onToggleVote={() => {}}
      />,
    );

    const button = screen.getByRole("button", { name: /Remove your vote/ });
    expect(button.className).toContain("animate-pop-in");
  });

  it("carries a per-candidate view-transition name for the Top-sort reorder", () => {
    const { container } = render(
      <CandidateCard
        item={item()}
        timezone="UTC"
        disabled={false}
        pending={false}
        onToggleVote={() => {}}
      />,
    );
    const article = container.querySelector("article");
    expect(article?.style.viewTransitionName).toBe("candidate-cq_1");
  });
});
