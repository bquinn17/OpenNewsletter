import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useCelebration } from "../../state/celebration";
import { appStorage } from "../../utils/appStorage";
import { formatInZone } from "../../utils/dates";
import { buttonClassName } from "../ui/Button";

type Props = {
  groupId: string;
  cycleId: string;
  /** Every question of this open edition has a non-empty draft or answer. */
  allAnswered: boolean;
  responseCloseAt: string;
  timezone: string;
};

function storageKey(groupId: string, cycleId: string): string {
  return `celebrated:${groupId}/${cycleId}`;
}

function hasCelebrated(groupId: string, cycleId: string): boolean {
  try {
    return appStorage().getItem(storageKey(groupId, cycleId)) === "1";
  } catch {
    return false;
  }
}

function markCelebrated(groupId: string, cycleId: string): void {
  try {
    appStorage().setItem(storageKey(groupId, cycleId), "1");
  } catch {
    // Best effort — private browsing etc. just means it may show again next time.
  }
}

/**
 * A one-time, full-screen "you completed the newsletter" overlay, shown once every
 * question of an open edition has a non-empty draft or answer (`docs/design/frontend.md`
 * §7.2). Tracked per group+cycle in `appStorage` (set the moment it's shown, not on
 * dismiss) so it doesn't reappear on a later visit to the same edition on this device.
 */
export function CompletionCelebration({
  groupId,
  cycleId,
  allAnswered,
  responseCloseAt,
  timezone,
}: Props) {
  const [open, setOpen] = useState(false);
  const celebrate = useCelebration((s) => s.celebrate);
  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!allAnswered || hasCelebrated(groupId, cycleId)) return;
    markCelebrated(groupId, cycleId);
    celebrate();
    setOpen(true);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `celebrate` is a stable zustand setter.
  }, [allAnswered, groupId, cycleId]);

  useEffect(() => {
    if (!open) return;
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    const focusTimer = setTimeout(() => dialogRef.current?.focus(), 0);
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") setOpen(false);
    }
    document.addEventListener("keydown", onKeyDown);
    return () => {
      clearTimeout(focusTimer);
      document.removeEventListener("keydown", onKeyDown);
      document.body.style.overflow = prevOverflow;
    };
  }, [open]);

  if (!open) return null;

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center p-5">
      <div aria-hidden="true" className="absolute inset-0 bg-ink/60 backdrop-blur-sm" />
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="completion-celebration-heading"
        tabIndex={-1}
        className="relative w-full max-w-sm animate-pop rounded-4xl border border-line bg-white p-6 text-center shadow-pop motion-reduce:animate-none"
      >
        <div aria-hidden="true" className="text-5xl">
          🎉
        </div>
        <h2 id="completion-celebration-heading" className="mt-3 font-display text-2xl font-bold">
          You completed the newsletter!
        </h2>
        <p className="mt-2 text-sm text-inkmuted">
          You can still edit any answer until the deadline,{" "}
          {formatInZone(responseCloseAt, timezone)}.
        </p>
        <button
          type="button"
          onClick={() => setOpen(false)}
          className={buttonClassName({ size: "lg", className: "mt-5 w-full" })}
        >
          Nice!
        </button>
      </div>
    </div>,
    document.body,
  );
}
