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
    <div className={clsx("sticky top-0 z-30 backdrop-blur bg-cream/85 border-b border-line", className)}>
      <div className="px-4 py-3 flex items-center gap-3">
        {back && (
          <button
            onClick={() => (typeof back === "string" ? navigate(back) : navigate(-1))}
            className="w-9 h-9 rounded-full bg-white border border-line grid place-items-center"
            aria-label="Back"
          >
            ←
          </button>
        )}
        <div className="flex-1 min-w-0">
          {eyebrow && <div className="text-xs text-inkmuted">{eyebrow}</div>}
          <div className="font-display font-bold text-lg leading-tight truncate">{title}</div>
        </div>
        {rightSlot}
      </div>
    </div>
  );
}
