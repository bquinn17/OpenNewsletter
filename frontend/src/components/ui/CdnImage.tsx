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
 * signed-cookie/query-param auth (`docs/design/media.md` §9: a stale cookie mid-session is a
 * recoverable 403, not a permanent failure).
 *
 * Loaded in CORS mode, so the service worker sees the real status and caches only 200s
 * (`docs/design/frontend.md` §5.2). CloudFront sends the matching credentialed CORS headers. Every CDN `<img>`
 * must use the same mode (`Avatar` too), or a cached copy from one mode fails the other.
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
      crossOrigin="use-credentials"
      alt={alt}
      className={className}
      onError={handleError}
    />
  );
}
