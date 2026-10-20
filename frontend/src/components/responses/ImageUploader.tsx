import { useEffect, useRef, useState } from "react";
import clsx from "clsx";
import { api, ApiError } from "../../api/client";
import type { components } from "../../types/api";
import { pollMediaStatus, sha256Base64, thumbSize } from "../../utils/media";
import { uploadToS3 } from "../../utils/uploadToS3";
import { useToasts } from "../../state/toast";
import { Spinner } from "../ui/Spinner";
import { MediaImage } from "../ui/MediaImage";
import { ConfirmDialog } from "../ui/ConfirmDialog";
import { PERMANENT_IMAGE_DELETE_MESSAGE } from "../../utils/permanentDelete";

type S = components["schemas"];

const ALLOWED_MIME_TYPES = new Set(["image/jpeg", "image/png", "image/webp", "image/gif"]);
const MAX_BYTES = 15_728_640;

type Status = "uploading" | "processing" | "ready" | "failed";

type PendingImage = {
  imageId: string;
  file: File | null;
  status: Status;
  progress: number;
  thumbUrl: string | null;
  displayUrl: string | null;
  width: number | null;
  height: number | null;
  altText: string;
  error: string | null;
};

export type UploaderImage = {
  imageId: string;
  status: Status;
  altText: string;
  thumbUrl: string | null;
  displayUrl: string | null;
  width: number | null;
  height: number | null;
};

type Props = {
  groupId: string;
  cycleId: string;
  questionId: string;
  purpose?: S["ImagePurpose"];
  maxImages?: number;
  initialImages?: S["ImageMediaResponse"][];
  onChange?: (images: UploaderImage[]) => void;
  /**
   * Ask before removing an uploaded image, because removal deletes it for
   * real (`docs/design/media.md` §8.0, D8). A purely local, never-presigned
   * entry is dropped without asking.
   */
  confirmRemove?: boolean;
  /**
   * Replaces the uploader's own `DELETE /uploads/{id}`. By the time it's
   * called the image is already hidden from `onChange`, so the parent has
   * dropped it from its own state; it must persist that, then delete the
   * upload. Resolving drops the image from the list; rejecting restores it.
   */
  onRemove?: (imageId: string) => Promise<void>;
};

function tempId(): string {
  const random =
    typeof crypto !== "undefined" && crypto.randomUUID
      ? crypto.randomUUID()
      : Math.random().toString(36).slice(2);
  return `local:${random}`;
}

function friendlyError(code: string | null | undefined, maxImages: number): string {
  switch (code) {
    case "IMAGE_TOO_LARGE":
      return "That image is larger than 15 MB.";
    case "IMAGE_BAD_TYPE":
      return "That file type isn't supported.";
    case "IMAGE_DECODE_FAILED":
      return "Couldn't process that image — try a different file.";
    case "IMAGE_LIMIT_EXCEEDED":
      return `You can attach up to ${maxImages} images.`;
    case "DELETED":
      return "This image was removed.";
    default:
      return "Couldn't upload — try again.";
  }
}

function validateFile(file: File, remainingSlots: number, maxImages: number): string | null {
  if (remainingSlots <= 0) return `You can attach up to ${maxImages} images.`;
  if (!ALLOWED_MIME_TYPES.has(file.type)) return "That file type isn't supported.";
  if (file.size > MAX_BYTES) return "That image is larger than 15 MB.";
  return null;
}

function toUploaderImage(image: PendingImage): UploaderImage {
  return {
    imageId: image.imageId,
    status: image.status,
    altText: image.altText,
    thumbUrl: image.thumbUrl,
    displayUrl: image.displayUrl,
    width: image.width,
    height: image.height,
  };
}

