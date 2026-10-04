import type { components } from "../../types/api";
import { EditionRow } from "./EditionRow";

type NewsletterSummary = components["schemas"]["NewsletterSummary"];

type Props = {
  groupId: string;
  timezone: string;
  /** Newest first, as `GET /groups/{g}/newsletters` returns them. */
  items: NewsletterSummary[];
};

export function GroupEditions({ groupId, timezone, items }: Props) {
  // While a cycle is open, next month's voting cycle already exists
  // (`06` §5.2), so both can be current at once — show each.
  const open = items.find((n) => n.status === "open");
  const voting = items.find((n) => n.status === "voting");
  const published = items.filter((n) => n.status === "published");
  const latest = published[0];
  const morePast = published.slice(1, 4);

  if (!open && !voting && !latest) {
    return <p className="text-sm text-inkmuted">No editions yet.</p>;
  }

  return (
    <div className="space-y-2">
      {open && <EditionRow groupId={groupId} timezone={timezone} newsletter={open} />}
      {voting && <EditionRow groupId={groupId} timezone={timezone} newsletter={voting} />}
      {latest && <EditionRow groupId={groupId} timezone={timezone} newsletter={latest} />}
      {morePast.map((n) => (
        <EditionRow key={n.cycleId} groupId={groupId} timezone={timezone} newsletter={n} />
      ))}
    </div>
  );
}
