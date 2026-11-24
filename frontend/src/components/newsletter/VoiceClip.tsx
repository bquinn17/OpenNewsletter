import { useEffect, useRef, useState, type KeyboardEvent, type MouseEvent } from "react";
import clsx from "clsx";
import { useAudioPlayback } from "../../state/audioPlayback";
import { primeAudioCache } from "../../utils/audioCache";
import { useMediaRetry } from "../ui/useMediaRetry";
import {
  canPlayOpusWebm,
  formatClipTime,
  nextPlaybackRate,
  voiceClipLabel,
  voiceClipValueText,
} from "../../utils/voiceClip";

const SEEK_STEP_MS = 5000;

type Props = {
  audioId: string;
  /** Presigned GET URL, or `null` while offline/uncached (`docs/design/frontend.md` §19.2). */
  url: string | null;
  durationMs: number;
  /** 48 waveform bucket amplitudes, 0-255. */
  peaks: number[];
  caption?: string | null;
  authorName: string;
  /** The author's avatar-color Tailwind classes, for the round play button (`avatarColorClass`). */
  colorClassName?: string;
};

/**
 * Plays back one voice clip (`docs/design/frontend.md` §19.2): a round play button in the
 * author's avatar color, a 48-bar waveform (filled as it plays, seekable), the elapsed/total time,
 * a 1×→1.5×→2× speed toggle, and `<audio preload="none" crossorigin="anonymous">` so a page with
 * many clips downloads nothing until someone presses play. Only one `VoiceClip` across the app
 * plays at a time (`state/audioPlayback.ts`).
 */
export function VoiceClip({
  audioId,
  url,
  durationMs,
  peaks,
  caption,
  authorName,
  colorClassName,
}: Props) {
  const audioRef = useRef<HTMLAudioElement>(null);
  const [positionMs, setPositionMs] = useState(0);
  const [rate, setRate] = useState(1);
  const playingId = useAudioPlayback((s) => s.playingId);
  const setPlaying = useAudioPlayback((s) => s.setPlaying);
  const isPlaying = playingId === audioId;
  const { failed, handleError } = useMediaRetry(url);

  // Another `VoiceClip` started: pause this one.
  useEffect(() => {
    if (!isPlaying) audioRef.current?.pause();
  }, [isPlaying]);

  useEffect(() => {
    const audio = audioRef.current;
    if (audio) audio.playbackRate = rate;
  }, [rate]);

  const codecSupported =
    typeof Audio === "undefined" || canPlayOpusWebm(audioRef.current ?? new Audio());

  function togglePlay() {
    const audio = audioRef.current;
    if (!audio || !url || failed) return;
    if (isPlaying) {
      audio.pause();
      setPlaying(null);
    } else {
      setPlaying(audioId);
      void audio.play();
      void primeAudioCache(url);
    }
  }

  function seekTo(nextMs: number) {
    const audio = audioRef.current;
    const clamped = Math.max(0, Math.min(durationMs, nextMs));
    if (audio) audio.currentTime = clamped / 1000;
    setPositionMs(clamped);
  }

  function handleSliderKeyDown(e: KeyboardEvent<HTMLDivElement>) {
    if (e.key === " " || e.key === "Spacebar") {
      e.preventDefault();
      togglePlay();
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      seekTo(positionMs + SEEK_STEP_MS);
    } else if (e.key === "ArrowLeft") {
      e.preventDefault();
      seekTo(positionMs - SEEK_STEP_MS);
    }
  }

  function handleWaveformInteraction(e: MouseEvent<HTMLDivElement>) {
    const rect = e.currentTarget.getBoundingClientRect();
    const ratio = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
    seekTo(ratio * durationMs);
  }

  const maxPeak = Math.max(...peaks, 1);
  const playedBars = durationMs > 0 ? Math.round((positionMs / durationMs) * peaks.length) : 0;

  if (!codecSupported) {
    return (
      <div className="mt-3 rounded-2xl border border-line bg-cream p-3 text-xs text-inkmuted">
        {formatClipTime(durationMs)} — Update your browser to play voice clips
        {caption && <p className="mt-1 italic">{caption}</p>}
      </div>
    );
  }

  if (!url) {
    return (
      <div className="mt-3 flex items-center gap-3 rounded-2xl border border-line bg-cream p-3">
        <button
          type="button"
          disabled
          aria-label="Voice clip unavailable offline"
          className="grid h-10 w-10 flex-none place-items-center rounded-full bg-inkmuted/40 text-white"
        >
          <span aria-hidden="true">▶</span>
        </button>
        <span className="text-xs text-inkmuted">Available when you&apos;re back online.</span>
      </div>
    );
  }

  return (
    <div className="mt-3 rounded-2xl border border-line bg-cream p-3">
      {/* A spoken voice clip, not a video — no caption track applies (`caption` below is a short text label, not WebVTT). */}
      {/* eslint-disable-next-line jsx-a11y/media-has-caption */}
      <audio
        ref={audioRef}
        src={url}
        preload="none"
        crossOrigin="anonymous"
        onTimeUpdate={(e) => setPositionMs(e.currentTarget.currentTime * 1000)}
        onEnded={() => {
          setPlaying(null);
          setPositionMs(0);
        }}
        onError={handleError}
        className="hidden"
      />
      <div className="flex items-center gap-3">
        <button
          type="button"
          onClick={togglePlay}
          aria-label={isPlaying ? "Pause voice clip" : "Play voice clip"}
          className={clsx(
            "grid h-10 w-10 flex-none place-items-center rounded-full text-white",
            colorClassName ?? "bg-grape",
          )}
        >
          <span aria-hidden="true">{isPlaying ? "❚❚" : "▶"}</span>
        </button>

        <div
          role="slider"
          tabIndex={0}
          aria-label={voiceClipLabel(authorName, durationMs)}
          aria-valuemin={0}
          aria-valuemax={durationMs}
          aria-valuenow={Math.round(positionMs)}
          aria-valuetext={voiceClipValueText(positionMs, durationMs)}
          onKeyDown={handleSliderKeyDown}
          onClick={handleWaveformInteraction}
          className="flex h-8 flex-1 cursor-pointer items-end gap-0.5 outline-none focus-visible:ring-2 focus-visible:ring-coral/40"
        >
          {peaks.map((peak, i) => (
            <span
              key={i}
              className={clsx(
                "w-1 flex-1 rounded-full",
                i < playedBars ? "bg-grape" : "bg-grape/30",
              )}
              style={{ height: `${Math.max(2, (peak / maxPeak) * 32)}px` }}
            />
          ))}
        </div>

        <span className="flex-none text-xs font-semibold tabular-nums text-inkmuted">
          {formatClipTime(positionMs)} / {formatClipTime(durationMs)}
        </span>

        <button
          type="button"
          onClick={() => setRate(nextPlaybackRate(rate))}
          className="flex-none rounded-full border border-line px-2 py-1 text-xs font-semibold text-inkmuted"
        >
          {rate}×
        </button>
      </div>
      {caption && <p className="mt-1.5 text-xs italic text-inkmuted">{caption}</p>}
    </div>
  );
}
