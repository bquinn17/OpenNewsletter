import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  cycleLabels,
  countdownTo,
  formatDate,
  formatInZone,
  formatRelative,
  shortCountdown,
} from "./dates";

// A UTC instant chosen so the local calendar date — and even the month/year
// — differs by timezone (`docs/design/frontend.md` §14a: every date on a
// group page renders in the *group's* zone, never the viewer's).
const NEW_YEAR_EVE_UTC = "2026-01-01T02:00:00Z";

describe("cycleLabels", () => {
  it("labels the same instant with different month/year depending on the group's timezone", () => {
    expect(cycleLabels(NEW_YEAR_EVE_UTC, "America/New_York")).toEqual({
      monthLabel: "December",
      yearLabel: "2025",
    });
    expect(cycleLabels(NEW_YEAR_EVE_UTC, "Asia/Tokyo")).toEqual({
      monthLabel: "January",
      yearLabel: "2026",
    });
    expect(cycleLabels(NEW_YEAR_EVE_UTC, "UTC")).toEqual({
      monthLabel: "January",
      yearLabel: "2026",
    });
  });
});

describe("formatInZone", () => {
  it("renders the same instant as a different local date/time per timezone", () => {
    expect(formatInZone(NEW_YEAR_EVE_UTC, "America/New_York")).toBe("Dec 31, 9:00 PM EST");
    expect(formatInZone(NEW_YEAR_EVE_UTC, "Asia/Tokyo")).toBe("Jan 1, 11:00 AM GMT+9");
    expect(formatInZone(NEW_YEAR_EVE_UTC, "Pacific/Auckland")).toBe("Jan 1, 3:00 PM GMT+13");
  });

  it("accepts custom Intl.DateTimeFormatOptions, still applied in the given zone", () => {
    expect(
      formatInZone(NEW_YEAR_EVE_UTC, "America/New_York", { month: "short", day: "numeric" }),
    ).toBe("Dec 31");
  });
});

describe("formatDate", () => {
  it("formats with the default 'MMM d' pattern", () => {
    expect(formatDate("2026-06-15T12:00:00Z")).toBe("Jun 15");
  });

  it("accepts a custom date-fns format string", () => {
    expect(formatDate("2026-06-15T12:00:00Z", "yyyy-MM-dd")).toBe("2026-06-15");
  });
});

describe("countdownTo / shortCountdown (fixed clock)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-01T00:00:00Z"));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("reports days/hours/minutes remaining and expired: false for a future instant", () => {
    // +2d 3h 30m from "now".
    const countdown = countdownTo("2026-06-03T03:30:00Z");
    expect(countdown).toEqual({ days: 2, hours: 3, minutes: 30, expired: false });
    expect(shortCountdown("2026-06-03T03:30:00Z")).toBe("2d 03h");
  });

  it("drops to an hours/minutes form once under a day remains", () => {
    const countdown = countdownTo("2026-06-01T05:15:00Z");
    expect(countdown).toEqual({ days: 0, hours: 5, minutes: 15, expired: false });
    expect(shortCountdown("2026-06-01T05:15:00Z")).toBe("5h 15m");
  });

  it("drops to a minutes-only form once under an hour remains", () => {
    expect(shortCountdown("2026-06-01T00:45:00Z")).toBe("45m");
  });

  it("reports expired: true and 'closed' for a past instant", () => {
    expect(countdownTo("2026-05-31T00:00:00Z")).toEqual({
      days: 0,
      hours: 0,
      minutes: 0,
      expired: true,
    });
    expect(shortCountdown("2026-05-31T00:00:00Z")).toBe("closed");
  });

  it("treats the current instant as expired, not a zero-length countdown", () => {
    expect(countdownTo("2026-06-01T00:00:00Z").expired).toBe(true);
  });
});

describe("formatRelative (fixed clock)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-06-01T12:00:00Z"));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("describes a past instant with an 'ago' suffix", () => {
    expect(formatRelative("2026-06-01T10:00:00Z")).toBe("2 hours ago");
  });

  it("describes a future instant with an 'in' prefix", () => {
    expect(formatRelative("2026-06-04T12:00:00Z")).toBe("in 3 days");
  });
});
