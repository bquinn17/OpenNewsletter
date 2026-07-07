import clsx from "clsx";
import type { ButtonHTMLAttributes, ReactNode } from "react";

type Variant = "primary" | "ink" | "ghost" | "soft";
type Size = "sm" | "md" | "lg";

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
  children: ReactNode;
}

const sizes: Record<Size, string> = {
  sm: "px-3 py-1.5 text-xs",
  md: "px-4 py-2 text-sm",
  lg: "px-5 py-3 text-base",
};

const variants: Record<Variant, string> = {
  primary: "bg-coral text-white shadow-pop hover:brightness-105",
  ink: "bg-ink text-cream hover:opacity-90",
  ghost: "bg-white border border-line text-ink hover:border-ink",
  soft: "bg-cream text-ink hover:bg-line/60",
};

export function Button({ variant = "primary", size = "md", className, children, ...rest }: Props) {
  return (
    <button
      {...rest}
      className={clsx(
        "rounded-full font-semibold inline-flex items-center justify-center gap-1.5 transition disabled:opacity-50 disabled:cursor-not-allowed",
        sizes[size],
        variants[variant],
        className,
      )}
    >
      {children}
    </button>
  );
}
