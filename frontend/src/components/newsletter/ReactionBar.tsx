import clsx from "clsx";
import { lazy, Suspense, useRef, useState } from "react";
import { useToggleReaction } from "../../api/mutations";
import type { components } from "../../types/api";

type ReactionGroup = components["schemas"]["ReactionGroupResponse"];

type Props = {
  groupId: string;
  cycleId: string;
  questionId: string;
  responseId: string;
  reactionGroups: ReactionGroup[];
};

// One-tap quick pills for anything not already shown as a group
// (`09-engagement.md` §2.5).
const QUICK_EMOJI = ["🔥", "🤣", "❤️", "😍", "👍", "🎉"];

// The full curated-grid picker is only needed once the caller asks for "+",
// so lazy-load it.
const EmojiPickerPopover = lazy(() => import("./EmojiPickerPopover"));

/** `[🔥 4] [🤣 2] [+]` — `09-engagement.md` §2.5, §4.2. */
export function ReactionBar({ groupId, cycleId, questionId, responseId, reactionGroups }: Props) {
  const toggle = useToggleReaction(groupId, cycleId, questionId);
  const [pickerOpen, setPickerOpen] = useState(false);
  const addButtonRef = useRef<HTMLButtonElement>(null);

  const shown = new Set(reactionGroups.map((g) => g.emoji));
  const quickPicks = QUICK_EMOJI.filter((emoji) => !shown.has(emoji));

  function toggleEmoji(emoji: string, active: boolean) {
    toggle.mutate({ responseId, emoji, active });
  }

  function closePicker() {
    setPickerOpen(false);
    addButtonRef.current?.focus();
  }

  return (
    <div className="mt-4 flex flex-wrap items-center gap-1.5">
      {reactionGroups.map((g) => (
        <button
          key={g.emoji}
          type="button"
          onClick={() => toggleEmoji(g.emoji, !g.reactedByMe)}
          aria-pressed={g.reactedByMe}
          aria-label={`React with ${g.emoji}, ${g.count} reaction${g.count === 1 ? "" : "s"}`}
          className={clsx(
            "rounded-full border px-2.5 py-1 text-sm font-semibold transition",
            g.reactedByMe
              ? "border-coral/30 bg-coral/10 text-coral"
              : "border-line bg-cream text-inkmuted hover:border-ink",
          )}
        >
          {g.emoji} {g.count}
        </button>
      ))}

      {quickPicks.map((emoji) => (
        <button
          key={emoji}
          type="button"
          onClick={() => toggleEmoji(emoji, true)}
          aria-label={`React with ${emoji}`}
          className="rounded-full border border-dashed border-line/70 bg-transparent px-2.5 py-1 text-sm text-inkmuted/60 transition hover:border-ink hover:text-ink"
        >
          {emoji}
        </button>
      ))}

      <button
        ref={addButtonRef}
        type="button"
        onClick={() => setPickerOpen((open) => !open)}
        aria-label="Add reaction"
        aria-expanded={pickerOpen}
        className="rounded-full border border-line bg-cream px-2.5 py-1 text-sm text-inkmuted"
      >
        ＋
      </button>

      {pickerOpen && (
        <Suspense fallback={null}>
          <EmojiPickerPopover
            anchorRef={addButtonRef}
            onPick={(emoji) => {
              toggleEmoji(emoji, true);
              closePicker();
            }}
            onClose={closePicker}
          />
        </Suspense>
      )}
    </div>
  );
}
