import clsx from "clsx";
import { useState } from "react";
import { useMediaRetry } from "./useMediaRetry";

type Props = {
  src: string;
  alt: string;
  className?: string;
  /**
   * The size of the variant at `src` (`thumbSize` for a thumb), not the original's. As on any
   * `<img>`, they set the rendered size wherever `className` doesn't.
   */
  width?: number;
  height?: number;
  /**
   * A smaller, already-loaded variant (the 400px thumb) shown blurred behind `src` until it loads
   * (`docs/design/frontend.md` §9). Omit for a plain `<img>` with no blur-up.
   */
  thumbSrc?: string | null;
};

/** A transparent `data:` SVG (allowed by the CSP's `img-src`) for the failed placeholder. */
const PLACEHOLDER_SRC = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg'/%3E";

/**
 * An `<img>` for an S3 presigned-URL media asset (`docs/design/media.md` §4.4). A presigned URL
 * can expire early (the signing Lambda's own role credentials expired first, §4.2), so the first
 * load error for a given image path refreshes the media queries (`refreshMediaUrls`) and remounts
 * the `<img>` once; a second error for the same path gives up and shows a placeholder instead of
 * retrying forever (`useMediaRetry`). Every `src` must come from a query flagged
 * `MEDIA_URLS_META`, or the refresh can't reach it.
 *
 * Loaded in CORS mode (`crossOrigin="anonymous"`) so the service worker sees the real response
 * status and caches only 200s — every media `<img>` must use the same mode (`Avatar` too), or a
 * cached copy from one mode fails a request made in the other.
 */
export function MediaImage({ src, alt, className, width, height, thumbSrc }: Props) {
  const { attempt, failed, handleError } = useMediaRetry(src);
  const [loaded, setLoaded] = useState(false);

  if (failed) {
    // Still an `<img>`, so `width`/`height` and `className` give it the same box as the image.
    return (
      <img
        src={PLACEHOLDER_SRC}
        alt={alt ? `Couldn't load image: ${alt}` : "Couldn't load image"}
        width={width}
        height={height}
        className={clsx("ph-img", className)}
      />
    );
  }

  if (!thumbSrc) {
    return (
      <img
        key={attempt}
        src={src}
        alt={alt}
        crossOrigin="anonymous"
        loading="lazy"
        decoding="async"
        width={width}
        height={height}
        className={className}
        onError={handleError}
      />
    );
  }

  // Mimics a plain `<img width height>`'s auto aspect-ratio box, since the wrapper (not an
  // `<img>`) can't rely on the browser inferring it from attributes.
  const boxStyle =
    width && height ? { aspectRatio: `${width} / ${height}`, maxWidth: `${width}px` } : undefined;

  return (
    <span
      className={clsx("relative block w-full max-w-full overflow-hidden", className)}
      style={boxStyle}
    >
      <img
        src={thumbSrc}
        alt=""
        aria-hidden="true"
        crossOrigin="anonymous"
        className="absolute inset-0 h-full w-full scale-110 object-cover blur-lg"
      />
      <img
        key={attempt}
        src={src}
        alt={alt}
        crossOrigin="anonymous"
        loading="lazy"
        decoding="async"
        onLoad={() => setLoaded(true)}
        onError={handleError}
        className={clsx(
          "absolute inset-0 h-full w-full object-cover transition-opacity motion-safe:duration-base",
          loaded ? "opacity-100" : "opacity-0",
        )}
      />
    </span>
  );
}
