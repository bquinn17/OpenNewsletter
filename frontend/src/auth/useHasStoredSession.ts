import { useEffect, useState } from "react";
import { userManager } from "./userManager";

/**
 * Whether this browser holds a real session `AuthProvider` would resume. The demo runs as a mock
 * user and never uses that session, but its banner tells a signed-in visitor it's still there.
 */
export function useHasStoredSession(): boolean {
  const [hasSession, setHasSession] = useState(false);

  useEffect(() => {
    if (!userManager) return;
    let cancelled = false;
    userManager
      .getUser()
      .then((stored) => {
        if (!cancelled) setHasSession(!!stored && (!stored.expired || !!stored.refresh_token));
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, []);

  return hasSession;
}
