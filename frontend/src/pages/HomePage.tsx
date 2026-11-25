import { useSearchParams } from "react-router-dom";
import { useAuth } from "../auth/useAuth";
import { useConfig, useNewsletters } from "../api/queries";
import { JoinForm } from "../components/join/JoinForm";
import { InstallPwaPrompt } from "../components/pwa/InstallPwaPrompt";
import { Button } from "../components/ui/Button";
import { Spinner } from "../components/ui/Spinner";
import { TabBar } from "../components/ui/TabBar";
import { GroupEditions } from "../components/home/GroupEditions";
import { Landing } from "../components/home/Landing";
import { GroupLeaderboard } from "../components/leaderboard/GroupLeaderboard";
import { GroupMembers } from "../components/members/GroupMembers";
import { useCurrentGroup } from "../state/currentGroup";

type Tab = "editions" | "leaderboard" | "members";

const TABS: { id: Tab; label: string }[] = [
  { id: "editions", label: "Editions" },
  { id: "leaderboard", label: "Leaderboard" },
  { id: "members", label: "Members" },
];

/** The `?tab=` value, defaulting to Editions for a missing or unknown one. */
function parseTab(value: string | null): Tab {
  return TABS.find((t) => t.id === value)?.id ?? "editions";
}

export function HomePage() {
  const { status } = useAuth();

  if (status === "loading") {
    return (
      <div className="flex justify-center py-20">
        <Spinner size="lg" />
      </div>
    );
  }

  if (status !== "authenticated") return <Landing />;

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
  // The tab lives in the URL, so a link can open Home on a tab and a reload keeps it. Switching
  // tabs replaces the history entry rather than adding one, and stays on this page, so nothing
  // remounts or refetches `/config` (`docs/design/frontend.md` §7.1).
  const [searchParams, setSearchParams] = useSearchParams();
  const tab = parseTab(searchParams.get("tab"));

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
        <h1 className="font-display text-2xl font-bold">
          {config.displayName ? `Hey ${config.displayName} 👋` : "Hey there 👋"}
        </h1>
        <p className="mt-2 text-inkmuted">
          You haven&apos;t joined a group yet. Groups are invite-only: ask an admin of the group for
          an invite code, then paste it below.
        </p>
        <div className="mt-5 rounded-3xl border border-line bg-white p-5 shadow-soft">
          <JoinForm />
        </div>
      </div>
    );
  }

  return (
    <div className="pb-12 pt-2">
      {/* The header's group switcher already names the group on screen. */}
      <h1 className="sr-only">{current.groupName}</h1>
      <div className="px-5 pt-3 empty:hidden">
        <InstallPwaPrompt />
      </div>
      <TabBar
        label="Group sections"
        idPrefix="home"
        tabs={TABS}
        selected={tab}
        onSelect={(id) => setSearchParams(id === "editions" ? {} : { tab: id }, { replace: true })}
      />
      <div
        role="tabpanel"
        id={`home-tabpanel-${tab}`}
        aria-labelledby={`home-tab-${tab}`}
        className="mt-3 px-5"
      >
        {tab === "editions" && (
          <>
            {isLoading && <Spinner size="sm" />}
            {isError && (
              <p className="text-sm text-coral">Couldn&apos;t load this group&apos;s editions.</p>
            )}
            {data && (
              <GroupEditions
                groupId={current.groupId}
                timezone={current.timezone}
                frequency={config.cycleCadence.frequency}
                items={data.items}
              />
            )}
          </>
        )}
        {tab === "leaderboard" && <GroupLeaderboard groupId={current.groupId} />}
        {tab === "members" && <GroupMembers groupId={current.groupId} />}
      </div>
    </div>
  );
}
