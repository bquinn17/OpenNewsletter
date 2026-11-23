import clsx from "clsx";
import { ChevronDown } from "lucide-react";
import type { components } from "../../types/api";
import { AnswerCard } from "./AnswerCard";
import { PollWidget } from "./PollWidget";
import { questionSectionId } from "./QuestionChips";
import { QuestionPrompt } from "./QuestionPrompt";

type S = components["schemas"];

type Props = {
  question: S["PublishedQuestionResponse"];
  groupId: string;
  cycleId: string;
  timezone: string;
  collapsed: boolean;
  onToggle: () => void;
  cadenceNoun: "month" | "week";
  /** `responseId` → its rise-in position, for the first 8 answers on the page. */
  riseIndexByResponseId: Map<string, number>;
  /** Where the prompt bar sticks: the measured height of the page header plus the chips row. */
  stickyTopPx: number;
};

/**
 * One question of a published edition: a prompt pinned under the question chips while its
 * answers scroll (`docs/design/frontend.md` §7.2), and its answers on one surface separated by
 * dividers instead of N bordered cards.
 */
export function PublishedQuestion({
  question,
  groupId,
  cycleId,
  timezone,
  collapsed,
  onToggle,
  cadenceNoun,
  riseIndexByResponseId,
  stickyTopPx,
}: Props) {
  const isExpanded = !collapsed;
  const answers = question.kind === "text" ? (question.answers ?? []) : [];
  const stickyTop = `calc(${stickyTopPx}px + env(safe-area-inset-top))`;

  return (
    <section
      id={questionSectionId(question.questionId)}
      style={{ scrollMarginTop: stickyTop }}
      className="mt-4"
    >
      {/* Full width and styled like the chips row, so once pinned it reads as part of the
          sticky header rather than a box floating over the page. */}
      <button
        type="button"
        aria-expanded={isExpanded}
        onClick={onToggle}
        style={{ top: stickyTop }}
        className="sticky z-10 flex w-full items-start justify-between gap-3 border-b border-line bg-mist px-5 py-3 text-left"
      >
        <QuestionPrompt
          prompt={question.prompt}
          kind={question.kind}
          askedBy={question.askedBy}
          isAnonymous={question.isAnonymous}
        />
        <span className="mt-1 flex shrink-0 items-center gap-1.5 text-inkmuted">
          {collapsed && question.kind === "text" && (
            <span className="text-xs font-semibold">
              {answers.length} answer{answers.length === 1 ? "" : "s"}
            </span>
          )}
          <ChevronDown
            aria-hidden
            className={clsx(
              "h-5 w-5 transition-transform motion-safe:duration-base",
              isExpanded && "rotate-180",
            )}
          />
        </span>
      </button>

      {isExpanded &&
        (question.kind === "text" ? (
          <div className="mx-5 mt-4 divide-y divide-line rounded-3xl bg-white px-5 shadow-soft">
            {answers.map((answer) => (
              <AnswerCard
                key={answer.responseId}
                answer={answer}
                groupId={groupId}
                cycleId={cycleId}
                questionId={question.questionId}
                timezone={timezone}
                riseIndex={riseIndexByResponseId.get(answer.responseId)}
              />
            ))}
            {answers.length === 0 && (
              <div className="py-6 text-center text-sm text-inkmuted">
                Nobody answered this one. Maybe next {cadenceNoun}.
              </div>
            )}
          </div>
        ) : (
          <div className="mx-5 mt-4">
            <PollWidget question={question} />
          </div>
        ))}
    </section>
  );
}
