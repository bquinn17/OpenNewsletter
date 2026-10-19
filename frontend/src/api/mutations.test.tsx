import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";
import { api } from "./client";
import { useUpdateProfile } from "./mutations";
import { queryKeys } from "./queries";
import { MEDIA_URLS_META } from "../utils/media";
import type { components } from "../types/api";

vi.mock("./client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./client")>();
  return { ...actual, api: { patchMe: vi.fn() } };
});

const patchMe = api.patchMe as ReturnType<typeof vi.fn>;

const me: components["schemas"]["UserResponse"] = {
  userId: "u1",
  email: "u1@example.com",
  displayName: "Sam",
  avatarColor: "violet",
  avatarMediaId: "av-new",
  avatarUrl: "https://media.test.invalid/avatar/av-new/display.webp?sig=a",
  createdAt: "2026-06-01T00:00:00Z",
};

let client: QueryClient;

function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

/** Seeds a group query flagged as carrying media URLs, as `useGroup` defines it. */
async function seedGroupQuery() {
  await client.prefetchQuery({
    queryKey: queryKeys.group("g1"),
    queryFn: () => ({ groupId: "g1" }),
    meta: MEDIA_URLS_META,
  });
}

describe("useUpdateProfile", () => {
  beforeEach(() => {
    client = new QueryClient();
    patchMe.mockReset().mockResolvedValue(me);
  });

  it("invalidates the queries carrying media URLs when the avatar changes", async () => {
    await seedGroupQuery();
    const { result } = renderHook(() => useUpdateProfile(), { wrapper });

    await act(() => result.current.mutateAsync({ avatarMediaId: "av-new" }));

    expect(client.getQueryState(queryKeys.group("g1"))?.isInvalidated).toBe(true);
  });

  it("leaves them alone when only the display name changes", async () => {
    await seedGroupQuery();
    const { result } = renderHook(() => useUpdateProfile(), { wrapper });

    await act(() => result.current.mutateAsync({ displayName: "Sam B." }));

    expect(client.getQueryState(queryKeys.group("g1"))?.isInvalidated).toBe(false);
  });
});
