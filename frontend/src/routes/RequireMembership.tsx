import { Navigate, Outlet, useParams } from "react-router-dom";
import { Spinner } from "../components/ui/Spinner";
import { useConfirmedAccess } from "./useConfirmedAccess";

/**
 * Layout guard for `/g/:groupId/*`, nested under `RequireAuth` so a signed-out
 * deep link goes to login (with `returnTo`) rather than bouncing home. A
 * non-member is sent to `/` (`04` §3).
 */
export function RequireMembership() {
  const { groupId = "" } = useParams();
  const access = useConfirmedAccess((config) =>
    config.memberships.some((m) => m.groupId === groupId),
  );

  if (access === "checking") {
    return (
      <div className="flex justify-center py-16">
        <Spinner size="lg" />
      </div>
    );
  }
  if (access === "denied") return <Navigate to="/" replace />;
  return <Outlet />;
}
