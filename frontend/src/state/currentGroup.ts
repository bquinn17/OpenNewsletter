import { create } from "zustand";
import { persist } from "zustand/middleware";

type State = {
  currentGroupId: string | null;
  setCurrentGroup: (groupId: string) => void;
  /** Forget the selection. Called on logout (`05-auth-flow.md` §8). */
  clear: () => void;
};

export const useCurrentGroup = create<State>()(
  persist(
    (set) => ({
      currentGroupId: null,
      setCurrentGroup: (groupId) => set({ currentGroupId: groupId }),
      clear: () => set({ currentGroupId: null }),
    }),
    { name: "on:currentGroup" },
  ),
);
