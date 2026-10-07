import { useCallback, useRef, useState } from "react";
import { useParams } from "react-router-dom";
import clsx from "clsx";
import { useGroup, useInvites, useMe } from "../api/queries";
import { useCreateInvite, useKickMember, usePatchMember, useRevokeInvite } from "../api/mutations";
import { ApiError } from "../api/client";
import { useToasts } from "../state/toast";
import { PageHeader } from "../components/layout/PageHeader";
import { GroupSettingsForm } from "../components/admin/GroupSettingsForm";
import { Avatar } from "../components/ui/Avatar";
import { Button } from "../components/ui/Button";
import { ConfirmDialog } from "../components/ui/ConfirmDialog";
import { Pill } from "../components/ui/Pill";
import { Spinner } from "../components/ui/Spinner";
import { useDismissableMenu } from "../components/ui/useDismissableMenu";
import { avatarColorClass } from "../utils/avatarColor";
import { formatRelative } from "../utils/dates";
import { inviteUrl } from "../utils/inviteUrl";
import type { components } from "../types/api";

type S = components["schemas"];
type GroupResponse = S["GroupResponse"];
type MemberResponse = S["MemberResponse"];
type InviteSummary = S["InviteSummary"];

type Tab = "members" | "invites" | "settings";

const TABS: { id: Tab; label: string }[] = [
  { id: "members", label: "Members" },
  { id: "invites", label: "Invites" },
  { id: "settings", label: "Settings" },
];

/** A 409's `LAST_ADMIN` gets a friendly, specific message; anything else falls back to `fallback`. */
function errorMessage(error: unknown, fallback: string): string {
  if (error instanceof ApiError && error.code === "LAST_ADMIN") {
    return "A group needs at least one admin — promote someone else first.";
  }
  return fallback;
}

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
      <div className="bg-cream pb-12">
        <PageHeader eyebrow="Admin" title="Couldn't load" back="/settings" />
        <div className="px-5 pt-8 text-center">
          <div className="mb-3 text-5xl">😬</div>
          <p className="font-semibold text-ink">Couldn&apos;t load this group&apos;s admin page.</p>
          <button
            onClick={() => group.refetch()}
            className="mt-5 rounded-full bg-ink px-5 py-3 font-semibold text-cream"
          >
            Try again
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="bg-cream pb-12">
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

function MembersTab({ group }: { group: GroupResponse }) {
  const me = useMe();
  const patchMember = usePatchMember(group.groupId);
  const kickMember = useKickMember(group.groupId);
  const pushToast = useToasts((s) => s.push);
  const [openMenuUserId, setOpenMenuUserId] = useState<string | null>(null);
  const [removeTarget, setRemoveTarget] = useState<MemberResponse | null>(null);

  async function handleMakeAdmin(member: MemberResponse) {
    try {
      await patchMember.mutateAsync({ userId: member.userId, role: "admin" });
      pushToast(`${member.displayName} is now an admin.`, "success");
    } catch (error) {
      pushToast(errorMessage(error, "Couldn't update that member's role."), "error");
    }
    setOpenMenuUserId(null);
  }

  async function handleMakeMember(member: MemberResponse) {
    try {
      await patchMember.mutateAsync({ userId: member.userId, role: "member" });
      pushToast(`${member.displayName} is now a member.`, "success");
    } catch (error) {
      pushToast(errorMessage(error, "Couldn't update that member's role."), "error");
    }
    setOpenMenuUserId(null);
  }

  async function handleRemove() {
    if (!removeTarget) return;
    try {
      await kickMember.mutateAsync(removeTarget.userId);
      pushToast(`Removed ${removeTarget.displayName}.`, "default");
    } catch (error) {
      pushToast(errorMessage(error, "Couldn't remove that member."), "error");
    } finally {
      setRemoveTarget(null);
      setOpenMenuUserId(null);
    }
  }

  return (
    <div className="px-5 pt-4">
      <div className="mb-2 ml-1 text-xs font-bold uppercase tracking-widest text-inkmuted">
        {group.memberCount} of {group.memberSoftCap} members
      </div>
      <div className="divide-y divide-line rounded-3xl border border-line bg-white shadow-soft">
        {group.members.map((member) => (
          <MemberRow
            key={member.userId}
            member={member}
            isMe={me.data?.userId === member.userId}
            menuOpen={openMenuUserId === member.userId}
            onMenuOpenChange={(open) => setOpenMenuUserId(open ? member.userId : null)}
            disabled={patchMember.isPending || kickMember.isPending}
            onMakeAdmin={member.role === "member" ? () => void handleMakeAdmin(member) : undefined}
            onMakeMember={member.role === "admin" ? () => void handleMakeMember(member) : undefined}
            onRemove={() => setRemoveTarget(member)}
          />
        ))}
      </div>

      <ConfirmDialog
        open={!!removeTarget}
        title={removeTarget ? `Remove ${removeTarget.displayName} from the group?` : ""}
        message="They'll lose access to this group's newsletters and need a new invite to rejoin."
        confirmLabel="Remove"
        tone="danger"
        busy={kickMember.isPending}
        onConfirm={handleRemove}
        onCancel={() => setRemoveTarget(null)}
      />
    </div>
  );
}

