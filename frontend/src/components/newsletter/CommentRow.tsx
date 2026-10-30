import { useState } from "react";
import { apiErrorMessage } from "../../api/client";
import { useEditComment, useDeleteComment } from "../../api/mutations";
import type { components } from "../../types/api";
import { formatInZone, formatRelative } from "../../utils/dates";
import { thumbSize } from "../../utils/media";
import { MarkdownBody } from "../../utils/markdown";
import { Avatar } from "../ui/Avatar";
import { MediaImage } from "../ui/MediaImage";
import { ConfirmDialog } from "../ui/ConfirmDialog";
import { ImageUploader, type UploaderImage } from "../responses/ImageUploader";
import { PERMANENT_IMAGE_DELETE_MESSAGE } from "../../utils/permanentDelete";
import { MAX_BODY_CHARS } from "./commentFormatting";

type S = components["schemas"];
type Comment = S["CommentResponse"];

/** The attached image's `max-h-48`, in px, so its `width`/`height` scale it rather than crop it. */
const IMAGE_MAX_HEIGHT_PX = 192;

type Props = {
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
};

export function CommentRow({
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
}: Props) {
  const [editing, setEditing] = useState(false);
  const [draftBody, setDraftBody] = useState(comment.body);
  const [draftImages, setDraftImages] = useState<UploaderImage[]>([]);
  const [imageRemoved, setImageRemoved] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [confirmImageRelease, setConfirmImageRelease] = useState(false);
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

  function saveEdit() {
    const trimmed = draftBody.trim();
    const readyImage = draftImages.find((img) => img.status === "ready");
    const keepsExistingImage = !!comment.image && !imageRemoved;
    if (!trimmed && !readyImage && !keepsExistingImage) {
      setError("Add some text or an image.");
      return;
    }
    setError(null);
    // Removing or replacing the attached image deletes it for real once the
    // edit commits (`docs/design/engagement.md` §1, D8), so confirm that first.
    if (comment.image && (imageRemoved || readyImage)) {
      setConfirmImageRelease(true);
      return;
    }
    void commitEdit();
  }

  async function commitEdit() {
    setConfirmImageRelease(false);
    const trimmed = draftBody.trim();
    const readyImage = draftImages.find((img) => img.status === "ready");
    try {
      await editComment.mutateAsync({
        responseId,
        commentId: comment.commentId,
        body: {
          body: trimmed,
          // A freshly uploaded image wins: "Remove image" is how the author
          // gets to the uploader to replace one, so it implies a swap.
          ...(readyImage
            ? { imageMediaId: readyImage.imageId }
            : imageRemoved
              ? { imageMediaId: null }
              : {}),
        },
      });
      setEditing(false);
    } catch (err) {
      setError(apiErrorMessage(err, "Couldn't save your edit — try again."));
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
                  <MediaImage
                    src={comment.image.thumbUrl}
                    alt=""
                    {...thumbSize(comment.image.width, comment.image.height)}
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
                  onClick={saveEdit}
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
                  <MediaImage
                    src={comment.image.thumbUrl}
                    alt={comment.image.caption ?? ""}
                    {...thumbSize(comment.image.width, comment.image.height, IMAGE_MAX_HEIGHT_PX)}
                    className="max-h-48 rounded-xl"
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
        message={
          comment.image
            ? "This can't be undone. This also permanently deletes the attached image. It can't be recovered."
            : "This can't be undone."
        }
        confirmLabel="Delete"
        tone="danger"
        busy={deleteComment.isPending}
        onConfirm={() => void confirmAndDelete()}
        onCancel={() => setConfirmDelete(false)}
      />

      <ConfirmDialog
        open={confirmImageRelease}
        title="Delete the attached image?"
        message={PERMANENT_IMAGE_DELETE_MESSAGE}
        confirmLabel="Delete image and save"
        tone="danger"
        busy={editComment.isPending}
        onConfirm={() => void commitEdit()}
        onCancel={() => setConfirmImageRelease(false)}
      />
    </div>
  );
}
