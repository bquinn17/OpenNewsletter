import { useAuth } from "../auth/useAuth";
import { useConfig, useNewsletters } from "../api/queries";
import { JoinForm } from "../components/join/JoinForm";
import { InstallPwaPrompt } from "../components/pwa/InstallPwaPrompt";
import { Button } from "../components/ui/Button";
import { Spinner } from "../components/ui/Spinner";
import { GroupEditions } from "../components/home/GroupEditions";
import { useCurrentGroup } from "../state/currentGroup";

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
  const configQuery = useConfig();
  const config = configQuery.data;
  const memberships = config?.memberships ?? [];
  const currentGroupId = useCurrentGroup((s) => s.currentGroupId);
  // Home shows only the group picked in the header's `GroupSwitcher`
  // (`docs/design/frontend.md` §7.1). Fall back to the first membership for
  // the render before `AppShell` defaults the selection.
  const current = memberships.find((m) => m.groupId === currentGroupId) ?? memberships[0];
  const { data, isLoading, isError } = useNewsletters(current?.groupId);

  const greeting = config?.displayName ? `Hey ${config.displayName} 👋` : "Hey there 👋";

  // Without `/config` there's no telling a member from someone yet to join,
  // so show neither the editions nor the join form until it loads. Offline,
  // TanStack Query pauses the fetch rather than failing it.
  if (!config) {
    if (configQuery.isError || configQuery.fetchStatus === "paused") {
      return (
        <div className="px-5 py-10 text-center">
          <p className="font-semibold text-ink">Couldn&apos;t load your groups.</p>
          <p className="mt-1 text-sm text-inkmuted">Check your connection and try again.</p>
          <Button className="mt-5" onClick={() => void configQuery.refetch()}>
            Try again
          </Button>
        </div>
      );
    }
    return (
      <div className="flex justify-center py-20">
        <Spinner size="lg" />
      </div>
    );
  }

  if (!current) {
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
      <section className="mt-3 px-5">
        {isLoading && <Spinner size="sm" />}
        {isError && (
          <p className="text-sm text-coral">Couldn&apos;t load this group&apos;s editions.</p>
        )}
        {data && (
          <GroupEditions groupId={current.groupId} timezone={current.timezone} items={data.items} />
        )}
      </section>
    </div>
  );
}
