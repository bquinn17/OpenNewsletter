import { Link } from "react-router-dom";
import { CandidateCard } from "./CandidateCard";
import type { components } from "../../types/api";

type CandidateItemResponse = components["schemas"]["CandidateItemResponse"];

type Props = {
  groupId: string;
  items: CandidateItemResponse[];
  timezone: string;
  myVoteCount: number;
  votesPerUserPerCycle: number;
  pendingQuestionId: string | null;
  onToggleVote: (questionId: string, voted: boolean) => void;
};

export function CandidateList({
  groupId,
  items,
  timezone,
  myVoteCount,
  votesPerUserPerCycle,
  pendingQuestionId,
  onToggleVote,
}: Props) {
  if (items.length === 0) {
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
      {items.map((item) => (
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
