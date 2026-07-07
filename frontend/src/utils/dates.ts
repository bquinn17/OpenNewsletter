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
