import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { takeStashedInvite } from "../auth/inviteStash";
import { useAuth } from "../auth/useAuth";
import { userManager } from "../auth/userManager";
import { Button } from "../components/ui/Button";
import { Spinner } from "../components/ui/Spinner";

type CallbackState = { invite?: string; returnTo?: string };

function readState(state: unknown): CallbackState {
  if (typeof state !== "object" || state === null) return {};
  // Narrowing `unknown` after the runtime object/property checks above and below.
  const candidate = state as Record<string, unknown>;
  return {
    invite: typeof candidate.invite === "string" ? candidate.invite : undefined,
    returnTo: typeof candidate.returnTo === "string" ? candidate.returnTo : undefined,
  };
}

let callbackPromise: Promise<CallbackState> | null = null;

// Memoized at module scope so React StrictMode's double-invoked effects (or a
// remount) don't send the one-time-use `code` to Cognito twice.
function runCallbackOnce(): Promise<CallbackState> {
  if (!userManager) {
    return Promise.reject(new Error("Auth callback reached without a configured UserManager"));
  }
  callbackPromise ??= userManager.signinRedirectCallback().then((user) => readState(user.state));
  return callbackPromise;
}

export function AuthCallbackPage(): JSX.Element {
  const navigate = useNavigate();
  const { login } = useAuth();
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    runCallbackOnce()
      .then((state) => {
        if (cancelled) return;
        const invite = state.invite ?? takeStashedInvite();
        if (invite) {
          navigate(`/join?code=${encodeURIComponent(invite)}`, { replace: true });
        } else {
          navigate(state.returnTo ?? "/", { replace: true });
        }
      })
      .catch(() => {
        if (!cancelled) setFailed(true);
      });
    return () => {
      cancelled = true;
    };
  }, [navigate]);

  if (failed) {
    return (
      <div className="flex min-h-[50vh] flex-col items-center justify-center gap-4 px-4 text-center">
        <p role="alert">Something went wrong finishing sign-in.</p>
        <Button type="button" onClick={() => void login()}>
          Try signing in again
        </Button>
      </div>
    );
  }

  return (
    <div className="flex min-h-[50vh] items-center justify-center">
      <Spinner size="lg" label="Finishing sign-in" />
    </div>
  );
}