export function ImageUploader({
  groupId,
  cycleId,
  questionId,
  purpose = "response",
  maxImages = 10,
  initialImages,
  onChange,
  confirmRemove = false,
  onRemove,
}: Props) {
  const [images, setImages] = useState<PendingImage[]>(() =>
    (initialImages ?? []).map((row) => ({
      imageId: row.imageId,
      file: null,
      status: row.status === "pending" ? "processing" : row.status,
      progress: row.status === "ready" ? 1 : 0,
      thumbUrl: row.thumbUrl,
      displayUrl: row.displayUrl,
      width: row.width,
      height: row.height,
      altText: row.caption ?? "",
      error: row.status === "failed" ? friendlyError(row.errorMessage, maxImages) : null,
    })),
  );
  const [fileErrors, setFileErrors] = useState<string[]>([]);
  const [dragOver, setDragOver] = useState(false);
  const [confirmTarget, setConfirmTarget] = useState<PendingImage | null>(null);
  // Images mid-removal: still listed here, but hidden from `onChange`.
  const [removingIds, setRemovingIds] = useState<ReadonlySet<string>>(() => new Set());

  const inputRef = useRef<HTMLInputElement>(null);
  const pollCancelRef = useRef(new Map<string, () => void>());
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  useEffect(() => {
    onChangeRef.current?.(
      images.filter((img) => !removingIds.has(img.imageId)).map(toUploaderImage),
    );
  }, [images, removingIds]);

  useEffect(() => {
    const cancels = pollCancelRef.current;
    return () => {
      for (const cancel of cancels.values()) cancel();
      cancels.clear();
    };
  }, []);

  // Resume polling for any `initialImages` still mid-processing when this
  // component mounts (M8 gap: a page refresh otherwise strands the row as a
  // permanent spinner since nothing re-triggers `GET /uploads/{id}`).
  useEffect(() => {
    for (const row of initialImages ?? []) {
      if (row.status === "pending") beginPolling(row.imageId);
    }
    // Deliberately mount-only: `beginPolling` registers a cancel keyed by
    // imageId in `pollCancelRef`, cancelled by the unmount effect above.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const pushToast = useToasts((s) => s.push);

  function updateImage(imageId: string, patch: Partial<PendingImage>) {
    setImages((prev) => prev.map((img) => (img.imageId === imageId ? { ...img, ...patch } : img)));
  }

  function replaceImageId(oldId: string, newId: string) {
    setImages((prev) =>
      prev.map((img) => (img.imageId === oldId ? { ...img, imageId: newId } : img)),
    );
  }

  function requestPresign(file: File, sha256: string | undefined) {
    return api.media.createUpload({
      groupId,
      cycleId,
      questionId,
      purpose,
      mimeType: file.type as S["ImageMimeType"],
      byteSize: file.size,
      ...(sha256 ? { sha256 } : {}),
    });
  }

  /**
   * `onExpired` for a ready image's `MediaImage` (`docs/design/media.md` §4.4): its URLs live in
   * this component's own state, not the react-query cache the global `refreshMediaUrls` reaches,
   * so a fresh presigned URL means re-fetching this one upload directly.
   */
  async function refreshImage(imageId: string): Promise<void> {
    const fresh = await api.media.getUpload(imageId, groupId, cycleId);
    updateImage(imageId, {
      thumbUrl: fresh.thumbUrl,
      displayUrl: fresh.displayUrl,
      width: fresh.width,
      height: fresh.height,
    });
  }

  function beginPolling(imageId: string) {
    const cancel = pollMediaStatus(() => api.media.getUpload(imageId, groupId, cycleId), {
      onReady: (data) => {
        pollCancelRef.current.delete(imageId);
        updateImage(imageId, {
          status: "ready",
          progress: 1,
          thumbUrl: data.thumbUrl,
          displayUrl: data.displayUrl,
          width: data.width,
          height: data.height,
        });
      },
      onFailed: (errorMessage) => {
        pollCancelRef.current.delete(imageId);
        updateImage(imageId, { status: "failed", error: friendlyError(errorMessage, maxImages) });
      },
      onTimeout: () => {
        pollCancelRef.current.delete(imageId);
        updateImage(imageId, {
          status: "failed",
          error: "This is taking longer than expected — try again.",
        });
      },
    });
    pollCancelRef.current.set(imageId, cancel);
  }

  async function startUpload(file: File) {
    const localId = tempId();
    setImages((prev) => [
      ...prev,
      {
        imageId: localId,
        file,
        status: "uploading",
        progress: 0,
        thumbUrl: null,
        displayUrl: null,
        width: null,
        height: null,
        altText: "",
        error: null,
      },
    ]);

    let sha256: string | undefined;
    try {
      sha256 = await sha256Base64(file);
    } catch {
      sha256 = undefined;
    }

    let presign: S["CreateUploadResponse"];
    try {
      presign = await requestPresign(file, sha256);
    } catch (err) {
      const code = err instanceof ApiError ? err.code : undefined;
      updateImage(localId, { status: "failed", error: friendlyError(code, maxImages) });
      return;
    }

    let currentId = presign.imageId;
    replaceImageId(localId, currentId);

    let result = await uploadToS3(presign.uploadUrl, file, presign.headers, (progress) =>
      updateImage(currentId, { progress }),
    ).catch(() => ({ status: 0 }));

    if (result.status === 403) {
      try {
        const fresh = await requestPresign(file, sha256);
        replaceImageId(currentId, fresh.imageId);
        currentId = fresh.imageId;
        result = await uploadToS3(fresh.uploadUrl, file, fresh.headers, (progress) =>
          updateImage(currentId, { progress }),
        ).catch(() => ({ status: 0 }));
      } catch {
        updateImage(currentId, { status: "failed", error: friendlyError(undefined, maxImages) });
        return;
      }
    }

    if (result.status < 200 || result.status >= 300) {
      updateImage(currentId, { status: "failed", error: friendlyError(undefined, maxImages) });
      return;
    }

    updateImage(currentId, { status: "processing", progress: 1 });
    try {
      await api.media.completeUpload(currentId, groupId, cycleId);
    } catch {
      // Best-effort short-circuit; polling below will catch up regardless.
    }
    beginPolling(currentId);
  }

  function handleFiles(files: File[]) {
    const errors: string[] = [];
    const accepted: File[] = [];
    let remaining = maxImages - images.length;

    for (const file of files) {
      const error = validateFile(file, remaining, maxImages);
      if (error) {
        errors.push(`${file.name}: ${error}`);
        continue;
      }
      remaining -= 1;
      accepted.push(file);
    }

    setFileErrors(errors);
    for (const file of accepted) void startUpload(file);
  }

  function setRemoving(imageId: string, removing: boolean) {
    setRemovingIds((prev) => {
      const next = new Set(prev);
      if (removing) next.add(imageId);
      else next.delete(imageId);
      return next;
    });
  }

  function requestRemove(image: PendingImage) {
    if (confirmRemove && !image.imageId.startsWith("local:")) {
      setConfirmTarget(image);
    } else {
      void handleRemove(image);
    }
  }

  async function handleRemove(image: PendingImage) {
    if (image.imageId.startsWith("local:")) {
      setImages((prev) => prev.filter((img) => img.imageId !== image.imageId));
      return;
    }

    setRemoving(image.imageId, true);
    try {
      if (onRemove) {
        await onRemove(image.imageId);
      } else {
        await api.media.deleteUpload(image.imageId, groupId, cycleId);
      }
      const cancel = pollCancelRef.current.get(image.imageId);
      if (cancel) {
        cancel();
        pollCancelRef.current.delete(image.imageId);
      }
      setImages((prev) => prev.filter((img) => img.imageId !== image.imageId));
    } catch (err) {
      const message =
        err instanceof ApiError && err.code === "IMAGE_IN_USE"
          ? "This image is used in a published response and can't be removed."
          : "Couldn't remove that image — try again.";
      pushToast(message, "error");
    } finally {
      setRemoving(image.imageId, false);
    }
  }

  function handleRetry(image: PendingImage) {
    if (!image.file) return;
    setImages((prev) => prev.filter((img) => img.imageId !== image.imageId));
    void startUpload(image.file);
  }

  function handleAltTextChange(imageId: string, value: string) {
    updateImage(imageId, { altText: value });
  }

  return (
    <div>
      <div
        role="button"
        tabIndex={0}
        aria-label="Add images"
        onClick={() => inputRef.current?.click()}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            inputRef.current?.click();
          }
        }}
        onDragOver={(e) => {
          e.preventDefault();
          setDragOver(true);
        }}
        onDragLeave={() => setDragOver(false)}
        onDrop={(e) => {
          e.preventDefault();
          setDragOver(false);
          handleFiles(Array.from(e.dataTransfer.files));
        }}
        className={clsx(
          "flex cursor-pointer flex-col items-center justify-center rounded-2xl border-2 border-dashed border-line bg-cream px-4 py-6 text-center text-sm text-inkmuted transition focus:outline-none focus:ring-2 focus:ring-coral/30",
          dragOver && "border-grape bg-grape/5",
        )}
      >
        <input
          ref={inputRef}
          type="file"
          multiple
          accept="image/jpeg,image/png,image/webp,image/gif"
          className="sr-only"
          onChange={(e) => {
            handleFiles(Array.from(e.target.files ?? []));
            e.target.value = "";
          }}
        />
        <p>
          Drag photos here, or <span className="font-semibold text-grape">choose files</span>
        </p>
        <p className="mt-1 text-xs">
          {images.length}/{maxImages} images · JPEG, PNG, WebP or GIF · up to 15 MB each
        </p>
      </div>

      {fileErrors.length > 0 && (
        <ul
          role="alert"
          aria-live="polite"
          className="mt-2 space-y-1 text-xs font-semibold text-coral"
        >
          {fileErrors.map((message) => (
            <li key={message}>{message}</li>
          ))}
        </ul>
      )}

      {images.length > 0 && (
        <ul aria-label="Attached images" className="mt-3 grid grid-cols-2 gap-3 sm:grid-cols-3">
          {images.map((image) => (
            <li
              key={image.imageId}
              aria-busy={removingIds.has(image.imageId) || undefined}
              className={clsx(
                "rounded-2xl border border-line bg-white p-2",
                removingIds.has(image.imageId) && "opacity-50",
              )}
            >
              <div className="relative">
                {image.status === "ready" && image.thumbUrl ? (
                  <MediaImage
                    src={image.thumbUrl}
                    alt={image.altText || "Uploaded image"}
                    {...thumbSize(image.width, image.height)}
                    className="h-24 w-full rounded-xl object-cover"
                    onExpired={() => refreshImage(image.imageId)}
                  />
                ) : (
                  <div
                    className="flex h-24 w-full flex-col items-center justify-center gap-1 rounded-xl bg-cream px-2 text-center text-xs text-inkmuted"
                    aria-live="polite"
                  >
                    {image.status === "failed" ? (
                      <>
                        <span className="text-coral">{image.error}</span>
                        {image.file && (
                          <button
                            type="button"
                            onClick={() => handleRetry(image)}
                            className="font-semibold text-grape"
                          >
                            Retry
                          </button>
                        )}
                      </>
                    ) : (
                      <>
                        <Spinner size="sm" />
                        <span>
                          {image.status === "processing"
                            ? "Processing…"
                            : `Uploading ${Math.round(image.progress * 100)}%`}
                        </span>
                      </>
                    )}
                  </div>
                )}
                <button
                  type="button"
                  onClick={() => requestRemove(image)}
                  disabled={removingIds.has(image.imageId)}
                  aria-label={`Remove image ${image.file?.name ?? image.imageId}`}
                  className="absolute -right-2 -top-2 grid h-6 w-6 place-items-center rounded-full bg-ink text-xs font-bold text-cream disabled:opacity-50"
                >
                  ×
                </button>
              </div>
              {image.status === "ready" && (
                <div className="mt-2">
                  <label htmlFor={`alt-${image.imageId}`} className="sr-only">
                    Describe this image for screen readers
                  </label>
                  <input
                    id={`alt-${image.imageId}`}
                    type="text"
                    value={image.altText}
                    onChange={(e) => handleAltTextChange(image.imageId, e.target.value)}
                    placeholder="Describe this image"
                    className="w-full rounded-lg border border-line bg-cream px-2 py-1 text-xs focus:outline-none focus:ring-2 focus:ring-coral/30"
                  />
                </div>
              )}
            </li>
          ))}
        </ul>
      )}

      <ConfirmDialog
        open={confirmTarget !== null}
        title="Delete this image?"
        message={PERMANENT_IMAGE_DELETE_MESSAGE}
        confirmLabel="Delete image"
        tone="danger"
        onConfirm={() => {
          const target = confirmTarget;
          setConfirmTarget(null);
          if (target) void handleRemove(target);
        }}
        onCancel={() => setConfirmTarget(null)}
      />
    </div>
  );
}
