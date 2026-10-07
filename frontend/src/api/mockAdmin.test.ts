import { beforeEach, describe, expect, it } from "vitest";
import type { components } from "../types/api";
import { mockAdminRoute } from "./mockAdmin";
import { CALLER_ID, groups, NO_MOCK_ROUTE, type MockRequest } from "./mockShared";

type S = components["schemas"];

function req(method: MockRequest["method"], rawPath: string, body?: unknown): MockRequest {
  const [path, queryString] = rawPath.split("?");
  return {
    method,
    rawPath: path!,
    segments: path!.split("/").filter(Boolean),
    query: new URLSearchParams(queryString ?? ""),
    body,
  };
}

// `g_trail` is the only fixture group where `CALLER_ID` is an admin
// (mirrors the memberships fixture in `mockTransport.ts`), so it's the only
// group the admin routes accept the caller into.
const ADMIN_GROUP_ID = "g_trail";
const NON_ADMIN_GROUP_ID = "g_game";

// Deep-cloned once, restored before every test — every test below mutates
// `groups` (settings, roles, membership), and the fixture is a shared
// module-level singleton within this file's test run.
const originalGroups = structuredClone(groups);

beforeEach(() => {
  for (const [groupId, group] of Object.entries(originalGroups)) {
    groups[groupId] = structuredClone(group);
  }
});

