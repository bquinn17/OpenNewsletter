import { useEffect, useRef, useState, type TouchEvent as ReactTouchEvent } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeSanitize from "rehype-sanitize";
import { Avatar } from "../ui/Avatar";
import { ReactionBar } from "./ReactionBar";
import { CommentList } from "./CommentList";
import { formatRelative } from "../../utils/dates";
import type { AnswerView, ImageMedia } from "../../mocks/types";

interface Props {
  answer: AnswerView;
  groupId: string;
  cycleId: string;
  questionId: string;
}

export function AnswerCard({ answer, groupId, cycleId, questionId }: Props) {
  const [showAllComments, setShowAllComments] = useState(false);
  const [lightboxIndex, setLightboxIndex] = useState<number | null>(null);
  const visibleComments = showAllComments ? answer.comments : answer.comments.slice(0, 2);
  return (
    <article className="mb-4 rounded-3xl border border-line bg-white p-5 shadow-soft">
      <header className="flex items-center gap-3">
        <Avatar name={answer.displayName} color={answer.avatarColor} />
        <div className="flex-1">
          <div className="font-semibold">{answer.displayName}</div>
          <div className="text-xs text-inkmuted">
            published {formatRelative(answer.publishedAt)}
          </div>
        </div>
      </header>
      <div className="prose prose-sm mt-3 max-w-none text-[15px] leading-relaxed">
        <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeSanitize]}>
          {answer.body}
        </ReactMarkdown>
      </div>

      {answer.images.length > 0 && (
        <div className={answer.images.length === 1 ? "mt-3" : "mt-3 grid grid-cols-2 gap-2"}>
          {answer.images.map((img, i) => (
            <figure
              key={img.imageId}
              className={answer.images.length === 3 && i === 2 ? "col-span-2" : undefined}
            >
              <button
                type="button"
                onClick={() => setLightboxIndex(i)}
                aria-label={img.alt ?? `Open image ${i + 1}`}
                className={`ph-img group block w-full overflow-hidden rounded-2xl ${
                  answer.images.length === 1
                    ? "aspect-[3/2]"
                    : answer.images.length === 3 && i === 2
                      ? "aspect-[2/1]"
                      : "aspect-square"
                }`}
              >
                {img.displayUrl && (
                  <img
                    src={img.displayUrl}
                    alt={img.alt ?? ""}
                    className="h-full w-full object-cover transition-transform duration-300 group-hover:scale-[1.02]"
                  />
                )}
              </button>
              {img.caption && (
                <figcaption className="mt-1.5 text-xs italic leading-snug text-inkmuted">
                  {img.caption}
                </figcaption>
              )}
            </figure>
          ))}
        </div>
      )}

      <ReactionBar
        reactions={answer.reactionGroups}
        groupId={groupId}
        cycleId={cycleId}
        questionId={questionId}
        responseId={answer.responseId}
      />

      <CommentList
        comments={visibleComments}
        totalCount={answer.comments.length}
        onShowAll={() => setShowAllComments(true)}
        groupId={groupId}
        cycleId={cycleId}
        questionId={questionId}
        responseId={answer.responseId}
      />

      {lightboxIndex !== null && (
        <Lightbox
          images={answer.images}
          index={lightboxIndex}
          onChange={setLightboxIndex}
          onClose={() => setLightboxIndex(null)}
        />
      )}
    </article>
  );
}

function Lightbox({
  images,
  index,
  onChange,
  onClose,
}: {
  images: ImageMedia[];
  index: number;
  onChange: (n: number) => void;
  onClose: () => void;
}) {
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
      if (e.key === "ArrowRight") onChange(Math.min(images.length - 1, index + 1));
      if (e.key === "ArrowLeft") onChange(Math.max(0, index - 1));
    }
    document.addEventListener("keydown", onKey);
    const prev = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.removeEventListener("keydown", onKey);
      document.body.style.overflow = prev;
    };
  }, [images.length, index, onChange, onClose]);

  const touchStart = useRef<{ x: number; y: number } | null>(null);
  function onTouchStart(e: ReactTouchEvent) {
    const t = e.touches[0];
    if (!t) return;
    touchStart.current = { x: t.clientX, y: t.clientY };
  }
  function onTouchEnd(e: ReactTouchEvent) {
    const start = touchStart.current;
    touchStart.current = null;
    if (!start) return;
    const t = e.changedTouches[0];
    if (!t) return;
    const dx = t.clientX - start.x;
    const dy = t.clientY - start.y;
    // Horizontal swipe: dominant on x-axis and at least 50px
    if (Math.abs(dx) < 50 || Math.abs(dx) < Math.abs(dy)) return;
    if (dx < 0 && index < images.length - 1) onChange(index + 1);
    if (dx > 0 && index > 0) onChange(index - 1);
  }

  const img = images[index]!;
  return (
    // Backdrop click-to-dismiss is a pointer convenience; keyboard users close via Escape.
    // eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-noninteractive-element-interactions
    <div
      role="dialog"
      aria-modal="true"
      onClick={onClose}
      onTouchStart={onTouchStart}
      onTouchEnd={onTouchEnd}
      className="fixed inset-0 z-50 flex animate-pop items-center justify-center bg-ink/85 p-4 backdrop-blur-sm"
    >
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          onClose();
        }}
        aria-label="Close"
        className="absolute right-4 top-4 grid h-10 w-10 place-items-center rounded-full bg-white/15 text-white hover:bg-white/25"
      >
        <svg
          width="16"
          height="16"
          viewBox="0 0 16 16"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          aria-hidden="true"
        >
          <path d="M3 3l10 10M13 3L3 13" />
        </svg>
      </button>
      {index > 0 && (
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onChange(index - 1);
          }}
          aria-label="Previous"
          className="absolute left-4 top-1/2 hidden h-10 w-10 -translate-y-1/2 place-items-center rounded-full bg-ink/60 text-white hover:bg-ink/75 sm:grid"
        >
          <svg
            width="16"
            height="16"
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <path d="M10 3L5 8l5 5" />
          </svg>
        </button>
      )}
      {index < images.length - 1 && (
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onChange(index + 1);
          }}
          aria-label="Next"
          className="absolute right-4 top-1/2 hidden h-10 w-10 -translate-y-1/2 place-items-center rounded-full bg-ink/60 text-white hover:bg-ink/75 sm:grid"
        >
          <svg
            width="16"
            height="16"
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <path d="M6 3l5 5-5 5" />
          </svg>
        </button>
      )}
      {/* Only stops backdrop dismissal from firing; not a real interaction. */}
      {/* eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-noninteractive-element-interactions */}
      <figure className="w-full max-w-5xl" onClick={(e) => e.stopPropagation()}>
        {img.displayUrl ? (
          <img
            src={img.displayUrl}
            alt={img.alt ?? ""}
            className="max-h-[80vh] w-full rounded-2xl object-contain"
          />
        ) : (
          <div className="ph-img aspect-[3/2] w-full rounded-2xl" />
        )}
        {(img.caption || img.alt) && (
          <figcaption className="mt-3 px-6 text-center text-sm text-cream/90">
            {img.caption ?? img.alt}
          </figcaption>
        )}
        <div className="mt-2 text-center text-xs text-cream/60">
          {index + 1} / {images.length}
        </div>
      </figure>
    </div>
  );
}
