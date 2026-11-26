import { forwardRef, useEffect, useImperativeHandle, useMemo, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { api, ApiError } from "../../api/client";
import { queryKeys, useAudioUploads } from "../../api/queries";
import type { components } from "../../types/api";
import { decodeLocalClip } from "../../utils/audioDecode";
import { pollMediaStatus, sha256Base64 } from "../../utils/media";
import { uploadToS3 } from "../../utils/uploadToS3";
import { useToasts } from "../../state/toast";
import { ConfirmDialog } from "../ui/ConfirmDialog";
import { PERMANENT_AUDIO_DELETE_MESSAGE } from "../../utils/permanentDelete";
import {
  ALLOWED_AUDIO_MIME_TYPES,
  clipLimitMessage,
  MAX_AUDIO_BYTES,
  normalizeAudioMimeType,
} from "../../utils/voiceClip";
import { useVoiceRecorder } from "./useVoiceRecorder";
import { VoiceClipRecorder } from "./VoiceClipRecorder";
import { VoiceClipCard } from "./VoiceClipCard";
import type { PendingClip } from "./voiceClipTypes";

type S = components["schemas"];

/** Debounce before a caption edit reaches `PATCH /audio-uploads/{id}` (`docs/design/frontend.md` §19.1). */
const CAPTION_DEBOUNCE_MS = 1500;

export type UploaderClip = {
  audioId: string;
  status: PendingClip["status"];
  caption: string;
  durationMs: number | null;
  peaks: number[] | null;
  url: string | null;
};

type Props = {
  groupId: string;
  cycleId: string;
  questionId: string;
  purpose?: S["ImagePurpose"];
  maxClips: number;
  initialClips?: S["AudioMediaResponse"][];
  onChange?: (clips: UploaderClip[]) => void;
  /** Ask before removing an uploaded clip, since removal deletes it for real (`docs/design/media.md` §12.4). */
  confirmRemove?: boolean;
  /** Replaces the uploader's own `DELETE /audio-uploads/{id}` — mirrors `ImageUploader`'s `onRemove`. */
  onRemove?: (audioId: string) => Promise<void>;
  /** The hidden file input's accessible name (`docs/design/frontend.md` §19.1). */
  inputLabel?: string;
};

export type VoiceClipUploaderHandle = {
  /** Opens the native file picker — the "+" menu's "Upload audio" item. */
  openPicker: () => void;
  /** Routes dropped or pasted `audio/*` files into the same validation/upload path as the picker. */
  acceptFiles: (files: File[]) => void;
  /** Starts the recorder — the "+" menu's "Record a voice clip" item. */
  startRecording: () => void;
};

function tempId(): string {
  const random =
    typeof crypto !== "undefined" && crypto.randomUUID
      ? crypto.randomUUID()
      : Math.random().toString(36).slice(2);
  return `local:${random}`;
}

function friendlyError(code: string | null | undefined): string {
  switch (code) {
    case "AUDIO_TOO_LARGE":
      return "That voice clip is larger than 25 MB.";
    case "AUDIO_BAD_TYPE":
      return "That file type isn't supported.";
    case "AUDIO_TOO_LONG":
      return "That voice clip is longer than 2 minutes.";
    case "AUDIO_DECODE_FAILED":
      return "Couldn't process that clip — try recording again.";
    case "DELETED":
      return "This voice clip was removed.";
    default:
      return "Couldn't upload — try again.";
  }
}

function validateFile(file: File, remainingSlots: number, maxClips: number): string | null {
  if (remainingSlots <= 0) return clipLimitMessage(maxClips);
  if (!ALLOWED_AUDIO_MIME_TYPES.has(normalizeAudioMimeType(file.type))) {
    return "That file type isn't supported.";
  }
  if (file.size > MAX_AUDIO_BYTES) return "That voice clip is larger than 25 MB.";
  return null;
}

function toInitialPendingClip(row: S["AudioMediaResponse"]): PendingClip {
  return {
    audioId: row.audioId,
    blob: null,
    status: row.status === "pending" ? "processing" : row.status,
    progress: row.status === "ready" ? 1 : 0,
    durationMs: row.durationMs,
    peaks: row.peaks,
    caption: row.caption ?? "",
    error: row.status === "failed" ? friendlyError(row.errorMessage) : null,
  };
}

function toUploaderClip(clip: PendingClip, row: S["AudioMediaResponse"] | undefined): UploaderClip {
  return {
    audioId: clip.audioId,
    status: clip.status,
    caption: clip.caption,
    durationMs: row?.durationMs ?? clip.durationMs,
    peaks: row?.peaks ?? clip.peaks,
    url: row?.url ?? null,
  };
}

/**
 * The voice-clip equivalent of `ImageUploader` (`docs/design/frontend.md` §19.1): a hidden
 * `<input type="file" accept="audio/*">` plus the recorder (`useVoiceRecorder`), driving the same
 * presign → PUT → poll upload state machine as images, on the audio routes
 * (`docs/design/media.md` §12, `docs/design/api.md` §9.8). A clip joins the caller's `onChange`
 * list — and so the draft/comment body — the moment its upload starts (`status: "uploading"`),
 * matching the one decided difference from images: the backend accepts a `pending`/`processing`
 * clip id, only rejecting `failed` ones.
 */
export const VoiceClipUploader = forwardRef<VoiceClipUploaderHandle, Props>(
  function VoiceClipUploader(
    {
      groupId,
      cycleId,
      questionId,
      purpose = "response",
      maxClips,
      initialClips,
      onChange,
      confirmRemove = false,
      onRemove,
      inputLabel = "Upload audio",
    },
    ref,
  ) {
    const qc = useQueryClient();
    const fileInputRef = useRef<HTMLInputElement>(null);
    const pushToast = useToasts((s) => s.push);
    const recorder = useVoiceRecorder(() => pushToast("That's the 2-minute limit."));

    const [clips, setClips] = useState<PendingClip[]>(() =>
      (initialClips ?? []).map(toInitialPendingClip),
    );
    const [initialRows] = useState(
      () => new Map((initialClips ?? []).map((row) => [row.audioId, row])),
    );
    const [fileErrors, setFileErrors] = useState<string[]>([]);
    const [confirmTarget, setConfirmTarget] = useState<PendingClip | null>(null);
    const [removingIds, setRemovingIds] = useState<ReadonlySet<string>>(() => new Set());

    const rows = useAudioUploads(
      groupId,
      cycleId,
      clips
        .filter((c) => c.status !== "failed" && !c.audioId.startsWith("local:"))
        .map((c) => c.audioId),
      initialRows,
    );
    const rowsById = useMemo(() => new Map(rows.map((row) => [row.audioId, row])), [rows]);

    const pollCancelRef = useRef(new Map<string, () => void>());
    const captionTimersRef = useRef(new Map<string, ReturnType<typeof setTimeout>>());
    const onChangeRef = useRef(onChange);
    onChangeRef.current = onChange;

    useEffect(() => {
      onChangeRef.current?.(
        clips
          .filter((c) => !removingIds.has(c.audioId))
          .map((c) => toUploaderClip(c, rowsById.get(c.audioId))),
      );
    }, [clips, removingIds, rowsById]);

    useEffect(() => {
      const cancels = pollCancelRef.current;
      const timers = captionTimersRef.current;
      return () => {
        for (const cancel of cancels.values()) cancel();
        cancels.clear();
        for (const timer of timers.values()) clearTimeout(timer);
        timers.clear();
      };
    }, []);

    // Resume polling for any `initialClips` still mid-processing on mount, mirroring `ImageUploader`.
    useEffect(() => {
      for (const row of initialClips ?? []) {
        if (row.status === "pending") beginPolling(row.audioId);
      }
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []);

    function updateClip(audioId: string, patch: Partial<PendingClip>) {
      setClips((prev) => prev.map((c) => (c.audioId === audioId ? { ...c, ...patch } : c)));
    }

    function replaceClipId(oldId: string, newId: string) {
      setClips((prev) => prev.map((c) => (c.audioId === oldId ? { ...c, audioId: newId } : c)));
    }

    function requestPresign(file: File, sha256: string | undefined) {
      return api.media.createAudioUpload({
        groupId,
        cycleId,
        questionId,
        purpose,
        mimeType: file.type,
        byteSize: file.size,
        ...(sha256 ? { sha256 } : {}),
      });
    }

    function beginPolling(audioId: string) {
      const cancel = pollMediaStatus(() => api.media.getAudioUpload(audioId, groupId, cycleId), {
        onReady: (data) => {
          pollCancelRef.current.delete(audioId);
          qc.setQueryData(queryKeys.audioUpload(groupId, cycleId, audioId), data);
          updateClip(audioId, {
            status: "ready",
            progress: 1,
            durationMs: data.durationMs,
            peaks: data.peaks,
          });
        },
        onFailed: (errorMessage) => {
          pollCancelRef.current.delete(audioId);
          updateClip(audioId, { status: "failed", error: friendlyError(errorMessage) });
        },
        onTimeout: () => {
          pollCancelRef.current.delete(audioId);
          updateClip(audioId, {
            status: "failed",
            error: "This is taking longer than expected — try again.",
          });
        },
      });
      pollCancelRef.current.set(audioId, cancel);
    }

    async function startUpload(file: File) {
      const localId = tempId();
      setClips((prev) => [
        ...prev,
        {
          audioId: localId,
          blob: file,
          status: "uploading",
          progress: 0,
          durationMs: null,
          peaks: null,
          caption: "",
          error: null,
        },
      ]);

      // Drawn locally while the upload's in flight (`docs/design/frontend.md` §19.1); the ready
      // row's own values (from `lambda-audio-process`) overwrite these once polling catches up.
      try {
        const decoded = await decodeLocalClip(file);
        updateClip(localId, { durationMs: decoded.durationMs, peaks: decoded.peaks });
      } catch {
        // Best effort — an undecodable local preview doesn't block the upload.
      }

      let sha256: string | undefined;
      try {
        sha256 = await sha256Base64(file);
      } catch {
        sha256 = undefined;
      }

      let presign: S["CreateAudioUploadResponse"];
      try {
        presign = await requestPresign(file, sha256);
      } catch (err) {
        const code = err instanceof ApiError ? err.code : undefined;
        updateClip(localId, { status: "failed", error: friendlyError(code) });
        return;
      }

      let currentId = presign.audioId;
      replaceClipId(localId, currentId);

      let result = await uploadToS3(presign.uploadUrl, file, presign.headers, (progress) =>
        updateClip(currentId, { progress }),
      ).catch(() => ({ status: 0 }));

      if (result.status === 403) {
        try {
          const fresh = await requestPresign(file, sha256);
          replaceClipId(currentId, fresh.audioId);
          currentId = fresh.audioId;
          result = await uploadToS3(fresh.uploadUrl, file, fresh.headers, (progress) =>
            updateClip(currentId, { progress }),
          ).catch(() => ({ status: 0 }));
        } catch {
          updateClip(currentId, { status: "failed", error: friendlyError(undefined) });
          return;
        }
      }

      if (result.status < 200 || result.status >= 300) {
        updateClip(currentId, { status: "failed", error: friendlyError(undefined) });
        return;
      }

      updateClip(currentId, { status: "processing", progress: 1 });
      beginPolling(currentId);
    }

    function handleFiles(files: File[]) {
      const errors: string[] = [];
      const accepted: File[] = [];
      let remaining = maxClips - clips.length;

      for (const file of files) {
        const error = validateFile(file, remaining, maxClips);
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

    useImperativeHandle(ref, () => ({
      openPicker: () => fileInputRef.current?.click(),
      acceptFiles: handleFiles,
      startRecording: () => void recorder.start(),
    }));

    // Hands a just-stopped recording straight to the upload path, the same way a picked file is.
    useEffect(() => {
      if (recorder.state.phase !== "stopped" || !recorder.state.blob) return;
      const blob = recorder.state.blob;
      recorder.reset();
      if (clips.length >= maxClips) return;
      void startUpload(new File([blob], "clip", { type: blob.type || "audio/webm" }));
      // `clips`/`startUpload` intentionally excluded: this effect only reacts to the recorder
      // transitioning into `stopped`, not to every clip-list change.
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [recorder.state]);

    function setRemoving(audioId: string, removing: boolean) {
      setRemovingIds((prev) => {
        const next = new Set(prev);
        if (removing) next.add(audioId);
        else next.delete(audioId);
        return next;
      });
    }

    function requestRemove(clip: PendingClip) {
      if (confirmRemove && !clip.audioId.startsWith("local:")) {
        setConfirmTarget(clip);
      } else {
        void handleRemove(clip);
      }
    }

    async function handleRemove(clip: PendingClip) {
      if (clip.audioId.startsWith("local:")) {
        setClips((prev) => prev.filter((c) => c.audioId !== clip.audioId));
        return;
      }

      setRemoving(clip.audioId, true);
      try {
        if (onRemove) {
          await onRemove(clip.audioId);
        } else {
          await api.media.deleteAudioUpload(clip.audioId, groupId, cycleId);
        }
        const cancel = pollCancelRef.current.get(clip.audioId);
        if (cancel) {
          cancel();
          pollCancelRef.current.delete(clip.audioId);
        }
        setClips((prev) => prev.filter((c) => c.audioId !== clip.audioId));
      } catch (err) {
        const message =
          err instanceof ApiError && err.code === "AUDIO_IN_USE"
            ? "This voice clip is used in a published response and can't be removed."
            : "Couldn't remove that voice clip — try again.";
        pushToast(message, "error");
      } finally {
        setRemoving(clip.audioId, false);
      }
    }

    function handleRetry(clip: PendingClip) {
      if (!clip.blob) return;
      const file =
        clip.blob instanceof File
          ? clip.blob
          : new File([clip.blob], "clip", { type: clip.blob.type });
      setClips((prev) => prev.filter((c) => c.audioId !== clip.audioId));
      void startUpload(file);
    }

    function handleCaptionChange(audioId: string, value: string) {
      updateClip(audioId, { caption: value });
      const timers = captionTimersRef.current;
      const existing = timers.get(audioId);
      if (existing) clearTimeout(existing);
      timers.set(
        audioId,
        setTimeout(() => {
          timers.delete(audioId);
          if (audioId.startsWith("local:")) return;
          void api.media
            .patchAudioUpload(audioId, groupId, cycleId, { caption: value.trim() ? value : null })
            .catch(() => pushToast("Couldn't save that caption — try again.", "error"));
        }, CAPTION_DEBOUNCE_MS),
      );
    }

    return (
      <div>
        <input
          ref={fileInputRef}
          type="file"
          accept="audio/*"
          aria-label={inputLabel}
          className="sr-only"
          onChange={(e) => {
            handleFiles(Array.from(e.target.files ?? []));
            e.target.value = "";
          }}
        />

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

        {recorder.state.phase === "recording" && (
          <div className="mt-3">
            <VoiceClipRecorder
              state={recorder.state}
              onStop={recorder.stop}
              onCancel={recorder.cancel}
            />
          </div>
        )}

        {recorder.state.phase === "denied" && (
          <p role="alert" className="mt-3 text-xs font-semibold text-coral">
            Microphone is blocked. Allow it in your browser settings, or{" "}
            <button
              type="button"
              onClick={() => fileInputRef.current?.click()}
              className="underline"
            >
              upload a recording instead
            </button>
            .
          </p>
        )}

        {clips.length > 0 && (
          <ul aria-label="Attached voice clips" className="mt-3 space-y-2">
            {clips.map((clip) => (
              <VoiceClipCard
                key={clip.audioId}
                clip={clip}
                url={rowsById.get(clip.audioId)?.url ?? null}
                removing={removingIds.has(clip.audioId)}
                onRemove={() => requestRemove(clip)}
                onRetry={() => handleRetry(clip)}
                onCaptionChange={(value) => handleCaptionChange(clip.audioId, value)}
              />
            ))}
          </ul>
        )}

        <ConfirmDialog
          open={confirmTarget !== null}
          title="Delete this voice clip?"
          message={PERMANENT_AUDIO_DELETE_MESSAGE}
          confirmLabel="Delete clip"
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
  },
);
