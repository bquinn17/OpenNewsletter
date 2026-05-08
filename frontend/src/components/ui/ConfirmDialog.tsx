import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import clsx from "clsx";

interface Props {
  open: boolean;
  title: string;
  message?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  // Visual treatment for the confirm button. "danger" → coral, otherwise ink.
  tone?: "danger" | "default";
  busy?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

// Reusable replacement for window.confirm(). Renders into document.body via a
// portal so it can sit above any stacking context, locks body scroll while open,
// and supports Escape / backdrop-click to cancel.
export function ConfirmDialog({
  open,
  title,
  message,
  confirmLabel = "Confirm",
  cancelLabel = "Cancel",
  tone = "default",
  busy = false,
  onConfirm,
  onCancel,
}: Props) {
  const confirmRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!open) return;
    // Focus the confirm button on open so keyboard users can act immediately,
    // and snap-restore focus / scroll on close.
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    const t = setTimeout(() => confirmRef.current?.focus(), 0);
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape" && !busy) onCancel();
    }
    document.addEventListener("keydown", onKey);
    return () => {
      clearTimeout(t);
      document.removeEventListener("keydown", onKey);
      document.body.style.overflow = prevOverflow;
    };
  }, [open, busy, onCancel]);

  if (!open) return null;

  return createPortal(
    <div
      role="dialog"
      aria-modal="true"
      aria-label={title}
      className="fixed inset-0 z-50 flex items-center justify-center p-5"
    >
      <button
        type="button"
        aria-label="Dismiss"
        onClick={() => !busy && onCancel()}
        className="absolute inset-0 bg-ink/40 backdrop-blur-sm"
        tabIndex={-1}
      />
      <div className="relative w-full max-w-sm bg-white rounded-3xl shadow-pop border border-line p-5 animate-pop">
        <h2 className="font-display text-xl font-bold leading-tight">{title}</h2>
        {message && <p className="text-sm text-inkmuted mt-2 leading-relaxed">{message}</p>}
        <div className="flex justify-end gap-2 mt-5">
          <button
            type="button"
            onClick={onCancel}
            disabled={busy}
            className="rounded-full px-4 py-2 text-sm font-semibold bg-cream text-ink hover:bg-line/60 disabled:opacity-50"
          >
            {cancelLabel}
          </button>
          <button
            ref={confirmRef}
            type="button"
            onClick={onConfirm}
            disabled={busy}
            className={clsx(
              "rounded-full px-4 py-2 text-sm font-bold text-white transition disabled:opacity-60 disabled:cursor-not-allowed",
              tone === "danger" ? "bg-coral shadow-pop hover:brightness-105" : "bg-ink hover:opacity-90",
            )}
          >
            {busy ? "Working…" : confirmLabel}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
