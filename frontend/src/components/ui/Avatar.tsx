import clsx from "clsx";
import { avatarClasses, initial } from "../../utils/avatar";

interface Props {
  name: string;
  color?: string;
  size?: "xs" | "sm" | "md" | "lg" | "xl";
  className?: string;
  ringed?: boolean;
}

const sizes: Record<NonNullable<Props["size"]>, string> = {
  xs: "w-7 h-7 text-xs",
  sm: "w-9 h-9 text-sm",
  md: "w-10 h-10 text-base",
  lg: "w-12 h-12 text-lg",
  xl: "w-16 h-16 text-2xl rounded-3xl",
};

export function Avatar({ name, color, size = "md", className, ringed }: Props) {
  return (
    <div
      className={clsx(
        "rounded-full font-bold grid place-items-center select-none",
        sizes[size],
        avatarClasses(color),
        ringed && "ring-2 ring-white",
        className,
      )}
    >
      {initial(name)}
    </div>
  );
}
