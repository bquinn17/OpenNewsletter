import { Link } from "react-router-dom";
import clsx from "clsx";
import { ArrowRight } from "lucide-react";
import { editionLabel, shortCountdown } from "../../utils/dates";
import { gradientClasses, type GradientSlug } from "../../utils/gradient";
import type { components } from "../../types/api";

type NewsletterSummary = components["schemas"]["NewsletterSummary"];
type CycleCadence = components["schemas"]["CycleCadence"];

type Props = {
  groupId: string;
  timezone: string;
  frequency: CycleCadence["frequency"];
  gradient: GradientSlug;
  newsletter: NewsletterSummary;
};

/**
 * The open edition's hero card on Home (`docs/design/frontend.md` §7.1): it has a
 * deadline and needs the member's answers, so it stands out on the group's
 * gradient instead of sitting as a plain row like past editions.
 */
export function OpenEditionHero({ groupId, timezone, frequency, gradient, newsletter }: Props) {
  const { title, tileTop, tileBottom } = editionLabel(newsletter, timezone, frequency);
  const remaining = Math.max(newsletter.questionCount - newsletter.myPublishedCount, 0);
  const allAnswered = remaining === 0;
  const pct =
    newsletter.questionCount > 0
      ? Math.round((newsletter.myPublishedCount / newsletter.questionCount) * 100)
      : 0;
  const countdown = `${shortCountdown(newsletter.responseCloseAt)} left`;
  const cta = allAnswered ? "Open the edition" : `Answer ${remaining} more`;

  return (
    <Link
      to={`/g/${groupId}/n/${newsletter.cycleId}`}
      viewTransition
      aria-label={`${title} · ${countdown} · you've published ${newsletter.myPublishedCount} of ${newsletter.questionCount}. ${cta}`}
      className={clsx(
        "block rounded-3xl p-5 shadow-pop transition motion-safe:active:scale-[.97]",
        gradientClasses(gradient),
      )}
    >
      <div className="flex items-center gap-3">
        <div
          style={{ viewTransitionName: `edition-${newsletter.cycleId}` }}
          className="grid h-12 w-12 shrink-0 place-items-center rounded-2xl bg-white/20 font-display font-bold"
        >
          <div className="text-[10px] uppercase leading-none tracking-widest opacity-80">
            {tileTop}
          </div>
          <div className="mt-0.5 text-sm leading-none">{tileBottom}</div>
        </div>
        <div className="min-w-0 flex-1">
          <div className="font-semibold">{title}</div>
          <div className="text-sm opacity-90">{countdown}</div>
        </div>
      </div>

      <div className="mt-4">
        <div className="flex items-center justify-between text-xs font-semibold opacity-90">
          <span>
            {newsletter.myPublishedCount} of {newsletter.questionCount} answered
          </span>
          <span>{pct}%</span>
        </div>
        <div
          role="progressbar"
          aria-valuenow={pct}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-label="Your progress on this edition"
          className="mt-1.5 h-2 rounded-full bg-white/40"
        >
          <div
            className="h-2 rounded-full bg-ink duration-slow ease-smooth motion-safe:transition-[width]"
            style={{ width: `${pct}%` }}
          />
        </div>
      </div>

      <div className="mt-3 inline-flex items-center gap-1 text-sm font-bold">
        {cta}
        <ArrowRight aria-hidden className="h-4 w-4" />
      </div>
    </Link>
  );
}
