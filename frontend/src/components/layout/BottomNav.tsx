import { NavLink } from "react-router-dom";
import clsx from "clsx";
import { useCurrentGroup } from "../../state/currentGroup";

type Tab = {
  label: string;
  icon: string;
  to: string;
  end?: boolean;
  disabled?: boolean;
};

export function BottomNav() {
  const groupId = useCurrentGroup((s) => s.currentGroupId);

  const tabs: Tab[] = [
    { label: "Home", icon: "🏠", to: "/", end: true },
    {
      label: "Upcoming",
      icon: "💡",
      to: groupId ? `/g/${groupId}/upcoming` : "/",
      disabled: !groupId,
    },
    // Lands on the redirect route, which forwards to the open/current cycle.
    { label: "Respond", icon: "✍️", to: groupId ? `/g/${groupId}` : "/", disabled: !groupId },
    { label: "You", icon: "⚙️", to: "/settings" },
  ];

  return (
    <div className="absolute bottom-3 left-3 right-3 z-40 md:hidden">
      <nav className="flex items-center justify-around rounded-full bg-ink p-1.5 text-cream shadow-pop">
        {tabs.map((tab) =>
          tab.disabled ? (
            <span
              key={tab.label}
              aria-disabled="true"
              className="flex cursor-not-allowed items-center gap-1.5 rounded-full px-4 py-2 text-sm opacity-40"
            >
              <span aria-hidden>{tab.icon}</span>
              <span className="hidden sm:inline">{tab.label}</span>
            </span>
          ) : (
            <NavLink
              key={tab.label}
              to={tab.to}
              end={tab.end}
              className={({ isActive }) =>
                clsx(
                  "flex items-center gap-1.5 rounded-full px-4 py-2 text-sm transition",
                  isActive ? "bg-cream font-semibold text-ink" : "opacity-80 hover:opacity-100",
                )
              }
            >
              <span aria-hidden>{tab.icon}</span>
              <span className="hidden sm:inline">{tab.label}</span>
            </NavLink>
          ),
        )}
      </nav>
    </div>
  );
}
