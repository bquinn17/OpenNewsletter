import { create } from "zustand";

export interface Toast {
  id: string;
  message: string;
  tone: "default" | "success" | "error";
}

interface State {
  toasts: Toast[];
  push: (message: string, tone?: Toast["tone"]) => void;
  dismiss: (id: string) => void;
}

export const useToasts = create<State>((set, get) => ({
  toasts: [],
  push: (message, tone = "default") => {
    const id = Math.random().toString(36).slice(2);
    set({ toasts: [...get().toasts, { id, message, tone }] });
    setTimeout(() => get().dismiss(id), 3500);
  },
  dismiss: (id) => set({ toasts: get().toasts.filter((t) => t.id !== id) }),
}));
