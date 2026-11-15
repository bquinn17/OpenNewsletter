import clsx from "clsx";

const STEPS = ["Invite code", "Sign in", "You're in!"] as const;

/** Where a newcomer is in joining: a code, then an IdP, then their group. */
export function SignUpSteps({ current }: { current: 1 | 2 }) {
  return (
    <ol aria-label="Joining your group" className="flex items-center gap-2 text-xs font-bold">
      {STEPS.map((label, index) => {
        const step = index + 1;
        const done = step < current;
        const active = step === current;
        return (
          <li
            key={label}
            aria-current={active ? "step" : undefined}
            className="flex min-w-0 flex-1 items-center gap-2"
          >
            <span
              className={clsx(
                "grid h-6 w-6 shrink-0 place-items-center rounded-full",
                done && "bg-mint text-white",
                active && "bg-ink text-cream",
                !done && !active && "bg-white/70 text-inkmuted",
              )}
            >
              {done ? "✓" : step}
            </span>
            <span className={clsx("truncate", active ? "text-ink" : "text-inkmuted")}>{label}</span>
            {step < STEPS.length && <span className="h-px flex-1 bg-ink/10" aria-hidden="true" />}
          </li>
        );
      })}
    </ol>
  );
}
