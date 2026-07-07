import { create } from "zustand";
import { persist } from "zustand/middleware";

interface State {
  currentGroupId: string | null;
  setCurrentGroup: (groupId: string) => void;
}

export const useCurrentGroup = create<State>()(
  persist(
    (set) => ({
      currentGroupId: "g_trail",
      setCurrentGroup: (groupId) => set({ currentGroupId: groupId }),
    }),
    { name: "on:currentGroup" },
  ),
);
