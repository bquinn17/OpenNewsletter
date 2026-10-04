import { useCallback, useRef, useState } from "react";
import { Avatar } from "../ui/Avatar";
import { useDismissableMenu } from "../ui/useDismissableMenu";
import { gradientClasses } from "../../utils/gradient";
import type { components } from "../../types/api";

type MembershipSummary = components["schemas"]["MembershipSummary"];

type Props = {
  memberships: MembershipSummary[];
  currentGroupId: string | null;
  onPick: (groupId: string) => void;
};

export function GroupSwitcher({ memberships, currentGroupId, onPick }: Props) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  const close = useCallback(() => setOpen(false), []);
  useDismissableMenu(ref, open, close);

  if (memberships.length === 0) return null;

  const current = memberships.find((m) => m.groupId === currentGroupId) ?? memberships[0]!;

  return (
    <div ref={ref} className="relative min-w-0">
      <button
        onClick={() => setOpen((v) => !v)}
        aria-haspopup="menu"
        aria-expanded={open}
        className="flex min-w-0 items-center gap-2 rounded-full px-2 py-1 hover:bg-white/60"
      >
        <Avatar
          name={current.groupName}
          size="xs"
          colorClassName={gradientClasses(current.gradient)}
        />
        <span className="max-w-[10rem] truncate text-sm font-semibold">{current.groupName}</span>
        {memberships.length > 1 && (
          <svg
            className="h-4 w-4 shrink-0 text-inkmuted"
            viewBox="0 0 20 20"
            fill="currentColor"
            aria-hidden
          >
            <path d="M5.23 7.21a.75.75 0 011.06.02L10 11.06l3.71-3.83a.75.75 0 111.08 1.04l-4.25 4.39a.75.75 0 01-1.08 0L5.21 8.27a.75.75 0 01.02-1.06z" />
          </svg>
        )}
      </button>
      {open && memberships.length > 1 && (
        <div
          role="menu"
          className="absolute left-0 z-40 mt-2 w-60 rounded-2xl border border-line bg-white p-1 shadow-pop"
        >
          {memberships.map((m) => (
            <button
              key={m.groupId}
              role="menuitem"
              onClick={() => {
                onPick(m.groupId);
                setOpen(false);
              }}
              className="flex w-full items-center gap-2 rounded-xl px-3 py-2 text-left hover:bg-cream"
            >
              <Avatar name={m.groupName} size="xs" colorClassName={gradientClasses(m.gradient)} />
              <span className="truncate text-sm font-semibold">{m.groupName}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
