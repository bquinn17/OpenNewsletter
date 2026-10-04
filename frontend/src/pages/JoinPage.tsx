import { useEffect, useRef } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
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
    default:
      return error.problem?.detail ?? error.message;
  }
}

export function JoinPage() {
  const [search] = useSearchParams();
  const code = search.get("code");
  const { status, login } = useAuth();
  const navigate = useNavigate();
  const setCurrentGroup = useCurrentGroup((s) => s.setCurrentGroup);
  const pushToast = useToasts((s) => s.push);
  const redeem = useRedeemInvite();
  const attemptedCode = useRef<string | null>(null);
  const stashedForCode = useRef<string | null>(null);

  // Logged out with a code in the URL: stash it and kick off login. Guarded
  // per-code so StrictMode's double-invoked effects don't redirect twice.
  useEffect(() => {
    if (!code || status !== "unauthenticated" || stashedForCode.current === code) return;
    stashedForCode.current = code;
    stashInvite(code);
    void login({ returnTo: `/join?code=${encodeURIComponent(code)}`, invite: code });
  }, [code, status, login]);

  // Logged in with a code in the URL: redeem exactly once per code.
  useEffect(() => {
    if (!code || status !== "authenticated" || attemptedCode.current === code) return;
    attemptedCode.current = code;
    redeem.mutate(code, {
      onSuccess: (result) => {
        takeStashedInvite();
        setCurrentGroup(result.groupId);
        pushToast(`Welcome to ${result.groupName}`, "success");
        navigate(`/g/${result.groupId}/upcoming`, { replace: true });
      },
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps -- redeem/pushToast/etc. are stable across the guarded, once-per-code call
  }, [code, status]);

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

  if (redeem.isError) {
    return (
      <div className="px-5 py-10">
        <h1 className="font-display text-2xl font-bold">Couldn&apos;t join</h1>
        <p role="alert" aria-live="polite" className="mt-2 text-coral">
          {messageForError(redeem.error)}
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
