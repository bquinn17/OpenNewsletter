import { useEffect, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { ErrorResponse } from "oidc-client-ts";
import { takeStashedInvite } from "../auth/inviteStash";
import { safeReturnTo } from "../auth/returnTo";
import { userManager } from "../auth/userManager";
import { buttonClassName } from "../components/ui/Button";

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

/** The IdP's or Cognito's OAuth `error`, when the redirect carried one. */
function oauthError(error: unknown): string | null {
  return error instanceof ErrorResponse ? error.error : null;
}

export function AuthCallbackPage(): JSX.Element {
  const navigate = useNavigate();
  // `undefined` while finishing; then the OAuth error code, or `null` for any other failure.
  const [failure, setFailure] = useState<string | null | undefined>(undefined);

  useEffect(() => {
    let cancelled = false;
    runCallbackOnce()
      .then((state) => {
        if (cancelled) return;
        const invite = state.invite ?? takeStashedInvite();
        if (invite) {
          navigate(`/join?code=${encodeURIComponent(invite)}`, { replace: true });
        } else {
          navigate(safeReturnTo(state.returnTo), { replace: true });
        }
      })
      .catch((error: unknown) => {
        console.error("Sign-in callback failed", error);
        if (!cancelled) setFailure(oauthError(error));
      });
    return () => {
      cancelled = true;
    };
  }, [navigate]);

  // Outside `AppShell`, so it draws the app's backdrop itself.
  return (
    <div className="canvas-bg min-h-screen">
      <div className="app-container flex flex-col items-center justify-center px-6 text-center">
        {failure === undefined ? (
          <div role="status" aria-label="Finishing sign-in" className="flex flex-col items-center">
            <div className="hero-feb grid h-20 w-20 place-items-center rounded-4xl text-4xl shadow-card">
              <span aria-hidden="true" className="animate-bounce motion-reduce:animate-none">
                ✉️
              </span>
            </div>
            <p className="mt-6 font-display text-2xl font-bold">Signing you in…</p>
            <p className="mt-1 text-sm text-inkmuted">Fetching your group&apos;s latest edition.</p>
          </div>
        ) : (
          <div className="max-w-sm">
            <div
              aria-hidden="true"
              className="mx-auto grid h-20 w-20 place-items-center rounded-4xl bg-white text-4xl shadow-card"
            >
              🙈
            </div>
            <h1 className="mt-6 font-display text-2xl font-bold">
              {failure === "access_denied" ? "Sign-in was cancelled" : "That didn't quite work"}
            </h1>
            <p role="alert" className="mt-2 text-inkmuted">
              {failure === "access_denied"
                ? "No harm done. Pick an account whenever you're ready."
                : "Something went wrong finishing sign-in. Give it another go."}
            </p>
            <Link to="/signin" className={buttonClassName({ size: "lg", className: "mt-6" })}>
              Try again
            </Link>
          </div>
        )}
      </div>
    </div>
  );
}
