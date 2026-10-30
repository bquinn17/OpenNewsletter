import { PageHeader } from "./PageHeader";
import type { EditionPhase } from "../../utils/phase";

type Props = {
  title: string;
  eyebrow?: string;
  back: string | true;
  phase?: EditionPhase;
  message: string;
  onRetry: () => void;
  className?: string;
};

/** The "couldn't load, try again" shell shared by every page that fetches its content on mount. */
export function PageErrorState({
  title,
  eyebrow = "Couldn't load",
  back,
  phase,
  message,
  onRetry,
  className = "pb-12",
}: Props) {
  return (
    <div className={className}>
      <PageHeader title={title} eyebrow={eyebrow} back={back} phase={phase} />
      <div className="px-5 pt-8 text-center">
        <div className="mb-3 text-5xl">😬</div>
        <p className="font-semibold text-ink">{message}</p>
        <button
          onClick={onRetry}
          className="mt-5 rounded-full bg-ink px-5 py-3 font-semibold text-cream"
        >
          Try again
        </button>
      </div>
    </div>
  );
}
