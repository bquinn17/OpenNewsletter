import clsx from "clsx";
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
   * Called (and awaited) instead of the default `refreshMediaUrls` on the first load error.
   * `ImageUploader` passes one that re-GETs its upload, because its URLs live in component state
   * rather than the query cache (`docs/design/media.md` §4.4).
   */
  onExpired?: () => Promise<unknown> | void;
};

/** A transparent `data:` SVG (allowed by the CSP's `img-src`) for the failed placeholder. */
const PLACEHOLDER_SRC = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg'/%3E";

/**
 * An `<img>` for an S3 presigned-URL media asset (`docs/design/media.md` §4.4). A presigned URL
 * can expire early (the signing Lambda's own role credentials expired first, §4.2), so the first
 * load error for a given image path refreshes — via `onExpired`, or `refreshMediaUrls` by default
 * — and remounts the `<img>` once; a second error for the same path gives up and shows a
 * placeholder instead of retrying forever (`useMediaRetry`).
 *
 * Loaded in CORS mode (`crossOrigin="anonymous"`) so the service worker sees the real response
 * status and caches only 200s — every media `<img>` must use the same mode (`Avatar` too), or a
 * cached copy from one mode fails a request made in the other.
 */
export function MediaImage({ src, alt, className, width, height, onExpired }: Props) {
  const { attempt, failed, handleError } = useMediaRetry(src, onExpired);

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