describe("mockAdminRoute", () => {
  it("returns NO_MOCK_ROUTE for an unrelated path", () => {
    expect(mockAdminRoute(req("GET", "/groups/g_trail/candidate-questions"))).toBe(NO_MOCK_ROUTE);
  });

  describe("POST /admin/invites", () => {
    it("422s when groupId is missing", () => {
      expect(() =>
        mockAdminRoute(req("POST", "/admin/invites", { roleOnRedeem: "member" })),
      ).toThrow(expect.objectContaining({ status: 422, code: "VALIDATION_FAILED" }));
    });

    it("403s when the caller isn't an admin of the group", () => {
      expect(() =>
        mockAdminRoute(
          req("POST", "/admin/invites", { groupId: NON_ADMIN_GROUP_ID, roleOnRedeem: "member" }),
        ),
      ).toThrow(expect.objectContaining({ status: 403, code: "FORBIDDEN" }));
    });

    it("creates a pending invite with a code in XXXX-XXXX-XXXX-XXXX form", () => {
      const result = mockAdminRoute(
        req("POST", "/admin/invites", {
          groupId: ADMIN_GROUP_ID,
          ttlDays: 3,
          roleOnRedeem: "admin",
        }),
      ) as S["CreateInviteResponse"];

      expect(result.code).toMatch(/^[A-Z0-9]{4}-[A-Z0-9]{4}-[A-Z0-9]{4}-[A-Z0-9]{4}$/);
      expect(result.groupId).toBe(ADMIN_GROUP_ID);
      expect(result.roleOnRedeem).toBe("admin");
      const expiresInDays =
        (new Date(result.expiresAt).getTime() - Date.now()) / (24 * 60 * 60 * 1000);
      expect(expiresInDays).toBeGreaterThan(2.9);
      expect(expiresInDays).toBeLessThan(3.1);
    });
  });

  describe("GET /admin/groups/{groupId}/invites", () => {
    it("403s when the caller isn't an admin of the group", () => {
      expect(() =>
        mockAdminRoute(req("GET", `/admin/groups/${NON_ADMIN_GROUP_ID}/invites`)),
      ).toThrow(expect.objectContaining({ status: 403, code: "FORBIDDEN" }));
    });

    it("lists an invite just created for the group", () => {
      const created = mockAdminRoute(
        req("POST", "/admin/invites", { groupId: ADMIN_GROUP_ID, roleOnRedeem: "member" }),
      ) as S["CreateInviteResponse"];

      const list = mockAdminRoute(
        req("GET", `/admin/groups/${ADMIN_GROUP_ID}/invites`),
      ) as S["InviteListResponse"];

      const match = list.items.find((i) => i.code === created.code);
      expect(match).toMatchObject({
        groupId: ADMIN_GROUP_ID,
        status: "pending",
        roleOnRedeem: "member",
        createdBy: CALLER_ID,
        consumedBy: null,
        consumedAt: null,
      });
    });
  });

  describe("POST /admin/invites/{code}/revoke", () => {
    it("404s for an unknown code", () => {
      expect(() =>
        mockAdminRoute(req("POST", "/admin/invites/NOPE-NOPE-NOPE-NOPE/revoke")),
      ).toThrow(expect.objectContaining({ status: 404, code: "NOT_FOUND" }));
    });

    it("revokes a pending invite, idempotently", () => {
      const created = mockAdminRoute(
        req("POST", "/admin/invites", { groupId: ADMIN_GROUP_ID, roleOnRedeem: "member" }),
      ) as S["CreateInviteResponse"];

      expect(mockAdminRoute(req("POST", `/admin/invites/${created.code}/revoke`))).toBeUndefined();

      const list = mockAdminRoute(
        req("GET", `/admin/groups/${ADMIN_GROUP_ID}/invites`),
      ) as S["InviteListResponse"];
      expect(list.items.find((i) => i.code === created.code)?.status).toBe("revoked");

      // Revoking an already-revoked invite is still a no-op success.
      expect(mockAdminRoute(req("POST", `/admin/invites/${created.code}/revoke`))).toBeUndefined();
    });
  });

  describe("PATCH /groups/{groupId}/members/{userId}", () => {
    it("403s when the caller isn't an admin of the group", () => {
      expect(() =>
        mockAdminRoute(
          req("PATCH", `/groups/${NON_ADMIN_GROUP_ID}/members/${CALLER_ID}`, { role: "admin" }),
        ),
      ).toThrow(expect.objectContaining({ status: 403, code: "FORBIDDEN" }));
    });

    it("422s on an invalid role", () => {
      expect(() =>
        mockAdminRoute(req("PATCH", `/groups/${ADMIN_GROUP_ID}/members/u_sam`, { role: "owner" })),
      ).toThrow(expect.objectContaining({ status: 422, code: "VALIDATION_FAILED" }));
    });

    it("404s for a member not in the group", () => {
      expect(() =>
        mockAdminRoute(
          req("PATCH", `/groups/${ADMIN_GROUP_ID}/members/u_nobody`, { role: "admin" }),
        ),
      ).toThrow(expect.objectContaining({ status: 404, code: "NOT_FOUND" }));
    });

    it("promotes a member to admin, then demotes them — the caller remains an admin, so no LAST_ADMIN", () => {
      const promoted = mockAdminRoute(
        req("PATCH", `/groups/${ADMIN_GROUP_ID}/members/u_sam`, { role: "admin" }),
      ) as S["MemberResponse"];
      expect(promoted.role).toBe("admin");

      const demoted = mockAdminRoute(
        req("PATCH", `/groups/${ADMIN_GROUP_ID}/members/u_sam`, { role: "member" }),
      ) as S["MemberResponse"];
      expect(demoted.role).toBe("member");
      expect(groups[ADMIN_GROUP_ID]!.members.find((m) => m.userId === "u_sam")?.role).toBe(
        "member",
      );
    });

    it("409s LAST_ADMIN when the caller demotes themselves as the sole admin", () => {
      expect(() =>
        mockAdminRoute(
          req("PATCH", `/groups/${ADMIN_GROUP_ID}/members/${CALLER_ID}`, { role: "member" }),
        ),
      ).toThrow(expect.objectContaining({ status: 409, code: "LAST_ADMIN" }));
    });
  });

  describe("PATCH /groups/{groupId}", () => {
    it("403s when the caller isn't an admin of the group", () => {
      expect(() =>
        mockAdminRoute(req("PATCH", `/groups/${NON_ADMIN_GROUP_ID}`, { name: "x" })),
      ).toThrow(expect.objectContaining({ status: 403, code: "FORBIDDEN" }));
    });

    it("422s on a blank name", () => {
      expect(() =>
        mockAdminRoute(req("PATCH", `/groups/${ADMIN_GROUP_ID}`, { name: "   " })),
      ).toThrow(expect.objectContaining({ status: 422, code: "VALIDATION_FAILED" }));
    });

    it("writes nothing when a later field fails validation", () => {
      const before = structuredClone(groups[ADMIN_GROUP_ID]);
      expect(() =>
        mockAdminRoute(
          req("PATCH", `/groups/${ADMIN_GROUP_ID}`, {
            name: "Renamed",
            gradient: "ocean-dusk",
            cycleSettings: { questionsPerCycle: 6 },
            notificationSettings: { offsetsHoursBeforeClose: [24, 48] },
          }),
        ),
      ).toThrow(expect.objectContaining({ status: 422, code: "VALIDATION_FAILED" }));
      expect(groups[ADMIN_GROUP_ID]).toEqual(before);
    });

    it("422s when votesPerUserPerCycle exceeds questionsPerCycle", () => {
      expect(() =>
        mockAdminRoute(
          req("PATCH", `/groups/${ADMIN_GROUP_ID}`, {
            cycleSettings: { questionsPerCycle: 2, votesPerUserPerCycle: 3 },
          }),
        ),
      ).toThrow(expect.objectContaining({ status: 422, code: "VALIDATION_FAILED" }));
    });

    it("422s when reminder offsets aren't strictly descending", () => {
      expect(() =>
        mockAdminRoute(
          req("PATCH", `/groups/${ADMIN_GROUP_ID}`, {
            notificationSettings: { offsetsHoursBeforeClose: [24, 48] },
          }),
        ),
      ).toThrow(expect.objectContaining({ status: 422, code: "VALIDATION_FAILED" }));
    });

    it("422s when memberSoftCap drops below the current member count", () => {
      expect(() =>
        mockAdminRoute(req("PATCH", `/groups/${ADMIN_GROUP_ID}`, { memberSoftCap: 0 })),
      ).toThrow(expect.objectContaining({ status: 422, code: "VALIDATION_FAILED" }));
    });

    it("applies a name/gradient/timezone patch", () => {
      const result = mockAdminRoute(
        req("PATCH", `/groups/${ADMIN_GROUP_ID}`, {
          name: "Trailblazers",
          gradient: "ember-rose",
          timezone: "America/Los_Angeles",
        }),
      ) as S["GroupResponse"];

      expect(result.name).toBe("Trailblazers");
      expect(result.gradient).toBe("ember-rose");
      expect(result.timezone).toBe("America/Los_Angeles");
      expect(groups[ADMIN_GROUP_ID]!.name).toBe("Trailblazers");
    });

    it("applies a whole-subobject cycleSettings patch", () => {
      const result = mockAdminRoute(
        req("PATCH", `/groups/${ADMIN_GROUP_ID}`, {
          cycleSettings: { questionsPerCycle: 6, votesPerUserPerCycle: 4, responseWindowDays: 5 },
        }),
      ) as S["GroupResponse"];

      expect(result.cycleSettings).toEqual({
        questionsPerCycle: 6,
        votesPerUserPerCycle: 4,
        responseWindowDays: 5,
      });
    });
  });
});
