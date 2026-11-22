import { useEffect, useRef, useState } from "react";
import clsx from "clsx";
import { Avatar } from "../ui/Avatar";
import { AskedBy } from "../newsletter/AskedBy";
import { avatarColorClass } from "../../utils/avatarColor";
import { formatInZone } from "../../utils/dates";
import { prefersReducedMotion } from "../../utils/motion";
import type { components } from "../../types/api";

type CandidateItemResponse = components["schemas"]["CandidateItemResponse"];

type Props = {
  item: CandidateItemResponse;
  timezone: string;
  disabled: boolean;
  pending: boolean;
  onToggleVote: () => void;
};

const SPARK_COUNT = 6;
const SPARK_DURATION_MS = 450;

export function CandidateCard({ item, timezone, disabled, pending, onToggleVote }: Props) {
  // Springs the vote tile and bursts a few coral sparks on a fresh vote, but not on an un-vote
  // (`docs/design/frontend.md` §18). `wasVotedRef` tracks the previous prop value across renders
  // so the effect only fires on the false→true transition, and `popKey` forces the button to
  // remount so the animation restarts on every vote, not just the first.
  const wasVotedRef = useRef(item.votedByMe);
  const [popKey, setPopKey] = useState(0);
  const [sparkBurst, setSparkBurst] = useState<number | null>(null);
  const sparkRefs = useRef<(HTMLSpanElement | null)[]>([]);

  useEffect(() => {
    if (item.votedByMe && !wasVotedRef.current) {
      setPopKey((k) => k + 1);
      if (!prefersReducedMotion()) setSparkBurst(Date.now());
    }
    wasVotedRef.current = item.votedByMe;
  }, [item.votedByMe]);

  useEffect(() => {
    if (sparkBurst === null) return;
    sparkRefs.current.forEach((el, i) => {
      if (!el || typeof el.animate !== "function") return;
      const angle = (360 / SPARK_COUNT) * i;
      const radians = (angle * Math.PI) / 180;
      const dx = Math.round(Math.cos(radians) * 26);
      const dy = Math.round(Math.sin(radians) * 26);
      el.animate(
        [
          { transform: "translate(-50%, -50%) scale(1)", opacity: 1 },
          { transform: `translate(${dx - 50}%, ${dy - 50}%) scale(0.3)`, opacity: 0 },
        ],
        { duration: SPARK_DURATION_MS, easing: "cubic-bezier(.2,.8,.2,1)" },
      );
    });
    const timer = setTimeout(() => setSparkBurst(null), SPARK_DURATION_MS);
    return () => clearTimeout(timer);
  }, [sparkBurst]);

  return (
    <article
      style={{ viewTransitionName: `candidate-${item.questionId}` }}
      className="rounded-3xl border border-line bg-white p-5 shadow-soft"
    >
      <div className="flex items-start gap-3">
        <div className="relative shrink-0">
          <button
            key={popKey}
            type="button"
            onClick={onToggleVote}
            disabled={disabled || pending}
            aria-pressed={item.votedByMe}
            aria-label={
              item.votedByMe ? `Remove your vote for "${item.prompt}"` : `Upvote "${item.prompt}"`
            }
            className={clsx(
              "flex h-14 w-12 flex-col items-center justify-center rounded-2xl font-bold transition disabled:cursor-not-allowed disabled:opacity-50 motion-safe:active:scale-[.97]",
              item.votedByMe
                ? "bg-coral text-ink shadow-pop"
                : "border border-line bg-cream text-inkmuted hover:border-ink",
              popKey > 0 && item.votedByMe && "motion-safe:animate-pop-in",
            )}
          >
            <span className="text-[10px] uppercase tracking-wider opacity-80">
              {item.votedByMe ? "votes" : "vote"}
            </span>
            <span className="text-lg leading-none">{item.votedByMe ? item.voteCount : "↑"}</span>
          </button>
          {sparkBurst !== null && (
            <div aria-hidden="true" className="pointer-events-none absolute inset-0">
              {Array.from({ length: SPARK_COUNT }, (_, i) => (
                <span
                  key={`${sparkBurst}-${i}`}
                  ref={(el) => {
                    sparkRefs.current[i] = el;
                  }}
                  className="absolute left-1/2 top-1/2 h-1.5 w-1.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-coral"
                />
              ))}
            </div>
          )}
        </div>

        <div className="min-w-0 flex-1">
          {item.kind === "poll" && (
            <span className="text-[10px] font-bold uppercase tracking-widest text-grape-ink">
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