function MemberRow({
  member,
  isMe,
  menuOpen,
  onMenuOpenChange,
  disabled,
  onMakeAdmin,
  onMakeMember,
  onRemove,
}: {
  member: MemberResponse;
  isMe: boolean;
  menuOpen: boolean;
  onMenuOpenChange: (open: boolean) => void;
  disabled: boolean;
  onMakeAdmin?: () => void;
  onMakeMember?: () => void;
  onRemove: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const close = useCallback(() => onMenuOpenChange(false), [onMenuOpenChange]);
  useDismissableMenu(ref, menuOpen, close);

  return (
    <div className="flex items-center gap-3 p-4">
      <Avatar
        name={member.displayName}
        url={member.avatarUrl}
        colorClassName={avatarColorClass(member.avatarColor)}
      />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2 text-sm font-semibold">
          {member.displayName}
          {isMe && <span className="text-[10px] font-normal text-inkmuted">(you)</span>}
          {member.role === "admin" && (
            <Pill tone="coral" className="!px-1.5 !py-0.5 !text-[10px]">
              admin
            </Pill>
          )}
        </div>
        <div className="text-xs text-inkmuted">
          {member.editionsAnswered > 0
            ? `${member.editionsAnswered} editions answered`
            : `joined ${formatRelative(member.joinedAt)} · pending first answer`}
        </div>
      </div>

      {/* The caller's own row has no menu — leaving a group lives in Settings
          → Your groups, and self-demotion is out of scope (`04` §7.7). */}
      {!isMe && (
        <div ref={ref} className="relative">
          <button
            onClick={() => onMenuOpenChange(!menuOpen)}
            disabled={disabled}
            aria-haspopup="menu"
            aria-expanded={menuOpen}
            aria-label={`Options for ${member.displayName}`}
            className="h-8 w-8 rounded-full text-inkmuted hover:bg-cream disabled:opacity-30"
          >
            ⋯
          </button>
          {menuOpen && (
            <div
              role="menu"
              className="absolute right-0 top-9 z-30 w-48 animate-pop rounded-2xl border border-line bg-white p-1 shadow-pop"
            >
              {onMakeAdmin && (
                <button
                  role="menuitem"
                  onClick={onMakeAdmin}
                  className="w-full rounded-xl px-3 py-2 text-left text-sm font-semibold hover:bg-cream"
                >
                  Make admin
                </button>
              )}
              {onMakeMember && (
                <button
                  role="menuitem"
                  onClick={onMakeMember}
                  className="w-full rounded-xl px-3 py-2 text-left text-sm font-semibold hover:bg-cream"
                >
                  Make member
                </button>
              )}
              <button
                role="menuitem"
                onClick={onRemove}
                className="w-full rounded-xl px-3 py-2 text-left text-sm font-semibold text-coral hover:bg-cream"
              >
                Remove from group
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

const INVITE_ROLE_OPTIONS: { value: S["Role"]; label: string }[] = [
  { value: "member", label: "Member" },
  { value: "admin", label: "Admin" },
];
// Server max is `INVITE_MAX_TTL_DAYS` = 90 (`shared/src/config.rs`).
const INVITE_TTL_OPTIONS = [1, 3, 7, 14, 30, 90];
const DEFAULT_TTL_DAYS = 7;

type DecoratedInvite = InviteSummary & { expired: boolean };

function InvitesTab({ groupId, group }: { groupId: string; group: GroupResponse }) {
  const invitesQuery = useInvites(groupId);
  const createInvite = useCreateInvite(groupId);
  const revokeInvite = useRevokeInvite(groupId);
  const pushToast = useToasts((s) => s.push);
  const [role, setRole] = useState<S["Role"]>("member");
  const [ttlDays, setTtlDays] = useState(DEFAULT_TTL_DAYS);
  const [justCreated, setJustCreated] = useState<S["CreateInviteResponse"] | null>(null);

  async function handleCreate() {
    try {
      const invite = await createInvite.mutateAsync({ groupId, roleOnRedeem: role, ttlDays });
      setJustCreated(invite);
    } catch {
      pushToast("Couldn't create that invite. Try again.", "error");
    }
  }

  async function handleRevoke(code: string) {
    try {
      await revokeInvite.mutateAsync(code);
      pushToast("Invite revoked.", "default");
      if (justCreated?.code === code) setJustCreated(null);
    } catch {
      pushToast("Couldn't revoke that invite. Try again.", "error");
    }
  }

  const now = Date.now();
  const decorated: DecoratedInvite[] = (invitesQuery.data?.items ?? []).map((invite) => ({
    ...invite,
    expired: invite.status === "pending" && new Date(invite.expiresAt).getTime() <= now,
  }));
  const sorted = [...decorated].sort((a, b) => {
    const aActive = a.status === "pending" && !a.expired;
    const bActive = b.status === "pending" && !b.expired;
    if (aActive !== bActive) return aActive ? -1 : 1;
    return b.createdAt.localeCompare(a.createdAt);
  });

  return (
    <div className="px-5 pt-4">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void handleCreate();
        }}
        className="mb-4 space-y-3 rounded-3xl border border-line bg-white p-4 shadow-soft"
      >
        <div className="flex gap-3">
          <label className="flex-1 text-sm">
            <span className="mb-1 block text-xs font-bold uppercase tracking-widest text-inkmuted">
              Role
            </span>
            <select
              value={role}
              onChange={(e) => setRole(e.target.value as S["Role"])}
              className="w-full rounded-2xl border border-line bg-cream px-3 py-2 text-sm"
            >
              {INVITE_ROLE_OPTIONS.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </select>
          </label>
          <label className="flex-1 text-sm">
            <span className="mb-1 block text-xs font-bold uppercase tracking-widest text-inkmuted">
              Expires in
            </span>
            <select
              value={ttlDays}
              onChange={(e) => setTtlDays(Number(e.target.value))}
              className="w-full rounded-2xl border border-line bg-cream px-3 py-2 text-sm"
            >
              {INVITE_TTL_OPTIONS.map((d) => (
                <option key={d} value={d}>
                  {d} day{d === 1 ? "" : "s"}
                </option>
              ))}
            </select>
          </label>
        </div>
        <Button type="submit" disabled={createInvite.isPending} className="w-full">
          {createInvite.isPending ? "Creating…" : "Create invite"}
        </Button>
      </form>

      {justCreated && <NewInvite invite={justCreated} onDismiss={() => setJustCreated(null)} />}

      <div className="divide-y divide-line rounded-3xl border border-line bg-white shadow-soft">
        {sorted.map((invite) => (
          <InviteRow
            key={invite.code}
            invite={invite}
            group={group}
            onRevoke={() => void handleRevoke(invite.code)}
            revoking={revokeInvite.isPending}
          />
        ))}
        {sorted.length === 0 && <div className="p-4 text-sm text-inkmuted">No invites yet.</div>}
      </div>
    </div>
  );
}

async function copyInviteLink(
  code: string,
  pushToast: (message: string, tone?: "default" | "success" | "error") => void,
): Promise<void> {
  try {
    await navigator.clipboard.writeText(inviteUrl(code));
    pushToast("Invite link copied.", "success");
  } catch {
    pushToast("Couldn't copy — long-press to select.", "error");
  }
}

function NewInvite({
  invite,
  onDismiss,
}: {
  invite: S["CreateInviteResponse"];
  onDismiss: () => void;
}) {
  const pushToast = useToasts((s) => s.push);
  const url = inviteUrl(invite.code);

  return (
    <div className="mb-4 rounded-3xl border border-grape/30 bg-grape/5 p-4">
      <div className="text-sm font-semibold text-grape">Invite created</div>
      <label htmlFor="new-invite-url" className="sr-only">
        Invite link
      </label>
      <input
        id="new-invite-url"
        readOnly
        value={url}
        onFocus={(e) => e.target.select()}
        className="mt-2 w-full rounded-2xl border border-line bg-white px-3 py-2 text-xs"
      />
      <div className="mt-2 flex justify-end gap-2">
        <button onClick={onDismiss} className="text-xs font-semibold text-inkmuted">
          Dismiss
        </button>
        <Button type="button" size="sm" onClick={() => void copyInviteLink(invite.code, pushToast)}>
          Copy
        </Button>
      </div>
    </div>
  );
}

function InviteRow({
  invite,
  group,
  onRevoke,
  revoking,
}: {
  invite: DecoratedInvite;
  group: GroupResponse;
  onRevoke: () => void;
  revoking: boolean;
}) {
  const pushToast = useToasts((s) => s.push);
  const active = invite.status === "pending" && !invite.expired;

  if (active) {
    return (
      <div className="flex items-center gap-3 p-4">
        <div className="min-w-0 flex-1">
          <div className="font-mono text-sm">{invite.code}</div>
          <div className="text-xs capitalize text-inkmuted">
            {invite.roleOnRedeem} · expires {formatRelative(invite.expiresAt)}
          </div>
        </div>
        <button
          onClick={() => void copyInviteLink(invite.code, pushToast)}
          className="shrink-0 text-xs font-semibold text-grape"
        >
          Copy
        </button>
        <button
          onClick={onRevoke}
          disabled={revoking}
          className="shrink-0 text-xs font-semibold text-coral disabled:opacity-50"
        >
          Revoke
        </button>
      </div>
    );
  }

  if (invite.status === "consumed") {
    const consumedByName =
      group.members.find((m) => m.userId === invite.consumedBy)?.displayName ?? "a former member";
    return (
      <div className="p-4 text-sm text-inkmuted opacity-60">
        <div className="font-mono">{invite.code}</div>
        <div className="text-xs">
          used by {consumedByName} · {formatRelative(invite.consumedAt!)}
        </div>
      </div>
    );
  }

  return (
    <div className="p-4 text-sm text-inkmuted opacity-60">
      <div className="font-mono">{invite.code}</div>
      <div className="text-xs">{invite.expired ? "expired" : "revoked"}</div>
    </div>
  );
}
