import { useState } from "react";
import type { components } from "../../types/api";
import { referencedImageIds } from "../../utils/markdown";
import { thumbSize } from "../../utils/media";
import { Lightbox } from "../ui/Lightbox";
import { MediaImage } from "../ui/MediaImage";

type Props = {
  images: components["schemas"]["PublishedImageResponse"][];
  body: string;
};

/**
 * Grid of an answer's images that aren't referenced inline via an `image:` token in its body.
 * Tapping one opens the in-app `Lightbox` (`docs/design/frontend.md` §9) instead of handing the
 * image off to a browser tab.
 */
export function ImageGallery({ images, body }: Props) {
  const [openIndex, setOpenIndex] = useState<number | null>(null);
  const referenced = referencedImageIds(body);
  const extra = images.filter((image) => !referenced.has(image.imageId));
  if (extra.length === 0) return null;

  return (
    <>
      <div aria-label="Additional images" className="mt-3 grid grid-cols-3 gap-2">
        {extra.map((image, index) => (
          <button
            key={image.imageId}
            type="button"
            onClick={() => setOpenIndex(index)}
            aria-label={`Open image ${index + 1} of ${extra.length}`}
            className="ph-img block aspect-square overflow-hidden rounded-xl motion-safe:active:scale-[.98]"
          >
            <MediaImage
              src={image.thumbUrl}
              alt=""
              {...thumbSize(image.width, image.height)}
              className="h-full w-full object-cover"
            />
          </button>
        ))}
      </div>
      {openIndex !== null && (
        <Lightbox
          images={extra.map((image) => ({ src: image.displayUrl, alt: "" }))}
          index={openIndex}
          onClose={() => setOpenIndex(null)}
          onIndexChange={setOpenIndex}
        />
      )}
    </>
  );
}
