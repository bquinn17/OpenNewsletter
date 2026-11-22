import { stripImageTokens } from "./markdown";

/** Words per minute used to estimate reading time (`docs/design/frontend.md` §14a). */
const WORDS_PER_MINUTE = 200;

/**
 * Minutes to read every published text answer's body in an edition, at
 * {@link WORDS_PER_MINUTE} words a minute, rounded up and never below 1
 * (`docs/design/frontend.md` §14a). Image tokens are stripped before counting.
 */
export function readingTimeMinutes(bodies: string[]): number {
  const wordCount = bodies.reduce((sum, body) => {
    const stripped = stripImageTokens(body);
    if (!stripped) return sum;
    return sum + stripped.split(/\s+/).filter(Boolean).length;
  }, 0);
  return Math.max(1, Math.ceil(wordCount / WORDS_PER_MINUTE));
}
