import clsx from "clsx";
import { Avatar } from "../ui/Avatar";
import { AskedBy } from "../newsletter/AskedBy";
import { avatarColorClass } from "../../utils/avatarColor";
import { formatInZone } from "../../utils/dates";
import type { components } from "../../types/api";

type CandidateItemResponse = components["schemas"]["CandidateItemResponse"];

type Props = {
  item: CandidateItemResponse;
  timezone: string;
  disabled: boolean;
  pending: boolean;
  onToggleVote: () => void;
};

export function CandidateCard({ item, timezone, disabled, pending, onToggleVote }: Props) {
  return (
    <article className="rounded-3xl border border-line bg-white p-5 shadow-soft">
      <div className="flex items-start gap-3">
        <button
          type="button"
          onClick={onToggleVote}
          disabled={disabled || pending}
          aria-pressed={item.votedByMe}
          aria-label={
            item.votedByMe ? `Remove your vote for "${item.prompt}"` : `Upvote "${item.prompt}"`
          }
          className={clsx(
            "flex h-14 w-12 shrink-0 flex-col items-center justify-center rounded-2xl font-bold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-coral/50 disabled:cursor-not-allowed disabled:opacity-50",
            item.votedByMe
              ? "bg-coral text-white shadow-pop"
              : "border border-line bg-cream text-inkmuted hover:border-ink",
          )}
        >
          <span className="text-[10px] uppercase tracking-wider opacity-80">
            {item.votedByMe ? "votes" : "vote"}
          </span>
          <span className="text-lg leading-none">{item.votedByMe ? item.voteCount : "↑"}</span>
        </button>

        <div className="min-w-0 flex-1">
          {item.kind === "poll" && (
            <span className="text-[10px] font-bold uppercase tracking-widest text-grape">
              Poll · {item.pollOptions?.length ?? 0} options
            </span>
          )}

          <div className="mt-0.5 flex items-center gap-2">
            {item.askedBy && (
              <Avatar
                name={item.askedBy.displayName}
                colorClassName={avatarColorClass(item.askedBy.avatarColor)}
                url={item.askedBy.avatarUrl}
                size="xs"
              />
            )}
            <AskedBy askedBy={item.askedBy} isAnonymous={item.isAnonymous} />
          </div>

          <p className="mt-0.5 font-display text-lg leading-snug">{item.prompt}</p>

          {item.kind === "poll" && item.pollOptions && (
            <div className="mt-2 flex flex-wrap gap-1.5">
              {item.pollOptions.map((p) => (
                <span key={p.optionId} className="rounded-full bg-cream px-2 py-0.5 text-xs">
                  {p.label}
                </span>
              ))}
            </div>
          )}

          <div className="mt-2 text-xs text-inkmuted">
            {item.voteCount} vote{item.voteCount === 1 ? "" : "s"} · submitted{" "}
            {formatInZone(item.submittedAt, timezone, {
              month: "short",
              day: "numeric",
            })}
            {item.isAnonymous ? " · anonymous" : ""}
          </div>
        </div>
      </div>
    </article>
  );
}
