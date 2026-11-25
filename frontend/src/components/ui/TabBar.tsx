import clsx from "clsx";

type Props<T extends string> = {
  /** Names the tab list for screen readers, e.g. "Admin sections". */
  label: string;
  /** Prefixes each tab's and panel's `id`, so two tab bars never collide. */
  idPrefix: string;
  tabs: { id: T; label: string }[];
  selected: T;
  onSelect: (id: T) => void;
};

/** A row of pill tabs. Pair it with a `role="tabpanel"` whose id is `{idPrefix}-tabpanel-{id}`. */
export function TabBar<T extends string>({ label, idPrefix, tabs, selected, onSelect }: Props<T>) {
  return (
    <div
      role="tablist"
      aria-label={label}
      className="no-scrollbar flex gap-1 overflow-x-auto px-4 pb-2 pt-3 text-sm"
    >
      {tabs.map((t) => (
        <button
          key={t.id}
          role="tab"
          id={`${idPrefix}-tab-${t.id}`}
          aria-selected={selected === t.id}
          aria-controls={`${idPrefix}-tabpanel-${t.id}`}
          onClick={() => onSelect(t.id)}
          className={clsx(
            "rounded-full px-3 py-1.5 font-semibold",
            selected === t.id ? "bg-ink text-cream" : "border border-line bg-white",
          )}
        >
          {t.label}
        </button>
      ))}
    </div>
  );
}
