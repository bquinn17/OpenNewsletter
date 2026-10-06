import { useGroup } from "../../api/queries";
import type { components } from "../../types/api";
import { avatarColorClass } from "../../utils/avatarColor";
import { formatInZone } from "../../utils/dates";
import { MarkdownBody } from "../../utils/markdown";
import { Avatar } from "../ui/Avatar";
import { CommentList } from "./CommentList";
import { ImageGallery } from "./ImageGallery";
import { ReactionBar } from "./ReactionBar";

type Props = {
  answer: components["schemas"]["PublishedAnswerResponse"];
  groupId: string;
  cycleId: string;
  questionId: string;
  timezone: string;
};

/** One published answer to a text question, with its reactions and comments (`09-engagement.md` §4.1). */
export function AnswerCard({ answer, groupId, cycleId, questionId, timezone }: Props) {
  const { data: group } = useGroup(groupId);
  const member = group?.members.find((m) => m.userId === answer.userId);

  return (
    <article className="mb-4 rounded-3xl border border-line bg-white p-5 shadow-soft">
      <header className="flex items-center gap-3">
        <Avatar
          name={member?.displayName ?? answer.displayName}
          colorClassName={member ? avatarColorClass(member.avatarColor) : undefined}
          url={member?.avatarUrl}
        />
        <div className="flex-1">
          <div className="font-semibold">{answer.displayName}</div>
          <div className="text-xs text-inkmuted">
            published {formatInZone(answer.publishedAt, timezone)}
          </div>
        </div>
      </header>

      {answer.body && (
        <MarkdownBody
          body={answer.body}
          images={answer.images}
          groupId={groupId}
          className="mt-3"
        />
      )}

      <ImageGallery images={answer.images} body={answer.body ?? ""} groupId={groupId} />

      <ReactionBar
        groupId={groupId}
        cycleId={cycleId}
        questionId={questionId}
        responseId={answer.responseId}
        reactionGroups={answer.reactionGroups}
      />
      <CommentList
        comments={answer.comments}
        groupId={groupId}
        cycleId={cycleId}
        questionId={questionId}
        responseId={answer.responseId}
        timezone={timezone}
      />
    </article>
  );
}
