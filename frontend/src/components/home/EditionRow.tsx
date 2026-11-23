import { Link } from "react-router-dom";
import clsx from "clsx";
import { cadenceNoun, editionLabel, shortCountdown } from "../../utils/dates";
import { phaseStyle } from "../../utils/phase";
import type { components } from "../../types/api";

type NewsletterSummary = components["schemas"]["NewsletterSummary"];
type CycleCadence = components["schemas"]["CycleCadence"];

type Props = {
  groupId: string;
  /** The group's IANA timezone, so date labels match the group's calendar (`docs/design/frontend.md` §14a). */
  timezone: string;
  frequency: CycleCadence["frequency"];
  newsletter: NewsletterSummary;
  /**
   * Votes left this cycle (`votesPerUserPerCycle − myVoteCount`), shown on a voting-status row
   * (`docs/design/frontend.md` §7.1). `undefined` while the candidates query hasn't loaded yet —
   * the text is hidden rather than guessed.
   */
  votesLeft?: number;
};

export function EditionRow({ groupId, timezone, frequency, newsletter, votesLeft }: Props) {
  const { title, tileTop, tileBottom } = editionLabel(newsletter, timezone, frequency);
  // Home never lists archived editions; style one as published if it slips through.
  const phase = phaseStyle(newsletter.status === "archived" ? "published" : newsletter.status);
  const isVoting = newsletter.status === "voting";
  const href = isVoting ? `/g/${groupId}/upcoming` : `/g/${groupId}/n/${newsletter.cycleId}`;
  // The voting row links to `/upcoming`, not an edition page with a masthead to morph into, so
  // only a row that lands on the edition page carries the shared `view-transition-name`
  // (`docs/design/frontend.md` §18) — unique per cycle, and only one element per cycle gets it.
  const description =
    newsletter.status === "open"
      ? `${shortCountdown(newsletter.responseCloseAt)} left · you've published ${newsletter.myPublishedCount} of ${newsletter.questionCount}`
      : newsletter.status === "voting"
        ? `Suggest and vote on questions for next ${cadenceNoun(frequency)}${
            votesLeft === undefined ? "" : ` · ${votesLeft} vote${votesLeft === 1 ? "" : "s"} left`
          }`
        : `${newsletter.questionCount} question${newsletter.questionCount === 1 ? "" : "s"}${
            // The weekly title is already the publication date, so the suffix would be redundant.
            newsletter.publishedAt && frequency === "monthly"
              ? ` · published ${new Date(newsletter.publishedAt).toLocaleDateString()}`
              : ""
          }`;

  return (
    <Link
      to={href}
      viewTransition={!isVoting}
      className="flex items-center gap-3 rounded-3xl border border-line bg-white p-4 transition hover:border-ink motion-safe:active:scale-[.97]"
    >
      <div
        style={isVoting ? undefined : { viewTransitionName: `edition-${newsletter.cycleId}` }}
        className={clsx(
          "grid h-12 w-12 shrink-0 place-items-center rounded-2xl font-display font-bold",
          phase.chip,
        )}
      >
        <div className="text-[10px] uppercase leading-none tracking-widest opacity-80">
          {tileTop}
        </div>
        <div className="mt-0.5 text-sm leading-none">{tileBottom}</div>
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="font-semibold">{title}</span>
          <span
            className={clsx(
              "rounded-full px-2 py-0.5 text-[10px] font-bold uppercase tracking-widest",
              phase.chip,
            )}
          >
            {phase.label}
          </span>
        </div>
        <div className="text-sm text-inkmuted">{description}</div>
      </div>
    </Link>
  );
}
