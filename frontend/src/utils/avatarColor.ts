import type { components } from "../types/api";

export type AvatarColorSlug = components["schemas"]["AvatarColorSlug"];

/** Every `AvatarColorSlug` value (`docs/design/api.md` §2.3). Keep in sync with `AVATAR_COLOR_CLASSES` below. */
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
// (docs/design/frontend.md §14a).
// Ink text on the lighter fills, white on the one (violet) dark enough to need it — each
// combination passes 4.5:1 (`docs/design/frontend.md` §4.2). `violet-600` instead of
// `-500` is still recognizably violet.
const AVATAR_COLOR_CLASSES: Record<AvatarColorSlug, string> = {
  red: "bg-red-500 text-ink",
  orange: "bg-orange-500 text-ink",
  amber: "bg-amber-500 text-ink",
  green: "bg-green-500 text-ink",
  teal: "bg-teal-500 text-ink",
  blue: "bg-blue-500 text-ink",
  violet: "bg-violet-600 text-white",
  pink: "bg-pink-500 text-ink",
};

export function avatarColorClass(slug: AvatarColorSlug): string {
  return AVATAR_COLOR_CLASSES[slug];
}

/** A stable (same name → same color every render) fallback for an avatar with no `avatarColor`. */
export function avatarColorClassForName(name: string): string {
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = (hash * 31 + name.charCodeAt(i)) | 0;
  }
  const index = Math.abs(hash) % AVATAR_COLOR_SLUGS.length;
  return AVATAR_COLOR_CLASSES[AVATAR_COLOR_SLUGS[index]!];
}
