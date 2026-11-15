import clsx from "clsx";
import type { ReactNode } from "react";

/**
 * The invite-only rule, worded the same wherever someone might try to sign up
 * (`docs/design/frontend.md` §7.1, §7.8, §7.10). `children` adds a next step.
 */
export function InviteOnlyNote({
  children,
  className,
}: {
  children?: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={clsx(
        "flex gap-3 rounded-3xl border border-grape/20 bg-white/70 p-4 text-left text-sm",
        className,
      )}
    >
      <span
        aria-hidden="true"
        className="grid h-9 w-9 shrink-0 place-items-center rounded-2xl bg-lavender text-lg"
      >
        🔒
      </span>
      <div>
        <p className="font-bold text-ink">Invite only</p>
        <p className="mt-0.5 text-inkmuted">
          You can&apos;t create an account without an invite code from a group.
        </p>
        {children && <div className="mt-2">{children}</div>}
      </div>
    </div>
  );
}
