/**
 * Pure helpers for voice clips (`docs/design/frontend.md` §19), shared by the recorder, the
 * uploader and the `VoiceClip` playback component. Kept framework-free so they're trivial to unit
 * test.
 */

/** Mirrors `MAX_AUDIO_CLIPS_PER_RESPONSE`/`MAX_AUDIO_CLIPS_PER_COMMENT` in `backend/crates/shared/src/config.rs`. */
export const MAX_AUDIO_CLIPS_PER_RESPONSE = 3;
export const MAX_AUDIO_CLIPS_PER_COMMENT = 1;
/** Mirrors `MAX_AUDIO_SECONDS`. */
export const MAX_AUDIO_SECONDS = 120;
/** Mirrors `MAX_AUDIO_BYTES`. */
export const MAX_AUDIO_BYTES = 25 * 1024 * 1024;
/** Mirrors `MAX_AUDIO_CAPTION_CHARS`. */
export const MAX_AUDIO_CAPTION_CHARS = 140;
/** Mirrors `AUDIO_PEAK_COUNT`. */
export const AUDIO_PEAK_COUNT = 48;
/** The MIME allowlist `POST /audio-uploads` checks, after parameters are stripped (`docs/design/api.md` §9.8). */
export const ALLOWED_AUDIO_MIME_TYPES = new Set([
  "audio/webm",
  "audio/ogg",
  "audio/mp4",
  "audio/x-m4a",
  "audio/aac",
  "audio/mpeg",
  "audio/wav",
  "audio/x-wav",
]);

/** `audio/webm;codecs=opus` → `audio/webm`, matching the backend's own normalization (§9.8). */
export function normalizeAudioMimeType(mimeType: string): string {
  return mimeType.split(";")[0]!.trim().toLowerCase();
}

export function clipLimitMessage(maxClips: number): string {
  return `You can attach up to ${maxClips} voice clip${maxClips === 1 ? "" : "s"}.`;
}

/** `0:12`, `1:04`, `12:00` — never over an hour for a 2-minute cap, but handled anyway. */
export function formatClipTime(ms: number): string {
  const totalSeconds = Math.max(0, Math.round(ms / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}

/** "12 seconds" / "1 minute" / "1 minute 4 seconds" — for `aria-valuetext` and the clip label. */
export function durationWords(ms: number): string {
  const totalSeconds = Math.max(0, Math.round(ms / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  const parts: string[] = [];
  if (minutes > 0) parts.push(`${minutes} minute${minutes === 1 ? "" : "s"}`);
  if (seconds > 0 || minutes === 0) parts.push(`${seconds} second${seconds === 1 ? "" : "s"}`);
  return parts.join(" ");
}

/** "Voice clip from Alex, 1 minute 4 seconds" (`docs/design/frontend.md` §19.2). */
export function voiceClipLabel(authorName: string, durationMs: number): string {
  return `Voice clip from ${authorName}, ${durationWords(durationMs)}`;
}

/** `aria-valuetext` while seeking: "12 seconds of 1 minute 4". */
export function voiceClipValueText(positionMs: number, durationMs: number): string {
  return `${durationWords(positionMs)} of ${durationWords(durationMs)}`;
}

const PLAYBACK_RATES = [1, 1.5, 2] as const;

/** Cycles the speed toggle 1× → 1.5× → 2× → 1× (`docs/design/frontend.md` §19.2). */
export function nextPlaybackRate(rate: number): number {
  const index = PLAYBACK_RATES.indexOf(rate as (typeof PLAYBACK_RATES)[number]);
  return PLAYBACK_RATES[(index + 1) % PLAYBACK_RATES.length]!;
}

/** Whether this `<audio>` element can play Opus-in-WebM (`docs/design/media.md` §12). */
export function canPlayOpusWebm(audio: { canPlayType(type: string): string }): boolean {
  return audio.canPlayType('audio/webm; codecs="opus"') !== "";
}

/**
 * Buckets `samples` (a decoded mono channel) into `bucketCount` peak amplitudes 0-255, scaled so
 * the loudest bucket is 255 — mirrors the backend's own peak computation
 * (`docs/design/media.md` §12.3 step 4), used to draw a local waveform for a clip that hasn't
 * uploaded yet. An all-silent clip is all zeros, not a division by zero.
 */
export function bucketPeaks(samples: Float32Array, bucketCount: number): number[] {
  if (samples.length === 0) return new Array(bucketCount).fill(0);
  const bucketSize = Math.ceil(samples.length / bucketCount);
  const rawPeaks: number[] = [];
  for (let i = 0; i < bucketCount; i++) {
    const start = i * bucketSize;
    const end = Math.min(start + bucketSize, samples.length);
    let max = 0;
    for (let j = start; j < end; j++) {
      max = Math.max(max, Math.abs(samples[j]!));
    }
    rawPeaks.push(max);
  }
  const loudest = Math.max(...rawPeaks, 0);
  if (loudest === 0) return rawPeaks.map(() => 0);
  return rawPeaks.map((p) => Math.round((p / loudest) * 255));
}

/**
 * Buckets a `Uint8Array` of analyser byte data into `barCount` averaged levels, for the recorder's
 * live level meter (`docs/design/frontend.md` §19.1).
 */
export function bucketLevels(data: Uint8Array, barCount: number): number[] {
  if (data.length === 0) return new Array(barCount).fill(0);
  const bucketSize = Math.ceil(data.length / barCount);
  const bars: number[] = [];
  for (let i = 0; i < barCount; i++) {
    const start = i * bucketSize;
    const end = Math.min(start + bucketSize, data.length);
    let sum = 0;
    let count = 0;
    for (let j = start; j < end; j++) {
      sum += data[j]!;
      count++;
    }
    bars.push(count > 0 ? Math.round(sum / count) : 0);
  }
  return bars;
}

/** Whether the browser can record at all — gates the "Record a voice clip" menu item (§19.1). */
export function isVoiceRecordingSupported(): boolean {
  return (
    typeof MediaRecorder !== "undefined" &&
    typeof navigator !== "undefined" &&
    !!navigator.mediaDevices?.getUserMedia
  );
}

/**
 * The `MediaRecorder` MIME type to request, preferring the smallest/most compatible options the
 * browser actually supports (`docs/design/frontend.md` §19.1): Opus-in-WebM (Chrome/Firefox),
 * then MP4/AAC (Safari). `undefined` lets the browser pick its own default, which still uploads
 * fine — the backend transcodes everything to one format regardless.
 */
export function pickRecordingMimeType(
  isTypeSupported: (type: string) => boolean = (type) =>
    typeof MediaRecorder !== "undefined" && MediaRecorder.isTypeSupported(type),
): string | undefined {
  for (const candidate of ["audio/webm;codecs=opus", "audio/mp4"]) {
    if (isTypeSupported(candidate)) return candidate;
  }
  return undefined;
}
