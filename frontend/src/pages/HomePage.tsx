import { useQueries } from "@tanstack/react-query";
import { useAuth } from "../auth/useAuth";
import { api } from "../api/client";
import { useConfig, queryKeys } from "../api/queries";
import { JoinForm } from "../components/join/JoinForm";
import { InstallPwaPrompt } from "../components/pwa/InstallPwaPrompt";
import { Button } from "../components/ui/Button";
import { Spinner } from "../components/ui/Spinner";
import { GroupSection } from "../components/home/GroupSection";

export function HomePage() {
  const { status, login } = useAuth();

  if (status === "loading") {
    return (
      <div className="flex justify-center py-20">
        <Spinner size="lg" />
      </div>
    );
  }

  if (status !== "authenticated") {
    return (
      <div className="px-5 py-10">
        <h1 className="font-display text-3xl font-bold leading-tight">
          Your group&apos;s newsletter, together.
        </h1>
        <p className="mt-2 text-inkmuted">Sign in to see your group&apos;s newsletters.</p>
        <Button className="mt-5" onClick={() => void login({ returnTo: "/" })}>
          Sign in
        </Button>
        <div className="mt-8 rounded-3xl border border-line bg-white p-5 shadow-soft">
          <p className="mb-3 text-sm text-inkmuted">Have an invite code?</p>
          <JoinForm />
        </div>
      </div>
    );
  }

  return <SignedInHome />;
}

function SignedInHome() {
  const { data: config } = useConfig();
  const memberships = config?.memberships ?? [];

  const results = useQueries({
    queries: memberships.map((m) => ({
      queryKey: queryKeys.newsletters(m.groupId),
      queryFn: () => api.listNewsletters(m.groupId),
      staleTime: 30_000,
    })),
  });

  const greeting = config?.displayName ? `Hey ${config.displayName} 👋` : "Hey there 👋";

  if (memberships.length === 0) {
    return (
      <div className="px-5 py-10">
        <h1 className="font-display text-2xl font-bold">{greeting}</h1>
        <p className="mt-2 text-inkmuted">
          You haven&apos;t joined a group yet — paste your invite code below.
        </p>
        <div className="mt-5 rounded-3xl border border-line bg-white p-5 shadow-soft">
          <JoinForm />
        </div>
      </div>
    );
  }

  return (
    <div className="pb-12">
      <div className="px-5 pb-2 pt-6">
        <h1 className="font-display text-2xl font-bold leading-tight">{greeting}</h1>
      </div>
      <div className="px-5 pb-2">
        <InstallPwaPrompt />
      </div>
      {memberships.map((m, i) => (
        <GroupSection key={m.groupId} membership={m} query={results[i]!} />
      ))}
    </div>
  );
}
