import { Camera } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { apiErrorMessage } from "../../api/client";
import { useAddComment } from "../../api/mutations";
import { Button } from "../ui/Button";
import { ImageUploader, type UploaderImage } from "../responses/ImageUploader";
import { COUNTER_THRESHOLD, MAX_BODY_CHARS } from "./commentFormatting";

type Props = {
  groupId: string;
  cycleId: string;
  questionId: string;
  responseId: string;
  onPosted?: () => void;
};

/**
 * Collapsed to a one-line "Add a comment…" button (`docs/design/frontend.md` §7.2). Tapping it
 * expands a focused textarea plus a photo button; the image dropzone only appears once the photo
 * button is tapped. Collapses back on a successful post or Cancel.
 */
export function CommentComposer({ groupId, cycleId, questionId, responseId, onPosted }: Props) {
  const [expanded, setExpanded] = useState(false);
  const [showUploader, setShowUploader] = useState(false);
  const [body, setBody] = useState("");
  const [images, setImages] = useState<UploaderImage[]>([]);
  // Bumped after a post to remount the uploader: it owns its image list, and
  // the posted image would otherwise stay in its single slot.
  const [uploaderKey, setUploaderKey] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const addComment = useAddComment(groupId, cycleId, questionId);

  useEffect(() => {
    if (expanded) textareaRef.current?.focus();
  }, [expanded]);

  const readyImage = images.find((img) => img.status === "ready");
  const uploading = images.some((img) => img.status === "uploading" || img.status === "processing");
  const trimmed = body.trim();
  const canPost = (!!trimmed || !!readyImage) && !uploading && !addComment.isPending;

  function collapse() {
    setExpanded(false);
    setShowUploader(false);
    setBody("");
    setImages([]);
    setUploaderKey((k) => k + 1);
    setError(null);
  }

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
      collapse();
      onPosted?.();
    } catch (err) {
      setError(apiErrorMessage(err, "Couldn't post your comment — try again."));
    }
  }

  if (!expanded) {
    return (
      <button
        type="button"
        onClick={() => setExpanded(true)}
        className="mt-2 w-full rounded-2xl border border-line bg-cream px-3 py-2 text-left text-sm text-inkmuted motion-safe:active:scale-[.99]"
      >
        Add a comment…
      </button>
    );
  }

  return (
    <div className="mt-2 space-y-2">
      <div className="flex items-start gap-2">
        <label htmlFor={`compose-comment-${responseId}`} className="sr-only">
          Add a comment
        </label>
        <textarea
          ref={textareaRef}
          id={`compose-comment-${responseId}`}
          value={body}
          onChange={(e) => setBody(e.target.value.slice(0, MAX_BODY_CHARS))}
          placeholder="Add a comment…"
          maxLength={MAX_BODY_CHARS}
          className="min-h-[44px] flex-1 rounded-2xl border border-line bg-cream px-3 py-2 text-sm focus:ring-2 focus:ring-coral/30"
        />
        <button
          type="button"
          onClick={() => setShowUploader(true)}
          aria-label="Add a photo"
          aria-pressed={showUploader}
          className="grid h-9 w-9 flex-shrink-0 place-items-center rounded-full border border-line bg-cream text-inkmuted motion-safe:active:scale-[.97]"
        >
          <Camera aria-hidden className="h-4 w-4" />
        </button>
      </div>
      {body.length > COUNTER_THRESHOLD && (
        <div className="text-right text-xs text-inkmuted">
          {body.length}/{MAX_BODY_CHARS}
        </div>
      )}
      {showUploader && (
        <ImageUploader
          key={uploaderKey}
          groupId={groupId}
          cycleId={cycleId}
          questionId={questionId}
          purpose="comment"
          maxImages={1}
          onChange={setImages}
        />
      )}
      {error && (
        <p role="alert" className="text-xs font-semibold text-danger">
          {error}
        </p>
      )}
      <div className="flex justify-end gap-2">
        <button type="button" onClick={collapse} className="text-xs font-semibold text-inkmuted">
          Cancel
        </button>
        <Button type="button" size="sm" onClick={() => void submit()} disabled={!canPost}>
          {addComment.isPending ? "Posting…" : "Post"}
        </Button>
      </div>
    </div>
  );
}
