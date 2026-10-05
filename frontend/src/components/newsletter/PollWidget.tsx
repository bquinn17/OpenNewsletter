import clsx from "clsx";
import type { components } from "../../types/api";

type Props = {
  question: components["schemas"]["PublishedQuestionResponse"];
};

// Poll widget on a *published* edition. Voting happens during the open cycle
// (via the response editor) and is locked once the edition publishes, so
// this view is read-only. The caller's pick is highlighted; everyone else
// just sees the tally.
export function PollWidget({ question }: Props) {
  const options = question.options ?? [];
  const totalVotes = options.reduce((sum, option) => sum + option.voteCount, 0);
  const totalForPct = Math.max(1, totalVotes);

  return (
    <div className="rounded-3xl border border-line bg-white p-5 shadow-soft">
      <div className="mb-3 text-xs text-inkmuted">
        {totalVotes} vote{totalVotes === 1 ? "" : "s"}
      </div>
      <ul className="space-y-3">
        {options.map((option) => {
          const pct = Math.round((option.voteCount / totalForPct) * 100);
          const mine = question.myVoteOptionId === option.optionId;
          return (
            <li key={option.optionId}>
              <div className="mb-1 text-sm font-semibold leading-snug">{option.label}</div>
              <div className="mb-1.5 flex items-center gap-2 text-xs text-inkmuted">
                {mine && (
                  <span className="rounded-full bg-grape px-1.5 py-0.5 font-semibold text-white">
                    Your vote
                  </span>
                )}
                <span className="ml-auto tabular-nums">
                  {option.voteCount} · {pct}%
                </span>
              </div>
              <div
                className={clsx(
                  "h-3 overflow-hidden rounded-full bg-cream",
                  mine && "ring-2 ring-grape/40",
                )}
              >
                <div
                  className={clsx("h-full transition-all", mine ? "bg-grape" : "bg-coral")}
                  style={{ width: `${Math.max(1, pct)}%` }}
                />
              </div>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
