import { useRef, useState } from "react";
import type { components } from "../../types/api";
import { MarkdownBody } from "../../utils/markdown";
import { ImageUploader, type UploaderImage } from "./ImageUploader";

type S = components["schemas"];

type Props = {
  groupId: string;
  cycleId: string;
  questionId: string;
  body: string;
  onBodyChange: (body: string) => void;
  onBlur: () => void;
  onImageMediaIdsChange: (imageMediaIds: string[]) => void;
  onUploadingChange?: (uploading: boolean) => void;
  initialImages?: S["ImageMediaResponse"][];
  readOnly: boolean;
};

function escapeForRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function removeImageToken(body: string, imageId: string): string {
  const re = new RegExp(`!\\[[^\\]]*\\]\\(image:${escapeForRegExp(imageId)}\\)\\n?`, "g");
  return body.replace(re, "").replace(/\n{3,}/g, "\n\n");
}

function insertImageToken(
  body: string,
  imageId: string,
  textarea: HTMLTextAreaElement | null,
): string {
  const pos = textarea?.selectionStart ?? body.length;
  const before = body.slice(0, pos);
  const after = body.slice(pos);
  const leading = before.length > 0 && !before.endsWith("\n") ? "\n" : "";
  return `${before}${leading}![](image:${imageId})\n${after}`;
}

/** The markdown textarea + image uploader for a text response (`04-frontend-architecture.md` §7.3, §8.4). */
export function ResponseEditor({
  groupId,
  cycleId,
  questionId,
  body,
  onBodyChange,
  onBlur,
  onImageMediaIdsChange,
  onUploadingChange,
  initialImages,
  readOnly,
}: Props) {
  const [preview, setPreview] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const knownIdsRef = useRef<Set<string>>(new Set((initialImages ?? []).map((i) => i.imageId)));
  const readyIdsRef = useRef<Set<string>>(
    new Set((initialImages ?? []).filter((i) => i.status === "ready").map((i) => i.imageId)),
  );

  const [liveImages, setLiveImages] = useState<UploaderImage[]>(() =>
    (initialImages ?? []).map((i) => ({
      imageId: i.imageId,
      status: i.status === "pending" ? "processing" : i.status,
      altText: i.caption ?? "",
      thumbUrl: i.thumbUrl,
      displayUrl: i.displayUrl,
    })),
  );

  const wordCount = body.trim() ? body.trim().split(/\s+/).length : 0;

  const previewImages = liveImages.map((i) => ({
    imageId: i.imageId,
    displayUrl: i.displayUrl,
    thumbUrl: i.thumbUrl,
  }));

  function handleUploaderChange(images: UploaderImage[]) {
    setLiveImages(images);
    const currentIds = new Set(images.map((i) => i.imageId));
    let next = body;

    for (const id of knownIdsRef.current) {
      if (!currentIds.has(id)) next = removeImageToken(next, id);
    }

    const readyIds = new Set<string>();
    for (const image of images) {
      if (image.status === "ready") {
        readyIds.add(image.imageId);
        if (!readyIdsRef.current.has(image.imageId)) {
          next = insertImageToken(next, image.imageId, textareaRef.current);
        }
      }
    }

    knownIdsRef.current = currentIds;
    readyIdsRef.current = readyIds;
    if (next !== body) onBodyChange(next);
    onImageMediaIdsChange([...readyIds].slice(0, 10));
    onUploadingChange?.(images.some((i) => i.status === "uploading" || i.status === "processing"));
  }

  return (
    <div className="mt-4 px-5">
      <div className="overflow-hidden rounded-3xl border border-line bg-white shadow-soft">
        <div className="flex items-center justify-end gap-1 border-b border-line px-3 py-2 text-sm text-inkmuted">
          <button
            type="button"
            onClick={() => setPreview((p) => !p)}
            className={`rounded-md px-2 py-1 text-xs font-medium ${
              preview ? "bg-ink text-cream" : "hover:bg-cream"
            }`}
          >
            {preview ? "Edit" : "Preview"}
          </button>
        </div>
        {preview ? (
          <div className="min-h-[200px] px-4 py-4">
            {body.trim() ? (
              <MarkdownBody body={body} images={previewImages} groupId={groupId} />
            ) : (
              <p className="italic text-inkmuted">Nothing to preview yet.</p>
            )}
          </div>
        ) : (
          <textarea
            ref={textareaRef}
            value={body}
            onChange={(e) => onBodyChange(e.target.value)}
            onBlur={onBlur}
            readOnly={readOnly}
            className="min-h-[200px] w-full resize-none bg-transparent px-4 py-4 text-[15px] leading-relaxed focus:outline-none"
            placeholder="Tell us about it…"
          />
        )}
        <div className="border-t border-line p-3">
          <ImageUploader
            groupId={groupId}
            cycleId={cycleId}
            questionId={questionId}
            purpose="response"
            initialImages={initialImages}
            onChange={handleUploaderChange}
          />
        </div>
      </div>
      <div className="mt-2 text-right text-xs text-inkmuted">
        {wordCount} word{wordCount === 1 ? "" : "s"}
      </div>
    </div>
  );
}
