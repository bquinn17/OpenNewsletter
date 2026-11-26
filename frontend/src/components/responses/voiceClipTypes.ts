type Status = "uploading" | "processing" | "ready" | "failed";

/**
 * A voice clip's local, in-progress state (`docs/design/frontend.md` §19.1). Mirrors
 * `PendingImage`, with `blob` in place of `file` — the recorded or picked bytes stay in memory
 * until the upload succeeds, so a clip can be re-played locally while it's still uploading, and
 * `durationMs`/`peaks` start out as the local `decodeLocalClip` estimate, then are overwritten by
 * the server's own values once the row is `ready`.
 */
export type PendingClip = {
  audioId: string;
  blob: Blob | null;
  status: Status;
  progress: number;
  durationMs: number | null;
  peaks: number[] | null;
  caption: string;
  error: string | null;
};
