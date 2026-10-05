import { Button } from "../ui/Button";

type Props = {
  isPublished: boolean;
  disabled: boolean;
  onPublish: () => void;
};

/** The primary publish/save CTA in the response editor's bottom bar. */
export function PublishButton({ isPublished, disabled, onPublish }: Props) {
  return (
    <Button onClick={onPublish} disabled={disabled}>
      {isPublished ? "Save changes" : "Publish"}
    </Button>
  );
}
