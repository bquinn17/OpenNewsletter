import { useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import clsx from "clsx";
import { useToasts, type Toast } from "../../state/toast";

/** Past this horizontal drag, a swipe dismisses instead of snapping back (`docs/design/frontend.md` §18). */
const SWIPE_DISMISS_PX = 80;

export function ToastStack() {
  const { toasts, dismiss } = useToasts();
  return (
    <div
      aria-live="polite"
      role="status"
      className="fixed bottom-24 left-1/2 z-[60] w-full max-w-md -translate-x-1/2 space-y-2 px-4"
    >
      {toasts.map((t) => (
        <ToastItem key={t.id} toast={t} onDismiss={() => dismiss(t.id)} />
      ))}
    </div>
  );
}

function ToastItem({ toast, onDismiss }: { toast: Toast; onDismiss: () => void }) {
  const [dragX, setDragX] = useState(0);
  const [dragging, setDragging] = useState(false);
  const startXRef = useRef<number | null>(null);

  function onPointerDown(e: ReactPointerEvent<HTMLButtonElement>) {
    startXRef.current = e.clientX;
    setDragging(true);
    // jsdom (unit tests) doesn't implement pointer capture.
    e.currentTarget.setPointerCapture?.(e.pointerId);
  }

  function onPointerMove(e: ReactPointerEvent<HTMLButtonElement>) {
    if (startXRef.current === null) return;
    setDragX(e.clientX - startXRef.current);
  }

  function endDrag() {
    if (startXRef.current === null) return;
    if (Math.abs(dragX) > SWIPE_DISMISS_PX) {
      onDismiss();
    } else {
      setDragX(0);
    }
    startXRef.current = null;
    setDragging(false);
  }

  return (
    <button
      onClick={onDismiss}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
      style={{
        transform: dragX ? `translateX(${dragX}px)` : undefined,
        opacity: Math.max(1 - Math.abs(dragX) / 200, 0.15),
      }}
      className={clsx(
        "block w-full touch-pan-y select-none rounded-2xl px-4 py-3 text-left text-sm font-semibold shadow-card motion-safe:animate-pop-in",
        !dragging && "transition-transform duration-base ease-smooth",
        toast.tone === "success" && "bg-mint text-ink",
        toast.tone === "error" && "bg-danger text-white",
        toast.tone === "default" && "bg-ink text-cream",
      )}
    >
      {toast.message}
    </button>
  );
}
