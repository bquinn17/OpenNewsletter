import { create } from "zustand";

interface State {
  /** Bumped by `celebrate()`; `ConfettiBurst` replays on each change. */
  burst: number;
  /** Whether the current burst is the bigger "all in" variant (`docs/design/frontend.md` §18). */
  big: boolean;
  celebrate: (options?: { big?: boolean }) => void;
}

/** A confetti burst over whatever page is showing, e.g. after joining a group or publishing. */
export const useCelebration = create<State>((set, get) => ({
  burst: 0,
  big: false,
  celebrate: (options) => set({ burst: get().burst + 1, big: options?.big ?? false }),
}));
