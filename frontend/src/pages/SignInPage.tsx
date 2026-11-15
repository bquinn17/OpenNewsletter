import { Link, Navigate, useSearchParams } from "react-router-dom";
import { safeReturnTo } from "../auth/returnTo";
import { useAuth } from "../auth/useAuth";
import { InviteOnlyNote } from "../components/auth/InviteOnlyNote";
import { ProviderButtons } from "../components/auth/ProviderButtons";
import { Spinner } from "../components/ui/Spinner";

function greeting(hour: number): string {
  if (hour < 5) return "Up late?";
  if (hour < 12) return "Good morning!";
  if (hour < 18) return "Good afternoon!";
  return "Good evening!";
}

/**
 * `/signin`: the app's own sign-in screen. `RequireAuth` sends signed-out
 * visitors here with `?returnTo=`; each button goes straight to its IdP
 * (`docs/design/frontend.md` §7.10).
 */
export function SignInPage() {
  const { status } = useAuth();
  const [search] = useSearchParams();
  const returnTo = safeReturnTo(search.get("returnTo"));

  if (status === "authenticated") return <Navigate to={returnTo} replace />;

  if (status === "loading") {
    return (
      <div className="flex justify-center py-20">
        <Spinner size="lg" />
      </div>
    );
  }

  return (
    <div className="px-5 pb-16 pt-10">
      <div className="mx-auto max-w-sm">
        <div
          aria-hidden="true"
          className="hero-feb mx-auto grid h-20 w-20 place-items-center rounded-4xl text-4xl shadow-card"
        >
          <span className="animate-wiggle motion-reduce:animate-none">✉️</span>
        </div>

        <p className="mt-6 text-center text-xs font-bold uppercase tracking-widest text-grape">
          {greeting(new Date().getHours())}
        </p>
        <h1 className="mt-1 text-center font-display text-3xl font-bold leading-tight">
          Welcome back
        </h1>
        <p className="mt-2 text-center text-inkmuted">
          Your group&apos;s been busy. Sign in to catch up on the latest edition.
        </p>

        <div className="mt-8 rounded-4xl border border-white/60 bg-white/70 p-4 shadow-card backdrop-blur">
          <ProviderButtons returnTo={returnTo} />
          <p className="px-1 text-xs leading-relaxed text-inkmuted">
            We only use your name and email address, and we&apos;ll never post to your accounts.
          </p>
        </div>

        <InviteOnlyNote className="mt-8">
          <p className="text-inkmuted">
            Signing in is for members. New here?{" "}
            <Link to="/join" className="font-semibold text-grape underline underline-offset-2">
              Sign up with your invite code
            </Link>
          </p>
        </InviteOnlyNote>
      </div>
    </div>
  );
}
