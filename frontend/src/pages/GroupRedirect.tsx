import { Navigate, useParams } from "react-router-dom";
import { useNewsletters } from "../api/queries";
import { Spinner } from "../components/ui/Spinner";

export function GroupRedirect() {
  const { groupId = "" } = useParams();
  const { data, isLoading } = useNewsletters(groupId);

  if (isLoading) {
    return (
      <div className="flex justify-center py-16">
        <Spinner size="lg" />
      </div>
    );
  }

  // `items` is newest-first; skip past a leading `voting` cycle to the most
  // recent non-voting one (open or published).
  const target = data?.items.find((n) => n.status !== "voting");
  if (target) return <Navigate to={`/g/${groupId}/n/${target.cycleId}`} replace />;
  return <Navigate to={`/g/${groupId}/upcoming`} replace />;
}
