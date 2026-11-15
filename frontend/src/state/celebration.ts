import { create } from "zustand";

interface State {
  /** Bumped by `celebrate()`; `ConfettiBurst` replays on each change. */
  burst: number;
  celebrate: () => void;
}

/** A confetti burst over whatever page is showing, e.g. after joining a group. */
export const useCelebration = create<State>((set, get) => ({
  burst: 0,
  celebrate: () => set({ burst: get().burst + 1 }),
}));
