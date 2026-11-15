import { useEffect, useState, type ComponentType } from "react";
import clsx from "clsx";
import { lastProvider } from "../../auth/lastProvider";
import { useAuth, type IdentityProvider } from "../../auth/useAuth";
import { Spinner } from "../ui/Spinner";
import { FacebookLogo, GoogleLogo } from "./ProviderLogos";

const PROVIDERS: { id: IdentityProvider; Logo: ComponentType<{ className?: string }> }[] = [
  { id: "Google", Logo: GoogleLogo },
  { id: "Facebook", Logo: FacebookLogo },
];

type Props = {
  returnTo: string;
  invite?: string;
};

/**
 * One button per IdP. Each goes straight to that IdP: Cognito's hosted page is
 * never shown (`docs/design/frontend.md` §6). The last provider this browser
 * used is marked, since picking the other one makes a second account.
 */
export function ProviderButtons({ returnTo, invite }: Props) {
  const { login } = useAuth();
  const [pending, setPending] = useState<IdentityProvider | null>(null);
  const [failed, setFailed] = useState(false);
  const [last] = useState(lastProvider);

  // Back from the IdP's page restores this one from the back/forward cache,
  // still showing "Opening Google…"; reset it so the buttons work again.
  useEffect(() => {
    const handlePageShow = (event: PageTransitionEvent): void => {
      if (event.persisted) setPending(null);
    };
    window.addEventListener("pageshow", handlePageShow);
    return () => window.removeEventListener("pageshow", handlePageShow);
  }, []);

  const start = (provider: IdentityProvider): void => {
    setPending(provider);
    setFailed(false);
    // On success the page navigates away. `signinRedirect` first fetches
    // Cognito's OIDC metadata, so offline it rejects instead.
    login({ provider, returnTo, invite }).catch(() => {
      setPending(null);
      setFailed(true);
    });
  };

  return (
    <div className="flex flex-col gap-3">
      {PROVIDERS.map(({ id, Logo }) => (
        <button
          key={id}
          type="button"
          onClick={() => start(id)}
          disabled={pending !== null}
          className={clsx(
            "group relative flex w-full items-center gap-3 rounded-2xl border bg-white px-4 py-3.5 text-left font-semibold text-ink shadow-soft transition",
            "hover:-translate-y-0.5 hover:border-ink hover:shadow-card focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-grape/40",
            "disabled:cursor-wait disabled:hover:translate-y-0 disabled:hover:border-line disabled:hover:shadow-soft motion-reduce:hover:translate-y-0",
            last === id ? "border-grape/40" : "border-line",
            pending !== null && pending !== id && "opacity-50",
          )}
        >
          <span className="grid h-9 w-9 shrink-0 place-items-center rounded-xl bg-cream">
            <Logo className="h-5 w-5" />
          </span>
          <span className="flex-1">
            {pending === id ? `Opening ${id}…` : `Continue with ${id}`}
          </span>
          {pending === id ? (
            <Spinner size="sm" label={`Opening ${id}`} />
          ) : last === id ? (
            <span className="rounded-full bg-lavender px-2 py-0.5 text-[11px] font-bold uppercase tracking-wider text-grape">
              Last used
            </span>
          ) : (
            <span
              aria-hidden="true"
              className="text-inkmuted transition group-hover:translate-x-0.5 group-hover:text-ink"
            >
              →
            </span>
          )}
        </button>
      ))}
      <p
        role="alert"
        aria-live="polite"
        className="min-h-[1.25rem] text-sm font-semibold text-coral"
      >
        {failed ? "Couldn't reach sign-in. Check your connection and try again." : ""}
      </p>
    </div>
  );
}
