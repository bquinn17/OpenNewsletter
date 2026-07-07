import clsx from "clsx";
import { lazy, Suspense, useRef, useState } from "react";
import { useToggleReaction } from "../../api/queries";
import type { ReactionGroup } from "../../api/types";

interface Props {
  reactions: ReactionGroup[];
  groupId: string;
  cycleId: string;
  questionId: string;
  responseId: string;
}

// Quick-pick defaults shown after the user opens the ＋ menu. Tweak freely —
// the full picker is one click further for anything not on this list.
const DEFAULTS: { emoji: string; label: string }[] = [
  { emoji: "❤️", label: "heart" },
  { emoji: "😮", label: "oh" },
  { emoji: "🔥", label: "fire" },
  { emoji: "😢", label: "cry" },
  { emoji: "🎉", label: "tada" },
  { emoji: "😠", label: "angry" },
];

// emoji-picker-element ships its own ~150KB bundle (with IndexedDB cache) and
// only matters once the user asks for "more", so lazy-load it.
const EmojiPickerPopover = lazy(() => import("./EmojiPickerPopover"));

export function ReactionBar({ reactions, groupId, cycleId, questionId, responseId }: Props) {
  const toggle = useToggleReaction(groupId, cycleId);
  const [quickOpen, setQuickOpen] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);
  const morePickerAnchorRef = useRef<HTMLButtonElement>(null);

  const handleToggle = (emoji: string) => {
    toggle.mutate({ questionId, responseId, emoji });
  };

  return (
    <div className="flex flex-wrap gap-1.5 mt-4 items-center">
      {reactions.map((r) => (
        <button
          key={r.emoji}
          onClick={() => handleToggle(r.emoji)}
          className={clsx(
            "px-2.5 py-1 rounded-full border text-sm font-semibold transition",
            r.reactedByMe
              ? "bg-coral/10 text-coral border-coral/30"
              : "bg-cream border-line text-inkmuted hover:border-ink",
          )}
        >
          {r.emoji} {r.count}
        </button>
      ))}
      <button
        onClick={() => setQuickOpen((s) => !s)}
        aria-label="Add reaction"
        aria-expanded={quickOpen}
        className="px-2.5 py-1 rounded-full bg-cream border border-line text-inkmuted text-sm"
      >
        ＋
      </button>
      {quickOpen && (
        <div className="w-full mt-2 flex flex-wrap items-center gap-1.5 animate-pop">
          {DEFAULTS.map((d) => (
            <button
              key={d.emoji}
              onClick={() => {
                handleToggle(d.emoji);
                setQuickOpen(false);
              }}
              aria-label={`React with ${d.label}`}
              className="px-2.5 py-1 rounded-full bg-white border border-line text-sm hover:border-coral"
            >
              {d.emoji}
            </button>
          ))}
          <button
            ref={morePickerAnchorRef}
            onClick={() => setPickerOpen(true)}
            aria-label="More emoji"
            aria-expanded={pickerOpen}
            className="px-2.5 py-1 rounded-full bg-white border border-line text-xs text-inkmuted hover:border-ink"
          >
            More…
          </button>
        </div>
      )}
      {pickerOpen && (
        <Suspense fallback={null}>
          <EmojiPickerPopover
            anchorRef={morePickerAnchorRef}
            onPick={(emoji) => {
              handleToggle(emoji);
              setPickerOpen(false);
              setQuickOpen(false);
            }}
            onClose={() => setPickerOpen(false)}
          />
        </Suspense>
      )}
    </div>
  );
}
