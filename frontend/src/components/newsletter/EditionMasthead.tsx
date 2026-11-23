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
  reactionCount: number;
};

/** Contributor avatars shown before collapsing into "+N" (`docs/design/frontend.md` §7.2). */
const MAX_AVATARS = 5;

/**
 * The published edition's masthead (`docs/design/frontend.md` §7.2, §14a): a slim band in the
 * group's gradient with a contributor avatar stack and "N answers · M min read · K reactions".
 * The edition label is already in the page header, so here it's a visually hidden heading only.
 * Shares a `view-transition-name` with the Home row's date tile it morphed from
 * (`docs/design/frontend.md` §18) — the same name as `EditionRow` uses, keyed by cycle.
 */
export function EditionMasthead({
  cycleId,
  title,
  gradient,
  contributors,
  answerCount,
  readingMinutes,
  reactionCount,
}: Props) {
  const shown = contributors.slice(0, MAX_AVATARS);
  const overflow = contributors.length - shown.length;
  const stats = [
    `${answerCount} answer${answerCount === 1 ? "" : "s"}`,
    `${readingMinutes} min read`,
    reactionCount > 0 ? `${reactionCount} reaction${reactionCount === 1 ? "" : "s"}` : null,
  ].filter(Boolean);

  return (
    <div
      style={{ viewTransitionName: `edition-${cycleId}` }}
      className={`${gradientClasses(gradient)} flex items-center gap-3 bg-[length:200%_200%] px-5 py-3 motion-safe:animate-gradient-drift`}
    >
      <h1 className="sr-only">{title}</h1>
      {shown.length > 0 && (
        <div className="flex shrink-0 -space-x-2">
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
      )}
      <p className="text-sm font-semibold opacity-90">{stats.join(" · ")}</p>
    </div>
  );
}
