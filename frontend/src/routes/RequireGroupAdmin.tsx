import { Navigate, Outlet, useParams } from "react-router-dom";
import { Spinner } from "../components/ui/Spinner";
import { useConfirmedAccess } from "./useConfirmedAccess";

/** Nested under `RequireMembership` for `/g/:groupId/admin`; a non-admin goes to the group home (`04` §3). */
export function RequireGroupAdmin() {
  const { groupId = "" } = useParams();
  const access = useConfirmedAccess((config) =>
    config.memberships.some((m) => m.groupId === groupId && m.role === "admin"),
  );

  if (access === "checking") {
    return (
      <div className="flex justify-center py-16">
        <Spinner size="lg" />
      </div>
    );
  }
  if (access === "denied") return <Navigate to={`/g/${groupId}`} replace />;
  return <Outlet />;
}
