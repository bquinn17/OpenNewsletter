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

  describe("media routes", () => {
    it("creates a pending upload row and flips to ready after a couple of polls", async () => {
      const created = (await mockFetch("POST", "/uploads", {
        groupId: "g1",
        cycleId: "202606",
        questionId: "q1",
        mimeType: "image/jpeg",
        byteSize: 1000,
      })) as components["schemas"]["CreateUploadResponse"];
      expect(created.uploadUrl).toContain("mock://upload/");

      const firstPoll = (await mockFetch(
        "GET",
        `/uploads/${created.imageId}?groupId=g1&cycleId=202606`,
      )) as components["schemas"]["ImageMediaResponse"];
      expect(firstPoll.status).toBe("pending");

      const secondPoll = (await mockFetch(
        "GET",
        `/uploads/${created.imageId}?groupId=g1&cycleId=202606`,
      )) as components["schemas"]["ImageMediaResponse"];
      expect(secondPoll.status).toBe("ready");
      expect(secondPoll.thumbUrl).toBeTruthy();
      expect(secondPoll.displayUrl).toBeTruthy();
    });

    it("sets a caption via PATCH and soft-deletes via DELETE", async () => {
      const created = (await mockFetch("POST", "/uploads", {
        groupId: "g1",
        cycleId: "202606",
        questionId: "q1",
        mimeType: "image/jpeg",
        byteSize: 1000,
      })) as components["schemas"]["CreateUploadResponse"];

      const patched = (await mockFetch(
        "PATCH",
        `/uploads/${created.imageId}?groupId=g1&cycleId=202606`,
        { caption: "Mile two." },
      )) as components["schemas"]["ImageMediaResponse"];
      expect(patched.caption).toBe("Mile two.");

      await mockFetch("DELETE", `/uploads/${created.imageId}?groupId=g1&cycleId=202606`);
      const afterDelete = (await mockFetch(
        "GET",
        `/uploads/${created.imageId}?groupId=g1&cycleId=202606`,
      )) as components["schemas"]["ImageMediaResponse"];
      expect(afterDelete.status).toBe("failed");
      expect(afterDelete.errorMessage).toBe("DELETED");
    });

    it("returns dummy signed-cookie values for the caller's group", async () => {
      const cookie = (await mockFetch(
        "GET",
        "/media-cookie?groupId=g1",
      )) as components["schemas"]["MediaCookieResponse"];
      expect(cookie.policy).toBeTruthy();
      expect(cookie.signature).toBeTruthy();
      expect(cookie.keyPairId).toBeTruthy();
    });

    it("creates a pending avatar and flips to ready after a couple of polls, then deletes it", async () => {
      const created = (await mockFetch("POST", "/avatars", {
        mimeType: "image/png",
        byteSize: 500,
      })) as components["schemas"]["CreateAvatarResponse"];
      expect(created.uploadUrl).toContain("mock://upload/");

      await mockFetch("GET", `/avatars/${created.avatarId}`);
      const ready = (await mockFetch(
        "GET",
        `/avatars/${created.avatarId}`,
      )) as components["schemas"]["AvatarMediaResponse"];
      expect(ready.status).toBe("ready");
      expect(ready.avatarUrl).toBeTruthy();

      await mockFetch("DELETE", `/avatars/${created.avatarId}`);
      const afterDelete = (await mockFetch(
        "GET",
        `/avatars/${created.avatarId}`,
      )) as components["schemas"]["AvatarMediaResponse"];
      expect(afterDelete.status).toBe("failed");
      expect(afterDelete.errorMessage).toBe("DELETED");
    });
  });
});
