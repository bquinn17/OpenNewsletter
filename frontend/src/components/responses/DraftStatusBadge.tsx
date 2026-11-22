import { useEffect, useState } from "react";
import clsx from "clsx";
import { formatRelative } from "../../utils/dates";
import type { EditorStatus } from "./useResponseEditor";

function useOnlineStatus(): boolean {
  const [online, setOnline] = useState(() =>
    typeof navigator === "undefined" ? true : navigator.onLine,
  );
  useEffect(() => {
    const goOnline = () => setOnline(true);
    const goOffline = () => setOnline(false);
    window.addEventListener("online", goOnline);
    window.addEventListener("offline", goOffline);
    return () => {
      window.removeEventListener("online", goOnline);
      window.removeEventListener("offline", goOffline);
    };
  }, []);
  return online;
}

/** The badge copy for a saved, non-empty, unpublished draft (`docs/design/frontend.md` §7.3, D2). */
export const AUTO_PUBLISH_BADGE = "Draft — publishes automatically at the deadline";

export type SaveStateTone = "grape" | "coral" | "sun" | "mint";

const DOT_TONES: Record<SaveStateTone, string> = {
  grape: "bg-grape",
  coral: "bg-coral",
  sun: "bg-sun",
  mint: "bg-mint",
};

type SaveStateDotProps = {
  tone: SaveStateTone;
  label: string;
  /** Pulses the dot while a save is in flight (`docs/design/frontend.md` §18). */
  pulsing?: boolean;
  /** Draws a small check instead of the dot, for a just-settled save. */
  justSaved?: boolean;
};

/**
 * A dot (or, once a save settles, a small drawn check) plus a short label —
 * the save-state line shown above the bottom bar's buttons, not a wrapping
 * badge beside them (`docs/design/frontend.md` §7.3, §18). Shared by the text
 * editor (via `DraftStatusBadge`) and the poll picker's save state.
 */
export function SaveStateDot({
  tone,
  label,
  pulsing = false,
  justSaved = false,
}: SaveStateDotProps) {
  return (
    <div className="flex items-center gap-2 text-xs text-cream/90">
      {justSaved ? (
        <svg
          key={label}
          viewBox="0 0 16 16"
          aria-hidden="true"
          className="h-3.5 w-3.5 shrink-0 text-mint"
        >
          <path
            d="M3 8.5l3.5 3.5L13 4.5"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            pathLength={1}
            style={{ strokeDasharray: 1, strokeDashoffset: 1 }}
            className="motion-safe:animate-draw"
          />
        </svg>
      ) : (
        <span
          aria-hidden="true"
          className={clsx(
            "h-2 w-2 shrink-0 rounded-full",
            DOT_TONES[tone],
            pulsing && "motion-safe:animate-pulse-soft",
          )}
        />
      )}
      <span>{label}</span>
    </div>
  );
}

type Props = {
  status: EditorStatus;
  /**
   * The saved answer is a non-empty draft that hasn't been published. It will
   * auto-publish when the response window closes (`docs/design/lifecycle.md` §5.3), so a settled
   * save says so instead of a bare "Saved".
   */
  unpublishedDraft?: boolean;
};

/** Save-state indicator for the response editor (`docs/design/frontend.md` §7.3, §8.3, §18). */
export function DraftStatusBadge({ status, unpublishedDraft = false }: Props) {
  const online = useOnlineStatus();

  if (!online) return <SaveStateDot tone="coral" label="You're offline" />;

  switch (status.kind) {
    case "saving":
      return <SaveStateDot tone="grape" label="Saving…" pulsing />;
    case "retrying":
      return <SaveStateDot tone="coral" label="Saving failed — retrying" pulsing />;
    case "dirty":
      return <SaveStateDot tone="sun" label="Unsaved changes" />;
    case "closed":
      return (
        <SaveStateDot
          key={status.at ?? "closed"}
          tone="mint"
          label={`Saved ${status.at ? formatRelative(status.at) : ""}`}
          justSaved
        />
      );
    case "saved":
      if (unpublishedDraft) {
        return <SaveStateDot tone="mint" label={AUTO_PUBLISH_BADGE} justSaved />;
      }
      return (
        <SaveStateDot
          key={status.at}
          tone="mint"
          label={`Saved ${formatRelative(status.at)}`}
          justSaved
        />
      );
  }
}
