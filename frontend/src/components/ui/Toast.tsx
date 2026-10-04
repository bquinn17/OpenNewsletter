import clsx from "clsx";
import { useToasts } from "../../state/toast";

export function ToastStack() {
  const { toasts, dismiss } = useToasts();
  return (
    <div className="fixed bottom-24 left-1/2 z-[60] w-full max-w-md -translate-x-1/2 space-y-2 px-4">
      {toasts.map((t) => (
        <button
          key={t.id}
          onClick={() => dismiss(t.id)}
          className={clsx(
            "block w-full animate-pop rounded-2xl px-4 py-3 text-left text-sm font-semibold shadow-card",
            t.tone === "success" && "bg-mint text-white",
            t.tone === "error" && "bg-coral text-white",
            t.tone === "default" && "bg-ink text-cream",
          )}
        >
          {t.message}
        </button>
      ))}
    </div>
  );
}
