import { useCallback, useRef, useState } from "react";
import { useConfig } from "../../api/queries";
import { useKickMember, usePatchMember } from "../../api/mutations";
import { ApiError } from "../../api/client";
import { useToasts } from "../../state/toast";
import { Avatar } from "../ui/Avatar";
import { ConfirmDialog } from "../ui/ConfirmDialog";
import { Pill } from "../ui/Pill";
import { useDismissableMenu } from "../ui/useDismissableMenu";
import { avatarColorClass } from "../../utils/avatarColor";
import { formatRelative } from "../../utils/dates";
import type { components } from "../../types/api";

type S = components["schemas"];
type GroupResponse = S["GroupResponse"];
type MemberResponse = S["MemberResponse"];

/** A 409's `LAST_ADMIN` gets a friendly, specific message; anything else falls back to `fallback`. */
function errorMessage(error: unknown, fallback: string): string {
  if (error instanceof ApiError && error.code === "LAST_ADMIN") {
    return "A group needs at least one admin — promote someone else first.";
  }
  return fallback;
}

type Props = { group: GroupResponse };

export function MembersTab({ group }: Props) {
  const config = useConfig();
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
        {group.memberCount} of {group.memberSoftCap} member{group.memberSoftCap === 1 ? "" : "s"}
      </div>
      <div className="divide-y divide-line rounded-3xl border border-line bg-white shadow-soft">
        {group.members.map((member) => (
          <MemberRow
            key={member.userId}
            member={member}
            isMe={config.data?.userId === member.userId}
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

type MemberRowProps = {
  member: MemberResponse;
  isMe: boolean;
  menuOpen: boolean;
  onMenuOpenChange: (open: boolean) => void;
  disabled: boolean;
  onMakeAdmin?: () => void;
  onMakeMember?: () => void;
  onRemove: () => void;
};

function MemberRow({
  member,
  isMe,
  menuOpen,
  onMenuOpenChange,
  disabled,
  onMakeAdmin,
  onMakeMember,
  onRemove,
}: MemberRowProps) {
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
            ? `${member.editionsAnswered} edition${member.editionsAnswered === 1 ? "" : "s"} answered`
            : `joined ${formatRelative(member.joinedAt)} · pending first answer`}
        </div>
      </div>

      {/* The caller's own row has no menu — leaving a group lives in Settings
          → Your groups, and self-demotion is out of scope (`docs/design/frontend.md` §7.7). */}
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
