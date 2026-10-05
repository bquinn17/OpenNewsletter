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

type Props = {
  status: EditorStatus;
};

/** Save-state indicator for the response editor (`04-frontend-architecture.md` §8.3). */
export function DraftStatusBadge({ status }: Props) {
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
      return <Pill tone="mint">Saved {formatRelative(status.at)}</Pill>;
  }
}
