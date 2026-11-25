import { Avatar } from "../ui/Avatar";
import { Pill } from "../ui/Pill";
import { avatarColorClass } from "../../utils/avatarColor";
import { formatRelative } from "../../utils/dates";
import type { components } from "../../types/api";

type MemberResponse = components["schemas"]["MemberResponse"];

/**
 * A member's avatar, name, role and answering record: the left side of a row in the Members
 * tab on Home and in the admin page (`docs/design/frontend.md` §7.1, §7.7).
 */
export function MemberSummary({ member, isMe }: { member: MemberResponse; isMe: boolean }) {
  return (
    <>
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
    </>
  );
}
