import clsx from "clsx";
import { initial } from "../../utils/avatar";
import { useMediaRetry } from "./useMediaRetry";

interface Props {
  name: string;
  /** Resolved Tailwind classes, e.g. from `utils/avatarColor.ts` (`avatarColorClass`). */
  colorClassName?: string;
  url?: string | null;
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

/**
 * A member/user avatar: the presigned S3 URL as-is (`docs/design/media.md` §4.4) in CORS mode
 * (matching `MediaImage`), with a colored-initials fallback when there's no `url` yet or the
 * image fails to load twice in a row.
 */
export function Avatar({ name, colorClassName, url, size = "md", className, ringed }: Props) {
  const { attempt, failed, handleError } = useMediaRetry(url);

  if (url && !failed) {
    return (
      <img
        key={attempt}
        src={url}
        crossOrigin="anonymous"
        loading="lazy"
        decoding="async"
        alt={name}
        onError={handleError}
        className={clsx(
          "select-none rounded-full object-cover",
          sizes[size],
          ringed && "ring-2 ring-white",
          className,
        )}
      />
    );
  }
  return (
    <div
      className={clsx(
        "grid select-none place-items-center rounded-full font-bold",
        sizes[size],
        colorClassName,
        ringed && "ring-2 ring-white",
        className,
      )}
    >
      {initial(name)}
    </div>
  );
}
