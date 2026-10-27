import { useRef, useState } from "react";
import clsx from "clsx";

type Props = {
  imageCount: number;
  maxImages: number;
  fileErrors: string[];
  onFilesSelected: (files: File[]) => void;
};

/** Click-or-drag file picker for `ImageUploader` (`docs/design/media.md` §3). */
export function ImageDropzone({ imageCount, maxImages, fileErrors, onFilesSelected }: Props) {
  const [dragOver, setDragOver] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  return (
    <div>
      <div
        role="button"
        tabIndex={0}
        aria-label="Add images"
        onClick={() => inputRef.current?.click()}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            inputRef.current?.click();
          }
        }}
        onDragOver={(e) => {
          e.preventDefault();
          setDragOver(true);
        }}
        onDragLeave={() => setDragOver(false)}
        onDrop={(e) => {
          e.preventDefault();
          setDragOver(false);
          onFilesSelected(Array.from(e.dataTransfer.files));
        }}
        className={clsx(
          "flex cursor-pointer flex-col items-center justify-center rounded-2xl border-2 border-dashed border-line bg-cream px-4 py-6 text-center text-sm text-inkmuted transition focus:outline-none focus:ring-2 focus:ring-coral/30",
          dragOver && "border-grape bg-grape/5",
        )}
      >
        <input
          ref={inputRef}
          type="file"
          multiple
          accept="image/jpeg,image/png,image/webp,image/gif"
          className="sr-only"
          onChange={(e) => {
            onFilesSelected(Array.from(e.target.files ?? []));
            e.target.value = "";
          }}
        />
        <p>
          Drag photos here, or <span className="font-semibold text-grape">choose files</span>
        </p>
        <p className="mt-1 text-xs">
          {imageCount}/{maxImages} image{maxImages === 1 ? "" : "s"} · JPEG, PNG, WebP or GIF · up
          to 15 MB each
        </p>
      </div>

      {fileErrors.length > 0 && (
        <ul
          role="alert"
          aria-live="polite"
          className="mt-2 space-y-1 text-xs font-semibold text-coral"
        >
          {fileErrors.map((message) => (
            <li key={message}>{message}</li>
          ))}
        </ul>
      )}
    </div>
  );
}
