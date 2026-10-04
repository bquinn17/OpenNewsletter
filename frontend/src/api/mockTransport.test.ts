import { describe, expect, it } from "vitest";
import type { components } from "../types/api";
import { ApiError } from "./client";
import { mockFetch } from "./mockTransport";

type ConfigResponse = components["schemas"]["ConfigResponse"];
type RedeemResponse = components["schemas"]["RedeemResponse"];

describe("mockFetch", () => {
  it("rejects an unknown invite code as ApiError INVITE_INVALID", async () => {
    await expect(
      mockFetch("POST", "/invites/redeem", { code: "NOT-A-REAL-CODE" }),
    ).rejects.toMatchObject({
      status: 400,
      code: "INVITE_INVALID",
    });
    await expect(
      mockFetch("POST", "/invites/redeem", { code: "NOT-A-REAL-CODE" }),
    ).rejects.toBeInstanceOf(ApiError);
  });

  it("redeems the demo invite code, adds the group to /config, and is idempotent on repeat", async () => {
    const before = (await mockFetch("GET", "/config")) as ConfigResponse;
    expect(before.memberships.some((m) => m.groupId === "g_newgroup")).toBe(false);

    const redeemed = (await mockFetch("POST", "/invites/redeem", {
      code: "DEMO-JOIN-CODE",
    })) as RedeemResponse;
    expect(redeemed).toEqual({ groupId: "g_newgroup", groupName: "New Group", role: "member" });

    await mockFetch("POST", "/invites/redeem", { code: "DEMO-JOIN-CODE" });

    const after = (await mockFetch("GET", "/config")) as ConfigResponse;
    expect(after.memberships.filter((m) => m.groupId === "g_newgroup")).toHaveLength(1);
  });

  it("throws a 404 ApiError for an unknown route", async () => {
    await expect(mockFetch("GET", "/nonexistent")).rejects.toMatchObject({
      status: 404,
      code: "NOT_FOUND",
    });
  });
});
