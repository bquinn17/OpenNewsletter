import { useEffect, useRef, useState } from "react";
import { useParams } from "react-router-dom";
import clsx from "clsx";
import {
  useConfig,
  useCreateInvite,
  useGroup,
  useInvites,
  useRemoveMember,
  useRevokeInvite,
  useUpdateGroup,
  useUpdateMemberRole,
} from "../mocks/legacyQueries";
import { useToasts } from "../state/toast";
import { PageHeader } from "../components/layout/PageHeader";
import { Avatar } from "../components/ui/Avatar";
import { Button } from "../components/ui/Button";
import { Pill } from "../components/ui/Pill";
import { formatRelative } from "../utils/dates";
import type { Group } from "../mocks/types";

type Tab = "members" | "invites" | "settings";

export function GroupAdminPage() {
  const { groupId = "" } = useParams();
  const { data: group } = useGroup(groupId);
  const [tab, setTab] = useState<Tab>("members");

  if (!group) return null;

  return (
    <div className="bg-cream pb-12">
      <PageHeader eyebrow="Admin" title={group.name} back="/settings" />
      <div className="no-scrollbar flex gap-1 overflow-x-auto px-4 pb-2 text-sm">
        {(["members", "invites", "settings"] as Tab[]).map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={clsx(
              "rounded-full px-3 py-1.5 font-semibold capitalize",
              tab === t ? "bg-ink text-cream" : "border border-line bg-white",
            )}
          >
            {t}
          </button>
        ))}
      </div>

      {tab === "members" && <MembersTab group={group} groupId={groupId} />}
      {tab === "invites" && <InvitesTab groupId={groupId} />}
      {tab === "settings" && <SettingsTab group={group} groupId={groupId} />}
    </div>
  );
}

