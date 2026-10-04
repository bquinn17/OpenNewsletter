import { useParams } from "react-router-dom";
import { useConfig } from "../api/queries";
import { PageHeader } from "../components/layout/PageHeader";

type Props = {
  message: string;
};

/** Placeholder for `/g/:groupId/*` routes not yet wired to the live API outside mock mode. */
export function PendingMilestonePage({ message }: Props) {
  const { groupId = "" } = useParams();
  const { data: config } = useConfig();
  const groupName =
    config?.memberships.find((m) => m.groupId === groupId)?.groupName ?? "Your group";

  return (
    <div className="bg-cream pb-12">
      <PageHeader eyebrow={groupName} title="Coming soon" back={`/g/${groupId}`} />
      <div className="px-5 pt-8 text-center">
        <div className="mb-3 text-5xl">🚧</div>
        <p className="text-inkmuted">{message}</p>
      </div>
    </div>
  );
}
