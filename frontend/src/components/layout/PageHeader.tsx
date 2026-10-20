import clsx from "clsx";
import type { ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import { phaseStyle, type EditionPhase } from "../../utils/phase";

interface Props {
  eyebrow?: string;
  title: ReactNode;
  rightSlot?: ReactNode;
  back?: string | true;
  className?: string;
  /** Tints the bar with an edition's phase color; pages outside an edition stay lavender. */
  phase?: EditionPhase;
}

export function PageHeader({ eyebrow, title, rightSlot, back, className, phase }: Props) {
  const navigate = useNavigate();
  return (
    <div
      className={clsx(
        "sticky top-0 z-30 border-b border-ink/5 backdrop-blur",
        phase ? phaseStyle(phase).header : "bg-lavender/85",
        className,
      )}
    >
      <div className="flex items-center gap-3 px-4 py-3">
        {back && (
          <button
            onClick={() => (typeof back === "string" ? navigate(back) : navigate(-1))}
            className="grid h-9 w-9 place-items-center rounded-full border border-line bg-white"
            aria-label="Back"
          >
            ←
          </button>
        )}
        <div className="min-w-0 flex-1">
          {eyebrow && <div className="text-xs text-inkmuted">{eyebrow}</div>}
          <div className="truncate font-display text-lg font-bold leading-tight">{title}</div>
        </div>
        {rightSlot}
      </div>
    </div>
  );
}
