type Status = "uploading" | "processing" | "ready" | "failed";

/**
 * An image's local, in-progress state. Its URLs and dimensions aren't here: once it's `ready`,
 * they're its `uploadQuery` cache entry's (`docs/design/media.md` §3.3, §4.4).
 */
export type PendingImage = {
  imageId: string;
  file: File | null;
  status: Status;
  progress: number;
  altText: string;
  error: string | null;
};
