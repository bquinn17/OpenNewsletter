import clsx from "clsx";
import type { components } from "../../types/api";
import { thumbSize } from "../../utils/media";
import { Spinner } from "../ui/Spinner";
import { MediaImage } from "../ui/MediaImage";
import type { PendingImage } from "./imageUploaderTypes";

type Props = {
  image: PendingImage;
  /** The image's cached `GET /uploads/{id}` row, once it's `ready`. */
  row: components["schemas"]["ImageMediaResponse"] | undefined;
  removing: boolean;
  onRemove: () => void;
  onRetry: () => void;
};

/** One attached image's thumbnail/progress tile, for `ImageUploader`'s grid. */
export function ImageUploaderTile({ image, row, removing, onRemove, onRetry }: Props) {
  return (
    <li
      aria-busy={removing || undefined}
      className={clsx("rounded-2xl border border-line bg-white p-2", removing && "opacity-50")}
    >
      <div className="relative">
        {image.status === "ready" && row?.thumbUrl ? (
          <MediaImage
            src={row.thumbUrl}
            alt="Uploaded image"
            {...thumbSize(row.width, row.height)}
            className="h-24 w-full rounded-xl object-cover"
          />
        ) : (
          <div
            className="flex h-24 w-full flex-col items-center justify-center gap-1 rounded-xl bg-cream px-2 text-center text-xs text-inkmuted"
            aria-live="polite"
          >
            {image.status === "failed" ? (
              <>
                <span className="text-coral">{image.error}</span>
                {image.file && (
                  <button type="button" onClick={onRetry} className="font-semibold text-grape">
                    Retry
                  </button>
                )}
              </>
            ) : (
              <>
                <Spinner size="sm" />
                <span>
                  {image.status === "processing"
                    ? "Processing…"
                    : `Uploading ${Math.round(image.progress * 100)}%`}
                </span>
              </>
            )}
          </div>
        )}
        <button
          type="button"
          onClick={onRemove}
          disabled={removing}
          aria-label={`Remove image ${image.file?.name ?? image.imageId}`}
          className="absolute -right-2 -top-2 grid h-6 w-6 place-items-center rounded-full bg-ink text-xs font-bold text-cream disabled:opacity-50"
        >
          ×
        </button>
      </div>
    </li>
  );
}
