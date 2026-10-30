import { useState } from "react";
import { useParams } from "react-router-dom";
import clsx from "clsx";
import { useGroup } from "../api/queries";
import { PageHeader } from "../components/layout/PageHeader";
import { PageErrorState } from "../components/layout/PageErrorState";
import { GroupSettingsForm } from "../components/admin/GroupSettingsForm";
import { MembersTab } from "../components/admin/MembersTab";
import { InvitesTab } from "../components/admin/InvitesTab";
import { Spinner } from "../components/ui/Spinner";

type Tab = "members" | "invites" | "settings";

const TABS: { id: Tab; label: string }[] = [
  { id: "members", label: "Members" },
  { id: "invites", label: "Invites" },
  { id: "settings", label: "Settings" },
];

export function GroupAdminPage() {
  const { groupId = "" } = useParams();
  const [tab, setTab] = useState<Tab>("members");
  const group = useGroup(groupId);

  if (group.isLoading) {
    return (
      <div className="flex justify-center py-20">
        <Spinner size="lg" />
      </div>
    );
  }

  if (group.isError || !group.data) {
    return (
      <PageErrorState
        eyebrow="Admin"
        title="Couldn't load"
        back="/settings"
        message="Couldn't load this group's admin page."
        onRetry={() => group.refetch()}
      />
    );
  }

  return (
    <div className="pb-12">
      <PageHeader eyebrow="Admin" title={group.data.name} back="/settings" />
      <div
        role="tablist"
        aria-label="Admin sections"
        className="no-scrollbar flex gap-1 overflow-x-auto px-4 pb-2 pt-3 text-sm"
      >
        {TABS.map((t) => (
          <button
            key={t.id}
            role="tab"
            id={`admin-tab-${t.id}`}
            aria-selected={tab === t.id}
            aria-controls={`admin-tabpanel-${t.id}`}
            onClick={() => setTab(t.id)}
            className={clsx(
              "rounded-full px-3 py-1.5 font-semibold",
              tab === t.id ? "bg-ink text-cream" : "border border-line bg-white",
            )}
          >
            {t.label}
          </button>
        ))}
      </div>

      <div role="tabpanel" id={`admin-tabpanel-${tab}`} aria-labelledby={`admin-tab-${tab}`}>
        {tab === "members" && <MembersTab group={group.data} />}
        {tab === "invites" && <InvitesTab groupId={groupId} group={group.data} />}
        {tab === "settings" && <GroupSettingsForm group={group.data} />}
      </div>
    </div>
  );
}
