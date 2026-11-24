import { useEffect, useRef, useState } from "react";
import { Link, useLocation, useNavigate, useSearchParams } from "react-router-dom";
import { useAuth } from "../auth/useAuth";
import { takeStashedInvite } from "../auth/inviteStash";
import { useRedeemInvite } from "../api/mutations";
import { ApiError } from "../api/client";
import { useCelebration } from "../state/celebration";
import { useCurrentGroup } from "../state/currentGroup";
import { useToasts } from "../state/toast";
import { InviteOnlyNote } from "../components/auth/InviteOnlyNote";
import { ProviderButtons } from "../components/auth/ProviderButtons";
import { SignUpSteps } from "../components/auth/SignUpSteps";
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
  const { status } = useAuth();
  const navigate = useNavigate();
  const setCurrentGroup = useCurrentGroup((s) => s.setCurrentGroup);
  const pushToast = useToasts((s) => s.push);
  const celebrate = useCelebration((s) => s.celebrate);
  const redeem = useRedeemInvite();
  const attemptedKey = useRef<string | null>(null);
  const [redeemError, setRedeemError] = useState<{ attemptKey: string; error: unknown } | null>(
    null,
  );
  // StrictMode's simulated unmount flips this back to `true` on remount; a real
  // unmount (the user tapped away mid-redeem) leaves it `false`.
  const mounted = useRef(false);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

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
        pushToast(`Welcome to ${result.groupName} 🎉`, "success");
        celebrate();
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
    if (status === "unauthenticated") return <SignUpStart />;
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

  // Logged out with a code in the URL: sign in first. The code rides through
  // the OAuth round trip (`login`'s `invite`) and is redeemed back here.
  if (status === "unauthenticated") return <InviteTicket code={code} />;

  // Keyed by attempt so resubmitting the form below, same code or not, shows the spinner.
  if (redeemError?.attemptKey === attemptKey) {
    return (
      <div className="px-5 py-10">
        <h1 className="font-display text-2xl font-bold">Couldn&apos;t join</h1>
        <p role="alert" aria-live="polite" className="mt-2 text-danger">
          {messageForError(redeemError.error)}
        </p>
        <div className="mt-5 rounded-3xl border border-line bg-white p-5 shadow-soft">
          <JoinForm />
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col items-center gap-4 py-20">
      <Spinner size="lg" />
      {status === "authenticated" && (
        <p className="text-sm font-semibold text-inkmuted">Saving your seat…</p>
      )}
    </div>
  );
}

/** `/join` for a signed-out visitor: step 1 of signing up. */
function SignUpStart() {
  return (
    <div className="px-5 pb-16 pt-8">
      <SignUpSteps current={1} />
      <div
        aria-hidden="true"
        className="hero-april mt-8 grid h-16 w-16 place-items-center rounded-3xl text-3xl shadow-card"
      >
        🎟️
      </div>
      <h1 className="mt-5 font-display text-3xl font-bold leading-tight">Join your group</h1>
      <p className="mt-2 text-inkmuted">
        Paste the code from your invite and we&apos;ll get you in.
      </p>
      <div className="mt-6 rounded-4xl bg-white p-5 shadow-card">
        <JoinForm />
      </div>
      <InviteOnlyNote className="mt-6">
        <p className="text-inkmuted">
          No code yet? Ask an admin of the group you want to join to send you an invite link.
        </p>
      </InviteOnlyNote>
      <p className="mt-4 text-sm text-inkmuted">
        Already a member?{" "}
        <Link to="/signin" className="font-semibold text-grape-ink underline underline-offset-2">
          Sign in
        </Link>
      </p>
    </div>
  );
}

/** `/join?code=…` for a signed-out visitor: step 2, picking how to sign in. */
function InviteTicket({ code }: { code: string }) {
  return (
    <div className="px-5 pb-16 pt-8">
      <SignUpSteps current={2} />
      <div className="mt-8 animate-pop overflow-hidden rounded-4xl bg-white shadow-card">
        <div className="hero-april px-6 pb-6 pt-7 text-ink">
          <p className="text-xs font-bold uppercase tracking-widest text-ink/90">
            You&apos;re invited
          </p>
          <h1 className="mt-1 font-display text-3xl font-bold leading-tight">
            We saved you a seat <span aria-hidden="true">🎉</span>
          </h1>
          <p className="mt-2 text-sm text-ink/90">
            Sign in to join the group. New here? Signing in creates your account.
          </p>
        </div>
        {/* The ticket's tear-off line, with a notch cut into each edge. */}
        <div aria-hidden="true" className="relative h-0 border-t-2 border-dashed border-line">
          <span className="absolute -left-3 -top-3 h-6 w-6 rounded-full bg-blush" />
          <span className="absolute -right-3 -top-3 h-6 w-6 rounded-full bg-blush" />
        </div>
        <div className="p-5">
          <p className="text-xs font-bold uppercase tracking-widest text-inkmuted">Invite code</p>
          <p className="mt-1 break-all font-mono text-lg font-semibold tracking-wider">{code}</p>
          <div className="mt-5">
            <ProviderButtons invite={code} returnTo={`/join?code=${encodeURIComponent(code)}`} />
          </div>
          <p className="text-xs leading-relaxed text-inkmuted">
            If the invite was sent to a particular email address, use the account with that address.
          </p>
        </div>
      </div>
      <p className="mt-6 text-center text-sm text-inkmuted">
        Wrong code?{" "}
        <Link to="/join" className="font-semibold text-grape-ink underline underline-offset-2">
          Enter a different one
        </Link>
      </p>
    </div>
  );
}
