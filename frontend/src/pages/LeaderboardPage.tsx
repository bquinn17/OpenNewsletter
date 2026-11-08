import { Link, useParams } from "react-router-dom";
import { useGroup, useLeaderboard, useMembership } from "../api/queries";
import { PageHeader } from "../components/layout/PageHeader";
import { PageErrorState } from "../components/layout/PageErrorState";
import { Avatar } from "../components/ui/Avatar";
import { Pill } from "../components/ui/Pill";
import { avatarColorClass } from "../utils/avatarColor";
import { buildBoards, type Board } from "../components/leaderboard/buildBoards";

const RANK_MEDAL: Record<number, string> = { 1: "🥇", 2: "🥈", 3: "🥉" };

function rankBadge(rank: number): string {
  return RANK_MEDAL[rank] ?? `#${rank}`;
}

function valueLabel(boardKey: string, value: number): string {
  if (boardKey === "editionsAnswered") return `${value} edition${value === 1 ? "" : "s"}`;
  // Streaks count published editions, whatever the cadence (`lifecycle.md` §5.3).
  if (boardKey === "longestStreak") return `${value} in a row`;
  if (boardKey === "questionsAsked") return `${value} question${value === 1 ? "" : "s"}`;
  if (boardKey === "commentsWritten") return `${value} comment${value === 1 ? "" : "s"}`;
  return `${value} reaction${value === 1 ? "" : "s"}`;
}

export function LeaderboardPage() {
  const { groupId = "" } = useParams();
  const leaderboard = useLeaderboard(groupId);
  const group = useGroup(groupId);
  const membership = useMembership(groupId);

  if (leaderboard.isLoading || group.isLoading) return <LoadingShell groupId={groupId} />;

  if (leaderboard.isError || group.isError) {
    const error = leaderboard.error ?? group.error;
    return (
      <PageErrorState
        title="Leaderboard"
        back={`/g/${groupId}`}
        message={error instanceof Error ? error.message : "Couldn't load the leaderboard."}
        onRetry={() => {
          leaderboard.refetch();
          group.refetch();
        }}
      />
    );
  }

  if (!leaderboard.data || !group.data) return null;

  const boards = buildBoards(leaderboard.data, group.data.members);

  return (
    <div className="pb-12">
      <PageHeader
        eyebrow={`${membership?.groupName ?? group.data.name} · superlatives`}
        title="Leaderboard"
        back={`/g/${groupId}`}
      />

      {boards.length === 0 ? (
        <div className="px-5 pt-10 text-center text-inkmuted">
          <div className="mb-3 text-4xl">🏅</div>
          <p className="font-semibold text-ink">
            Superlatives show up once your group&apos;s first edition is out.
          </p>
        </div>
      ) : (
        <div className="space-y-4 px-5 pt-5">
          {boards.map((board) => (
            <BoardCard key={board.key} board={board} groupId={groupId} />
          ))}
        </div>
      )}
    </div>
  );
}

function LoadingShell({ groupId }: { groupId: string }) {
  return (
    <div className="pb-12">
      <PageHeader title="Leaderboard" eyebrow="Loading…" back={`/g/${groupId}`} />
      <div className="space-y-3 px-5 pt-6">
        {[0, 1, 2].map((i) => (
          <div key={i} className="h-28 animate-pulse rounded-3xl border border-line bg-white" />
        ))}
      </div>
    </div>
  );
}

function BoardCard({ board, groupId }: { board: Board; groupId: string }) {
  return (
    <section className="rounded-3xl border border-line bg-white p-5 shadow-soft">
      <h2 className="flex items-center gap-2 font-display text-base font-bold">
        <span aria-hidden>{board.icon}</span>
        {board.title}
      </h2>
      <ul className="mt-3 space-y-3">
        {board.kind === "topAnswer"
          ? board.entries.map((entry) => (
              <li key={entry.responseId}>
                <Link
                  to={`/g/${groupId}/n/${entry.cycleId}`}
                  className="-m-2 flex items-start gap-3 rounded-2xl p-2 transition hover:bg-cream"
                >
                  <span
                    className="mt-1 w-6 flex-shrink-0 text-center"
                    aria-label={`Rank ${entry.rank}`}
                  >
                    {rankBadge(entry.rank)}
                  </span>
                  <Avatar
                    name={entry.displayName}
                    colorClassName={avatarColorClass(entry.avatarColor)}
                    url={entry.avatarUrl}
                    size="sm"
                  />
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm font-semibold text-ink">
                      {entry.displayName}
                    </div>
                    <div className="truncate text-xs text-inkmuted">{entry.questionPrompt}</div>
                  </div>
                  <Pill tone="coral" className="flex-shrink-0">
                    {entry.reactionCount} ❤️
                  </Pill>
                </Link>
              </li>
            ))
          : board.entries.map((entry) => (
              <li key={entry.userId} className="flex items-center gap-3">
                <span className="w-6 flex-shrink-0 text-center" aria-label={`Rank ${entry.rank}`}>
                  {rankBadge(entry.rank)}
                </span>
                <Avatar
                  name={entry.displayName}
                  colorClassName={avatarColorClass(entry.avatarColor)}
                  url={entry.avatarUrl}
                  size="sm"
                />
                <div className="min-w-0 flex-1 truncate text-sm font-semibold text-ink">
                  {entry.displayName}
                </div>
                <div className="flex-shrink-0 text-right text-xs text-inkmuted">
                  <div className="font-semibold text-ink">{valueLabel(board.key, entry.value)}</div>
                  {board.key === "longestStreak" && entry.currentStreak > 0 && (
                    <div>current: {entry.currentStreak}</div>
                  )}
                </div>
              </li>
            ))}
      </ul>
    </section>
  );
}
