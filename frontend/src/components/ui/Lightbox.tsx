import { ChevronLeft, ChevronRight, X } from "lucide-react";
import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";

export type LightboxImage = {
  src: string;
  alt: string;
};

type Props = {
  images: LightboxImage[];
  index: number;
  onClose: () => void;
  onIndexChange: (index: number) => void;
};

/** Swipe gestures shorter than this are a tap, not a page change. */
const SWIPE_THRESHOLD_PX = 50;

/**
 * An in-app image viewer for an answer's gallery or a comment's attached
 * image (`docs/design/frontend.md` §9), so the installed PWA never hands an
 * image off to a browser tab. Moves focus in on open and restores it to
 * whatever was focused before on close, closes on Escape, and steps between
 * images with the arrow keys or a touch swipe.
 */
export function Lightbox({ images, index, onClose, onIndexChange }: Props) {
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const previouslyFocused = useRef<HTMLElement | null>(null);
  const touchStartX = useRef<number | null>(null);
  const image = images[index];

  useEffect(() => {
    previouslyFocused.current = document.activeElement as HTMLElement | null;
    closeButtonRef.current?.focus();
    return () => previouslyFocused.current?.focus();
  }, []);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
      if (e.key === "ArrowLeft" && index > 0) onIndexChange(index - 1);
      if (e.key === "ArrowRight" && index < images.length - 1) onIndexChange(index + 1);
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [index, images.length, onClose, onIndexChange]);

  function onTouchStart(e: React.TouchEvent) {
    touchStartX.current = e.touches[0]?.clientX ?? null;
  }

  function onTouchEnd(e: React.TouchEvent) {
    const startX = touchStartX.current;
    touchStartX.current = null;
    if (startX === null) return;
    const deltaX = (e.changedTouches[0]?.clientX ?? startX) - startX;
    if (Math.abs(deltaX) < SWIPE_THRESHOLD_PX) return;
    if (deltaX < 0 && index < images.length - 1) onIndexChange(index + 1);
    if (deltaX > 0 && index > 0) onIndexChange(index - 1);
  }

  if (!image) return null;

  return createPortal(
    <div
      role="dialog"
      aria-modal="true"
      aria-label={`Image ${index + 1} of ${images.length}`}
      className="fixed inset-0 z-50 flex flex-col bg-ink/95"
      onTouchStart={onTouchStart}
      onTouchEnd={onTouchEnd}
    >
      <div className="flex items-center justify-end p-3 pt-[max(.75rem,env(safe-area-inset-top))]">
        <button
          ref={closeButtonRef}
          type="button"
          onClick={onClose}
          aria-label="Close"
          className="grid h-10 w-10 place-items-center rounded-full bg-white/10 text-white motion-safe:active:scale-[.97]"
        >
          <X aria-hidden className="h-5 w-5" />
        </button>
      </div>

      <div className="relative flex min-h-0 flex-1 items-center justify-center px-4">
        {index > 0 && (
          <button
            type="button"
            onClick={() => onIndexChange(index - 1)}
            aria-label="Previous image"
            className="absolute left-2 grid h-10 w-10 place-items-center rounded-full bg-white/10 text-white motion-safe:active:scale-[.97]"
          >
            <ChevronLeft aria-hidden className="h-6 w-6" />
          </button>
        )}
        <img src={image.src} alt={image.alt} className="max-h-full max-w-full object-contain" />
        {index < images.length - 1 && (
          <button
            type="button"
            onClick={() => onIndexChange(index + 1)}
            aria-label="Next image"
            className="absolute right-2 grid h-10 w-10 place-items-center rounded-full bg-white/10 text-white motion-safe:active:scale-[.97]"
          >
            <ChevronRight aria-hidden className="h-6 w-6" />
          </button>
        )}
      </div>

      {image.alt && (
        <p className="px-4 pb-[max(1rem,env(safe-area-inset-bottom))] text-center text-sm text-white/80">
          {image.alt}
        </p>
      )}
    </div>,
    document.body,
  );
}
