import { useRef, useState, type ClipboardEvent, type DragEvent } from "react";
import clsx from "clsx";
import { api } from "../../api/client";
import { env } from "../../env";
import type { components } from "../../types/api";
import type { GiphyGif } from "../../giphy/giphyClient";
import { GiphyLookupProvider } from "../../giphy/GiphyLookupProvider";
import { giphyLimitRemainingMs, useGiphyLimit } from "../../state/giphyLimit";
import { MarkdownBody, referencedGiphyIds } from "../../utils/markdown";
import { AttachMenu, type AttachMenuHandle, type AttachMenuItem } from "./AttachMenu";
import { GiphyPicker } from "./GiphyPicker";
import { ImageUploader, type ImageUploaderHandle, type UploaderImage } from "./ImageUploader";

type S = components["schemas"];

const MAX_IMAGES = 10;
const MAX_GIFS_PER_ANSWER = 5;

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
  /**
   * Drops an image (its ID, and its tokens via `transformBody`) from the
   * draft and resolves once that draft has saved — `useResponseEditor`'s
   * `detachImage`.
   */
  onDetachImage: (imageId: string, transformBody: (body: string) => string) => Promise<void>;
};

function escapeForRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function removeImageToken(body: string, imageId: string): string {
  const re = new RegExp(`!\\[[^\\]]*\\]\\(image:${escapeForRegExp(imageId)}\\)\\n?`, "g");
  return body.replace(re, "").replace(/\n{3,}/g, "\n\n");
}

function insertTokenAtCursor(
  body: string,
  token: string,
  textarea: HTMLTextAreaElement | null,
): string {
  const pos = textarea?.selectionStart ?? body.length;
  const before = body.slice(0, pos);
  const after = body.slice(pos);
  const leading = before.length > 0 && !before.endsWith("\n") ? "\n" : "";
  return `${before}${leading}${token}\n${after}`;
}

function insertImageToken(
  body: string,
  imageId: string,
  textarea: HTMLTextAreaElement | null,
): string {
  return insertTokenAtCursor(body, `![](image:${imageId})`, textarea);
}

function minutesLeft(ms: number): number {
  return Math.max(1, Math.ceil(ms / 60_000));
}

