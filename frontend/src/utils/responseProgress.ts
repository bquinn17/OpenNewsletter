import type { components } from "../types/api";

type S = components["schemas"];
type OpenQuestion = S["OpenQuestionResponse"];

/**
 * A question counts as "answered" once there's a non-empty draft or answer,
 * regardless of publish status (`docs/design/frontend.md` §7.3): a non-blank
 * body, at least one image or at least one voice clip for a text question, a
 * picked option for a poll. Used both by the "Done" CTA's next-question search and the edition
 * page's completion celebration.
 */
export function isQuestionAnswered(question: OpenQuestion): boolean {
  const response = question.myResponse;
  if (!response) return false;
  if (question.kind === "poll") return !!response.pollOptionId;
  return (
    (response.body?.trim().length ?? 0) > 0 ||
    response.imageMediaIds.length > 0 ||
    response.audioMediaIds.length > 0
  );
}

export function allQuestionsAnswered(questions: OpenQuestion[]): boolean {
  return questions.length > 0 && questions.every(isQuestionAnswered);
}

/**
 * The next unanswered question's ID after `afterQuestionId`, in `displayOrder`,
 * wrapping around and skipping `afterQuestionId` itself. `null` when every
 * other question is already answered (or `afterQuestionId` isn't in `questions`).
 */
export function nextUnansweredQuestionId(
  questions: OpenQuestion[],
  afterQuestionId: string,
): string | null {
  const sorted = [...questions].sort((a, b) => a.displayOrder - b.displayOrder);
  const startIndex = sorted.findIndex((q) => q.questionId === afterQuestionId);
  if (startIndex === -1) return null;

  for (let step = 1; step < sorted.length; step++) {
    const candidate = sorted[(startIndex + step) % sorted.length];
    if (!isQuestionAnswered(candidate)) return candidate.questionId;
  }
  return null;
}
