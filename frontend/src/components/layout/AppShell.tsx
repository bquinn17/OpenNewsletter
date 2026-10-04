import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, Outlet, useNavigate } from "react-router-dom";
import { useAuth } from "../../auth/useAuth";
import { useConfig } from "../../api/queries";
import { useCurrentGroup } from "../../state/currentGroup";
import { Avatar } from "../ui/Avatar";
import { useDismissableMenu } from "../ui/useDismissableMenu";
import { ToastStack } from "../ui/Toast";
import { BottomNav } from "./BottomNav";
import { GroupSwitcher } from "./GroupSwitcher";

export function AppShell() {
  const { status, user, login, logout } = useAuth();
  const { data: config } = useConfig();
  const memberships = useMemo(() => config?.memberships ?? [], [config]);
  const { currentGroupId, setCurrentGroup } = useCurrentGroup();
  const navigate = useNavigate();
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  // Default the current group to the first membership whenever there is no
  // valid selection (fresh session, or the previously-selected group was left).
  useEffect(() => {
    if (memberships.length === 0) return;
    if (memberships.some((m) => m.groupId === currentGroupId)) return;
    setCurrentGroup(memberships[0]!.groupId);
  }, [memberships, currentGroupId, setCurrentGroup]);

  const closeMenu = useCallback(() => setMenuOpen(false), []);
  useDismissableMenu(menuRef, menuOpen, closeMenu);

  return (
    <div className="canvas-bg min-h-screen">
      <div className="app-container">
        <header className="sticky top-0 z-30 border-b border-line bg-cream/90 backdrop-blur">
          <div className="flex h-14 items-center justify-between gap-3 px-4">
            <div className="flex min-w-0 items-center gap-2">
              <Link
                to="/"
                className="grid h-8 w-8 shrink-0 place-items-center rounded-2xl bg-ink font-display font-bold text-cream"
                aria-label="Home"
              >
                O
              </Link>
              {status === "authenticated" && (
                <GroupSwitcher
                  memberships={memberships}
                  currentGroupId={currentGroupId}
                  onPick={(groupId) => {
                    setCurrentGroup(groupId);
                    navigate(`/g/${groupId}`);
                  }}
                />
              )}
            </div>

            <div ref={menuRef} className="relative shrink-0">
              {status === "authenticated" ? (
                <>
                  <button
                    onClick={() => setMenuOpen((v) => !v)}
                    aria-label="Account menu"
                    aria-haspopup="menu"
                    aria-expanded={menuOpen}
                  >
                    <Avatar name={user?.name ?? user?.email ?? "?"} size="sm" />
                  </button>
                  {menuOpen && (
                    <div
                      role="menu"
                      className="absolute right-0 z-40 mt-2 w-44 rounded-2xl border border-line bg-white p-1 shadow-pop"
                    >
                      <Link
                        role="menuitem"
                        to="/settings"
                        onClick={() => setMenuOpen(false)}
                        className="block rounded-xl px-3 py-2 text-sm font-semibold hover:bg-cream"
                      >
                        Settings
                      </Link>
                      <button
                        role="menuitem"
                        onClick={() => {
                          setMenuOpen(false);
                          void logout();
                        }}
                        className="w-full rounded-xl px-3 py-2 text-left text-sm font-semibold text-coral hover:bg-cream"
                      >
                        Sign out
                      </button>
                    </div>
                  )}
                </>
              ) : status === "unauthenticated" ? (
                <button
                  onClick={() => void login()}
                  className="rounded-full bg-ink px-3 py-1.5 text-sm font-semibold text-cream"
                >
                  Sign in
                </button>
              ) : null}
            </div>
          </div>
        </header>

        <Outlet />
      </div>
      <BottomNav />
      <ToastStack />
    </div>
  );
}
