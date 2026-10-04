import clsx from "clsx";
import { lazy, Suspense, useRef, useState } from "react";
import { useToggleReaction } from "../../mocks/legacyQueries";
import type { ReactionGroup } from "../../mocks/types";

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
    <div className="mt-4 flex flex-wrap items-center gap-1.5">
      {reactions.map((r) => (
        <button
          key={r.emoji}
          onClick={() => handleToggle(r.emoji)}
          className={clsx(
            "rounded-full border px-2.5 py-1 text-sm font-semibold transition",
            r.reactedByMe
              ? "border-coral/30 bg-coral/10 text-coral"
              : "border-line bg-cream text-inkmuted hover:border-ink",
          )}
        >
          {r.emoji} {r.count}
        </button>
      ))}
      <button
        onClick={() => setQuickOpen((s) => !s)}
        aria-label="Add reaction"
        aria-expanded={quickOpen}
        className="rounded-full border border-line bg-cream px-2.5 py-1 text-sm text-inkmuted"
      >
        ＋
      </button>
      {quickOpen && (
        <div className="mt-2 flex w-full animate-pop flex-wrap items-center gap-1.5">
          {DEFAULTS.map((d) => (
            <button
              key={d.emoji}
              onClick={() => {
                handleToggle(d.emoji);
                setQuickOpen(false);
              }}
              aria-label={`React with ${d.label}`}
              className="rounded-full border border-line bg-white px-2.5 py-1 text-sm hover:border-coral"
            >
              {d.emoji}
            </button>
          ))}
          <button
            ref={morePickerAnchorRef}
            onClick={() => setPickerOpen(true)}
            aria-label="More emoji"
            aria-expanded={pickerOpen}
            className="rounded-full border border-line bg-white px-2.5 py-1 text-xs text-inkmuted hover:border-ink"
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
