import { useState } from "react";
import { useConfig, useGroup, useMembership } from "../../api/queries";
import type { components } from "../../types/api";
import { avatarColorClass } from "../../utils/avatarColor";
import { Avatar } from "../ui/Avatar";
import { CommentComposer } from "./CommentComposer";
import { CommentRow } from "./CommentRow";

type S = components["schemas"];
type Comment = S["CommentResponse"];

/** Avatars shown in the collapsed summary row (`docs/design/frontend.md` §7.2). */
const SUMMARY_AVATAR_COUNT = 3;

type Props = {
  comments: Comment[];
  groupId: string;
  cycleId: string;
  questionId: string;
  responseId: string;
  timezone: string;
};

/**
 * Collapsible comment thread + composer for one published answer
 * (`docs/design/engagement.md` §1, §4.3). With one or more comments, the thread collapses by
 * default to a summary row ("💬 N" plus up to three commenter avatars) that expands into the full
 * thread. The composer always shows, and posting expands the thread so the new comment is visible.
 */
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

  const composer = (
    <CommentComposer
      groupId={groupId}
      cycleId={cycleId}
      questionId={questionId}
      responseId={responseId}
      onPosted={() => setExpanded(true)}
    />
  );

  if (comments.length === 0) {
    return <div className="mt-4 border-t border-line pt-4 text-sm">{composer}</div>;
  }

  if (!expanded) {
    const summaryAvatars = comments.slice(-SUMMARY_AVATAR_COUNT);
    return (
      <div className="mt-4 border-t border-line pt-4 text-sm">
        <button
          type="button"
          onClick={() => setExpanded(true)}
          aria-label={`Show ${comments.length} comment${comments.length === 1 ? "" : "s"}`}
          className="flex items-center gap-2 font-semibold text-grape-ink"
        >
          <span aria-hidden>💬 {comments.length}</span>
          <span className="flex -space-x-2">
            {summaryAvatars.map((comment) => {
              const author = group?.members.find((m) => m.userId === comment.authorUserId);
              return (
                <Avatar
                  key={comment.commentId}
                  name={comment.displayName}
                  size="xs"
                  colorClassName={author ? avatarColorClass(author.avatarColor) : undefined}
                  url={author?.avatarUrl}
                  ringed
                />
              );
            })}
          </span>
        </button>
        {composer}
      </div>
    );
  }

  return (
    <div className="mt-4 space-y-3 border-t border-line pt-4 text-sm">
      {comments.map((comment) => {
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

      {composer}
    </div>
  );
}
