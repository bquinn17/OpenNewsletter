import { useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeSanitize from "rehype-sanitize";
import { Avatar } from "../ui/Avatar";
import { ReactionBar } from "./ReactionBar";
import { CommentList } from "./CommentList";
import { formatRelative } from "../../utils/dates";
import type { AnswerView } from "../../api/types";

interface Props {
  answer: AnswerView;
  groupId: string;
  cycleId: string;
  questionId: string;
}

export function AnswerCard({ answer, groupId, cycleId, questionId }: Props) {
  const [showAllComments, setShowAllComments] = useState(false);
  const visibleComments = showAllComments ? answer.comments : answer.comments.slice(0, 2);
  return (
    <article className="bg-white rounded-3xl border border-line shadow-soft p-5 mb-4">
      <header className="flex items-center gap-3">
        <Avatar name={answer.displayName} color={answer.avatarColor} />
        <div className="flex-1">
          <div className="font-semibold">{answer.displayName}</div>
          <div className="text-xs text-inkmuted">published {formatRelative(answer.publishedAt)}</div>
        </div>
      </header>
      <div className="prose prose-sm max-w-none mt-3 text-[15px] leading-relaxed">
        <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeSanitize]}>
          {answer.body}
        </ReactMarkdown>
      </div>

      {answer.images.length > 0 && (
        <div
          className={
            answer.images.length === 1
              ? "mt-3"
              : answer.images.length === 2
                ? "mt-3 grid grid-cols-2 gap-2"
                : "mt-3 grid grid-cols-2 gap-2"
          }
        >
          {answer.images.map((img, i) => (
            <div
              key={img.imageId}
              aria-label={img.alt ?? "image"}
              className={`ph-img rounded-2xl ${
                answer.images.length === 1
                  ? "aspect-[3/2]"
                  : answer.images.length === 3 && i === 2
                    ? "aspect-[2/1] col-span-2"
                    : "aspect-square"
              }`}
            />
          ))}
        </div>
      )}

      <ReactionBar
        reactions={answer.reactionGroups}
        groupId={groupId}
        cycleId={cycleId}
        questionId={questionId}
        responseId={answer.responseId}
      />

      <CommentList
        comments={visibleComments}
        totalCount={answer.comments.length}
        onShowAll={() => setShowAllComments(true)}
        groupId={groupId}
        cycleId={cycleId}
        questionId={questionId}
        responseId={answer.responseId}
      />
    </article>
  );
}
