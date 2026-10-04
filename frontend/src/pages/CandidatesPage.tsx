import { Link, useParams } from "react-router-dom";
import clsx from "clsx";
import { useCandidates, useCastVote, useWithdrawVote } from "../mocks/legacyQueries";
import { PageHeader } from "../components/layout/PageHeader";
import { Button } from "../components/ui/Button";
import { AskedBy } from "../components/newsletter/AskedBy";
import { useToasts } from "../state/toast";
import { formatRelative } from "../utils/dates";
import { useState } from "react";
import type { CandidateQuestion } from "../mocks/types";

export function CandidatesPage() {
  const { groupId = "" } = useParams();
  const { data, isLoading, isError, error, refetch } = useCandidates(groupId);
  const cast = useCastVote(groupId);
  const withdraw = useWithdrawVote(groupId);
  const pushToast = useToasts((s) => s.push);
  const [sort, setSort] = useState<"top" | "recent">("top");

  if (isLoading) {
    return (
      <div className="bg-cream pb-12">
        <PageHeader title="Loading…" eyebrow="Upcoming" back="/" />
        <div className="space-y-3 px-5 pt-6">
          {[0, 1, 2].map((i) => (
            <div key={i} className="h-24 animate-pulse rounded-3xl border border-line bg-white" />
          ))}
        </div>
      </div>
    );
  }
  if (isError || !data) {
    return (
      <div className="bg-cream pb-12">
        <PageHeader title="Upcoming" eyebrow="Couldn't load" back="/" />
        <div className="px-5 pt-8 text-center">
          <div className="mb-3 text-5xl">😬</div>
          <p className="font-semibold text-ink">
            {error instanceof Error ? error.message : "Couldn't load candidate questions."}
          </p>
          <button
            onClick={() => refetch()}
            className="mt-5 rounded-full bg-ink px-5 py-3 font-semibold text-cream"
          >
            Try again
          </button>
        </div>
      </div>
    );
  }

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
      pushToast(
        `You've used all ${data.votesPerUserPerCycle} votes. Withdraw one to vote here.`,
        "error",
      );
      return;
    }
    cast.mutate(c.questionId);
  };

  return (
    <div className="bg-cream pb-28">
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
        <div
          className={`${data.gradientClass} relative overflow-hidden rounded-4xl p-5 text-white`}
        >
          <div className="absolute -right-10 -top-10 h-40 w-40 rounded-full bg-white/15" />
          <div className="text-xs font-bold uppercase tracking-widest opacity-90">
            Vote on the questions
          </div>
          <h1 className="mt-1 font-display text-3xl font-bold leading-tight">
            Top {Math.min(5, sorted.length)} win the {data.monthLabel} edition
          </h1>
          <div className="mt-4 flex items-center justify-between text-sm">
            <span className="opacity-90">Opens {formatRelative(data.responseOpenAt)}</span>
            <div className="rounded-full bg-white/20 px-3 py-1.5 font-semibold backdrop-blur">
              {data.myVoteCount} / {data.votesPerUserPerCycle} votes used
            </div>
          </div>
        </div>
      </div>

      <div className="mt-5 flex gap-1 px-5">
        <button
          onClick={() => setSort("top")}
          className={clsx(
            "rounded-full px-3 py-1.5 text-xs font-bold",
            sort === "top" ? "bg-ink text-cream" : "border border-line bg-white text-inkmuted",
          )}
        >
          Top voted
        </button>
        <button
          onClick={() => setSort("recent")}
          className={clsx(
            "rounded-full px-3 py-1.5 text-xs font-semibold",
            sort === "recent" ? "bg-ink text-cream" : "border border-line bg-white text-inkmuted",
          )}
        >
          Most recent
        </button>
      </div>

      <div className="mt-3 space-y-3 px-5">
        {sorted.map((c, idx) => (
          <article
            key={c.questionId}
            className={clsx(
              "rounded-3xl border border-line bg-white p-5 shadow-soft",
              idx === 0 && "ring-2 ring-coral/30",
              c.kind === "poll" && idx > 0 && "ring-2 ring-grape/40",
            )}
          >
            <div className="flex items-start gap-3">
              <button
                onClick={() => handleToggle(c)}
                className={clsx(
                  "flex h-14 w-12 shrink-0 flex-col items-center justify-center rounded-2xl font-bold transition",
                  c.votedByMe
                    ? "bg-coral text-white shadow-pop"
                    : "border border-line bg-cream text-inkmuted hover:border-ink",
                )}
                aria-label={c.votedByMe ? "Withdraw vote" : "Upvote"}
              >
                <span className="text-[10px] uppercase tracking-wider opacity-80">
                  {c.votedByMe ? "votes" : "vote"}
                </span>
                <span className="text-lg leading-none">{c.votedByMe ? c.voteCount : "↑"}</span>
              </button>
              <div className="min-w-0 flex-1">
                {c.kind === "poll" && (
                  <span className="text-[10px] font-bold uppercase tracking-widest text-grape">
                    Poll · {c.pollOptions?.length ?? 0} options
                  </span>
                )}
                <AskedBy askedBy={c.askedBy} isAnonymous={c.isAnonymous} className="mt-0.5" />
                <p className="mt-0.5 font-display text-lg leading-snug">{c.prompt}</p>
                {c.kind === "poll" && c.pollOptions && (
                  <div className="mt-2 flex flex-wrap gap-1.5">
                    {c.pollOptions.map((p) => (
                      <span key={p.optionId} className="rounded-full bg-cream px-2 py-0.5 text-xs">
                        {p.label}
                      </span>
                    ))}
                  </div>
                )}
                <div className="mt-2 text-xs text-inkmuted">
                  {c.voteCount} vote{c.voteCount === 1 ? "" : "s"} · submitted{" "}
                  {formatRelative(c.submittedAt)}
                  {c.isAnonymous ? " · anonymous" : ""}
                </div>
              </div>
            </div>
          </article>
        ))}
      </div>

      <div className="mt-6 px-5">
        <Link to={`/g/${groupId}/upcoming/suggest`}>
          <button className="w-full rounded-3xl border-2 border-dashed border-line p-5 text-center text-inkmuted transition hover:border-coral hover:text-coral">
            <div className="mb-1 text-2xl">✨</div>
            <div className="font-semibold">Got an idea? Suggest a question</div>
            <div className="mt-0.5 text-xs">Asked by you — anonymous is one tap away</div>
          </button>
        </Link>
      </div>
    </div>
  );
}
