import { useEffect, useMemo, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { api, ApiError } from "../../api/client";
import { queryKeys, useUploads } from "../../api/queries";
import type { components } from "../../types/api";
import { pollMediaStatus, sha256Base64 } from "../../utils/media";
import { uploadToS3 } from "../../utils/uploadToS3";
import { useToasts } from "../../state/toast";
import { ConfirmDialog } from "../ui/ConfirmDialog";
import { PERMANENT_IMAGE_DELETE_MESSAGE } from "../../utils/permanentDelete";
import { ImageDropzone } from "./ImageDropzone";
import { ImageUploaderTile } from "./ImageUploaderTile";
import type { PendingImage } from "./imageUploaderTypes";

type S = components["schemas"];

const ALLOWED_MIME_TYPES = new Set(["image/jpeg", "image/png", "image/webp", "image/gif"]);
const MAX_BYTES = 15_728_640;

export type UploaderImage = {
  imageId: string;
  status: PendingImage["status"];
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

function limitMessage(maxImages: number): string {
  return `You can attach up to ${maxImages} image${maxImages === 1 ? "" : "s"}.`;
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
      return limitMessage(maxImages);
    case "DELETED":
      return "This image was removed.";
    default:
      return "Couldn't upload — try again.";
  }
}

function validateFile(file: File, remainingSlots: number, maxImages: number): string | null {
  if (remainingSlots <= 0) return limitMessage(maxImages);
  if (!ALLOWED_MIME_TYPES.has(file.type)) return "That file type isn't supported.";
  if (file.size > MAX_BYTES) return "That image is larger than 15 MB.";
  return null;
}

function toUploaderImage(
  image: PendingImage,
  row: S["ImageMediaResponse"] | undefined,
): UploaderImage {
  return {
    imageId: image.imageId,
    status: image.status,
    altText: image.altText,
    thumbUrl: row?.thumbUrl ?? null,
    displayUrl: row?.displayUrl ?? null,
    width: row?.width ?? null,
    height: row?.height ?? null,
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
  const qc = useQueryClient();
  const [images, setImages] = useState<PendingImage[]>(() =>
    (initialImages ?? []).map((row) => ({
      imageId: row.imageId,
      file: null,
      status: row.status === "pending" ? "processing" : row.status,
      progress: row.status === "ready" ? 1 : 0,
      altText: row.caption ?? "",
      error: row.status === "failed" ? friendlyError(row.errorMessage, maxImages) : null,
    })),
  );
  const [initialRows] = useState(
    () => new Map((initialImages ?? []).map((row) => [row.imageId, row])),
  );
  const [fileErrors, setFileErrors] = useState<string[]>([]);
  const [confirmTarget, setConfirmTarget] = useState<PendingImage | null>(null);
  // Images mid-removal: still listed here, but hidden from `onChange`.
  const [removingIds, setRemovingIds] = useState<ReadonlySet<string>>(() => new Set());

  // A ready image's URLs live in its `uploadQuery` entry, not here, so the global
  // `refreshMediaUrls` re-signs them when one expires (`docs/design/media.md` §4.4).
  const rows = useUploads(
    groupId,
    cycleId,
    images.filter((img) => img.status === "ready").map((img) => img.imageId),
    initialRows,
  );
  const rowsById = useMemo(() => new Map(rows.map((row) => [row.imageId, row])), [rows]);

  const pollCancelRef = useRef(new Map<string, () => void>());
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  useEffect(() => {
    onChangeRef.current?.(
      images
        .filter((img) => !removingIds.has(img.imageId))
        .map((img) => toUploaderImage(img, rowsById.get(img.imageId))),
    );
  }, [images, removingIds, rowsById]);

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

  function beginPolling(imageId: string) {
    const cancel = pollMediaStatus(() => api.media.getUpload(imageId, groupId, cycleId), {
      onReady: (data) => {
        pollCancelRef.current.delete(imageId);
        // Into the cache first, so the `uploadQuery` this image gains as `ready` has its row and
        // doesn't fetch it again.
        qc.setQueryData(queryKeys.upload(groupId, cycleId, imageId), data);
        updateImage(imageId, { status: "ready", progress: 1 });
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
      <ImageDropzone
        imageCount={images.length}
        maxImages={maxImages}
        fileErrors={fileErrors}
        onFilesSelected={handleFiles}
      />

      {images.length > 0 && (
        <ul aria-label="Attached images" className="mt-3 grid grid-cols-2 gap-3 sm:grid-cols-3">
          {images.map((image) => (
            <ImageUploaderTile
              key={image.imageId}
              image={image}
              row={rowsById.get(image.imageId)}
              removing={removingIds.has(image.imageId)}
              onRemove={() => requestRemove(image)}
              onRetry={() => handleRetry(image)}
              onAltTextChange={(value) => handleAltTextChange(image.imageId, value)}
            />
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
