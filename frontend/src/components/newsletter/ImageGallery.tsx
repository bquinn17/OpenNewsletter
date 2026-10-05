import type { components } from "../../types/api";
import { referencedImageIds } from "../../utils/markdown";
import { CdnImage } from "../ui/CdnImage";

type Props = {
  images: components["schemas"]["PublishedImageResponse"][];
  body: string;
  groupId: string;
};

/** Grid of an answer's images that aren't referenced inline via an `image:` token in its body. */
export function ImageGallery({ images, body, groupId }: Props) {
  const referenced = referencedImageIds(body);
  const extra = images.filter((image) => !referenced.has(image.imageId));
  if (extra.length === 0) return null;

  return (
    <div aria-label="Additional images" className="mt-3 grid grid-cols-3 gap-2">
      {extra.map((image) => (
        <a
          key={image.imageId}
          href={image.displayUrl}
          target="_blank"
          rel="noopener noreferrer"
          className="ph-img block aspect-square overflow-hidden rounded-xl"
        >
          <CdnImage
            src={image.thumbUrl}
            groupId={groupId}
            alt=""
            className="h-full w-full object-cover"
          />
        </a>
      ))}
    </div>
  );
}
