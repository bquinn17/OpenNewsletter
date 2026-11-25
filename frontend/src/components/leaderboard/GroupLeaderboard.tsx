import { Link } from "react-router-dom";
import { useGroup, useLeaderboard } from "../../api/queries";
import { Avatar } from "../ui/Avatar";
import { Button } from "../ui/Button";
import { Pill } from "../ui/Pill";
import { avatarColorClass } from "../../utils/avatarColor";
import { buildBoards, type Board } from "./buildBoards";

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

/** Home's Leaderboard tab: the group's superlatives (`docs/design/frontend.md` §7.9). */
export function GroupLeaderboard({ groupId }: { groupId: string }) {
  const leaderboard = useLeaderboard(groupId);
  const group = useGroup(groupId);

  if (leaderboard.isLoading || group.isLoading) {
    return (
      <div className="space-y-3" aria-label="Loading the leaderboard">
        {[0, 1, 2].map((i) => (
          <div key={i} className="h-28 animate-pulse rounded-3xl border border-line bg-white" />
        ))}
      </div>
    );
  }

  if (leaderboard.isError || group.isError || !leaderboard.data || !group.data) {
    return (
      <div className="py-6 text-center">
        <p className="text-sm text-coral">Couldn&apos;t load the leaderboard.</p>
        <Button
          className="mt-3"
          size="sm"
          variant="soft"
          onClick={() => {
            void leaderboard.refetch();
            void group.refetch();
          }}
        >
          Try again
        </Button>
      </div>
    );
  }

  const boards = buildBoards(leaderboard.data, group.data.members);

  if (boards.length === 0) {
    return (
      <div className="pt-6 text-center text-inkmuted">
        <div className="mb-3 text-4xl">🏅</div>
        <p className="font-semibold text-ink">
          Superlatives show up once your group&apos;s first edition is out.
        </p>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {boards.map((board) => (
        <BoardCard key={board.key} board={board} groupId={groupId} />
      ))}
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
