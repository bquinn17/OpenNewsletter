import { useEffect } from "react";
import { X } from "lucide-react";

const AUTO_DISMISS_MS = 3000;

type Props = {
  open: boolean;
  /** Undefined when the edition label isn't loaded yet — the copy then skips it rather than guess. */
  editionLabel: string | undefined;
  onDismiss: () => void;
};

/**
 * Full-screen celebration for publishing the member's last unanswered
 * question in the edition (`docs/design/frontend.md` §7.3, §18). Auto-dismisses after
 * ~3s, announced via `role="status"`/`aria-live` so screen readers get it
 * without the page having to scroll, and stays visible (statically) under
 * reduced motion — only the confetti burst behind it is hidden.
 */
export function AllInOverlay({ open, editionLabel, onDismiss }: Props) {
  useEffect(() => {
    if (!open) return;
    const timer = setTimeout(onDismiss, AUTO_DISMISS_MS);
    return () => clearTimeout(timer);
  }, [open, onDismiss]);

  if (!open) return null;

  return (
    <div
      role="status"
      aria-live="polite"
      className="fixed inset-0 z-[70] grid place-items-center bg-ink/70 p-6 text-center motion-safe:animate-pop-in"
    >
      <div className="relative max-w-sm rounded-4xl bg-cream p-8 shadow-pop">
        <button
          type="button"
          onClick={onDismiss}
          aria-label="Dismiss"
          className="absolute right-3 top-3 grid h-8 w-8 place-items-center rounded-full border border-line bg-white text-ink"
        >
          <X aria-hidden className="h-4 w-4" />
        </button>
        <p className="mt-2 font-display text-2xl font-bold leading-tight text-ink">
          You&apos;re all in{editionLabel ? ` for ${editionLabel}` : ""} 🎉
        </p>
      </div>
    </div>
  );
}
