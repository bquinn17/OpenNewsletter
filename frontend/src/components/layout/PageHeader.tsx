import clsx from "clsx";
import type { ReactNode } from "react";
import { useNavigate } from "react-router-dom";

interface Props {
  eyebrow?: string;
  title: ReactNode;
  rightSlot?: ReactNode;
  back?: string | true;
  className?: string;
}

export function PageHeader({ eyebrow, title, rightSlot, back, className }: Props) {
  const navigate = useNavigate();
  return (
    <div
      className={clsx(
        "sticky top-0 z-30 border-b border-line bg-cream/85 backdrop-blur",
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
