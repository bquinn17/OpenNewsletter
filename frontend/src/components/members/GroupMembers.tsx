import { useConfig, useGroup } from "../../api/queries";
import { Button } from "../ui/Button";
import { MemberSummary } from "./MemberSummary";

/** Home's Members tab: who's in the group, read-only (`docs/design/frontend.md` §7.1). */
export function GroupMembers({ groupId }: { groupId: string }) {
  const { data: config } = useConfig();
  const group = useGroup(groupId);

  if (group.isLoading) {
    return (
      <div className="space-y-2">
        {[0, 1, 2].map((i) => (
          <div key={i} className="h-16 animate-pulse rounded-3xl border border-line bg-white" />
        ))}
      </div>
    );
  }

  if (group.isError || !group.data) {
    return (
      <div className="py-6 text-center">
        <p className="text-sm text-coral">Couldn&apos;t load this group&apos;s members.</p>
        <Button className="mt-3" size="sm" variant="soft" onClick={() => void group.refetch()}>
          Try again
        </Button>
      </div>
    );
  }

  const { memberCount, members } = group.data;

  return (
    <section aria-label="Members">
      <h2 className="mb-2 px-1 text-xs font-bold uppercase tracking-widest text-inkmuted">
        {memberCount} member{memberCount === 1 ? "" : "s"}
      </h2>
      <ul className="divide-y divide-line rounded-3xl border border-line bg-white shadow-soft">
        {members.map((member) => (
          <li key={member.userId} className="flex items-center gap-3 p-4">
            <MemberSummary member={member} isMe={config?.userId === member.userId} />
          </li>
        ))}
      </ul>
    </section>
  );
}
