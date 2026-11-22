import clsx from "clsx";
import { SmilePlus } from "lucide-react";
import { useRef, useState } from "react";
import { useToggleReaction } from "../../api/mutations";
import type { components } from "../../types/api";
import { prefersReducedMotion } from "../../utils/motion";
import EmojiPickerPopover from "./EmojiPickerPopover";

type ReactionGroup = components["schemas"]["ReactionGroupResponse"];

type Props = {
  groupId: string;
  cycleId: string;
  questionId: string;
  responseId: string;
  reactionGroups: ReactionGroup[];
};

/** Matches `animate-pop-in`'s duration (`tailwind.config.ts`), so the class is removed once it's done playing. */
const POP_DURATION_MS = 400;
/** Matches `animate-float-up`'s duration. */
const FLOAT_DURATION_MS = 800;

/**
 * `[🔥 4] [🤣 2] [+]` — only the emoji already in use, plus one button that opens the full picker
 * (`docs/design/engagement.md` §2.5, §4.2). Toggling a
 * reaction on pops the pill and floats a cloned emoji up and away
 * (`docs/design/frontend.md` §18), skipped under `prefers-reduced-motion`.
 */
export function ReactionBar({ groupId, cycleId, questionId, responseId, reactionGroups }: Props) {
  const toggle = useToggleReaction(groupId, cycleId, questionId);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [poppingEmoji, setPoppingEmoji] = useState<string | null>(null);
  const [floatingEmoji, setFloatingEmoji] = useState<{ key: number; emoji: string } | null>(null);
  const addButtonRef = useRef<HTMLButtonElement>(null);

  function toggleEmoji(emoji: string, active: boolean) {
    toggle.mutate({ responseId, emoji, active });
    if (!active) return;
    if (prefersReducedMotion()) return;
    setPoppingEmoji(emoji);
    setTimeout(
      () => setPoppingEmoji((current) => (current === emoji ? null : current)),
      POP_DURATION_MS,
    );
    const key = Date.now();
    setFloatingEmoji({ key, emoji });
    setTimeout(
      () => setFloatingEmoji((current) => (current?.key === key ? null : current)),
      FLOAT_DURATION_MS,
    );
  }

  function closePicker() {
    setPickerOpen(false);
    addButtonRef.current?.focus();
  }

  return (
    <div className="relative mt-4 flex flex-wrap items-center gap-1.5">
      {reactionGroups.map((g) => (
        <button
          key={g.emoji}
          type="button"
          onClick={() => toggleEmoji(g.emoji, !g.reactedByMe)}
          aria-pressed={g.reactedByMe}
          aria-label={`React with ${g.emoji}, ${g.count} reaction${g.count === 1 ? "" : "s"}`}
          className={clsx(
            "rounded-full border px-2.5 py-1 text-sm font-semibold transition motion-safe:active:scale-[.97]",
            g.emoji === poppingEmoji && "motion-safe:animate-pop-in",
            g.reactedByMe
              ? "border-coral/30 bg-coral/10 text-coral-ink"
              : "border-line bg-cream text-inkmuted hover:border-ink",
          )}
        >
          {g.emoji} {g.count}
        </button>
      ))}

      <button
        ref={addButtonRef}
        type="button"
        onClick={() => setPickerOpen((open) => !open)}
        aria-label="Add reaction"
        aria-expanded={pickerOpen}
        className="grid rounded-full border border-line bg-cream px-2.5 py-1 text-sm text-inkmuted transition motion-safe:active:scale-[.97]"
      >
        <SmilePlus aria-hidden className="h-4 w-4" />
      </button>

      {floatingEmoji && (
        <span
          key={floatingEmoji.key}
          aria-hidden
          className="pointer-events-none absolute bottom-full left-0 animate-float-up text-lg"
        >
          {floatingEmoji.emoji}
        </span>
      )}

      {pickerOpen && (
        <EmojiPickerPopover
          anchorRef={addButtonRef}
          usedEmoji={reactionGroups.map((g) => g.emoji)}
          onPick={(emoji) => {
            toggleEmoji(emoji, true);
            closePicker();
          }}
          onClose={closePicker}
        />
      )}
    </div>
  );
}
