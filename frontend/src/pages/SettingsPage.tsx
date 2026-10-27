import { Navigate } from "react-router-dom";
import { useAuth } from "../auth/useAuth";
import { useConfig, useMe } from "../api/queries";
import { PageHeader } from "../components/layout/PageHeader";
import { InstallPwaPrompt } from "../components/pwa/InstallPwaPrompt";
import { NotificationsSection } from "../components/settings/NotificationsSection";
import { ProfileCard } from "../components/settings/ProfileCard";
import { YourGroupsSection } from "../components/settings/YourGroupsSection";
import { Spinner } from "../components/ui/Spinner";

export function SettingsPage() {
  const { logout } = useAuth();
  const { data: me } = useMe();
  const { data: config } = useConfig();

  // No profile exists before an invite is redeemed; Home shows the join form.
  if (config && !config.userId) return <Navigate to="/" replace />;

  if (!me || !config) {
    return (
      <div className="flex justify-center py-20">
        <Spinner size="lg" />
      </div>
    );
  }

  return (
    <div className="pb-12">
      <PageHeader title="You" back="/" />

      <div className="px-5 pt-5">
        <ProfileCard me={me} />
      </div>

      <div className="mt-5 px-5">
        <InstallPwaPrompt />
      </div>

      <NotificationsSection config={config} />

      <YourGroupsSection memberships={config.memberships} />

      <div className="mt-6 px-5">
        <button
          onClick={() => void logout()}
          className="w-full rounded-2xl border border-coral/20 bg-white p-3 text-sm font-semibold text-coral"
        >
          Sign out
        </button>
      </div>
    </div>
  );
}
