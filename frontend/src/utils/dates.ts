import { formatDistanceToNowStrict } from "date-fns";
import type { components } from "../types/api";

type CycleCadence = components["schemas"]["CycleCadence"];

export function formatRelative(iso: string): string {
  return formatDistanceToNowStrict(new Date(iso), { addSuffix: true });
}

export interface Countdown {
  days: number;
  hours: number;
  minutes: number;
  expired: boolean;
}

export function countdownTo(iso: string): Countdown {
  const ms = new Date(iso).getTime() - Date.now();
  if (ms <= 0) return { days: 0, hours: 0, minutes: 0, expired: true };
  const days = Math.floor(ms / (24 * 60 * 60 * 1000));
  const hours = Math.floor((ms % (24 * 60 * 60 * 1000)) / (60 * 60 * 1000));
  const minutes = Math.floor((ms % (60 * 60 * 1000)) / (60 * 1000));
  return { days, hours, minutes, expired: false };
}

export function shortCountdown(iso: string): string {
  const c = countdownTo(iso);
  if (c.expired) return "closed";
  if (c.days > 0) return `${c.days}d ${c.hours.toString().padStart(2, "0")}h`;
  if (c.hours > 0) return `${c.hours}h ${c.minutes.toString().padStart(2, "0")}m`;
  return `${c.minutes}m`;
}

export interface CycleLabels {
  monthLabel: string;
  yearLabel: string;
}

/**
 * Month/year labels for a cycle, computed client-side from `responseOpenAt`
 * in the given IANA timezone (`docs/design/frontend.md` §14a — the
 * server never sends a display label). Uses `Intl.DateTimeFormat` directly
 * rather than `date-fns-tz` (not an installed dependency).
 */
export function cycleLabels(responseOpenAt: string, timezone: string): CycleLabels {
  const date = new Date(responseOpenAt);
  const monthLabel = new Intl.DateTimeFormat("en-US", { month: "long", timeZone: timezone }).format(
    date,
  );
  const yearLabel = new Intl.DateTimeFormat("en-US", {
    year: "numeric",
    timeZone: timezone,
  }).format(date);
  return { monthLabel, yearLabel };
}

/** Whether a cycle (by `responseOpenAt`) falls in the same calendar month as `now`, in the group's timezone. */
export function isCurrentCycleMonth(
  responseOpenAt: string,
  timezone: string,
  now: Date = new Date(),
): boolean {
  // en-CA formats as "2026-10", so the strings compare directly.
  const yearMonth = new Intl.DateTimeFormat("en-CA", {
    year: "numeric",
    month: "2-digit",
    timeZone: timezone,
  });
  return yearMonth.format(new Date(responseOpenAt)) === yearMonth.format(now);
}

const DAY_MS = 24 * 60 * 60 * 1000;
const CURRENT_EDITION_WEEKLY_WINDOW_DAYS = 7;

/**
 * Whether a published edition still counts as "current" on Home
 * (`docs/design/frontend.md` §7.1 / §14a). Monthly: same calendar month as
 * `now`, in the group's timezone. Weekly: published within the last
 * {@link CURRENT_EDITION_WEEKLY_WINDOW_DAYS} days of `now` — month names
 * don't distinguish weekly editions, so a rolling window stands in for "this
 * month's".
 */
export function isCurrentEdition(
  edition: { responseOpenAt: string; publishedAt: string | null },
  timezone: string,
  frequency: CycleCadence["frequency"],
  now: Date = new Date(),
): boolean {
  if (frequency === "weekly") {
    if (!edition.publishedAt) return false;
    const ageMs = now.getTime() - new Date(edition.publishedAt).getTime();
    return ageMs >= 0 && ageMs < CURRENT_EDITION_WEEKLY_WINDOW_DAYS * DAY_MS;
  }
  return isCurrentCycleMonth(edition.responseOpenAt, timezone, now);
}

/** "month" or "week", for copy that reads naturally for either cadence. */
export function cadenceNoun(frequency: CycleCadence["frequency"]): "month" | "week" {
  return frequency === "weekly" ? "week" : "month";
}

export type EditionLabel = {
  /** e.g. "October 2026" (monthly) or "Nov 15, 2026" (weekly). */
  title: string;
  /** Date tile's top line, e.g. "Oct" or "Nov" — CSS upper-cases it. */
  tileTop: string;
  /** Date tile's bottom line, e.g. "2026" (monthly) or "15" (weekly, day of month). */
  tileBottom: string;
  /** Short form for running copy, no year, e.g. "October" or "Nov 15". */
  short: string;
};

/**
 * Labels an edition for display (`docs/design/frontend.md` §14a). Monthly
 * labels are month/year, from `responseOpenAt` exactly as `cycleLabels`
 * always has. Weekly labels are the edition's publication date — `publishedAt`
 * once published, otherwise the scheduled `responseCloseAt` — in the group's
 * timezone.
 */
export function editionLabel(
  edition: { responseOpenAt?: string; responseCloseAt: string; publishedAt?: string | null },
  timezone: string,
  frequency: CycleCadence["frequency"],
): EditionLabel {
  if (frequency === "weekly") {
    const date = edition.publishedAt ?? edition.responseCloseAt;
    return {
      title: formatInZone(date, timezone, { month: "short", day: "numeric", year: "numeric" }),
      tileTop: formatInZone(date, timezone, { month: "short" }),
      tileBottom: formatInZone(date, timezone, { day: "numeric" }),
      short: formatInZone(date, timezone, { month: "short", day: "numeric" }),
    };
  }
  const { monthLabel, yearLabel } = cycleLabels(
    edition.responseOpenAt ?? edition.responseCloseAt,
    timezone,
  );
  return {
    title: `${monthLabel} ${yearLabel}`,
    tileTop: monthLabel.slice(0, 3),
    tileBottom: yearLabel,
    short: monthLabel,
  };
}

/**
 * Formats an instant in the group's IANA timezone (M9: every date shown on a
 * group page is in the group's zone, not the viewer's). `options` defaults to
 * e.g. "Oct 5, 9:00 PM EDT".
 */
export function formatInZone(
  iso: string,
  timezone: string,
  options: Intl.DateTimeFormatOptions = {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
    timeZoneName: "short",
  },
): string {
  return new Intl.DateTimeFormat("en-US", { ...options, timeZone: timezone }).format(new Date(iso));
}
