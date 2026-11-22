import { useState, type ReactNode } from "react";
import clsx from "clsx";
import { isAllowedGiphyRating } from "../../giphy/giphyClient";
import { useGiphyGif } from "../../giphy/GiphyLookupProvider";
import { giphyLimitRemainingMs } from "../../state/giphyLimit";
import { prefersReducedMotion } from "../../utils/reducedMotion";
import { Spinner } from "../ui/Spinner";

type Props = {
  id: string;
  alt: string;
  /** Answers are capped at content width; comments use the 200px `fixed_width` (`docs/design/frontend.md` §18.3). */
  variant: "answer" | "comment";
};

/** A local demo/mock fixture is already an animated GIF file; real GIPHY gives an mp4. */
function isAnimatedImageFile(url: string): boolean {
  return url.endsWith(".gif");
}

function minutesLeft(ms: number): number {
  return Math.max(1, Math.ceil(ms / 60_000));
}

function PlaceholderCard({
  alt,
  variant,
  children,
}: {
  alt: string;
  variant: "answer" | "comment";
  children: ReactNode;
}) {
  return (
    <div
      className={clsx(
        "flex aspect-[4/3] flex-col items-center justify-center gap-1 rounded-xl border border-line bg-cream p-3 text-center text-xs text-inkmuted",
        variant === "comment" ? "w-[200px]" : "max-w-full",
      )}
    >
      <span className="font-semibold text-ink">{alt || "GIF"}</span>
      {children}
    </div>
  );
}

/** Renders a `giphy:{id}` token (`docs/design/frontend.md` §18.3/§9.2/§9.4). */
export function GiphyEmbed({ id, alt, variant }: Props) {
  const entry = useGiphyGif(id);
  const [reduced] = useState(prefersReducedMotion);
  const [playing, setPlaying] = useState(!reduced);

  if (entry.status === "loading") {
    return (
      <span
        className={clsx(
          "flex aspect-[4/3] items-center justify-center rounded-xl bg-cream",
          variant === "comment" ? "w-[200px]" : "max-w-full",
        )}
      >
        <Spinner size="sm" />
      </span>
    );
  }

  if (entry.status === "rate-limited") {
    return (
      <PlaceholderCard alt={alt} variant={variant}>
        <p>GIF limit reached — it&apos;ll load later.</p>
        <button
          type="button"
          disabled
          className="mt-1 rounded-full bg-grape px-3 py-1 text-xs font-bold text-white disabled:opacity-40"
        >
          Load GIF · {minutesLeft(giphyLimitRemainingMs())} min
        </button>
      </PlaceholderCard>
    );
  }

  if (entry.status === "error") {
    return (
      <PlaceholderCard alt={alt} variant={variant}>
        <p>GIF unavailable offline.</p>
      </PlaceholderCard>
    );
  }

  if (!entry.gif) {
    return (
      <PlaceholderCard alt={alt} variant={variant}>
        <p>GIF unavailable</p>
      </PlaceholderCard>
    );
  }

  const gif = entry.gif;
  if (!isAllowedGiphyRating(gif.rating)) return null;

  const aspectRatio = gif.width > 0 && gif.height > 0 ? gif.width / gif.height : 4 / 3;
  const widthClass = variant === "comment" ? "w-[200px]" : "max-w-full";
  const animated = isAnimatedImageFile(gif.videoUrl);

  return (
    <span
      className={clsx("relative inline-block overflow-hidden rounded-xl", widthClass)}
      style={{ aspectRatio }}
    >
      {playing ? (
        animated ? (
          <img
            src={gif.videoUrl}
            alt={alt}
            referrerPolicy="no-referrer"
            crossOrigin="anonymous"
            className="h-full w-full object-cover"
          />
        ) : (
          <video
            src={gif.videoUrl}
            autoPlay
            loop
            muted
            playsInline
            crossOrigin="anonymous"
            className="h-full w-full object-cover"
            aria-label={alt}
          />
        )
      ) : (
        <button
          type="button"
          onClick={() => setPlaying(true)}
          aria-label={`Play ${alt || "GIF"}`}
          className="relative block h-full w-full"
        >
          <img
            src={gif.stillUrl}
            alt={alt}
            referrerPolicy="no-referrer"
            crossOrigin="anonymous"
            className="h-full w-full object-cover"
          />
          <span className="absolute inset-0 flex items-center justify-center bg-ink/20 text-2xl text-white">
            ▶
          </span>
        </button>
      )}
      {reduced && playing && (
        <button
          type="button"
          onClick={() => setPlaying(false)}
          aria-label={`Pause ${alt || "GIF"}`}
          className="absolute bottom-1 right-1 rounded-full bg-ink/60 px-2 py-0.5 text-xs text-white"
        >
          ❚❚
        </button>
      )}
      <span className="absolute bottom-1 left-1 rounded bg-ink/60 px-1 text-[10px] font-bold text-white">
        GIPHY
      </span>
    </span>
  );
}
