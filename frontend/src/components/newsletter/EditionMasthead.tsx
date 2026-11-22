import type { components } from "../../types/api";
import { gradientClasses } from "../../utils/gradient";
import { Avatar } from "../ui/Avatar";

type Contributor = {
  userId: string;
  displayName: string;
  avatarUrl?: string | null;
  colorClassName?: string;
};

type Props = {
  cycleId: string;
  title: string;
  gradient: components["schemas"]["GradientSlug"];
  contributors: Contributor[];
  answerCount: number;
  readingMinutes: number;
};

/** Contributor avatars shown before collapsing into "+N" (`docs/design/frontend.md` §7.2). */
const MAX_AVATARS = 5;

/**
 * The published edition's hero tier (`docs/design/frontend.md` §7.2, §14a): the group's gradient, the edition label in large
 * Fraunces, a contributor avatar stack, and "N answers · M min read". Shares a
 * `view-transition-name` with the Home row's date tile it morphed from (`docs/design/frontend.md`
 * §18) — the same name as `EditionRow` uses, keyed by cycle.
 */
export function EditionMasthead({
  cycleId,
  title,
  gradient,
  contributors,
  answerCount,
  readingMinutes,
}: Props) {
  const shown = contributors.slice(0, MAX_AVATARS);
  const overflow = contributors.length - shown.length;

  return (
    <div
      style={{ viewTransitionName: `edition-${cycleId}` }}
      className={`${gradientClasses(gradient)} bg-[length:200%_200%] px-5 py-8 motion-safe:animate-gradient-drift`}
    >
      <h1 className="font-display text-3xl font-bold leading-tight">{title}</h1>
      <div className="mt-4 flex items-center gap-3">
        <div className="flex -space-x-2">
          {shown.map((contributor) => (
            <Avatar
              key={contributor.userId}
              name={contributor.displayName}
              url={contributor.avatarUrl}
              colorClassName={contributor.colorClassName}
              size="sm"
              ringed
            />
          ))}
          {overflow > 0 && (
            <span className="grid h-9 w-9 place-items-center rounded-full bg-black/20 text-xs font-bold ring-2 ring-white">
              +{overflow}
            </span>
          )}
        </div>
        <p className="text-sm font-semibold opacity-90">
          {answerCount} answer{answerCount === 1 ? "" : "s"} · {readingMinutes} min read
        </p>
      </div>
    </div>
  );
}
