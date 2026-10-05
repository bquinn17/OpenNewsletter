import { useRef, useState } from "react";
import { refreshMediaAuth, withMediaAuth } from "../../utils/media";

type Props = {
  src: string;
  groupId: string;
  alt: string;
  className?: string;
};

/**
 * An `<img>` for a CDN-served asset that retries once on error after refreshing the group's
 * signed-cookie/query-param auth (`08-media-uploads.md` §9: a stale cookie mid-session is a
 * recoverable 403, not a permanent failure).
 */
export function CdnImage({ src, groupId, alt, className }: Props) {
  const [attempt, setAttempt] = useState(0);
  const retried = useRef(false);

  const handleError = () => {
    if (retried.current) return;
    retried.current = true;
    void refreshMediaAuth(groupId).then(() => setAttempt((n) => n + 1));
  };

  return (
    <img
      key={attempt}
      src={withMediaAuth(src)}
      alt={alt}
      className={className}
      onError={handleError}
    />
  );
}
