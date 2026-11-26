import { AUDIO_PEAK_COUNT, bucketPeaks } from "./voiceClip";

const SAMPLE_RATE = 8000;
const DURATION_SECONDS = 3;
// Two gentle, close tones (a soft "hum"), well under the ear-fatiguing range.
const TONE_HZ = [330, 392];

/** 16-bit little-endian PCM WAV bytes for a short, synthesized hum — no shipped audio file, no
 * licensing (`docs/design/frontend.md` §19.4): a couple of quiet sine tones with a fade in/out. */
function synthesizeWavBytes(): { bytes: Uint8Array; samples: Float32Array } {
  const sampleCount = SAMPLE_RATE * DURATION_SECONDS;
  const samples = new Float32Array(sampleCount);
  for (let i = 0; i < sampleCount; i++) {
    const t = i / SAMPLE_RATE;
    const fade = Math.min(1, Math.min(t, DURATION_SECONDS - t) * 4);
    let value = 0;
    for (const hz of TONE_HZ) value += Math.sin(2 * Math.PI * hz * t);
    samples[i] = (value / TONE_HZ.length) * 0.2 * fade;
  }

  const dataSize = sampleCount * 2;
  const buffer = new ArrayBuffer(44 + dataSize);
  const view = new DataView(buffer);
  const writeString = (offset: number, text: string) => {
    for (let i = 0; i < text.length; i++) view.setUint8(offset + i, text.charCodeAt(i));
  };
  writeString(0, "RIFF");
  view.setUint32(4, 36 + dataSize, true);
  writeString(8, "WAVE");
  writeString(12, "fmt ");
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true); // PCM
  view.setUint16(22, 1, true); // mono
  view.setUint32(24, SAMPLE_RATE, true);
  view.setUint32(28, SAMPLE_RATE * 2, true);
  view.setUint16(32, 2, true);
  view.setUint16(34, 16, true);
  writeString(36, "data");
  view.setUint32(40, dataSize, true);
  for (let i = 0; i < sampleCount; i++) {
    view.setInt16(44 + i * 2, Math.max(-1, Math.min(1, samples[i]!)) * 32767, true);
  }
  return { bytes: new Uint8Array(buffer), samples };
}

export type SynthesizedClip = { url: string; durationMs: number; peaks: number[] };

/**
 * One short, synthesized voice clip for the demo (`docs/design/frontend.md` §19.4): rendered to a
 * WAV at runtime, with no shipped audio file and so no `CREDITS.txt`/licensing entry, and no
 * off-site request. Computed once at module load, since `RICH_PUBLISHED_QUESTIONS`'s fixtures
 * are themselves evaluated eagerly.
 */
export function synthesizeDemoClip(): SynthesizedClip {
  const { bytes, samples } = synthesizeWavBytes();
  const peaks = bucketPeaks(samples, AUDIO_PEAK_COUNT);
  const durationMs = DURATION_SECONDS * 1000;

  if (typeof Blob === "function" && typeof URL !== "undefined" && URL.createObjectURL) {
    const blob = new Blob([bytes.buffer as ArrayBuffer], { type: "audio/wav" });
    return { url: URL.createObjectURL(blob), durationMs, peaks };
  }

  // jsdom (Vitest) has no `createObjectURL`; a data URL works just as well for wiring tests.
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  const base64 = typeof btoa === "function" ? btoa(binary) : "";
  return { url: `data:audio/wav;base64,${base64}`, durationMs, peaks };
}
