/**
 * Mirrors the server's emoji predicate for reactions (`09-engagement.md`
 * §2.2): NFC-normalized, 1-12 codepoints, with at least one codepoint that is
 * `Extended_Pictographic` or a regional-indicator (flag) codepoint
 * (U+1F1E6-1F1FF). This rules out plain ASCII while allowing arbitrary emoji,
 * including ZWJ sequences (👨‍👩‍👧‍👦 = 7 codepoints) and flags.
 */

const EXTENDED_PICTOGRAPHIC_RE = /\p{Extended_Pictographic}/u;
const REGIONAL_INDICATOR_RE = /^[\u{1F1E6}-\u{1F1FF}]$/u;

export function normalizeEmoji(value: string): string {
  return value.normalize("NFC");
}

/** Splits by Unicode codepoint (not UTF-16 code unit) — surrogate pairs stay whole. */
function codepoints(value: string): string[] {
  return Array.from(value);
}

export function isValidEmoji(value: string): boolean {
  if (!value) return false;
  const points = codepoints(normalizeEmoji(value));
  if (points.length < 1 || points.length > 12) return false;
  return points.some((cp) => EXTENDED_PICTOGRAPHIC_RE.test(cp) || REGIONAL_INDICATOR_RE.test(cp));
}

/**
 * The first grapheme cluster of `value` — a full emoji as a person perceives
 * it, including ZWJ sequences and flag pairs, not just one UTF-16 code unit.
 * Used to keep the picker's native text input limited to a single emoji as
 * the user types (mobile keyboards insert one grapheme per tap).
 */
export function firstGrapheme(value: string): string {
  if (!value) return "";
  if (typeof Intl !== "undefined" && "Segmenter" in Intl) {
    const segmenter = new Intl.Segmenter(undefined, { granularity: "grapheme" });
    const first = segmenter.segment(value)[Symbol.iterator]().next();
    return first.done ? "" : first.value.segment;
  }
  return codepoints(value)[0] ?? "";
}
