import type { components } from "../../types/api";

/**
 * Renders the "asked by" attribution line for a question. Falls back to
 * "Asked anonymously" when `askedBy` is null (i.e. the submitter opted into
 * anonymity and the caller is not a group admin).
 *
 * Visually subtle by default — the question prompt is the headline, this is
 * the byline beneath it.
 */
export function AskedBy({
  askedBy,
  isAnonymous,
  className = "",
}: {
  askedBy: components["schemas"]["AskedBy"] | null;
  isAnonymous: boolean;
  className?: string;
}) {
  const label = askedBy ? `${askedBy.displayName} asked` : isAnonymous ? "Asked anonymously" : null;
  if (!label) return null;
  return (
    <div className={`text-xs font-bold uppercase tracking-widest text-inkmuted ${className}`}>
      {label}
    </div>
  );
}