function MembersTab({ group, groupId }: { group: Group; groupId: string }) {
  const { data: config } = useConfig();
  const updateRole = useUpdateMemberRole(groupId);
  const removeMember = useRemoveMember(groupId);
  const pushToast = useToasts((s) => s.push);
  const [openMenu, setOpenMenu] = useState<string | null>(null);

  return (
    <div className="px-5 pt-4">
      <div className="mb-2 flex items-baseline justify-between">
        <div className="text-xs font-bold uppercase tracking-widest text-inkmuted">
          {group.members.length} of {group.memberSoftCap} members
        </div>
      </div>
      <div className="divide-y divide-line rounded-3xl border border-line bg-white shadow-soft">
        {group.members.map((m) => {
          const isMe = config?.user.userId === m.userId;
          return (
            <div key={m.userId} className="relative flex items-center gap-3 p-4">
              <Avatar name={m.displayName} color={m.avatarColor} />
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2 text-sm font-semibold">
                  {m.displayName}
                  {isMe && <span className="text-[10px] text-inkmuted">(you)</span>}
                  {m.role === "admin" && (
                    <Pill tone="coral" className="!px-1.5 !py-0.5 !text-[10px]">
                      admin
                    </Pill>
                  )}
                </div>
                <div className="text-xs text-inkmuted">
                  {m.editionsAnswered > 0
                    ? `${m.editionsAnswered} editions answered`
                    : `joined ${formatRelative(m.joinedAt)} · pending first answer`}
                </div>
              </div>
              <MemberMenu
                open={openMenu === m.userId}
                onOpenChange={(o) => setOpenMenu(o ? m.userId : null)}
                disabled={updateRole.isPending || removeMember.isPending}
                onMakeAdmin={
                  m.role === "member"
                    ? async () => {
                        await updateRole.mutateAsync({ userId: m.userId, role: "admin" });
                        pushToast(`${m.displayName} is now an admin.`, "success");
                        setOpenMenu(null);
                      }
                    : undefined
                }
                onMakeMember={
                  m.role === "admin" && !isMe
                    ? async () => {
                        await updateRole.mutateAsync({ userId: m.userId, role: "member" });
                        pushToast(`${m.displayName} is now a member.`, "success");
                        setOpenMenu(null);
                      }
                    : undefined
                }
                onRemove={
                  !isMe
                    ? async () => {
                        if (!confirm(`Remove ${m.displayName} from ${group.name}?`)) return;
                        await removeMember.mutateAsync(m.userId);
                        pushToast(`Removed ${m.displayName}.`, "default");
                        setOpenMenu(null);
                      }
                    : undefined
                }
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}

function MemberMenu({
  open,
  onOpenChange,
  disabled,
  onMakeAdmin,
  onMakeMember,
  onRemove,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  disabled: boolean;
  onMakeAdmin?: () => void;
  onMakeMember?: () => void;
  onRemove?: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    function onDoc(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) onOpenChange(false);
    }
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [open, onOpenChange]);

  const hasActions = !!(onMakeAdmin || onMakeMember || onRemove);

  return (
    <div ref={ref} className="relative">
      <button
        onClick={() => hasActions && onOpenChange(!open)}
        disabled={disabled || !hasActions}
        className="h-8 w-8 rounded-full text-inkmuted hover:bg-cream disabled:opacity-30"
        aria-label="Member options"
      >
        ⋯
      </button>
      {open && (
        <div className="absolute right-0 top-9 z-30 w-48 animate-pop rounded-2xl border border-line bg-white p-1 shadow-pop">
          {onMakeAdmin && <MenuItem onClick={onMakeAdmin}>Make admin</MenuItem>}
          {onMakeMember && <MenuItem onClick={onMakeMember}>Make member</MenuItem>}
          {onRemove && (
            <MenuItem onClick={onRemove} danger>
              Remove from group
            </MenuItem>
          )}
        </div>
      )}
    </div>
  );
}

function MenuItem({
  onClick,
  danger,
  children,
}: {
  onClick: () => void;
  danger?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      className={clsx(
        "w-full rounded-xl px-3 py-2 text-left text-sm font-semibold hover:bg-cream",
        danger && "text-coral",
      )}
    >
      {children}
    </button>
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
      <div className="mb-2 ml-1 text-xs font-bold uppercase tracking-widest text-inkmuted">
        Open invites
      </div>
      <div className="divide-y divide-line rounded-3xl border border-line bg-white shadow-soft">
        {invites.map((inv) => {
          const used = inv.status === "consumed";
          return (
            <div
              key={inv.code}
              className={clsx("flex items-center gap-3 p-4", used && "opacity-60")}
            >
              <div
                className={clsx(
                  "rounded-lg bg-cream px-2 py-1 font-mono text-sm",
                  used && "line-through",
                )}
              >
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
                    className="text-xs font-semibold text-grape"
                  >
                    Copy
                  </button>
                  <button
                    onClick={() => revoke.mutate(inv.code)}
                    className="text-xs font-semibold text-coral"
                  >
                    Revoke
                  </button>
                </>
              )}
            </div>
          );
        })}
        {invites.length === 0 && <div className="p-4 text-sm text-inkmuted">No invites yet.</div>}
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

function SettingsTab({ group, groupId }: { group: Group; groupId: string }) {
  const update = useUpdateGroup(groupId);
  const pushToast = useToasts((s) => s.push);

  const setCycle = (patch: Partial<Group["cycleSettings"]>) =>
    update.mutate({ cycleSettings: { ...group.cycleSettings, ...patch } });
  const setNotifications = (patch: Partial<Group["notificationSettings"]>) =>
    update.mutate({ notificationSettings: { ...group.notificationSettings, ...patch } });

  return (
    <div className="space-y-4 px-5 pt-4">
      <Section title="Cycle">
        <NumberField
          label="Questions per cycle"
          value={group.cycleSettings.questionsPerCycle}
          min={1}
          max={20}
          onSave={(v) => setCycle({ questionsPerCycle: v })}
        />
        <NumberField
          label="Votes per user per cycle"
          value={group.cycleSettings.votesPerUserPerCycle}
          min={1}
          max={20}
          onSave={(v) => setCycle({ votesPerUserPerCycle: v })}
        />
        <NumberField
          label="Response window (days)"
          value={group.cycleSettings.responseWindowDays}
          min={1}
          max={30}
          onSave={(v) => setCycle({ responseWindowDays: v })}
        />
        <ReadOnlyField label="Timezone" value={group.timezone} />
      </Section>
      <Section title="Notifications">
        <ReadOnlyField
          label="Reminders before deadline"
          value={`${group.notificationSettings.offsetsHoursBeforeClose.join("h, ")}h`}
        />
        <ToggleField
          label="Send when cycle opens"
          value={group.notificationSettings.onCycleOpen}
          onChange={(v) => {
            setNotifications({ onCycleOpen: v });
            pushToast(`Cycle-open notifications ${v ? "on" : "off"}.`, "default");
          }}
        />
      </Section>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div>
      <div className="mb-2 ml-1 text-xs font-bold uppercase tracking-widest text-inkmuted">
        {title}
      </div>
      <div className="divide-y divide-line rounded-3xl border border-line bg-white shadow-soft">
        {children}
      </div>
    </div>
  );
}

function ReadOnlyField({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center gap-3 p-4">
      <div className="flex-1 text-sm">{label}</div>
      <div className="text-sm font-semibold text-inkmuted">{value}</div>
    </div>
  );
}

function NumberField({
  label,
  value,
  min,
  max,
  onSave,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  onSave: (v: number) => void;
}) {
  const [local, setLocal] = useState(String(value));
  useEffect(() => setLocal(String(value)), [value]);

  function commit() {
    const next = Number.parseInt(local, 10);
    if (Number.isNaN(next)) {
      setLocal(String(value));
      return;
    }
    const clamped = Math.max(min, Math.min(max, next));
    setLocal(String(clamped));
    if (clamped !== value) onSave(clamped);
  }

  return (
    <div className="flex items-center gap-3 p-4">
      <div className="flex-1 text-sm">{label}</div>
      <input
        type="number"
        min={min}
        max={max}
        value={local}
        onChange={(e) => setLocal(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") (e.target as HTMLInputElement).blur();
        }}
        className="w-20 rounded-xl border border-line bg-cream px-3 py-1.5 text-right text-sm font-semibold focus:outline-none focus:ring-2 focus:ring-coral/30"
      />
    </div>
  );
}

function ToggleField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div className="flex items-center gap-3 p-4">
      <div className="flex-1 text-sm">{label}</div>
      <button
        onClick={() => onChange(!value)}
        className={`toggle ${value ? "on" : ""}`}
        aria-label={`Toggle ${label}`}
      />
    </div>
  );
}
