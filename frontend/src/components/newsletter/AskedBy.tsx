import clsx from "clsx";
import type { components } from "../../types/api";

/**
 * Renders the "asked by" attribution line for a question. Falls back to
 * "Asked anonymously" when `askedBy` is null (i.e. the submitter opted into
 * anonymity and the caller is not a group admin), and reads "Every edition"
 * for the standing question, which nobody asked (`docs/design/lifecycle.md` §14).
 *
 * Visually subtle by default — the question prompt is the headline, this is
 * the byline beneath it.
 */
export function AskedBy({
  askedBy,
  isAnonymous,
  isStanding = false,
  className = "",
}: {
  askedBy: components["schemas"]["AskedBy"] | null;
  isAnonymous: boolean;
  isStanding?: boolean;
  className?: string;
}) {
  const label = isStanding
    ? "Every edition"
    : askedBy
      ? `${askedBy.displayName} asked`
      : isAnonymous
        ? "Asked anonymously"
        : null;
  if (!label) return null;
  return (
    <div className={clsx("text-xs font-bold uppercase tracking-widest text-inkmuted", className)}>
      {label}
    </div>
  );
}
