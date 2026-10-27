import { useState } from "react";
import { useInvites } from "../../api/queries";
import { useCreateInvite, useRevokeInvite } from "../../api/mutations";
import { useToasts } from "../../state/toast";
import { Button } from "../ui/Button";
import { formatRelative } from "../../utils/dates";
import { inviteUrl } from "../../utils/inviteUrl";
import type { components } from "../../types/api";

type S = components["schemas"];
type GroupResponse = S["GroupResponse"];
type InviteSummary = S["InviteSummary"];

const INVITE_ROLE_OPTIONS: { value: S["Role"]; label: string }[] = [
  { value: "member", label: "Member" },
  { value: "admin", label: "Admin" },
];
// Server max is `INVITE_MAX_TTL_DAYS` = 90 (`shared/src/config.rs`).
const INVITE_TTL_OPTIONS = [1, 3, 7, 14, 30, 90];
const DEFAULT_TTL_DAYS = 7;

type DecoratedInvite = InviteSummary & { expired: boolean };

type Props = { groupId: string; group: GroupResponse };

export function InvitesTab({ groupId, group }: Props) {
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
