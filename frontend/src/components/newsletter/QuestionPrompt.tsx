import type { components } from "../../types/api";

type Props = {
  prompt: string;
  kind: components["schemas"]["QuestionKind"];
  askedBy: components["schemas"]["AskedBy"] | null;
  isAnonymous: boolean;
  className?: string;
};

/**
 * One meta line ("Poll · asked anonymously") instead of separate kind/byline labels
 * (`docs/design/frontend.md` §7.2).
 */
function metaLine(
  kind: Props["kind"],
  askedBy: Props["askedBy"],
  isAnonymous: boolean,
): string | null {
  const parts: string[] = [];
  if (kind === "poll") parts.push("Poll");
  if (askedBy) parts.push(`${askedBy.displayName} asked`);
  else if (isAnonymous) parts.push("asked anonymously");
  return parts.length > 0 ? parts.join(" · ") : null;
}

/** The prompt + one meta line shared by the newsletter list, published sections, and the respond page. */
export function QuestionPrompt({ prompt, kind, askedBy, isAnonymous, className = "" }: Props) {
  const meta = metaLine(kind, askedBy, isAnonymous);
  return (
    <div className={className}>
      {meta && (
        <div className="text-xs font-bold uppercase tracking-widest text-inkmuted">{meta}</div>
      )}
      <h2 className="mt-0.5 font-display text-xl font-bold leading-tight">{prompt}</h2>
    </div>
  );
}
