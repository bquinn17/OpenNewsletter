import { useState } from "react";
import { apiErrorMessage } from "../../api/client";
import { useAddComment } from "../../api/mutations";
import { ImageUploader, type UploaderImage } from "../responses/ImageUploader";
import { COUNTER_THRESHOLD, MAX_BODY_CHARS } from "./commentFormatting";

type Props = {
  groupId: string;
  cycleId: string;
  questionId: string;
  responseId: string;
};

export function CommentComposer({ groupId, cycleId, questionId, responseId }: Props) {
  const [body, setBody] = useState("");
  const [images, setImages] = useState<UploaderImage[]>([]);
  // Bumped after a post to remount the uploader: it owns its image list, and
  // the posted image would otherwise stay in its single slot.
  const [uploaderKey, setUploaderKey] = useState(0);
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
      setUploaderKey((k) => k + 1);
    } catch (err) {
      setError(apiErrorMessage(err, "Couldn't post your comment — try again."));
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
        key={uploaderKey}
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
