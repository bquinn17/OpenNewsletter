import { useEffect, useRef, type RefObject } from "react";
import { createPortal } from "react-dom";
// Side-effect import: registers <emoji-picker> as a custom element.
import "emoji-picker-element";

// emoji-picker-element fires a CustomEvent<{ unicode: string; ... }> on pick.
interface EmojiClickDetail {
  unicode: string;
  emoji: { annotation: string };
  skinTone?: number;
}

// React doesn't know about <emoji-picker>; declare it for JSX. We only ever
// attach an event listener via ref, so an empty prop set is enough.
declare module "react" {
  namespace JSX {
    interface IntrinsicElements {
      "emoji-picker": React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement> & {
        class?: string;
      };
    }
  }
}

interface Props {
  anchorRef: RefObject<HTMLElement | null>;
  onPick: (emoji: string) => void;
  onClose: () => void;
}

// Portal-rendered popover anchored to the ＋ button. Closes on outside click,
// Escape, and viewport resize/scroll (since the absolute position would drift).
export default function EmojiPickerPopover({ anchorRef, onPick, onClose }: Props) {
  const popRef = useRef<HTMLDivElement>(null);
  const pickerRef = useRef<HTMLElement>(null);

  // Position the popover above the anchor (or below if there's no room).
  useEffect(() => {
    function reposition() {
      const el = popRef.current;
      const anchor = anchorRef.current;
      if (!el || !anchor) return;
      const rect = anchor.getBoundingClientRect();
      const margin = 8;
      // emoji-picker-element default size is ~360x400; clamp to viewport.
      const popW = Math.min(360, window.innerWidth - 16);
      const popH = 400;
      let left = rect.left + rect.width / 2 - popW / 2;
      left = Math.max(8, Math.min(left, window.innerWidth - popW - 8));
      const placeAbove = rect.top - popH - margin > 8;
      const top = placeAbove
        ? rect.top - popH - margin
        : Math.min(rect.bottom + margin, window.innerHeight - popH - 8);
      el.style.left = `${left}px`;
      el.style.top = `${Math.max(8, top)}px`;
      el.style.width = `${popW}px`;
    }
    reposition();
    window.addEventListener("resize", reposition);
    window.addEventListener("scroll", reposition, true);
    return () => {
      window.removeEventListener("resize", reposition);
      window.removeEventListener("scroll", reposition, true);
    };
  }, [anchorRef]);

  // Outside-click + Escape close. Skip clicks on the anchor itself so the
  // toggling button doesn't immediately reopen us.
  useEffect(() => {
    function onDoc(e: MouseEvent) {
      const el = popRef.current;
      const anchor = anchorRef.current;
      const target = e.target as Node;
      if (el && !el.contains(target) && anchor && !anchor.contains(target)) {
        onClose();
      }
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [anchorRef, onClose]);

  // Wire the picker's emoji-click event. The element is a custom element so we
  // attach via addEventListener rather than React's synthetic-event system.
  useEffect(() => {
    const el = pickerRef.current;
    if (!el) return;
    function handler(e: Event) {
      const detail = (e as CustomEvent<EmojiClickDetail>).detail;
      if (detail?.unicode) onPick(detail.unicode);
    }
    el.addEventListener("emoji-click", handler);
    return () => el.removeEventListener("emoji-click", handler);
  }, [onPick]);

  return createPortal(
    <div
      ref={popRef}
      className="fixed z-50 rounded-2xl shadow-pop border border-line bg-white overflow-hidden animate-pop"
      style={{ height: 400 }}
      role="dialog"
      aria-label="Choose an emoji"
    >
      <emoji-picker ref={pickerRef as unknown as RefObject<HTMLElement>} class="block w-full h-full" />
    </div>,
    document.body,
  );
}