/** The markdown textarea + attach menu for a text response (`docs/design/frontend.md` §7.3, §8.4, §10). */
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
  onDetachImage,
}: Props) {
  const [preview, setPreview] = useState(false);
  const [dragOver, setDragOver] = useState(false);
  const [gifPickerOpen, setGifPickerOpen] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const uploaderRef = useRef<ImageUploaderHandle>(null);
  const attachMenuRef = useRef<AttachMenuHandle>(null);
  const knownIdsRef = useRef<Set<string>>(new Set((initialImages ?? []).map((i) => i.imageId)));
  const readyIdsRef = useRef<Set<string>>(
    new Set((initialImages ?? []).filter((i) => i.status === "ready").map((i) => i.imageId)),
  );
  const giphyUntil = useGiphyLimit((s) => s.until);
  const giphyRateLimited = giphyUntil !== null && giphyUntil > Date.now();

  const [liveImages, setLiveImages] = useState<UploaderImage[]>(() =>
    (initialImages ?? []).map((i) => ({
      imageId: i.imageId,
      status: i.status === "pending" ? "processing" : i.status,
      thumbUrl: i.thumbUrl,
      displayUrl: i.displayUrl,
      width: i.width,
      height: i.height,
    })),
  );

  const wordCount = body.trim() ? body.trim().split(/\s+/).length : 0;
  const gifCount = referencedGiphyIds(body).size;

  // The uploader's `onChange` reads its URLs from each image's `uploadQuery` entry, so a URL
  // refresh reaches the preview too (`docs/design/media.md` §4.4).
  const previewImages = liveImages.map((i) => ({
    imageId: i.imageId,
    displayUrl: i.displayUrl,
    thumbUrl: i.thumbUrl,
    width: i.width,
    height: i.height,
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
    onImageMediaIdsChange([...readyIds].slice(0, MAX_IMAGES));
    onUploadingChange?.(images.some((i) => i.status === "uploading" || i.status === "processing"));
  }

  /**
   * "Remove image", after the uploader's permanent-delete confirm (D8). In
   * order (`docs/design/api.md` §9.4): strip its tokens and drop its ID, save
   * the draft without it, then `DELETE /uploads/{id}` — a draft still listing
   * a deleted image would fail its next save.
   */
  async function handleRemoveImage(imageId: string): Promise<void> {
    await onDetachImage(imageId, (current) => removeImageToken(current, imageId));
    await api.media.deleteUpload(imageId, groupId, cycleId);
  }

  function handlePickGif(gif: GiphyGif) {
    setGifPickerOpen(false);
    onBodyChange(
      insertTokenAtCursor(body, `![${gif.title}](giphy:${gif.id})`, textareaRef.current),
    );
  }

  function handleDrop(e: DragEvent<HTMLDivElement>) {
    e.preventDefault();
    setDragOver(false);
    uploaderRef.current?.acceptFiles(Array.from(e.dataTransfer.files));
  }

  function handlePaste(e: ClipboardEvent<HTMLTextAreaElement>) {
    const files = Array.from(e.clipboardData?.files ?? []).filter((f) =>
      f.type.startsWith("image/"),
    );
    if (files.length > 0) uploaderRef.current?.acceptFiles(files);
  }

  const imagesAtLimit = liveImages.length >= MAX_IMAGES;
  const gifsAtLimit = gifCount >= MAX_GIFS_PER_ANSWER;

  const attachItems: AttachMenuItem[] = [
    {
      key: "photo",
      icon: "📷",
      label: "Photo",
      subtitle: imagesAtLimit
        ? `${liveImages.length}/${MAX_IMAGES} images`
        : `${liveImages.length}/${MAX_IMAGES} images · up to 15 MB each`,
      disabled: imagesAtLimit,
      onSelect: () => uploaderRef.current?.openPicker(),
    },
  ];
  if (env.giphyApiKey) {
    attachItems.push({
      key: "gif",
      icon: "GIF",
      label: "GIF",
      subtitle: giphyRateLimited
        ? `GIF limit reached · try again in ${minutesLeft(giphyLimitRemainingMs())} min`
        : gifsAtLimit
          ? `${gifCount}/${MAX_GIFS_PER_ANSWER} GIFs`
          : "Search GIPHY",
      disabled: gifsAtLimit || giphyRateLimited,
      onSelect: () => setGifPickerOpen(true),
    });
  }

  const giphyIds = [...referencedGiphyIds(body)];

  return (
    <div className="mt-4 px-5">
      <div
        className="relative overflow-hidden rounded-3xl border border-line bg-white shadow-soft"
        onDragOver={(e) => {
          e.preventDefault();
          setDragOver(true);
        }}
        onDragLeave={() => setDragOver(false)}
        onDrop={handleDrop}
      >
        {dragOver && (
          <div className="absolute inset-0 z-10 flex items-center justify-center rounded-3xl bg-grape/10 text-sm font-semibold text-grape">
            Drop to attach
          </div>
        )}
        <div className="flex items-center justify-end gap-1 border-b border-line px-3 py-2 text-sm text-inkmuted">
          <button
            type="button"
            onClick={() => setPreview((p) => !p)}
            className={clsx(
              "rounded-md px-2 py-1 text-xs font-medium",
              preview ? "bg-ink text-cream" : "hover:bg-cream",
            )}
          >
            {preview ? "Edit" : "Preview"}
          </button>
        </div>
        {preview ? (
          <div className="min-h-[200px] px-4 py-4">
            {body.trim() ? (
              <GiphyLookupProvider ids={giphyIds}>
                <MarkdownBody
                  body={body}
                  images={previewImages}
                  maxGiphy={MAX_GIFS_PER_ANSWER}
                  giphyVariant="answer"
                />
              </GiphyLookupProvider>
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
            onPaste={handlePaste}
            readOnly={readOnly}
            className="min-h-[200px] w-full resize-none bg-transparent px-4 py-4 text-[15px] leading-relaxed focus:outline-none"
            placeholder="Tell us about it…"
          />
        )}
        <div className="border-t border-line p-3">
          <div className="mb-2">
            <AttachMenu
              ref={attachMenuRef}
              items={attachItems}
              ariaLabel="Add a photo or GIF"
              showLabel
            />
          </div>
          <ImageUploader
            ref={uploaderRef}
            groupId={groupId}
            cycleId={cycleId}
            questionId={questionId}
            purpose="response"
            maxImages={MAX_IMAGES}
            initialImages={initialImages}
            onChange={handleUploaderChange}
            confirmRemove
            onRemove={handleRemoveImage}
            inputLabel="Add photos"
          />
        </div>
      </div>
      <div className="mt-2 text-right text-xs text-inkmuted">
        {wordCount} word{wordCount === 1 ? "" : "s"}
      </div>

      {gifPickerOpen && (
        <GiphyPicker
          anchorRef={{ current: attachMenuRef.current?.getAnchorEl() ?? null }}
          onPick={handlePickGif}
          onClose={() => setGifPickerOpen(false)}
        />
      )}
    </div>
  );
}
