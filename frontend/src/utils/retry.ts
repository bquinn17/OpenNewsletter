const BASE_DELAY_MS = 1000;
const MAX_DELAY_MS = 30_000;

/**
 * Exponential backoff delay for the `attempt`th retry (0-indexed): 1s, 2s,
 * 4s, 8s, 16s, capped at 30s (`04-frontend-architecture.md` §8.3).
 */
export function backoffDelayMs(attempt: number): number {
  return Math.min(BASE_DELAY_MS * 2 ** attempt, MAX_DELAY_MS);
}
