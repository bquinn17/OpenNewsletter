import { Link } from "react-router-dom";
import { cycleLabels, shortCountdown } from "../../utils/dates";
import type { components } from "../../types/api";

type NewsletterSummary = components["schemas"]["NewsletterSummary"];

type Props = {
  groupId: string;
  /** The group's IANA timezone, so month labels match the group's calendar (`04` §14a). */
  timezone: string;
  newsletter: NewsletterSummary;
};

export function EditionRow({ groupId, timezone, newsletter }: Props) {
  const { monthLabel, yearLabel } = cycleLabels(newsletter.responseOpenAt, timezone);
  const href =
    newsletter.status === "voting"
      ? `/g/${groupId}/upcoming`
      : `/g/${groupId}/n/${newsletter.cycleId}`;
  const description =
    newsletter.status === "open"
      ? `${shortCountdown(newsletter.responseCloseAt)} left · you've published ${newsletter.myPublishedCount} of ${newsletter.questionCount}`
      : newsletter.status === "voting"
        ? "Suggest and vote on questions for next month"
        : `${newsletter.questionCount} questions${
            newsletter.publishedAt
              ? ` · published ${new Date(newsletter.publishedAt).toLocaleDateString()}`
              : ""
          }`;

  return (
    <Link
      to={href}
      className="flex items-center gap-3 rounded-3xl border border-line bg-white p-4 transition hover:border-ink"
    >
      <div className="grid h-12 w-12 shrink-0 place-items-center rounded-2xl bg-cream font-display font-bold text-ink">
        <div className="text-[10px] uppercase leading-none tracking-widest opacity-80">
          {monthLabel.slice(0, 3)}
        </div>
        <div className="mt-0.5 text-sm leading-none">{yearLabel}</div>
      </div>
      <div className="min-w-0 flex-1">
        <div className="font-semibold">
          {monthLabel} {yearLabel}
        </div>
        <div className="text-sm text-inkmuted">{description}</div>
      </div>
    </Link>
  );
}
