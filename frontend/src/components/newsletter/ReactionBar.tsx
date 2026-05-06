import clsx from "clsx";
import { useState } from "react";
import { useToggleReaction } from "../../api/queries";
import type { ReactionGroup } from "../../api/types";

interface Props {
  reactions: ReactionGroup[];
  groupId: string;
  cycleId: string;
  questionId: string;
  responseId: string;
}

const QUICK_PICKS = ["🔥", "🤣", "❤️", "😍", "👍", "🎉"];

export function ReactionBar({ reactions, groupId, cycleId, questionId, responseId }: Props) {
  const toggle = useToggleReaction(groupId, cycleId);
  const [showPicker, setShowPicker] = useState(false);
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
        onClick={() => setShowPicker((s) => !s)}
        className="px-2.5 py-1 rounded-full bg-cream border border-line text-inkmuted text-sm"
        aria-label="Add reaction"
      >
        ＋
      </button>
      {showPicker && (
        <div className="w-full mt-2 flex flex-wrap gap-1.5 animate-pop">
          {QUICK_PICKS.map((emoji) => (
            <button
              key={emoji}
              onClick={() => {
                handleToggle(emoji);
                setShowPicker(false);
              }}
              className="px-2.5 py-1 rounded-full bg-white border border-line text-sm hover:border-coral"
            >
              {emoji}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
