import { useCandidates } from "../../api/queries";
import type { components } from "../../types/api";
import { isCurrentEdition } from "../../utils/dates";
import type { GradientSlug } from "../../utils/gradient";
import { EditionRow } from "./EditionRow";
import { OpenEditionHero } from "./OpenEditionHero";

type NewsletterSummary = components["schemas"]["NewsletterSummary"];
type CycleCadence = components["schemas"]["CycleCadence"];

type Props = {
  groupId: string;
  timezone: string;
  frequency: CycleCadence["frequency"];
  /** The group's gradient, for the open edition's hero card (`docs/design/frontend.md` §7.1). */
  gradient: GradientSlug;
  /** Newest first, as `GET /groups/{g}/newsletters` returns them. */
  items: NewsletterSummary[];
};

const MAX_PAST = 3;

export function GroupEditions({ groupId, timezone, frequency, gradient, items }: Props) {
  // Newest first, grouped by phase (`docs/design/frontend.md` §7.1). While a cycle
  // is open, the next edition's voting cycle already exists (`docs/design/lifecycle.md`
  // §5.2), so both can be current at once — show each.
  const chronological = [...items].sort((a, b) => b.responseOpenAt.localeCompare(a.responseOpenAt));
  const published = chronological.filter((n) => n.status === "published");
  const voting = chronological.filter((n) => n.status === "voting");
  const open = chronological.filter((n) => n.status === "open");

  // The voting row's "N votes left" (§7.1) reuses `CandidatesPage`'s own query/cache key, so the
  // two never drift — only fetched when there's actually a voting cycle to show it on.
  const candidates = useCandidates(voting.length > 0 ? groupId : undefined);
  const votesLeft = candidates.data
    ? Math.max(candidates.data.votesPerUserPerCycle - candidates.data.myVoteCount, 0)
    : undefined;

  const sections: { title: string; items: NewsletterSummary[] }[] = [
    { title: "Taking suggestions", items: voting },
    { title: "Awaiting responses", items: open },
    {
      title: frequency === "weekly" ? "This week's edition" : "This month's edition",
      items: published.filter((n) => isCurrentEdition(n, timezone, frequency)),
    },
    {
      title: "Past editions",
      items: published.filter((n) => !isCurrentEdition(n, timezone, frequency)).slice(0, MAX_PAST),
    },
  ].filter((s) => s.items.length > 0);

  if (sections.length === 0) {
    return <p className="text-sm text-inkmuted">No editions yet.</p>;
  }

  return (
    <div className="space-y-6">
      {sections.map((section) => (
        <section key={section.title} aria-label={section.title}>
          <h2 className="mb-2 px-1 text-xs font-bold uppercase tracking-widest text-inkmuted">
            {section.title}
          </h2>
          <div className="space-y-2">
            {section.items.map((n) =>
              n.status === "open" ? (
                <OpenEditionHero
                  key={n.cycleId}
                  groupId={groupId}
                  timezone={timezone}
                  frequency={frequency}
                  gradient={gradient}
                  newsletter={n}
                />
              ) : (
                <EditionRow
                  key={n.cycleId}
                  groupId={groupId}
                  timezone={timezone}
                  frequency={frequency}
                  newsletter={n}
                  votesLeft={n.status === "voting" ? votesLeft : undefined}
                />
              ),
            )}
          </div>
        </section>
      ))}
    </div>
  );
}
