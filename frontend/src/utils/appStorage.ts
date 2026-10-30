import { isDemo } from "../demo";

export type KeyValueStorage = Pick<Storage, "getItem" | "setItem" | "removeItem">;

function createMemoryStorage(): KeyValueStorage {
  const items = new Map<string, string>();
  return {
    getItem: (key) => items.get(key) ?? null,
    setItem: (key, value) => void items.set(key, String(value)),
    removeItem: (key) => void items.delete(key),
  };
}

const demoStorage = createMemoryStorage();

/**
 * Where client state that outlives a reload is kept: `localStorage`, except in the demo,
 * which keeps it in memory. However the demo page is left, its state goes with it, and a
 * real session's stored state is never read or overwritten (`docs/design/frontend.md` §17).
 * May throw outside the demo, as `localStorage` access can; callers already guard for that.
 */
export function appStorage(): KeyValueStorage {
  return isDemo ? demoStorage : window.localStorage;
}
