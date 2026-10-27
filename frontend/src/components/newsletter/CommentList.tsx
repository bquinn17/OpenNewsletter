import { useState } from "react";
import { useConfig, useGroup, useMembership } from "../../api/queries";
import type { components } from "../../types/api";
import { avatarColorClass } from "../../utils/avatarColor";
import { CommentComposer } from "./CommentComposer";
import { CommentRow } from "./CommentRow";

type S = components["schemas"];
type Comment = S["CommentResponse"];

const COLLAPSED_COUNT = 3;

type Props = {
  comments: Comment[];
  groupId: string;
  cycleId: string;
  questionId: string;
  responseId: string;
  timezone: string;
};

/** Collapsible comment thread + composer for one published answer (`docs/design/engagement.md` §1, §4.3). */
export function CommentList({
  comments,
  groupId,
  cycleId,
  questionId,
  responseId,
  timezone,
}: Props) {
  const [expanded, setExpanded] = useState(false);
  const { data: config } = useConfig();
  const membership = useMembership(groupId);
  const { data: group } = useGroup(groupId);
  const isAdmin = membership?.role === "admin";
  const callerId = config?.userId ?? null;

  const visible = expanded ? comments : comments.slice(-COLLAPSED_COUNT);
  const hiddenCount = comments.length - visible.length;

  return (
    <div className="mt-4 space-y-3 border-t border-line pt-4 text-sm">
      {hiddenCount > 0 && (
        <button
          type="button"
          onClick={() => setExpanded(true)}
          className="font-semibold text-grape"
        >
          Show all {comments.length} comments
        </button>
      )}

      {visible.map((comment) => {
        const author = group?.members.find((m) => m.userId === comment.authorUserId);
        return (
          <CommentRow
            key={comment.commentId}
            comment={comment}
            isMine={!!callerId && callerId === comment.authorUserId}
            canDelete={isAdmin || (!!callerId && callerId === comment.authorUserId)}
            avatarColorClassName={author ? avatarColorClass(author.avatarColor) : undefined}
            avatarUrl={author?.avatarUrl}
            groupId={groupId}
            cycleId={cycleId}
            questionId={questionId}
            responseId={responseId}
            timezone={timezone}
          />
        );
      })}

      <CommentComposer
        groupId={groupId}
        cycleId={cycleId}
        questionId={questionId}
        responseId={responseId}
      />
    </div>
  );
}
