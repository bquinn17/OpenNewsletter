import { useEffect, useState } from "react";
import { Pill } from "../ui/Pill";
import { formatRelative } from "../../utils/dates";
import type { EditorStatus } from "./useResponseEditor";

function useOnlineStatus(): boolean {
  const [online, setOnline] = useState(() =>
    typeof navigator === "undefined" ? true : navigator.onLine,
  );
  useEffect(() => {
    const goOnline = () => setOnline(true);
    const goOffline = () => setOnline(false);
    window.addEventListener("online", goOnline);
    window.addEventListener("offline", goOffline);
    return () => {
      window.removeEventListener("online", goOnline);
      window.removeEventListener("offline", goOffline);
    };
  }, []);
  return online;
}

/** The badge copy for a saved, non-empty, unpublished draft (`docs/design/frontend.md` §7.3, D2). */
export const AUTO_PUBLISH_BADGE = "Draft — publishes automatically at the deadline";

type Props = {
  status: EditorStatus;
  /**
   * The saved answer is a non-empty draft that hasn't been published. It will
   * auto-publish when the response window closes (`docs/design/lifecycle.md` §5.3), so a settled
   * save says so instead of a bare "Saved".
   */
  unpublishedDraft?: boolean;
};

/** Save-state indicator for the response editor (`docs/design/frontend.md` §7.3, §8.3). */
export function DraftStatusBadge({ status, unpublishedDraft = false }: Props) {
  const online = useOnlineStatus();

  if (!online) return <Pill tone="coral">You&apos;re offline</Pill>;

  switch (status.kind) {
    case "saving":
      return <Pill tone="grape">● Saving…</Pill>;
    case "retrying":
      return <Pill tone="coral">Saving failed — retrying</Pill>;
    case "dirty":
      return <Pill tone="sun">Unsaved changes</Pill>;
    case "closed":
      return <Pill tone="mint">Saved {status.at ? formatRelative(status.at) : ""}</Pill>;
    case "saved":
      if (unpublishedDraft) return <Pill tone="mint">{AUTO_PUBLISH_BADGE}</Pill>;
      return <Pill tone="mint">Saved {formatRelative(status.at)}</Pill>;
  }
}
