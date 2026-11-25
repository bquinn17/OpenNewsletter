import { useState } from "react";
import { useParams } from "react-router-dom";
import { useGroup } from "../api/queries";
import { PageHeader } from "../components/layout/PageHeader";
import { PageErrorState } from "../components/layout/PageErrorState";
import { GroupSettingsForm } from "../components/admin/GroupSettingsForm";
import { MembersTab } from "../components/admin/MembersTab";
import { InvitesTab } from "../components/admin/InvitesTab";
import { Spinner } from "../components/ui/Spinner";
import { TabBar } from "../components/ui/TabBar";

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
      <TabBar
        label="Admin sections"
        idPrefix="admin"
        tabs={TABS}
        selected={tab}
        onSelect={setTab}
      />

      <div role="tabpanel" id={`admin-tabpanel-${tab}`} aria-labelledby={`admin-tab-${tab}`}>
        {tab === "members" && <MembersTab group={group.data} />}
        {tab === "invites" && <InvitesTab groupId={groupId} group={group.data} />}
        {tab === "settings" && <GroupSettingsForm group={group.data} />}
      </div>
    </div>
  );
}
