import { AUDIO_PEAK_COUNT, bucketPeaks } from "./voiceClip";

export type DecodedClip = { peaks: number[]; durationMs: number };

type AudioContextLike = {
  decodeAudioData(
    buffer: ArrayBuffer,
  ): Promise<{ duration: number; getChannelData(ch: number): Float32Array }>;
  close(): Promise<void>;
};

/**
 * Decodes a locally-recorded or picked clip's bytes into a 48-bar waveform and its duration
 * (`docs/design/frontend.md` §19.1): "a clip card appears with ... a waveform drawn from the local
 * blob (`decodeAudioData`)". Runs client-side with the Web Audio API, which is why this is a
 * separate module from `voiceClip.ts`'s pure math — `createContext` is injectable for tests, since
 * jsdom has no `AudioContext`.
 */
export async function decodeLocalClip(
  blob: Blob,
  createContext: () => AudioContextLike = () =>
    new (
      window.AudioContext ||
      (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext
    )(),
): Promise<DecodedClip> {
  const ctx = createContext();
  try {
    const buffer = await blob.arrayBuffer();
    const decoded = await ctx.decodeAudioData(buffer);
    return {
      peaks: bucketPeaks(decoded.getChannelData(0), AUDIO_PEAK_COUNT),
      durationMs: Math.round(decoded.duration * 1000),
    };
  } finally {
    void ctx.close();
  }
}
