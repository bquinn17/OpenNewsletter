import { useState } from "react";
import { useParams } from "react-router-dom";
import clsx from "clsx";
import { useCreateInvite, useGroup, useInvites, useRevokeInvite } from "../api/queries";
import { useToasts } from "../state/toast";
import { PageHeader } from "../components/layout/PageHeader";
import { Avatar } from "../components/ui/Avatar";
import { Button } from "../components/ui/Button";
import { Pill } from "../components/ui/Pill";
import { formatRelative } from "../utils/dates";

type Tab = "members" | "invites" | "curate" | "settings";

export function GroupAdminPage() {
  const { groupId = "" } = useParams();
  const { data: group } = useGroup(groupId);
  const [tab, setTab] = useState<Tab>("members");

  if (!group) return null;

  return (
    <div className="bg-cream pb-12">
      <PageHeader eyebrow="Admin" title={group.name} back="/settings" />
      <div className="px-4 pb-2 flex gap-1 overflow-x-auto no-scrollbar text-sm">
        {(["members", "invites", "curate", "settings"] as Tab[]).map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={clsx(
              "px-3 py-1.5 rounded-full font-semibold capitalize",
              tab === t ? "bg-ink text-cream" : "bg-white border border-line",
            )}
          >
            {t}
          </button>
        ))}
      </div>

      {tab === "members" && <MembersTab group={group} />}
      {tab === "invites" && <InvitesTab groupId={groupId} />}
      {tab === "curate" && <CurateTab />}
      {tab === "settings" && <SettingsTab group={group} />}
    </div>
  );
}

function MembersTab({ group }: { group: ReturnType<typeof useGroup>["data"] }) {
  if (!group) return null;
  return (
    <div className="px-5 pt-4">
      <div className="flex items-baseline justify-between mb-2">
        <div className="text-xs uppercase tracking-widest text-inkmuted font-bold">
          {group.members.length} of {group.memberSoftCap} members
        </div>
      </div>
      <div className="bg-white rounded-3xl border border-line shadow-soft divide-y divide-line">
        {group.members.map((m) => (
          <div key={m.userId} className="p-4 flex items-center gap-3">
            <Avatar name={m.displayName} color={m.avatarColor} />
            <div className="flex-1 min-w-0">
              <div className="font-semibold text-sm flex items-center gap-2">
                {m.displayName}
                {m.role === "admin" && (
                  <Pill tone="coral" className="!px-1.5 !py-0.5 !text-[10px]">
                    admin
                  </Pill>
                )}
                {m.nickname && <span className="text-[10px] text-inkmuted">"{m.nickname}"</span>}
              </div>
              <div className="text-xs text-inkmuted">
                {m.editionsAnswered > 0
                  ? `${m.editionsAnswered} editions answered`
                  : `joined ${formatRelative(m.joinedAt)} · pending first answer`}
              </div>
            </div>
            <button className="text-xs text-inkmuted">⋯</button>
          </div>
        ))}
      </div>
    </div>
  );
}

function InvitesTab({ groupId }: { groupId: string }) {
  const { data: invites = [] } = useInvites(groupId);
  const create = useCreateInvite(groupId);
  const revoke = useRevokeInvite(groupId);
  const pushToast = useToasts((s) => s.push);

  const handleCopy = async (code: string) => {
    try {
      await navigator.clipboard.writeText(`https://opennewsletter.example.com/join?code=${code}`);
      pushToast("Invite link copied.", "success");
    } catch {
      pushToast("Couldn't copy — long-press to select.", "error");
    }
  };

  return (
    <div className="px-5 pt-4">
      <div className="text-xs uppercase tracking-widest text-inkmuted font-bold mb-2 ml-1">Open invites</div>
      <div className="bg-white rounded-3xl border border-line shadow-soft divide-y divide-line">
        {invites.map((inv) => {
          const used = inv.status === "consumed";
          return (
            <div key={inv.code} className={clsx("p-4 flex items-center gap-3", used && "opacity-60")}>
              <div className={clsx("font-mono text-sm bg-cream rounded-lg px-2 py-1", used && "line-through")}>
                {inv.code}
              </div>
              <div className="flex-1 text-xs text-inkmuted">
                {used
                  ? `used by ${inv.consumedByName ?? "—"} · ${formatRelative(inv.consumedAt!)}`
                  : `expires ${formatRelative(inv.expiresAt)} · ${inv.roleOnRedeem}`}
              </div>
              {used ? (
                <span className="text-xs text-inkmuted">used</span>
              ) : inv.status === "revoked" ? (
                <span className="text-xs text-inkmuted">revoked</span>
              ) : (
                <>
                  <button
                    onClick={() => handleCopy(inv.code)}
                    className="text-xs text-grape font-semibold"
                  >
                    Copy
                  </button>
                  <button
                    onClick={() => revoke.mutate(inv.code)}
                    className="text-xs text-coral font-semibold"
                  >
                    Revoke
                  </button>
                </>
              )}
            </div>
          );
        })}
      </div>
      <div className="mt-3 flex gap-2">
        <Button onClick={() => create.mutate("member")} disabled={create.isPending}>
          ＋ Member invite
        </Button>
        <Button variant="ghost" onClick={() => create.mutate("admin")} disabled={create.isPending}>
          ＋ Admin invite
        </Button>
      </div>
    </div>
  );
}

function CurateTab() {
  return (
    <div className="px-5 pt-4">
      <div className="bg-gradient-to-br from-grape to-sky text-white rounded-3xl p-5">
        <div className="text-xs uppercase tracking-widest opacity-90 font-bold">Curate · June</div>
        <div className="font-display text-xl mt-1">Auto-promote in 12 days</div>
        <p className="text-sm opacity-90 mt-1">
          Top 5 voted will lock in. You can override anytime before then.
        </p>
        <Button variant="ghost" className="mt-3 !bg-white">
          Review picks →
        </Button>
      </div>
    </div>
  );
}

function SettingsTab({ group }: { group: ReturnType<typeof useGroup>["data"] }) {
  if (!group) return null;
  return (
    <div className="px-5 pt-4 space-y-4">
      <Section title="Cycle">
        <Field label="Questions per cycle" value={String(group.cycleSettings.questionsPerCycle)} />
        <Field label="Votes per user per cycle" value={String(group.cycleSettings.votesPerUserPerCycle)} />
        <Field label="Response window (days)" value={String(group.cycleSettings.responseWindowDays)} />
        <Field label="Timezone" value={group.timezone} />
      </Section>
      <Section title="Notifications">
        <Field
          label="Reminders before deadline"
          value={`${group.notificationSettings.offsetsHoursBeforeClose.join("h, ")}h`}
        />
        <Field label="Send when cycle opens" value={group.notificationSettings.onCycleOpen ? "Yes" : "No"} />
      </Section>
      <p className="text-xs text-inkmuted text-center">Editing form not wired up in this prototype.</p>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div>
      <div className="text-xs uppercase tracking-widest text-inkmuted font-bold mb-2 ml-1">{title}</div>
      <div className="bg-white rounded-3xl border border-line shadow-soft divide-y divide-line">{children}</div>
    </div>
  );
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <div className="p-4 flex items-center gap-3">
      <div className="flex-1 text-sm">{label}</div>
      <div className="text-sm font-semibold">{value}</div>
    </div>
  );
}
