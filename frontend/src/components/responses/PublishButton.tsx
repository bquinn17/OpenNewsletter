import { Check } from "lucide-react";
import { Button } from "../ui/Button";

type Props = {
  isPublished: boolean;
  disabled: boolean;
  onPublish: () => void;
  /**
   * True for the render right after a fresh publish succeeds — the button
   * turns into a ✓ "Published" (`docs/design/frontend.md` §7.3, §18) instead of
   * falling straight to its usual "Save changes" state.
   */
  justPublished?: boolean;
};

/** The primary publish/save CTA in the response editor's bottom bar. */
export function PublishButton({ isPublished, disabled, onPublish, justPublished = false }: Props) {
  if (justPublished) {
    return (
      <Button disabled className="motion-safe:animate-pop-in">
        <Check aria-hidden className="h-4 w-4" />
        Published
      </Button>
    );
  }
  return (
    <Button onClick={onPublish} disabled={disabled}>
      {isPublished ? "Save changes" : "Publish"}
    </Button>
  );
}
