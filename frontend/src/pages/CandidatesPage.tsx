import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import clsx from "clsx";
import { useCandidates, useMembership, useNewsletters, type CandidateSort } from "../api/queries";
import { useToggleVote } from "../api/mutations";
import { ApiError } from "../api/client";
import { PageHeader } from "../components/layout/PageHeader";
import { CandidateList } from "../components/candidates/CandidateList";
import { useToasts } from "../state/toast";
import { countdownTo, cycleLabels } from "../utils/dates";

function opensLabel(responseOpenAt: string, timezone: string): string {
  const { days, expired } = countdownTo(responseOpenAt);
  const { monthLabel } = cycleLabels(responseOpenAt, timezone);
  if (expired || days <= 0) return `The ${monthLabel} newsletter opens today`;
  return `${days} day${days === 1 ? "" : "s"} until the ${monthLabel} newsletter opens`;
}

export function CandidatesPage() {
  const { groupId = "" } = useParams();
  const membership = useMembership(groupId);
  const timezone = membership?.timezone ?? "UTC";
  const [sort, setSort] = useState<CandidateSort>("top");
  const [showVotePanel, setShowVotePanel] = useState(false);
  const pushToast = useToasts((s) => s.push);

  const newsletters = useNewsletters(groupId);
  const candidates = useCandidates(groupId, sort);
  const toggleVote = useToggleVote(groupId);

  const votingCycle = newsletters.data?.items.find((n) => n.status === "voting");
  const openCycle = newsletters.data?.items.find((n) => n.status === "open");

  function handleToggleVote(questionId: string, voted: boolean) {
    toggleVote.mutate(
      { questionId, voted },
      {
        onError: (error) => {
          if (error instanceof ApiError && error.code === "VOTE_CAP_REACHED") {
            pushToast(
              `You've used all ${candidates.data?.votesPerUserPerCycle ?? 0} votes this cycle. Remove one to vote here.`,
              "error",
            );
            return;
          }
          pushToast("Couldn't update your vote. Try again.", "error");
        },
      },
    );
  }

  if (candidates.isLoading || newsletters.isLoading) {
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

  const cycleNotVoting =
    candidates.isError &&
    candidates.error instanceof ApiError &&
    candidates.error.code === "CYCLE_NOT_VOTING";

  if (candidates.isError && !cycleNotVoting) {
    return (
      <div className="bg-cream pb-12">
        <PageHeader title="Upcoming" eyebrow="Couldn't load" back="/" />
        <div className="px-5 pt-8 text-center">
          <div className="mb-3 text-5xl">😬</div>
          <p className="font-semibold text-ink">
            {candidates.error instanceof Error
              ? candidates.error.message
              : "Couldn't load candidate questions."}
          </p>
          <button
            onClick={() => candidates.refetch()}
            className="mt-5 rounded-full bg-ink px-5 py-3 font-semibold text-cream"
          >
            Try again
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="bg-cream pb-28">
      <PageHeader
        eyebrow={`${membership?.groupName ?? "Group"} · upcoming`}
        title="Suggested questions"
        back="/"
        rightSlot={
          <Link
            to={`/g/${groupId}/upcoming/suggest`}
            className="inline-flex items-center justify-center gap-1.5 rounded-full bg-coral px-3 py-1.5 text-xs font-semibold text-white shadow-pop transition hover:brightness-105"
          >
            ＋ Idea
          </Link>
        }
      />

      <div className="space-y-3 px-5 pt-5">
        {votingCycle && (
          <div className="rounded-3xl bg-gradient-to-br from-grape to-sky p-5 text-white">
            <div className="text-xs font-bold uppercase tracking-widest opacity-90">
              Vote on the questions
            </div>
            <h1 className="mt-1 font-display text-xl font-bold leading-tight">
              {opensLabel(votingCycle.responseOpenAt, timezone)}
            </h1>
          </div>
        )}

        {openCycle && (
          <Link
            to={`/g/${groupId}/n/${openCycle.cycleId}`}
            className="block rounded-3xl border border-line bg-white p-4 text-sm font-semibold text-ink shadow-soft hover:border-coral"
          >
            The {cycleLabels(openCycle.responseOpenAt, timezone).monthLabel} edition is open —
            answer now
          </Link>
        )}
      </div>

      {cycleNotVoting ? (
        <div className="px-5 pt-8 text-center text-inkmuted">
          <div className="mb-3 text-4xl">🗳️</div>
          <p className="font-semibold text-ink">
            Voting for the next edition hasn&apos;t opened yet.
          </p>
          <p className="mt-1 text-sm">Check back soon — candidate questions will appear here.</p>
        </div>
      ) : (
        candidates.data && (
          <>
            <div className="mt-5 flex items-center justify-between gap-3 px-5">
              <div className="flex gap-1">
                <button
                  type="button"
                  aria-pressed={sort === "top"}
                  onClick={() => setSort("top")}
                  className={clsx(
                    "rounded-full px-3 py-1.5 text-xs font-bold",
                    sort === "top"
                      ? "bg-ink text-cream"
                      : "border border-line bg-white text-inkmuted",
                  )}
                >
                  Top
                </button>
                <button
                  type="button"
                  aria-pressed={sort === "recent"}
                  onClick={() => setSort("recent")}
                  className={clsx(
                    "rounded-full px-3 py-1.5 text-xs font-bold",
                    sort === "recent"
                      ? "bg-ink text-cream"
                      : "border border-line bg-white text-inkmuted",
                  )}
                >
                  Recent
                </button>
              </div>

              <div className="flex items-center gap-2 text-xs text-inkmuted">
                <span>
                  You&apos;ve used {candidates.data.myVoteCount} of{" "}
                  {candidates.data.votesPerUserPerCycle} votes
                </span>
                {candidates.data.myVoteCount > 0 && (
                  <button
                    type="button"
                    aria-expanded={showVotePanel}
                    onClick={() => setShowVotePanel((v) => !v)}
                    className="rounded-full border border-line bg-white px-2.5 py-1 font-semibold text-ink"
                  >
                    Manage votes
                  </button>
                )}
              </div>
            </div>

            {showVotePanel && candidates.data.myVoteCount > 0 && (
              <div className="mx-5 mt-3 rounded-3xl border border-line bg-white p-4">
                <h2 className="text-xs font-bold uppercase tracking-widest text-inkmuted">
                  Your votes
                </h2>
                <ul className="mt-2 space-y-2">
                  {candidates.data.items
                    .filter((item) => item.votedByMe)
                    .map((item) => (
                      <li key={item.questionId} className="flex items-center justify-between gap-3">
                        <span className="min-w-0 flex-1 truncate text-sm">{item.prompt}</span>
                        <button
                          type="button"
                          onClick={() => handleToggleVote(item.questionId, false)}
                          disabled={toggleVote.isPending}
                          className="shrink-0 rounded-full border border-line px-3 py-1 text-xs font-semibold text-coral hover:border-coral"
                        >
                          Remove vote
                        </button>
                      </li>
                    ))}
                </ul>
              </div>
            )}

            <div className="px-5">
              <CandidateList
                groupId={groupId}
                items={candidates.data.items}
                timezone={timezone}
                myVoteCount={candidates.data.myVoteCount}
                votesPerUserPerCycle={candidates.data.votesPerUserPerCycle}
                pendingQuestionId={
                  toggleVote.isPending ? (toggleVote.variables?.questionId ?? null) : null
                }
                onToggleVote={handleToggleVote}
              />
            </div>
          </>
        )
      )}

      <div className="mt-6 px-5">
        <Link
          to={`/g/${groupId}/upcoming/suggest`}
          className="block w-full rounded-3xl border-2 border-dashed border-line p-5 text-center text-inkmuted transition hover:border-coral hover:text-coral"
        >
          <div className="mb-1 text-2xl">✨</div>
          <div className="font-semibold">Got an idea? Suggest a question</div>
          <div className="mt-0.5 text-xs">Anonymous is one tap away</div>
        </Link>
      </div>
    </div>
  );
}
