import { useEffect, useRef } from "react";
import { Outlet, useLocation } from "react-router-dom";
import { Spinner } from "../components/ui/Spinner";
import { useAuth } from "./useAuth";

export function RequireAuth(): JSX.Element {
  const { status, login } = useAuth();
  const location = useLocation();
  const hasStartedLogin = useRef(false);

  useEffect(() => {
    if (status !== "unauthenticated" || hasStartedLogin.current) return;
    hasStartedLogin.current = true;
    void login({ returnTo: location.pathname + location.search });
  }, [status, location, login]);

  if (status === "authenticated") return <Outlet />;

  return (
    <div className="flex min-h-[50vh] items-center justify-center">
      <Spinner size="lg" />
    </div>
  );
}
