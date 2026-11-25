import { forwardRef, useImperativeHandle, useRef } from "react";

export type ImageDropzoneHandle = {
  openPicker: () => void;
};

type Props = {
  /** The hidden input's accessible name — queryable via `getByLabelText` (`docs/design/frontend.md` §18.2). */
  label: string;
  onFilesSelected: (files: File[]) => void;
};

/**
 * The hidden file input behind `ImageUploader`'s "Photo" attach-menu item
 * (`docs/design/frontend.md` §18.2). The visible dashed box is gone — the "+" menu
 * (`AttachMenu`) opens this via `openPicker`, and the whole composer is the drop target
 * (`ResponseEditor`/`CommentComposer`, §18.2), not this element.
 */
export const ImageDropzone = forwardRef<ImageDropzoneHandle, Props>(function ImageDropzone(
  { label, onFilesSelected },
  ref,
) {
  const inputRef = useRef<HTMLInputElement>(null);
  useImperativeHandle(ref, () => ({ openPicker: () => inputRef.current?.click() }));

  return (
    <input
      ref={inputRef}
      type="file"
      multiple
      accept="image/jpeg,image/png,image/webp,image/gif"
      aria-label={label}
      className="sr-only"
      onChange={(e) => {
        onFilesSelected(Array.from(e.target.files ?? []));
        e.target.value = "";
      }}
    />
  );
});
