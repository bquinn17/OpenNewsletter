import { useEffect, useRef, useState, type RefObject } from "react";
import { createPortal } from "react-dom";
import { firstGrapheme, isValidEmoji, normalizeEmoji } from "../../utils/emoji";

// A curated grid (~64) covering the common reaction categories, plus a
// native text input for anything else (`09-engagement.md` §2.5). No external
// emoji-picker dependency — this is all plain Unicode text.
const CURATED_EMOJI = [
  "😀",
  "😂",
  "🤣",
  "😊",
  "😍",
  "🥰",
  "😘",
  "😎",
  "🤔",
  "🙄",
  "😮",
  "😢",
  "😭",
  "😡",
  "🥳",
  "😴",
  "🤯",
  "🥺",
  "😅",
  "😇",
  "🙂",
  "😉",
  "😏",
  "🤩",
  "👍",
  "👎",
  "👏",
  "🙌",
  "🙏",
  "🤝",
  "💪",
  "👋",
  "❤️",
  "🧡",
  "💛",
  "💚",
  "💙",
  "💜",
  "🖤",
  "💔",
  "🔥",
  "✨",
  "🎉",
  "🎊",
  "🎈",
  "🏆",
  "⭐",
  "💯",
  "☕",
  "🍕",
  "🍺",
  "🎵",
  "⚽",
  "🏔️",
  "🚀",
  "🌈",
  "🐶",
  "🐱",
  "🦆",
  "🐢",
  "🌸",
  "☀️",
  "🌧️",
  "❄️",
];

interface Props {
  anchorRef: RefObject<HTMLElement | null>;
  onPick: (emoji: string) => void;
  onClose: () => void;
}

// Portal-rendered popover anchored to the ＋ button. Closes on outside click,
// Escape, and viewport resize/scroll (since the absolute position would drift).
// Focus-return to the anchor on close is the caller's responsibility (so it
// works the same whether `onClose` fires from here or from an emoji pick).
export default function EmojiPickerPopover({ anchorRef, onPick, onClose }: Props) {
  const popRef = useRef<HTMLDivElement>(null);
  const [customInput, setCustomInput] = useState("");

  // Position the popover above the anchor (or below if there's no room).
  useEffect(() => {
    function reposition() {
      const el = popRef.current;
      const anchor = anchorRef.current;
      if (!el || !anchor) return;
      const rect = anchor.getBoundingClientRect();
      const margin = 8;
      const popW = Math.min(300, window.innerWidth - 16);
      const popH = 280;
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

  function submitCustom() {
    if (!isValidEmoji(customInput)) return;
    onPick(normalizeEmoji(customInput));
    setCustomInput("");
  }

  return createPortal(
    <div
      ref={popRef}
      className="fixed z-50 flex animate-pop flex-col overflow-hidden rounded-2xl border border-line bg-white shadow-pop"
      style={{ height: 280 }}
      role="dialog"
      aria-label="Choose an emoji"
    >
      <div className="grid grid-cols-8 gap-0.5 overflow-y-auto p-2">
        {CURATED_EMOJI.map((emoji) => (
          <button
            key={emoji}
            type="button"
            onClick={() => onPick(emoji)}
            aria-label={`React with ${emoji}`}
            className="rounded-lg p-1 text-lg leading-none hover:bg-cream"
          >
            {emoji}
          </button>
        ))}
      </div>
      <div className="flex gap-2 border-t border-line p-2">
        <label htmlFor="emoji-picker-custom-input" className="sr-only">
          Type an emoji
        </label>
        <input
          id="emoji-picker-custom-input"
          type="text"
          value={customInput}
          onChange={(e) => setCustomInput(firstGrapheme(e.target.value))}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              submitCustom();
            }
          }}
          placeholder="Type an emoji…"
          className="min-w-0 flex-1 rounded-lg border border-line px-2 py-1 text-sm focus:outline-none focus:ring-2 focus:ring-coral/30"
        />
        <button
          type="button"
          onClick={submitCustom}
          disabled={!isValidEmoji(customInput)}
          className="rounded-lg bg-grape px-3 py-1 text-xs font-bold text-white disabled:cursor-not-allowed disabled:opacity-40"
        >
          Add
        </button>
      </div>
    </div>,
    document.body,
  );
}
