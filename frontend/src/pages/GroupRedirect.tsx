import { Navigate, useParams } from "react-router-dom";
import { useNewsletters } from "../api/queries";

export function GroupRedirect() {
  const { groupId = "" } = useParams();
  const { data, isLoading } = useNewsletters(groupId);
  if (isLoading) return null;
  const open = data?.find((n) => n.status === "open");
  const latest = data?.find((n) => n.status === "published");
  const target = open ?? latest;
  if (target) return <Navigate to={`/g/${groupId}/n/${target.cycleId}`} replace />;
  return <Navigate to={`/g/${groupId}/upcoming`} replace />;
}
