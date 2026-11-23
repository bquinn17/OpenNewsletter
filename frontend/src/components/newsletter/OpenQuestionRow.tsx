import clsx from "clsx";
import { Link } from "react-router-dom";
import type { components } from "../../types/api";
import { stripImageTokens } from "../../utils/markdown";
import { QuestionPrompt } from "./QuestionPrompt";

type S = components["schemas"];

const PREVIEW_MAX_CHARS = 140;

function draftPreview(question: S["OpenQuestionResponse"]): string | null {
  const myResponse = question.myResponse;
  if (!myResponse) return null;
  if (question.kind === "poll") {
    const option = question.pollOptions?.find((o) => o.optionId === myResponse.pollOptionId);
    return option ? option.label : null;
  }
  if (!myResponse.body) return null;
  const stripped = stripImageTokens(myResponse.body);
  if (!stripped) return null;
  return stripped.length > PREVIEW_MAX_CHARS
    ? `${stripped.slice(0, PREVIEW_MAX_CHARS).trimEnd()}…`
    : stripped;
}

type StatusLabel = "Not started" | "Draft" | "Published";

function statusLabel(myResponse: S["MyResponseSummary"] | null): StatusLabel {
  if (!myResponse) return "Not started";
  return myResponse.status === "published" ? "Published" : "Draft";
}

/**
 * Icon + accessible status text for the row's tile. Color pairs with a distinct icon and
 * sr-only text, never signals status alone (`docs/development/coding-standards.md` §3.10).
 */
const STATUS_TILE: Record<StatusLabel, { className: string; icon: string }> = {
  Published: { className: "bg-mint/15 text-mint-ink", icon: "✓" },
  Draft: { className: "bg-sun/30 text-sun-ink", icon: "…" },
  "Not started": { className: "bg-cream text-inkmuted", icon: "○" },
};

type Props = {
  question: S["OpenQuestionResponse"];
  groupId: string;
  cycleId: string;
};

/**
 * One row of the open edition's question list: one meta line instead of stacked uppercase labels
 * (`docs/design/frontend.md` §7.2), with the response status in the icon tile.
 */
export function OpenQuestionRow({ question, groupId, cycleId }: Props) {
  const status = statusLabel(question.myResponse);
  const preview = draftPreview(question);
  const tile = STATUS_TILE[status];

  return (
    <Link
      to={`/g/${groupId}/n/${cycleId}/respond/${question.questionId}`}
      className="flex w-full items-start gap-4 rounded-3xl border border-line bg-white p-5 text-left shadow-soft"
    >
      <div
        className={clsx(
          "grid h-9 w-9 flex-shrink-0 place-items-center rounded-2xl font-bold",
          tile.className,
        )}
      >
        <span aria-hidden>{tile.icon}</span>
        <span className="sr-only">{status}</span>
      </div>
      <div className="min-w-0 flex-1">
        <QuestionPrompt
          prompt={question.prompt}
          kind={question.kind}
          askedBy={question.askedBy}
          isAnonymous={question.isAnonymous}
        />
        {preview && <div className="mt-1 text-sm text-inkmuted">{preview}</div>}
      </div>
      <span className="text-grape-ink">→</span>
    </Link>
  );
}
