import { create } from "zustand";

type State = {
  /** The one `audioId` currently playing, or `null`. `docs/design/frontend.md` §19.2. */
  playingId: string | null;
  setPlaying: (audioId: string | null) => void;
};

/**
 * The shared "only one clip plays at a time" store (`docs/design/frontend.md` §19.2): a `VoiceClip`
 * starting playback calls `setPlaying(audioId)`, and every other instance reacts by pausing its own
 * `<audio>`.
 */
export const useAudioPlayback = create<State>((set) => ({
  playingId: null,
  setPlaying: (audioId) => set({ playingId: audioId }),
}));
