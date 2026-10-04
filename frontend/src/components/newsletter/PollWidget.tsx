import { useState } from "react";
import clsx from "clsx";
import type { PublishedPollQuestion } from "../../mocks/types";
import { CommentList } from "./CommentList";

interface Props {
  question: PublishedPollQuestion;
  groupId: string;
  cycleId: string;
}

// Poll widget on a *published* edition.
// Voting happens during the open cycle (via the response editor) and is locked
// once the edition publishes — so this view is read-only. The caller's pick
// is highlighted; everyone else just sees the tally.
export function PollWidget({ question, groupId, cycleId }: Props) {
  const total = Math.max(1, question.totalVotes);
  const [showAllComments, setShowAllComments] = useState(false);
  const visibleComments = showAllComments ? question.comments : question.comments.slice(0, 2);
  return (
    <div className="rounded-3xl border border-line bg-white p-5 shadow-soft">
      <div className="mb-3 text-xs text-inkmuted">
        {question.totalVotes} vote{question.totalVotes === 1 ? "" : "s"}
      </div>
      <ul className="space-y-3">
        {question.options.map((opt) => {
          const pct = Math.round((opt.voteCount / total) * 100);
          const mine = question.myVoteOptionId === opt.optionId;
          return (
            <li key={opt.optionId}>
              <div className="mb-1 text-sm font-semibold leading-snug">{opt.label}</div>
              <div className="mb-1.5 flex items-center gap-2 text-xs text-inkmuted">
                {mine && (
                  <span className="rounded-full bg-grape px-1.5 py-0.5 font-semibold text-white">
                    your pick
                  </span>
                )}
                <span className="ml-auto tabular-nums">
                  {opt.voteCount} · {pct}%
                </span>
              </div>
              <div
                className={clsx(
                  "h-3 overflow-hidden rounded-full bg-cream",
                  mine && "ring-2 ring-grape/40",
                )}
              >
                <div
                  className={clsx("h-full transition-all", mine ? "hero-may" : "bg-coral")}
                  style={{ width: `${Math.max(1, pct)}%` }}
                />
              </div>
            </li>
          );
        })}
      </ul>
      <CommentList
        comments={visibleComments}
        totalCount={question.comments.length}
        onShowAll={() => setShowAllComments(true)}
        groupId={groupId}
        cycleId={cycleId}
        questionId={question.questionId}
        responseId={question.questionId}
      />
    </div>
  );
}
