import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { components } from "../../types/api";
import { PollWidget } from "./PollWidget";

type S = components["schemas"];

function question(
  overrides: Partial<S["PublishedQuestionResponse"]> = {},
): S["PublishedQuestionResponse"] {
  return {
    questionId: "q1",
    kind: "poll",
    prompt: "Which trail should we do next?",
    displayOrder: 0,
    askedBy: null,
    isAnonymous: true,
    options: [
      { optionId: "a", label: "Mt Si", voteCount: 3 },
      { optionId: "b", label: "Lake 22", voteCount: 1 },
    ],
    ...overrides,
  };
}

describe("PollWidget", () => {
  it("tallies votes as a percentage of the total and pluralizes the vote count", () => {
    render(<PollWidget question={question()} />);

    expect(screen.getByText("4 votes")).toBeInTheDocument();
    expect(screen.getByText("3 · 75%")).toBeInTheDocument();
    expect(screen.getByText("1 · 25%")).toBeInTheDocument();
  });

  it("singularizes the header for exactly one vote", () => {
    render(
      <PollWidget
        question={question({
          options: [
            { optionId: "a", label: "Mt Si", voteCount: 1 },
            { optionId: "b", label: "Lake 22", voteCount: 0 },
          ],
        })}
      />,
    );

    expect(screen.getByText("1 vote")).toBeInTheDocument();
  });

  it("highlights the caller's own pick with a 'Your vote' badge, and no one else's", () => {
    render(<PollWidget question={question({ myVoteOptionId: "b" })} />);

    expect(screen.getAllByText("Your vote")).toHaveLength(1);
  });

  it("doesn't divide by zero when nobody has voted yet", () => {
    render(
      <PollWidget
        question={question({
          options: [
            { optionId: "a", label: "Mt Si", voteCount: 0 },
            { optionId: "b", label: "Lake 22", voteCount: 0 },
          ],
        })}
      />,
    );

    expect(screen.getByText("0 votes")).toBeInTheDocument();
    expect(screen.getAllByText("0 · 0%")).toHaveLength(2);
  });
});
