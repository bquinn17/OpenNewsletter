import { useEffect, useState } from "react";
import { ApiError } from "../../api/client";
import { useAddComment, useDeleteComment, useEditComment } from "../../api/mutations";
import { useConfig, useGroup, useMembership } from "../../api/queries";
import type { components } from "../../types/api";
import { avatarColorClass } from "../../utils/avatarColor";
import { formatInZone, formatRelative } from "../../utils/dates";
import { ensureCookie } from "../../utils/media";
import { MarkdownBody } from "../../utils/markdown";
import { Avatar } from "../ui/Avatar";
import { CdnImage } from "../ui/CdnImage";
import { ConfirmDialog } from "../ui/ConfirmDialog";
import { ImageUploader, type UploaderImage } from "../responses/ImageUploader";

type S = components["schemas"];
type Comment = S["CommentResponse"];

const COLLAPSED_COUNT = 3;
const MAX_BODY_CHARS = 2000;
const COUNTER_THRESHOLD = MAX_BODY_CHARS - 200;

type Props = {
  comments: Comment[];
  groupId: string;
  cycleId: string;
  questionId: string;
  responseId: string;
  timezone: string;
};

function errorDetail(err: unknown, fallback: string): string {
  if (err instanceof ApiError) return err.problem?.detail ?? err.message;
  return fallback;
}

/** Collapsible comment thread + composer for one published answer (`09-engagement.md` §1, §4.3). */
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

  useEffect(() => {
    if (comments.some((c) => c.image)) void ensureCookie(groupId);
  }, [comments, groupId]);

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

function CommentRow({
  comment,
  isMine,
  canDelete,
  avatarColorClassName,
  avatarUrl,
  groupId,
  cycleId,
  questionId,
  responseId,
  timezone,
}: {
  comment: Comment;
  isMine: boolean;
  canDelete: boolean;
  avatarColorClassName?: string;
  avatarUrl?: string | null;
  groupId: string;
  cycleId: string;
  questionId: string;
  responseId: string;
  timezone: string;
}) {
  const [editing, setEditing] = useState(false);
  const [draftBody, setDraftBody] = useState(comment.body);
  const [draftImages, setDraftImages] = useState<UploaderImage[]>([]);
  const [imageRemoved, setImageRemoved] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const editComment = useEditComment(groupId, cycleId, questionId);
  const deleteComment = useDeleteComment(groupId, cycleId, questionId);

  const isDeleted = !!comment.deletedAt;
  const busy = editComment.isPending || deleteComment.isPending;

  function startEdit() {
    setDraftBody(comment.body);
    setDraftImages([]);
    setImageRemoved(false);
    setError(null);
    setEditing(true);
  }

  async function saveEdit() {
    const trimmed = draftBody.trim();
    const readyImage = draftImages.find((img) => img.status === "ready");
    const keepsExistingImage = !!comment.image && !imageRemoved;
    if (!trimmed && !readyImage && !keepsExistingImage) {
      setError("Add some text or an image.");
      return;
    }
    setError(null);
    try {
      await editComment.mutateAsync({
        responseId,
        commentId: comment.commentId,
        body: {
          body: trimmed,
          ...(imageRemoved
            ? { imageMediaId: null }
            : readyImage
              ? { imageMediaId: readyImage.imageId }
              : {}),
        },
      });
      setEditing(false);
    } catch (err) {
      setError(errorDetail(err, "Couldn't save your edit — try again."));
    }
  }

  async function confirmAndDelete() {
    try {
      await deleteComment.mutateAsync({ responseId, commentId: comment.commentId });
    } finally {
      setConfirmDelete(false);
    }
  }

  return (
    <div>
      <div className="flex items-start gap-2">
        <Avatar
          name={comment.displayName}
          colorClassName={avatarColorClassName}
          url={avatarUrl}
          size="xs"
        />
        <div className="min-w-0 flex-1">
          <div className="flex items-baseline gap-2">
            <span className="font-semibold">{comment.displayName}</span>
            {!isDeleted && (
              <time
                dateTime={comment.createdAt}
                title={formatInZone(comment.createdAt, timezone)}
                className="text-xs text-inkmuted"
              >
                {formatRelative(comment.createdAt)}
              </time>
            )}
            {comment.editedAt && <span className="text-xs italic text-inkmuted">(edited)</span>}
            {!isDeleted && !editing && (isMine || canDelete) && (
              <div className="ml-auto flex gap-2">
                {isMine && (
                  <button
                    type="button"
                    onClick={startEdit}
                    disabled={busy}
                    className="text-xs font-semibold text-grape disabled:opacity-50"
                  >
                    Edit
                  </button>
                )}
                {canDelete && (
                  <button
                    type="button"
                    onClick={() => setConfirmDelete(true)}
                    disabled={busy}
                    className="text-xs font-semibold text-coral disabled:opacity-50"
                  >
                    Delete
                  </button>
                )}
              </div>
            )}
          </div>

          {isDeleted ? (
            <p className="italic text-inkmuted">[deleted]</p>
          ) : editing ? (
            <div className="mt-1">
              <label htmlFor={`edit-comment-${comment.commentId}`} className="sr-only">
                Edit comment
              </label>
              <textarea
                id={`edit-comment-${comment.commentId}`}
                value={draftBody}
                onChange={(e) => setDraftBody(e.target.value.slice(0, MAX_BODY_CHARS))}
                maxLength={MAX_BODY_CHARS}
                disabled={busy}
                className="min-h-[60px] w-full rounded-2xl border border-line bg-cream px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-coral/30"
              />
              {comment.image && !imageRemoved ? (
                <div className="mt-2 flex items-center gap-2">
                  <CdnImage
                    src={comment.image.thumbUrl}
                    groupId={groupId}
                    alt=""
                    className="h-16 w-16 rounded-xl object-cover"
                  />
                  <button
                    type="button"
                    onClick={() => setImageRemoved(true)}
                    disabled={busy}
                    className="text-xs font-semibold text-coral disabled:opacity-50"
                  >
                    Remove image
                  </button>
                </div>
              ) : (
                <div className="mt-2">
                  <ImageUploader
                    groupId={groupId}
                    cycleId={cycleId}
                    questionId={questionId}
                    purpose="comment"
                    maxImages={1}
                    onChange={setDraftImages}
                  />
                </div>
              )}
              {error && (
                <p role="alert" className="mt-1 text-xs font-semibold text-coral">
                  {error}
                </p>
              )}
              <div className="mt-1.5 flex justify-end gap-2">
                <button
                  type="button"
                  onClick={() => setEditing(false)}
                  disabled={busy}
                  className="text-xs font-semibold text-inkmuted disabled:opacity-50"
                >
                  Cancel
                </button>
                <button
                  type="button"
                  onClick={() => void saveEdit()}
                  disabled={busy}
                  className="text-xs font-bold text-grape disabled:opacity-50"
                >
                  {busy ? "Saving…" : "Save"}
                </button>
              </div>
            </div>
          ) : (
            <>
              <MarkdownBody
                body={comment.body}
                images={[]}
                groupId={groupId}
                allowImages={false}
                className="mt-0.5"
              />
              {comment.image && (
                <a
                  href={comment.image.displayUrl}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="mt-1.5 block"
                >
                  <CdnImage
                    src={comment.image.thumbUrl}
                    groupId={groupId}
                    alt={comment.image.caption ?? ""}
                    className="max-h-48 rounded-xl object-cover"
                  />
                </a>
              )}
            </>
          )}
        </div>
      </div>

      <ConfirmDialog
        open={confirmDelete}
        title="Delete this comment?"
        message="This can't be undone."
        confirmLabel="Delete"
        tone="danger"
        busy={deleteComment.isPending}
        onConfirm={() => void confirmAndDelete()}
        onCancel={() => setConfirmDelete(false)}
      />
    </div>
  );
}

