import type { components } from "../../types/api";
import { AskedBy } from "./AskedBy";

type Props = {
  prompt: string;
  kind: components["schemas"]["QuestionKind"];
  askedBy: components["schemas"]["AskedBy"] | null;
  isAnonymous: boolean;
  className?: string;
};

/** The prompt + "asked by" byline shared by the newsletter list, published sections, and the respond page. */
export function QuestionPrompt({ prompt, kind, askedBy, isAnonymous, className = "" }: Props) {
  return (
    <div className={className}>
      {kind === "poll" && (
        <div className="text-xs font-bold uppercase tracking-widest text-grape">Poll</div>
      )}
      <AskedBy askedBy={askedBy} isAnonymous={isAnonymous} />
      <h2 className="mt-0.5 font-display text-xl font-bold leading-tight">{prompt}</h2>
    </div>
  );
}
