import { useEffect, useState, type ReactNode } from "react";
import { prefersReducedMotion } from "../../utils/motion";

type ProgressQuestion = { questionId: string; prompt: string };

type Props = {
  questions: ProgressQuestion[];
  /** Where the bar sticks: the measured height of the page header above it. */
  stickyTopPx: number;
  /** A question counts as being read once its section's top scrolls above this line. */
  readingLinePx: number;
  navRef?: (node: HTMLElement | null) => void;
  /** Pinned at the bar's right end (the collapse-all toggle). */
  trailing?: ReactNode;
};

/** The id `PublishedQuestion` gives its section, so the bar and the scroll listener can find it. */
export function questionSectionId(questionId: string): string {
  return `question-${questionId}`;
}

/** How far the reading line has moved through each section, from 0 to 1. */
function readFractions(ids: string[], readingLinePx: number): number[] {
  return ids.map((id) => {
    const section = document.getElementById(questionSectionId(id));
    if (!section) return 0;
    const { top, height } = section.getBoundingClientRect();
    if (height <= 0) return top <= readingLinePx ? 1 : 0;
    return Math.min(Math.max((readingLinePx - top) / height, 0), 1);
  });
}

/**
 * Sticky reading progress bar, notched into one segment per question
 * (`docs/design/frontend.md` §7.2). Each segment fills as the reader moves through its question;
 * tapping a segment scrolls to it, smoothly unless `prefers-reduced-motion` is set.
 */
export function QuestionProgress({
  questions,
  stickyTopPx,
  readingLinePx,
  navRef,
  trailing,
}: Props) {
  const [fractions, setFractions] = useState<number[]>([]);
  const questionIds = questions.map((q) => q.questionId).join(",");

  useEffect(() => {
    const ids = questionIds.split(",");
    // Sections are looked up on every scroll rather than once on mount, because they can
    // render after the bar does, and collapsing one changes the others' positions.
    function update() {
      // The reading line can't reach the end of the last section, so the page's bottom fills it.
      const atBottom =
        window.innerHeight + window.scrollY >= document.documentElement.scrollHeight - 2;
      setFractions(atBottom ? ids.map(() => 1) : readFractions(ids, readingLinePx));
    }
    let frame = 0;
    function schedule() {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(update);
    }
    update();
    window.addEventListener("scroll", schedule, { passive: true });
    window.addEventListener("resize", schedule);
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("scroll", schedule);
      window.removeEventListener("resize", schedule);
    };
  }, [questionIds, readingLinePx]);

  function jumpTo(questionId: string) {
    const el = document.getElementById(questionSectionId(questionId));
    if (!el) return;
    el.scrollIntoView({ behavior: prefersReducedMotion() ? "auto" : "smooth", block: "start" });
  }

  if (questions.length < 2) return null;

  // The question being read is the last one the reading line has entered.
  let activeIndex = 0;
  fractions.forEach((f, i) => {
    if (f > 0) activeIndex = i;
  });

  return (
    <nav
      ref={navRef}
      aria-label="Jump to a question"
      style={{ top: `calc(${stickyTopPx}px + env(safe-area-inset-top))` }}
      className="sticky z-20 flex items-center gap-3 border-b border-line bg-mist py-1 pl-5 pr-3"
    >
      <div className="flex min-w-0 flex-1 gap-1">
        {questions.map((q, i) => (
          <button
            key={q.questionId}
            type="button"
            onClick={() => jumpTo(q.questionId)}
            aria-label={`Question ${i + 1}: ${q.prompt}`}
            aria-current={activeIndex === i ? "true" : undefined}
            className="group min-w-0 flex-1 py-3"
          >
            <span className="block h-1.5 overflow-hidden rounded-full bg-ink/10 transition group-hover:bg-ink/20">
              <span
                className="block h-full origin-left rounded-full bg-grape"
                style={{ transform: `scaleX(${fractions[i] ?? 0})` }}
              />
            </span>
          </button>
        ))}
      </div>
      {trailing}
    </nav>
  );
}

export type { ProgressQuestion };
