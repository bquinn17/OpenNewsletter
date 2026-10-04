import { useEffect, type RefObject } from "react";

/**
 * Close behaviour for a dropdown menu rendered inside `containerRef`, whose
 * first `<button>` is the trigger. Closes on an outside click or Escape.
 * Opening moves focus to the first `[role="menuitem"]`; Escape returns focus
 * to the trigger, so keyboard users never get stranded (`04` §11).
 */
export function useDismissableMenu(
  containerRef: RefObject<HTMLElement>,
  open: boolean,
  close: () => void,
): void {
  useEffect(() => {
    if (!open) return;
    const container = containerRef.current;
    container?.querySelector<HTMLElement>('[role="menuitem"]')?.focus();

    function onMouseDown(e: MouseEvent) {
      if (container && e.target instanceof Node && !container.contains(e.target)) close();
    }
    function onKeyDown(e: KeyboardEvent) {
      if (e.key !== "Escape") return;
      close();
      container?.querySelector<HTMLElement>("button")?.focus();
    }
    document.addEventListener("mousedown", onMouseDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("mousedown", onMouseDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [containerRef, open, close]);
}
