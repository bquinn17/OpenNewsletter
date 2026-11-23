import clsx from "clsx";
import { useEffect, useState } from "react";
import { prefersReducedMotion } from "../../utils/motion";

type ChipQuestion = { questionId: string; prompt: string };

type Props = {
  questions: ChipQuestion[];
  /** Where the row sticks: the measured height of the page header above it. */
  stickyTopPx: number;
  /** A question counts as being read once its section's top scrolls above this line. */
  readingLinePx: number;
  navRef?: (node: HTMLElement | null) => void;
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
export function QuestionChips({ questions, stickyTopPx, readingLinePx, navRef }: Props) {
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
      className="no-scrollbar sticky z-20 flex gap-2 overflow-x-auto border-b border-line bg-mist px-5 py-2"
    >
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
    </nav>
  );
}

export type { ChipQuestion };
