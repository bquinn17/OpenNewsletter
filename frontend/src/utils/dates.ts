import { format, formatDistanceToNowStrict } from "date-fns";

export function formatRelative(iso: string): string {
  return formatDistanceToNowStrict(new Date(iso), { addSuffix: true });
}

export function formatDate(iso: string, fmt = "MMM d"): string {
  return format(new Date(iso), fmt);
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
 * in the given IANA timezone (`04-frontend-architecture.md` §14a — the
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
