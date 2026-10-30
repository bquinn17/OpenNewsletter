import { describe, expect, it } from "vitest";
import type { components } from "../types/api";
import { ApiError } from "./client";
import { mockFetch } from "./mockTransport";

type ConfigResponse = components["schemas"]["ConfigResponse"];
type GroupResponse = components["schemas"]["GroupResponse"];
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

  it("403s when a non-admin removes another member, before looking up the target", async () => {
    // `g_meeple`: the caller is a plain member and Tara is the admin.
    await expect(mockFetch("DELETE", "/groups/g_meeple/members/u_m_tara")).rejects.toMatchObject({
      status: 403,
      code: "FORBIDDEN",
    });
    await expect(mockFetch("DELETE", "/groups/g_meeple/members/u_nobody")).rejects.toMatchObject({
      status: 403,
      code: "FORBIDDEN",
    });
    const group = (await mockFetch("GET", "/groups/g_meeple")) as GroupResponse;
    expect(group.members.some((m) => m.userId === "u_m_tara")).toBe(true);
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

    it("sets a caption via PATCH and really deletes via DELETE (D8)", async () => {
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
      await expect(
        mockFetch("GET", `/uploads/${created.imageId}?groupId=g1&cycleId=202606`),
      ).rejects.toMatchObject({ status: 404 });
      // A repeat DELETE is idempotent.
      await expect(
        mockFetch("DELETE", `/uploads/${created.imageId}?groupId=g1&cycleId=202606`),
      ).resolves.toBeUndefined();
    });

    it("creates a pending avatar and flips to ready after a couple of polls, then really deletes it", async () => {
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
      await expect(mockFetch("GET", `/avatars/${created.avatarId}`)).rejects.toMatchObject({
        status: 404,
      });
    });

    it("deleting the current avatar clears it from /me, and replacing or clearing one deletes the old (D8)", async () => {
      async function readyAvatar(): Promise<string> {
        const { avatarId } = (await mockFetch("POST", "/avatars", {
          mimeType: "image/png",
          byteSize: 500,
        })) as components["schemas"]["CreateAvatarResponse"];
        await mockFetch("GET", `/avatars/${avatarId}`);
        await mockFetch("GET", `/avatars/${avatarId}`);
        return avatarId;
      }

      const first = await readyAvatar();
      const set = (await mockFetch("PATCH", "/me", {
        avatarMediaId: first,
      })) as components["schemas"]["UserResponse"];
      expect(set.avatarUrl).toBeTruthy();

      const second = await readyAvatar();
      await mockFetch("PATCH", "/me", { avatarMediaId: second });
      await expect(mockFetch("GET", `/avatars/${first}`)).rejects.toMatchObject({ status: 404 });

      await mockFetch("DELETE", `/avatars/${second}`);
      const me = (await mockFetch("GET", "/me")) as components["schemas"]["UserResponse"];
      expect(me.avatarMediaId).toBeNull();
      expect(me.avatarUrl).toBeNull();

      // Clearing the photo deletes it too (`docs/design/api.md` §2.3).
      const third = await readyAvatar();
      await mockFetch("PATCH", "/me", { avatarMediaId: third });
      const cleared = (await mockFetch("PATCH", "/me", {
        avatarMediaId: null,
      })) as components["schemas"]["UserResponse"];
      expect(cleared.avatarMediaId).toBeNull();
      await expect(mockFetch("GET", `/avatars/${third}`)).rejects.toMatchObject({ status: 404 });
    });
  });
});
