import { useEffect, useRef, useState, type TouchEvent as ReactTouchEvent } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeSanitize from "rehype-sanitize";
import { Avatar } from "../ui/Avatar";
import { ReactionBar } from "./ReactionBar";
import { CommentList } from "./CommentList";
import { formatRelative } from "../../utils/dates";
import type { AnswerView, ImageMedia } from "../../api/types";

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
    <article className="bg-white rounded-3xl border border-line shadow-soft p-5 mb-4">
      <header className="flex items-center gap-3">
        <Avatar name={answer.displayName} color={answer.avatarColor} />
        <div className="flex-1">
          <div className="font-semibold">{answer.displayName}</div>
          <div className="text-xs text-inkmuted">published {formatRelative(answer.publishedAt)}</div>
        </div>
      </header>
      <div className="prose prose-sm max-w-none mt-3 text-[15px] leading-relaxed">
        <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeSanitize]}>
          {answer.body}
        </ReactMarkdown>
      </div>

      {answer.images.length > 0 && (
        <div
          className={
            answer.images.length === 1
              ? "mt-3"
              : "mt-3 grid grid-cols-2 gap-2"
          }
        >
          {answer.images.map((img, i) => (
            <figure
              key={img.imageId}
              className={answer.images.length === 3 && i === 2 ? "col-span-2" : undefined}
            >
              <button
                type="button"
                onClick={() => setLightboxIndex(i)}
                aria-label={img.alt ?? `Open image ${i + 1}`}
                className={`ph-img rounded-2xl overflow-hidden block w-full group ${
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
                    className="w-full h-full object-cover transition-transform duration-300 group-hover:scale-[1.02]"
                  />
                )}
              </button>
              {img.caption && (
                <figcaption className="mt-1.5 text-xs text-inkmuted italic leading-snug">
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
    <div
      role="dialog"
      aria-modal="true"
      onClick={onClose}
      onTouchStart={onTouchStart}
      onTouchEnd={onTouchEnd}
      className="fixed inset-0 z-50 bg-ink/85 backdrop-blur-sm flex items-center justify-center p-4 animate-pop"
    >
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          onClose();
        }}
        aria-label="Close"
        className="absolute top-4 right-4 w-10 h-10 rounded-full bg-white/15 hover:bg-white/25 text-white grid place-items-center"
      >
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
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
          className="absolute left-4 top-1/2 -translate-y-1/2 w-10 h-10 rounded-full bg-ink/60 hover:bg-ink/75 text-white hidden sm:grid place-items-center"
        >
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
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
          className="absolute right-4 top-1/2 -translate-y-1/2 w-10 h-10 rounded-full bg-ink/60 hover:bg-ink/75 text-white hidden sm:grid place-items-center"
        >
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <path d="M6 3l5 5-5 5" />
          </svg>
        </button>
      )}
      <figure className="max-w-5xl w-full" onClick={(e) => e.stopPropagation()}>
        {img.displayUrl ? (
          <img
            src={img.displayUrl}
            alt={img.alt ?? ""}
            className="w-full max-h-[80vh] object-contain rounded-2xl"
          />
        ) : (
          <div className="ph-img w-full aspect-[3/2] rounded-2xl" />
        )}
        {(img.caption || img.alt) && (
          <figcaption className="text-cream/90 text-sm mt-3 text-center px-6">
            {img.caption ?? img.alt}
          </figcaption>
        )}
        <div className="text-cream/60 text-xs text-center mt-2">
          {index + 1} / {images.length}
        </div>
      </figure>
    </div>
  );
}
