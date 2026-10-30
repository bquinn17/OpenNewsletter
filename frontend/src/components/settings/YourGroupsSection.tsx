import { useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { ApiError, apiErrorMessage } from "../../api/client";
import { useLeaveGroup } from "../../api/mutations";
import { useToasts } from "../../state/toast";
import { ConfirmDialog } from "../ui/ConfirmDialog";
import type { components } from "../../types/api";

type S = components["schemas"];

type Props = { memberships: S["MembershipSummary"][] };

export function YourGroupsSection({ memberships }: Props) {
  const navigate = useNavigate();
  const leaveGroup = useLeaveGroup();
  const pushToast = useToasts((s) => s.push);
  const [leaveTarget, setLeaveTarget] = useState<{ groupId: string; groupName: string } | null>(
    null,
  );

  const handleLeave = async () => {
    if (!leaveTarget) return;
    try {
      await leaveGroup.mutateAsync(leaveTarget.groupId);
      pushToast(`Left ${leaveTarget.groupName}.`, "default");
    } catch (e) {
      const message =
        e instanceof ApiError && e.code === "LAST_ADMIN"
          ? "You're the only admin — promote someone else first."
          : apiErrorMessage(e, "Couldn't leave that group.");
      pushToast(message, "error");
    } finally {
      setLeaveTarget(null);
    }
  };

  return (
    <div className="mt-5 px-5">
      <div className="mb-2 ml-1 text-xs font-bold uppercase tracking-widest text-inkmuted">
        Your groups
      </div>
      <div className="divide-y divide-line rounded-3xl border border-line bg-white shadow-soft">
        {memberships.map((m) => (
          <div key={m.groupId} className="flex items-center gap-3 p-4">
            <div className="min-w-0 flex-1">
              <div className="truncate text-sm font-semibold">{m.groupName}</div>
              <div className="text-xs capitalize text-inkmuted">{m.role}</div>
            </div>
            <button
              onClick={() => navigate(`/g/${m.groupId}`)}
              className="text-xs font-semibold text-grape"
            >
              View
            </button>
            {m.role === "admin" && (
              <Link to={`/g/${m.groupId}/admin`} className="text-xs font-semibold text-grape">
                Manage
              </Link>
            )}
            <button
              onClick={() => setLeaveTarget({ groupId: m.groupId, groupName: m.groupName })}
              className="text-xs font-semibold text-coral"
              disabled={leaveGroup.isPending}
            >
              Leave
            </button>
          </div>
        ))}
        {memberships.length === 0 && (
          <div className="p-4 text-sm text-inkmuted">You haven&apos;t joined a group yet.</div>
        )}
        <Link
          to="/join"
          className="block flex w-full items-center gap-3 p-4 text-left hover:bg-cream"
        >
          <div className="grid h-10 w-10 place-items-center rounded-2xl border-2 border-dashed border-line bg-cream text-inkmuted">
            ＋
          </div>
          <div className="flex-1 text-sm font-semibold">Join with an invite code</div>
          <span className="text-grape">→</span>
        </Link>
      </div>

      <ConfirmDialog
        open={!!leaveTarget}
        title={leaveTarget ? `Leave ${leaveTarget.groupName}?` : ""}
        message="You'll lose access to its newsletters and need a new invite to rejoin."
        confirmLabel="Leave"
        tone="danger"
        busy={leaveGroup.isPending}
        onConfirm={handleLeave}
        onCancel={() => setLeaveTarget(null)}
      />
    </div>
  );
}
