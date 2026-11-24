import clsx from "clsx";
import { formatClipTime, MAX_AUDIO_SECONDS } from "../../utils/voiceClip";
import type { RecorderState } from "./useVoiceRecorder";

const CAP_MS = MAX_AUDIO_SECONDS * 1000;
const CORAL_THRESHOLD_MS = CAP_MS - 15_000;

type Props = {
  state: RecorderState;
  onStop: () => void;
  onCancel: () => void;
};

/**
 * The recorder bar that replaces the composer's attachment row while recording
 * (`docs/design/frontend.md` §19.1): a pulsing dot, the elapsed/cap timer (coral in the last 15s),
 * a live level meter, and Stop (primary, 48px target) / Cancel (no confirm — nothing's uploaded
 * yet).
 */
export function VoiceClipRecorder({ state, onStop, onCancel }: Props) {
  const coral = state.elapsedMs >= CORAL_THRESHOLD_MS;

  return (
    <div
      role="group"
      aria-label="Recording a voice clip"
      className="flex items-center gap-3 rounded-2xl border border-line bg-cream p-3"
    >
      <span aria-hidden="true" className="h-3 w-3 flex-none animate-pulse rounded-full bg-coral" />
      <span
        aria-live="polite"
        className={clsx("w-20 flex-none text-sm font-semibold tabular-nums", coral && "text-coral")}
      >
        {formatClipTime(state.elapsedMs)} / {formatClipTime(CAP_MS)}
      </span>
      <div className="flex flex-1 items-center gap-0.5" aria-hidden="true">
        {state.levels.map((level, i) => (
          <span
            key={i}
            className="w-1.5 flex-1 rounded-full bg-grape"
            style={{ height: `${4 + (level / 255) * 20}px` }}
          />
        ))}
      </div>
      <button
        type="button"
        onClick={onCancel}
        className="h-10 rounded-full px-3 text-xs font-semibold text-inkmuted"
      >
        Cancel
      </button>
      <button
        type="button"
        onClick={onStop}
        aria-label="Stop recording"
        className="grid h-12 w-12 flex-none place-items-center rounded-full bg-coral text-white"
      >
        <span aria-hidden="true" className="h-4 w-4 rounded-sm bg-white" />
      </button>
    </div>
  );
}
