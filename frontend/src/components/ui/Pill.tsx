import clsx from "clsx";
import type { ReactNode } from "react";

type Tone = "ink" | "coral" | "grape" | "mint" | "sky" | "sun" | "muted";

const tones: Record<Tone, string> = {
  ink: "bg-ink text-cream",
  coral: "bg-coral text-white",
  grape: "bg-grape/10 text-grape",
  mint: "bg-mint/15 text-mint",
  sky: "bg-sky/15 text-sky",
  sun: "bg-sun/30 text-ink",
  muted: "bg-cream border border-line text-inkmuted",
};

interface Props {
  tone?: Tone;
  children: ReactNode;
  className?: string;
}

export function Pill({ tone = "muted", children, className }: Props) {
  return (
    <span
      className={clsx(
        "inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-xs font-semibold",
        tones[tone],
        className,
      )}
    >
      {children}
    </span>
  );
}
