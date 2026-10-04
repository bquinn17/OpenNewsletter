import type { UseQueryResult } from "@tanstack/react-query";
import { Spinner } from "../ui/Spinner";
import type { components } from "../../types/api";
import { GroupEditions } from "./GroupEditions";

type S = components["schemas"];

type Props = {
  membership: S["MembershipSummary"];
  query: UseQueryResult<S["NewsletterListResponse"]>;
};

export function GroupSection({ membership, query }: Props) {
  const { data, isLoading, isError } = query;

  return (
    <section className="mt-5 px-5">
      <h2 className="mb-2 font-display text-base font-semibold uppercase tracking-wide text-inkmuted">
        {membership.groupName}
      </h2>
      {isLoading && <Spinner size="sm" />}
      {isError && (
        <p className="text-sm text-coral">Couldn&apos;t load this group&apos;s editions.</p>
      )}
      {data && (
        <GroupEditions
          groupId={membership.groupId}
          timezone={membership.timezone}
          items={data.items}
        />
      )}
    </section>
  );
}
