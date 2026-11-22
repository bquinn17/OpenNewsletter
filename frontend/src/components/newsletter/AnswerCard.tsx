import clsx from "clsx";
import type { CSSProperties } from "react";
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
  /**
   * This answer's position among the first 8 on the page, for the edition-load stagger
   * (`docs/design/frontend.md` §18) — `undefined` plays no animation.
   */
  riseIndex?: number;
};

/**
 * One published answer to a text question, with its reactions and comments
 * (`docs/design/engagement.md` §4.1). A question's answers share one surface separated by
 * dividers (`docs/design/frontend.md` §7.2), so this renders unbordered content only — the
 * surface and its dividers are the parent's job.
 */
export function AnswerCard({ answer, groupId, cycleId, questionId, timezone, riseIndex }: Props) {
  const { data: group } = useGroup(groupId);
  const member = group?.members.find((m) => m.userId === answer.userId);

  return (
    <article
      className={clsx(
        "py-5 first:pt-0 last:pb-0",
        riseIndex !== undefined &&
          "motion-safe:animate-rise motion-safe:[animation-delay:calc(var(--i)*40ms)]",
      )}
      style={riseIndex !== undefined ? ({ "--i": riseIndex } as CSSProperties) : undefined}
    >
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
        <MarkdownBody body={answer.body} images={answer.images} size="reading" className="mt-3" />
      )}

      <ImageGallery images={answer.images} body={answer.body ?? ""} />

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