function CommentComposer({
  groupId,
  cycleId,
  questionId,
  responseId,
}: {
  groupId: string;
  cycleId: string;
  questionId: string;
  responseId: string;
}) {
  const [body, setBody] = useState("");
  const [images, setImages] = useState<UploaderImage[]>([]);
  const [error, setError] = useState<string | null>(null);
  const addComment = useAddComment(groupId, cycleId, questionId);

  const readyImage = images.find((img) => img.status === "ready");
  const uploading = images.some((img) => img.status === "uploading" || img.status === "processing");
  const trimmed = body.trim();
  const canPost = (!!trimmed || !!readyImage) && !uploading && !addComment.isPending;

  async function submit() {
    if (!canPost) return;
    setError(null);
    try {
      await addComment.mutateAsync({
        responseId,
        body: {
          body: trimmed,
          ...(readyImage ? { imageMediaId: readyImage.imageId } : {}),
        },
      });
      setBody("");
      setImages([]);
    } catch (err) {
      setError(errorDetail(err, "Couldn't post your comment — try again."));
    }
  }

  return (
    <div className="mt-2 space-y-2">
      <label htmlFor={`compose-comment-${responseId}`} className="sr-only">
        Add a comment
      </label>
      <textarea
        id={`compose-comment-${responseId}`}
        value={body}
        onChange={(e) => setBody(e.target.value.slice(0, MAX_BODY_CHARS))}
        placeholder="Add a comment…"
        maxLength={MAX_BODY_CHARS}
        className="min-h-[44px] w-full rounded-2xl border border-line bg-cream px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-coral/30"
      />
      {body.length > COUNTER_THRESHOLD && (
        <div className="text-right text-xs text-inkmuted">
          {body.length}/{MAX_BODY_CHARS}
        </div>
      )}
      <ImageUploader
        groupId={groupId}
        cycleId={cycleId}
        questionId={questionId}
        purpose="comment"
        maxImages={1}
        onChange={setImages}
      />
      {error && (
        <p role="alert" className="text-xs font-semibold text-coral">
          {error}
        </p>
      )}
      <div className="flex justify-end">
        <button
          type="button"
          onClick={() => void submit()}
          disabled={!canPost}
          className="rounded-full bg-grape px-4 py-2 text-xs font-bold text-white disabled:cursor-not-allowed disabled:opacity-40"
        >
          {addComment.isPending ? "Posting…" : "Post"}
        </button>
      </div>
    </div>
  );
}
