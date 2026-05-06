import { useState } from "react";
import { Avatar } from "../ui/Avatar";
import { useAddComment, useConfig } from "../../api/queries";
import { formatRelative } from "../../utils/dates";
import type { Comment } from "../../api/types";

interface Props {
  comments: Comment[];
  totalCount: number;
  onShowAll: () => void;
  groupId: string;
  cycleId: string;
  questionId: string;
  responseId: string;
}

export function CommentList({
  comments,
  totalCount,
  onShowAll,
  groupId,
  cycleId,
  questionId,
  responseId,
}: Props) {
  const { data: config } = useConfig();
  const [draft, setDraft] = useState("");
  const addComment = useAddComment(groupId, cycleId);
  const hidden = totalCount - comments.length;

  const submit = () => {
    const body = draft.trim();
    if (!body) return;
    addComment.mutate({ questionId, responseId, body });
    setDraft("");
  };

  return (
    <div className="mt-4 pt-4 border-t border-line space-y-3 text-sm">
      {comments.map((c) => (
        <div key={c.commentId}>
          <div className="flex items-baseline gap-2">
            <span className="font-semibold">{c.authorDisplayName}</span>
            <span className="text-xs text-inkmuted">{formatRelative(c.createdAt)}</span>
            {c.editedAt && <span className="text-xs text-inkmuted italic">(edited)</span>}
          </div>
          <p className="text-ink/80">{c.body}</p>
        </div>
      ))}
      {hidden > 0 && (
        <button onClick={onShowAll} className="text-grape font-semibold text-sm">
          Show {hidden} more comment{hidden === 1 ? "" : "s"}
        </button>
      )}
      <div className="flex items-center gap-2 mt-2">
        <Avatar name={config?.user.displayName ?? "?"} color={config?.user.avatarColor} size="xs" />
        <input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              submit();
            }
          }}
          className="flex-1 bg-cream border border-line rounded-full px-4 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-coral/30"
          placeholder="Add a comment…"
        />
      </div>
    </div>
  );
}
