import { useEffect, useRef, useState } from "react";
import clsx from "clsx";
import { Spinner } from "../ui/Spinner";
import { formatClipTime, MAX_AUDIO_CAPTION_CHARS } from "../../utils/voiceClip";
import type { PendingClip } from "./voiceClipTypes";

type Props = {
  clip: PendingClip;
  /** The clip's presigned `url`, once its `GET /audio-uploads/{id}` row is `ready`. */
  url: string | null;
  removing: boolean;
  onRemove: () => void;
  onRetry: () => void;
  onReRecord?: () => void;
  onCaptionChange: (value: string) => void;
};

/**
 * One attached voice clip's card (`docs/design/frontend.md` §19.1): play/pause, a locally- or
 * server-drawn waveform, the duration, Re-record/Remove, a caption field, and the
 * uploading → processing → ready/failed states an image tile shows (`docs/design/media.md` §3.3).
 */
export function VoiceClipCard({
  clip,
  url,
  removing,
  onRemove,
  onRetry,
  onReRecord,
  onCaptionChange,
}: Props) {
  const [playing, setPlaying] = useState(false);
  const audioRef = useRef<HTMLAudioElement>(null);
  const objectUrlRef = useRef<string | null>(null);

  // A clip's blob stays in memory until the upload succeeds, so it can be replayed locally before
  // then (`docs/design/frontend.md` §19.1); once a `url` is served, prefer it.
  const playableSrc =
    url ??
    (clip.blob && typeof URL.createObjectURL === "function"
      ? (objectUrlRef.current ??= URL.createObjectURL(clip.blob))
      : null);

  useEffect(() => {
    return () => {
      if (objectUrlRef.current) URL.revokeObjectURL(objectUrlRef.current);
    };
  }, []);

  function togglePlay() {
    const audio = audioRef.current;
    if (!audio) return;
    if (playing) {
      audio.pause();
    } else {
      void audio.play();
    }
  }

  const peaks = clip.peaks ?? [];
  const maxPeak = Math.max(...peaks, 1);

  const live =
    clip.status === "ready" || clip.status === "uploading" || clip.status === "processing";

  return (
    <li
      aria-busy={removing || undefined}
      className={clsx("rounded-2xl border border-line bg-white p-3", removing && "opacity-50")}
    >
      {playableSrc && (
        // eslint-disable-next-line jsx-a11y/media-has-caption -- a spoken voice clip, no track applies.
        <audio
          ref={audioRef}
          src={playableSrc}
          onPlay={() => setPlaying(true)}
          onPause={() => setPlaying(false)}
          onEnded={() => setPlaying(false)}
          className="hidden"
        />
      )}

      <div className="flex items-center gap-3">
        {live ? (
          <>
            <button
              type="button"
              onClick={togglePlay}
              disabled={!playableSrc}
              aria-label={playing ? "Pause voice clip" : "Play voice clip"}
              className="grid h-10 w-10 flex-none place-items-center rounded-full bg-grape text-white disabled:opacity-40"
            >
              <span aria-hidden="true">{playing ? "❚❚" : "▶"}</span>
            </button>
            <div className="flex h-8 flex-1 items-end gap-0.5" aria-hidden="true">
              {peaks.length > 0 ? (
                peaks.map((peak, i) => (
                  <span
                    key={i}
                    className="w-1 flex-1 rounded-full bg-grape/60"
                    style={{ height: `${Math.max(2, (peak / maxPeak) * 32)}px` }}
                  />
                ))
              ) : (
                <Spinner size="sm" />
              )}
            </div>
            <span className="flex-none text-xs font-semibold tabular-nums text-inkmuted">
              {clip.status === "uploading"
                ? `${Math.round(clip.progress * 100)}%`
                : clip.status === "processing"
                  ? "Processing…"
                  : clip.durationMs !== null
                    ? formatClipTime(clip.durationMs)
                    : "--:--"}
            </span>
          </>
        ) : (
          <div className="flex-1 text-xs text-coral" aria-live="polite">
            {clip.error}
            <button type="button" onClick={onRetry} className="ml-2 font-semibold text-grape">
              Retry
            </button>
          </div>
        )}

        <div className="flex flex-none items-center gap-2">
          {onReRecord && (
            <button type="button" onClick={onReRecord} className="text-xs font-semibold text-grape">
              Re-record
            </button>
          )}
          <button
            type="button"
            onClick={onRemove}
            disabled={removing}
            aria-label="Remove voice clip"
            className="grid h-6 w-6 place-items-center rounded-full bg-ink text-xs font-bold text-cream disabled:opacity-50"
          >
            ×
          </button>
        </div>
      </div>

      <div className="mt-1.5">
        <label htmlFor={`clip-caption-${clip.audioId}`} className="sr-only">
          Describe this clip
        </label>
        <input
          id={`clip-caption-${clip.audioId}`}
          type="text"
          value={clip.caption}
          onChange={(e) => onCaptionChange(e.target.value.slice(0, MAX_AUDIO_CAPTION_CHARS))}
          placeholder="Describe this clip"
          maxLength={MAX_AUDIO_CAPTION_CHARS}
          className="w-full rounded-lg border border-line bg-cream px-2 py-1 text-xs focus:outline-none focus:ring-2 focus:ring-coral/30"
        />
      </div>
    </li>
  );
}
