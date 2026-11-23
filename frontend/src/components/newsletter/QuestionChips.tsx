import clsx from "clsx";
import { useEffect, useState, type ReactNode } from "react";
import { prefersReducedMotion } from "../../utils/motion";

type ChipQuestion = { questionId: string; prompt: string };

type Props = {
  questions: ChipQuestion[];
  /** Where the row sticks: the measured height of the page header above it. */
  stickyTopPx: number;
  /** A question counts as being read once its section's top scrolls above this line. */
  readingLinePx: number;
  navRef?: (node: HTMLElement | null) => void;
  /** Pinned at the row's right end, outside the scrolling chips (the collapse-all toggle). */
  trailing?: ReactNode;
};

/** The id `PublishedQuestion` gives its section, so chips and the scroll listener can find it. */
export function questionSectionId(questionId: string): string {
  return `question-${questionId}`;
}

/**
 * Sticky, horizontally scrolling row of chips, one per question
 * (`docs/design/frontend.md` §7.2). The chip of the question being read is highlighted; tapping a
 * chip scrolls to it, smoothly unless `prefers-reduced-motion` is set.
 */
export function QuestionChips({ questions, stickyTopPx, readingLinePx, navRef, trailing }: Props) {
  const [activeId, setActiveId] = useState<string | null>(questions[0]?.questionId ?? null);
  const questionIds = questions.map((q) => q.questionId).join(",");

  useEffect(() => {
    const ids = questionIds.split(",");
    // Sections are looked up on every scroll rather than once on mount, because they can
    // render after the chips do.
    function update() {
      let current = ids[0] ?? null;
      for (const id of ids) {
        const section = document.getElementById(questionSectionId(id));
        if (section && section.getBoundingClientRect().top <= readingLinePx) current = id;
      }
      setActiveId(current);
    }
    let frame = 0;
    function onScroll() {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(update);
    }
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("scroll", onScroll);
    };
  }, [questionIds, readingLinePx]);

  function jumpTo(questionId: string) {
    const el = document.getElementById(questionSectionId(questionId));
    if (!el) return;
    el.scrollIntoView({ behavior: prefersReducedMotion() ? "auto" : "smooth", block: "start" });
  }

  if (questions.length < 2) return null;

  return (
    <nav
      ref={navRef}
      aria-label="Jump to a question"
      style={{ top: `calc(${stickyTopPx}px + env(safe-area-inset-top))` }}
      className="sticky z-20 flex items-center gap-2 border-b border-line bg-mist py-2 pl-5 pr-3"
    >
      <div className="no-scrollbar flex min-w-0 flex-1 gap-2 overflow-x-auto">
        {questions.map((q, i) => (
          <button
            key={q.questionId}
            type="button"
            onClick={() => jumpTo(q.questionId)}
            aria-current={activeId === q.questionId ? "true" : undefined}
            className={clsx(
              "shrink-0 rounded-full border px-3 py-1.5 text-xs font-semibold transition motion-safe:active:scale-[.97]",
              activeId === q.questionId
                ? "border-coral/40 bg-coral/15 text-coral-ink"
                : "border-line bg-white text-inkmuted",
            )}
          >
            Q{i + 1}
          </button>
        ))}
      </div>
      {trailing}
    </nav>
  );
}

export type { ChipQuestion };
