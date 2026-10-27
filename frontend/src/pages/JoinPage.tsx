import { useEffect, useRef, useState } from "react";
import { useLocation, useNavigate, useSearchParams } from "react-router-dom";
import { useAuth } from "../auth/useAuth";
import { stashInvite, takeStashedInvite } from "../auth/inviteStash";
import { useRedeemInvite } from "../api/mutations";
import { ApiError } from "../api/client";
import { useCurrentGroup } from "../state/currentGroup";
import { useToasts } from "../state/toast";
import { JoinForm } from "../components/join/JoinForm";
import { Spinner } from "../components/ui/Spinner";

function messageForError(error: unknown): string {
  if (!(error instanceof ApiError)) return "Couldn't redeem that invite.";
  switch (error.code) {
    case "INVITE_INVALID":
      return "That invite code isn't valid.";
    case "INVITE_EXPIRED":
      return "That invite has expired — ask for a new one.";
    case "INVITE_CONSUMED":
      return "That invite has already been used.";
    case "MEMBER_CAP_REACHED":
      return "That group is full.";
    // A `boundEmail` invite redeemed by another, or unverified, email (`docs/design/api.md` §3, D5).
    case "INVITE_EMAIL_MISMATCH":
      return "This invite is for a different email address. Sign in with Google using the invited email.";
    default:
      return error.problem?.detail ?? error.message;
  }
}

export function JoinPage() {
  const [search] = useSearchParams();
  const code = search.get("code");
  // Each `JoinForm` submit is a new navigation with its own key, even for the
  // code already in the URL, so keying attempts on it lets the user retry a
  // code that failed (e.g. once an admin raises the member cap).
  const { key: attemptKey } = useLocation();
  const { status, login } = useAuth();
  const navigate = useNavigate();
  const setCurrentGroup = useCurrentGroup((s) => s.setCurrentGroup);
  const pushToast = useToasts((s) => s.push);
  const redeem = useRedeemInvite();
  const attemptedKey = useRef<string | null>(null);
  const [redeemError, setRedeemError] = useState<{ attemptKey: string; error: unknown } | null>(
    null,
  );
  const stashedForCode = useRef<string | null>(null);
  // StrictMode's simulated unmount flips this back to `true` on remount; a real
  // unmount (the user tapped away mid-redeem) leaves it `false`.
  const mounted = useRef(false);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  // Logged out with a code in the URL: stash it and kick off login. Guarded
  // per-code so StrictMode's double-invoked effects don't redirect twice.
  useEffect(() => {
    if (!code || status !== "unauthenticated" || stashedForCode.current === code) return;
    stashedForCode.current = code;
    stashInvite(code);
    void login({ returnTo: `/join?code=${encodeURIComponent(code)}`, invite: code });
  }, [code, status, login]);

  // Logged in with a code in the URL: redeem exactly once per navigation, which
  // StrictMode's double-invoked effects share. The outcome comes from the
  // returned promise, not `redeem`'s observer state: StrictMode's simulated
  // unmount detaches the observer from the in-flight mutation, so
  // `redeem.isError` never flips and the page would spin forever.
  useEffect(() => {
    if (!code || status !== "authenticated" || attemptedKey.current === attemptKey) return;
    attemptedKey.current = attemptKey;
    redeem.mutateAsync(code).then(
      (result) => {
        if (attemptedKey.current !== attemptKey) return;
        takeStashedInvite();
        pushToast(`Welcome to ${result.groupName}`, "success");
        // The user has left the page: they're joined, but don't pull them off where they went.
        if (!mounted.current) return;
        setCurrentGroup(result.groupId);
        navigate(`/g/${result.groupId}/upcoming`, { replace: true });
      },
      (error: unknown) => setRedeemError({ attemptKey, error }),
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps -- redeem/pushToast/etc. are stable across the guarded, once-per-navigation call
  }, [code, status, attemptKey]);

  if (!code) {
    return (
      <div className="px-5 py-10">
        <h1 className="font-display text-2xl font-bold">Join a group</h1>
        <p className="mt-2 text-inkmuted">Paste the invite code a friend sent you.</p>
        <div className="mt-5 rounded-3xl border border-line bg-white p-5 shadow-soft">
          <JoinForm />
        </div>
      </div>
    );
  }

  // Keyed by attempt so resubmitting the form below, same code or not, shows the spinner.
  if (redeemError?.attemptKey === attemptKey) {
    return (
      <div className="px-5 py-10">
        <h1 className="font-display text-2xl font-bold">Couldn&apos;t join</h1>
        <p role="alert" aria-live="polite" className="mt-2 text-coral">
          {messageForError(redeemError.error)}
        </p>
        <div className="mt-5 rounded-3xl border border-line bg-white p-5 shadow-soft">
          <JoinForm />
        </div>
      </div>
    );
  }

  return (
    <div className="flex justify-center py-20">
      <Spinner size="lg" />
    </div>
  );
}
