import {
  forwardRef,
  useCallback,
  useImperativeHandle,
  useRef,
  useState,
  type KeyboardEvent,
} from "react";
import clsx from "clsx";
import { useDismissableMenu } from "../ui/useDismissableMenu";

export type AttachMenuItem = {
  key: string;
  icon: string;
  label: string;
  /** Shown under the label; also how a disabled item explains itself ("3/10 photos", `docs/design/frontend.md` §18.1). */
  subtitle?: string;
  disabled?: boolean;
  onSelect: () => void;
};

type Props = {
  items: AttachMenuItem[];
  /** `aria-label` and tooltip for the **+** button (`docs/design/frontend.md` §18.1). */
  ariaLabel: string;
  /** Shows the "+ Add" text label beside the icon on wide screens (response editor only). */
  showLabel?: boolean;
};

export type AttachMenuHandle = {
  /** The button's own wrapper, for a caller (e.g. `GiphyPicker`) to anchor a popover to it. */
  getAnchorEl: () => HTMLElement | null;
};

/**
 * The "+" attach menu (`docs/design/frontend.md` §10): a single button that opens a menu of every
 * attachment kind, built on `useDismissableMenu` (outside click / Escape) the same way
 * `GroupSwitcher` and `MembersTab` do. `items` is a seam — a caller lists only what it has built
 * (§10.2), so a later voice-clips item is one more entry, not a layout change.
 */
export const AttachMenu = forwardRef<AttachMenuHandle, Props>(function AttachMenu(
  { items, ariaLabel, showLabel = false },
  ref,
) {
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const close = useCallback(() => setOpen(false), []);
  useDismissableMenu(containerRef, open, close);
  useImperativeHandle(ref, () => ({ getAnchorEl: () => containerRef.current }));

  function handleMenuKeyDown(e: KeyboardEvent<HTMLDivElement>) {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp" && e.key !== "Home" && e.key !== "End") return;
    e.preventDefault();
    const menuItems = Array.from(
      containerRef.current?.querySelectorAll<HTMLButtonElement>(
        '[role="menuitem"]:not(:disabled)',
      ) ?? [],
    );
    if (menuItems.length === 0) return;
    const current = menuItems.indexOf(document.activeElement as HTMLButtonElement);
    let next: number;
    if (e.key === "Home") next = 0;
    else if (e.key === "End") next = menuItems.length - 1;
    else if (e.key === "ArrowDown") next = (current + 1) % menuItems.length;
    else next = (current - 1 + menuItems.length) % menuItems.length;
    menuItems[next]?.focus();
  }

  function pick(item: AttachMenuItem) {
    setOpen(false);
    item.onSelect();
  }

  if (items.length === 0) return null;

  return (
    <div ref={containerRef} className="relative inline-block">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={ariaLabel}
        title={ariaLabel}
        className={clsx(
          "flex h-10 min-w-[2.5rem] items-center justify-center gap-1.5 rounded-full bg-grape px-2.5 text-lg font-bold text-white",
        )}
      >
        <span aria-hidden="true">{open ? "×" : "+"}</span>
        {showLabel && !open && <span className="hidden text-sm font-semibold sm:inline">Add</span>}
      </button>

      {open && (
        <div
          role="menu"
          tabIndex={-1}
          aria-label={ariaLabel}
          onKeyDown={handleMenuKeyDown}
          className="absolute bottom-full left-0 z-40 mb-2 w-64 rounded-2xl border border-line bg-white p-1 shadow-pop"
        >
          {items.map((item) => (
            <button
              key={item.key}
              type="button"
              role="menuitem"
              disabled={item.disabled}
              aria-disabled={item.disabled || undefined}
              onClick={() => pick(item)}
              className={clsx(
                "flex w-full items-center gap-3 rounded-xl px-3 py-2 text-left",
                item.disabled ? "cursor-not-allowed opacity-40" : "hover:bg-cream",
              )}
            >
              <span aria-hidden="true" className="text-xl leading-none">
                {item.icon}
              </span>
              <span className="min-w-0 flex-1">
                <span className="block text-sm font-semibold text-ink">{item.label}</span>
                {item.subtitle && (
                  <span className="block text-xs text-inkmuted">{item.subtitle}</span>
                )}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
});
