import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";
import { appStorage } from "../utils/appStorage";

type State = {
  currentGroupId: string | null;
  setCurrentGroup: (groupId: string) => void;
  /** Forget the selection. Called on logout (`docs/design/auth.md` §8). */
  clear: () => void;
};

export const useCurrentGroup = create<State>()(
  persist(
    (set) => ({
      currentGroupId: null,
      setCurrentGroup: (groupId) => set({ currentGroupId: groupId }),
      clear: () => set({ currentGroupId: null }),
    }),
    { name: "on:currentGroup", storage: createJSONStorage(appStorage) },
  ),
);
