import { NavLink, useNavigate } from "react-router-dom";
import clsx from "clsx";
import { useCurrentGroup } from "../../state/currentGroup";

export function BottomNav() {
  const groupId = useCurrentGroup((s) => s.currentGroupId) ?? "g_trail";
  const navigate = useNavigate();

  const tabs = [
    { to: "/", icon: "🏠", label: "Home", end: true },
    { to: `/g/${groupId}/upcoming`, icon: "💡", label: "Ideas" },
    { to: `/g/${groupId}/n/202605/respond`, icon: "✍️", label: "Write", isWrite: true },
    { to: "/settings", icon: "⚙️", label: "You" },
  ];

  return (
    <div className="absolute bottom-3 left-3 right-3 z-40">
      <nav className="bg-ink text-cream rounded-full shadow-pop p-1.5 flex items-center justify-around">
        {tabs.map((tab) => (
          <NavLink
            key={tab.to}
            to={tab.to}
            end={tab.end}
            onClick={(e) => {
              if (tab.isWrite) {
                e.preventDefault();
                navigate(`/g/${groupId}/n/202605`);
              }
            }}
            className={({ isActive }) =>
              clsx(
                "px-4 py-2 rounded-full text-sm flex items-center gap-1.5 transition",
                isActive ? "bg-cream text-ink font-semibold" : "opacity-80 hover:opacity-100",
              )
            }
          >
            <span aria-hidden>{tab.icon}</span>
            <span className="hidden sm:inline">{tab.label}</span>
          </NavLink>
        ))}
      </nav>
    </div>
  );
}
