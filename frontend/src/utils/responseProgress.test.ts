import { describe, expect, it } from "vitest";
import type { components } from "../types/api";
import {
  allQuestionsAnswered,
  isQuestionAnswered,
  nextUnansweredQuestionId,
} from "./responseProgress";

type S = components["schemas"];
type OpenQuestion = S["OpenQuestionResponse"];

function textQuestion(
  questionId: string,
  displayOrder: number,
  myResponse: OpenQuestion["myResponse"] = null,
): OpenQuestion {
  return {
    questionId,
    kind: "text",
    prompt: "p",
    displayOrder,
    askedBy: null,
    isAnonymous: false,
    isStanding: false,
    myResponse,
  };
}

function pollQuestion(
  questionId: string,
  displayOrder: number,
  myResponse: OpenQuestion["myResponse"] = null,
): OpenQuestion {
  return {
    questionId,
    kind: "poll",
    prompt: "p",
    displayOrder,
    askedBy: null,
    isAnonymous: false,
    isStanding: false,
    pollOptions: [{ optionId: "optA", label: "A" }],
    myResponse,
  };
}

function draft(overrides: Partial<NonNullable<OpenQuestion["myResponse"]>> = {}) {
  return {
    responseId: "r1",
    status: "draft" as const,
    imageMediaIds: [],
    updatedAt: "2026-06-01T00:00:00Z",
    publishedAt: null,
    ...overrides,
  };
}

describe("isQuestionAnswered", () => {
  it("is false with no response at all", () => {
    expect(isQuestionAnswered(textQuestion("q1", 0, null))).toBe(false);
  });

  it("is false for a text response with a blank body and no images", () => {
    expect(isQuestionAnswered(textQuestion("q1", 0, draft({ body: "   " })))).toBe(false);
  });

  it("is true for a text response with a non-blank body", () => {
    expect(isQuestionAnswered(textQuestion("q1", 0, draft({ body: "hi" })))).toBe(true);
  });

  it("is true for a text response with an image and no body", () => {
    expect(
      isQuestionAnswered(textQuestion("q1", 0, draft({ body: "", imageMediaIds: ["img1"] }))),
    ).toBe(true);
  });

  it("is false for a poll response with no pick", () => {
    expect(isQuestionAnswered(pollQuestion("q2", 0, draft()))).toBe(false);
  });

  it("is true for a poll response with a pick", () => {
    expect(isQuestionAnswered(pollQuestion("q2", 0, draft({ pollOptionId: "optA" })))).toBe(true);
  });
});

describe("allQuestionsAnswered", () => {
  it("is false for an edition with zero questions", () => {
    expect(allQuestionsAnswered([])).toBe(false);
  });

  it("is false when at least one question is unanswered", () => {
    expect(
      allQuestionsAnswered([
        textQuestion("q1", 0, draft({ body: "hi" })),
        textQuestion("q2", 1, null),
      ]),
    ).toBe(false);
  });

  it("is true when every question has a non-empty draft or answer", () => {
    expect(
      allQuestionsAnswered([
        textQuestion("q1", 0, draft({ body: "hi" })),
        pollQuestion("q2", 1, draft({ pollOptionId: "optA" })),
      ]),
    ).toBe(true);
  });
});

describe("nextUnansweredQuestionId", () => {
  const answered = draft({ body: "hi" });

  it("finds the next unanswered question after the current one, in display order", () => {
    const questions = [
      textQuestion("q1", 0, answered),
      textQuestion("q2", 1, null),
      textQuestion("q3", 2, null),
    ];
    expect(nextUnansweredQuestionId(questions, "q1")).toBe("q2");
  });

  it("wraps around to an unanswered question earlier in the list", () => {
    const questions = [
      textQuestion("q1", 0, null),
      textQuestion("q2", 1, answered),
      textQuestion("q3", 2, answered),
    ];
    expect(nextUnansweredQuestionId(questions, "q2")).toBe("q1");
  });

  it("skips the current question even if it's unanswered", () => {
    const questions = [textQuestion("q1", 0, null), textQuestion("q2", 1, answered)];
    expect(nextUnansweredQuestionId(questions, "q1")).toBe(null);
  });

  it("returns null once every other question is answered", () => {
    const questions = [textQuestion("q1", 0, answered), textQuestion("q2", 1, answered)];
    expect(nextUnansweredQuestionId(questions, "q1")).toBe(null);
  });

  it("ignores displayOrder's listing order, sorting before searching", () => {
    const questions = [
      textQuestion("q3", 2, null),
      textQuestion("q1", 0, answered),
      textQuestion("q2", 1, null),
    ];
    expect(nextUnansweredQuestionId(questions, "q1")).toBe("q2");
  });

  it("returns null when the current question isn't in the list", () => {
    const questions = [textQuestion("q1", 0, null)];
    expect(nextUnansweredQuestionId(questions, "missing")).toBe(null);
  });
});
