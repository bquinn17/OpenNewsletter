import clsx from "clsx";

type Props = {
  size?: "sm" | "md" | "lg";
  className?: string;
  label?: string;
};

const sizes: Record<NonNullable<Props["size"]>, string> = {
  sm: "w-4 h-4 border-2",
  md: "w-6 h-6 border-2",
  lg: "w-10 h-10 border-[3px]",
};

export function Spinner({ size = "md", className, label = "Loading" }: Props) {
  return (
    <span
      role="status"
      aria-label={label}
      className={clsx(
        "inline-block animate-spin rounded-full border-line border-t-ink",
        sizes[size],
        className,
      )}
    />
  );
}
