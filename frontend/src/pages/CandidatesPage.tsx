import { Link, useParams } from "react-router-dom";
import clsx from "clsx";
import { useCandidates, useCastVote, useWithdrawVote } from "../api/queries";
import { PageHeader } from "../components/layout/PageHeader";
import { Button } from "../components/ui/Button";
import { useToasts } from "../state/toast";
import { formatRelative } from "../utils/dates";
import { useState } from "react";
import type { CandidateQuestion } from "../api/types";

export function CandidatesPage() {
  const { groupId = "" } = useParams();
  const { data, isLoading } = useCandidates(groupId);
  const cast = useCastVote(groupId);
  const withdraw = useWithdrawVote(groupId);
  const pushToast = useToasts((s) => s.push);
  const [sort, setSort] = useState<"top" | "recent">("top");

  if (isLoading || !data) return null;

  const sorted = [...data.candidates].sort((a, b) =>
    sort === "top"
      ? b.voteCount - a.voteCount
      : new Date(b.submittedAt).getTime() - new Date(a.submittedAt).getTime(),
  );

  const handleToggle = (c: CandidateQuestion) => {
    if (c.votedByMe) {
      withdraw.mutate(c.questionId);
      return;
    }
    if (data.myVoteCount >= data.votesPerUserPerCycle) {
      pushToast(`You've used all ${data.votesPerUserPerCycle} votes. Withdraw one to vote here.`, "error");
      return;
    }
    cast.mutate(c.questionId);
  };

  return (
    <div className="bg-cream  pb-28">
      <PageHeader
        eyebrow={`${data.groupName} · upcoming`}
        title={`${data.monthLabel} ${data.yearLabel}`}
        back="/"
        rightSlot={
          <Link to={`/g/${groupId}/upcoming/suggest`}>
            <Button size="sm">＋ Idea</Button>
          </Link>
        }
      />

      <div className="px-5 pt-5">
        <div className={`${data.gradientClass} text-white rounded-4xl p-5 relative overflow-hidden`}>
          <div className="absolute -right-10 -top-10 w-40 h-40 rounded-full bg-white/15" />
          <div className="text-xs uppercase tracking-widest opacity-90 font-bold">Vote on the questions</div>
          <h1 className="font-display text-3xl font-bold leading-tight mt-1">
            Top {Math.min(5, sorted.length)} win the {data.monthLabel} edition
          </h1>
          <div className="flex items-center justify-between mt-4 text-sm">
            <span className="opacity-90">
              Opens {formatRelative(data.responseOpenAt)}
            </span>
            <div className="bg-white/20 backdrop-blur rounded-full px-3 py-1.5 font-semibold">
              {data.myVoteCount} / {data.votesPerUserPerCycle} votes used
            </div>
          </div>
        </div>
      </div>

      <div className="px-5 mt-5 flex gap-1">
        <button
          onClick={() => setSort("top")}
          className={clsx(
            "px-3 py-1.5 rounded-full text-xs font-bold",
            sort === "top" ? "bg-ink text-cream" : "bg-white border border-line text-inkmuted",
          )}
        >
          Top voted
        </button>
        <button
          onClick={() => setSort("recent")}
          className={clsx(
            "px-3 py-1.5 rounded-full text-xs font-semibold",
            sort === "recent" ? "bg-ink text-cream" : "bg-white border border-line text-inkmuted",
          )}
        >
          Most recent
        </button>
      </div>

      <div className="px-5 mt-3 space-y-3">
        {sorted.map((c, idx) => (
          <article
            key={c.questionId}
            className={clsx(
              "bg-white rounded-3xl border border-line shadow-soft p-5",
              idx === 0 && "ring-2 ring-coral/30",
              c.kind === "poll" && idx > 0 && "ring-2 ring-grape/40",
            )}
          >
            <div className="flex items-start gap-3">
              <button
                onClick={() => handleToggle(c)}
                className={clsx(
                  "w-12 h-14 rounded-2xl flex flex-col items-center justify-center font-bold shrink-0 transition",
                  c.votedByMe
                    ? "bg-coral text-white shadow-pop"
                    : "bg-cream border border-line text-inkmuted hover:border-ink",
                )}
                aria-label={c.votedByMe ? "Withdraw vote" : "Upvote"}
              >
                <span className="text-[10px] opacity-80 uppercase tracking-wider">
                  {c.votedByMe ? "votes" : "vote"}
                </span>
                <span className="text-lg leading-none">{c.votedByMe ? c.voteCount : "↑"}</span>
              </button>
              <div className="flex-1 min-w-0">
                {c.kind === "poll" && (
                  <span className="text-[10px] uppercase tracking-widest text-grape font-bold">
                    Poll · {c.pollOptions?.length ?? 0} options
                  </span>
                )}
                <p className="font-display text-lg leading-snug mt-0.5">{c.prompt}</p>
                {c.kind === "poll" && c.pollOptions && (
                  <div className="flex flex-wrap gap-1.5 mt-2">
                    {c.pollOptions.map((p) => (
                      <span key={p.optionId} className="px-2 py-0.5 rounded-full bg-cream text-xs">
                        {p.label}
                      </span>
                    ))}
                  </div>
                )}
                <div className="text-xs text-inkmuted mt-2">
                  {c.voteCount} vote{c.voteCount === 1 ? "" : "s"} · submitted {formatRelative(c.submittedAt)} · anonymous
                </div>
              </div>
            </div>
          </article>
        ))}
      </div>

      <div className="px-5 mt-6">
        <Link to={`/g/${groupId}/upcoming/suggest`}>
          <button className="w-full rounded-3xl border-2 border-dashed border-line p-5 text-center text-inkmuted hover:border-coral hover:text-coral transition">
            <div className="text-2xl mb-1">✨</div>
            <div className="font-semibold">Got an idea? Suggest a question</div>
            <div className="text-xs mt-0.5">Submitted anonymously to the group</div>
          </button>
        </Link>
      </div>
    </div>
  );
}
