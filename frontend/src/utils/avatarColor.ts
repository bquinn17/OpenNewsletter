import type { components } from "../types/api";

export type AvatarColorSlug = components["schemas"]["AvatarColorSlug"];

/** Every `AvatarColorSlug` value (`03-api-contract.md` §2.3). Keep in sync with `AVATAR_COLOR_CLASSES` below. */
export const AVATAR_COLOR_SLUGS = [
  "red",
  "orange",
  "amber",
  "green",
  "teal",
  "blue",
  "violet",
  "pink",
] as const;

// Tailwind classes for the eight `AvatarColorSlug` values. `Record` over the
// literal union forces this to stay exhaustive if the enum ever changes
// (04-frontend-architecture.md §14a).
const AVATAR_COLOR_CLASSES: Record<AvatarColorSlug, string> = {
  red: "bg-red-500 text-white",
  orange: "bg-orange-500 text-white",
  amber: "bg-amber-500 text-ink",
  green: "bg-green-500 text-white",
  teal: "bg-teal-500 text-white",
  blue: "bg-blue-500 text-white",
  violet: "bg-violet-500 text-white",
  pink: "bg-pink-500 text-white",
};

export function avatarColorClass(slug: AvatarColorSlug): string {
  return AVATAR_COLOR_CLASSES[slug];
}
