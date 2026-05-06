import clsx from "clsx";
import { useCastPollVote } from "../../api/queries";
import type { PublishedPollQuestion } from "../../api/types";

interface Props {
  question: PublishedPollQuestion;
  groupId: string;
  cycleId: string;
}

export function PollWidget({ question, groupId, cycleId }: Props) {
  const cast = useCastPollVote(groupId, cycleId);
  const total = Math.max(1, question.totalVotes);
  return (
    <div className="bg-white rounded-3xl border border-line shadow-soft p-5">
      <div className="text-xs text-inkmuted mb-3">
        {question.totalVotes} vote{question.totalVotes === 1 ? "" : "s"}
        {question.myVoteOptionId && (
          <>
            {" · "}you picked{" "}
            <strong className="text-ink">
              {question.options.find((o) => o.optionId === question.myVoteOptionId)?.label}
            </strong>
          </>
        )}
      </div>
      <ul className="space-y-3">
        {question.options.map((opt) => {
          const pct = Math.round((opt.voteCount / total) * 100);
          const mine = question.myVoteOptionId === opt.optionId;
          return (
            <li key={opt.optionId}>
              <button
                onClick={() => cast.mutate({ questionId: question.questionId, optionId: opt.optionId })}
                className="w-full text-left"
              >
                <div className="flex items-baseline justify-between text-sm font-semibold mb-1.5">
                  <span className="flex items-center gap-2">
                    {opt.label}
                    {mine && (
                      <span className="text-xs px-1.5 py-0.5 rounded-full bg-grape text-white">your pick</span>
                    )}
                  </span>
                  <span className="text-inkmuted">
                    {opt.voteCount} · {pct}%
                  </span>
                </div>
                <div
                  className={clsx(
                    "h-3 rounded-full bg-cream overflow-hidden",
                    mine && "ring-2 ring-grape/40",
                  )}
                >
                  <div
                    className={clsx("h-full transition-all", mine ? "hero-may" : "bg-coral")}
                    style={{ width: `${Math.max(1, pct)}%` }}
                  />
                </div>
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
