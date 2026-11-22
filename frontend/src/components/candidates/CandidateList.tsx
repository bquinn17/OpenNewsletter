import { useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { flushSync } from "react-dom";
import { CandidateCard } from "./CandidateCard";
import { withViewTransition } from "../../utils/motion";
import type { CandidateSort } from "../../api/queries";
import type { components } from "../../types/api";

type CandidateItemResponse = components["schemas"]["CandidateItemResponse"];

type Props = {
  groupId: string;
  items: CandidateItemResponse[];
  timezone: string;
  myVoteCount: number;
  votesPerUserPerCycle: number;
  pendingQuestionId: string | null;
  /** Only the Top sort reorders as votes change — Recent's order doesn't depend on vote count. */
  sort: CandidateSort;
  onToggleVote: (questionId: string, voted: boolean) => void;
};

function order(items: CandidateItemResponse[]): string {
  return items.map((i) => i.questionId).join(",");
}

export function CandidateList({
  groupId,
  items,
  timezone,
  myVoteCount,
  votesPerUserPerCycle,
  pendingQuestionId,
  sort,
  onToggleVote,
}: Props) {
  // Held back from `items` until the effect below applies it, so a Top-sort reorder (a vote
  // changing the list's order) plays as a view transition instead of jumping
  // (`docs/design/frontend.md` §18) — the DOM still shows the old order at the moment
  // `startViewTransition` snapshots it, and `flushSync` commits the new order synchronously
  // inside its callback so the browser can diff the two.
  const [displayItems, setDisplayItems] = useState(items);
  const prevOrderRef = useRef(order(items));

  useEffect(() => {
    const nextOrder = order(items);
    if (sort === "top" && nextOrder !== prevOrderRef.current) {
      // Deferred a tick so `flushSync` never nests inside this same commit (React warns, and
      // under test's synchronous `act()` it's a hard error) — the old order stays on screen for
      // `startViewTransition` to snapshot either way.
      queueMicrotask(() => withViewTransition(() => flushSync(() => setDisplayItems(items))));
    } else {
      setDisplayItems(items);
    }
    prevOrderRef.current = nextOrder;
  }, [items, sort]);

  if (displayItems.length === 0) {
    return (
      <div className="mt-3 rounded-3xl border-2 border-dashed border-line p-8 text-center text-inkmuted">
        <div className="mb-2 text-3xl">🗳️</div>
        <p className="font-semibold text-ink">No one&apos;s suggested a question yet.</p>
        <p className="mt-1 text-sm">Be the first — your friends will thank you.</p>
        <Link
          to={`/g/${groupId}/upcoming/suggest`}
          className="mt-4 inline-block rounded-full bg-ink px-5 py-2.5 text-sm font-semibold text-cream"
        >
          Suggest a question
        </Link>
      </div>
    );
  }

  const atCap = myVoteCount >= votesPerUserPerCycle;

  return (
    <div className="mt-3 space-y-3">
      {displayItems.map((item) => (
        <CandidateCard
          key={item.questionId}
          item={item}
          timezone={timezone}
          disabled={!item.votedByMe && atCap}
          pending={pendingQuestionId === item.questionId}
          onToggleVote={() => onToggleVote(item.questionId, !item.votedByMe)}
        />
      ))}
    </div>
  );
}
