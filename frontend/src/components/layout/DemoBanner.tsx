import { useAuth } from "../../auth/useAuth";
import { useHasStoredSession } from "../../auth/useHasStoredSession";
import { exitDemo } from "../../demo";

/** Shown on every demo page, so it's never mistaken for a real group (`docs/design/frontend.md` §17). */
export function DemoBanner() {
  const { login } = useAuth();
  const hasStoredSession = useHasStoredSession();

  return (
    <section
      aria-label="Demo"
      className="flex items-center gap-3 border-b border-ink/10 bg-sun px-4 py-2 text-sm"
    >
      <p className="min-w-0 flex-1">
        <strong className="font-bold">Demo.</strong>{" "}
        {hasStoredSession
          ? "You're still signed in, but these aren't your groups, and nothing you do here is saved."
          : "Sample groups — nothing you do here is saved."}
      </p>
      {!hasStoredSession && (
        <button
          onClick={() => void login({ returnTo: "/" })}
          className="shrink-0 font-semibold underline"
        >
          Sign in
        </button>
      )}
      <button
        onClick={exitDemo}
        className="shrink-0 rounded-full bg-ink px-3 py-1 text-xs font-semibold text-cream"
      >
        Exit demo
      </button>
    </section>
  );
}
