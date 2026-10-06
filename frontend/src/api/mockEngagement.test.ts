import { describe, expect, it } from "vitest";
import type { components } from "../types/api";
import { mockEngagementRoute } from "./mockEngagement";
import { findPublishedAnswer } from "./mockNewsletters";
import { NO_MOCK_ROUTE, OPEN_CYCLE_ID, PUBLISHED_CYCLE_ID, type MockRequest } from "./mockShared";

// This module mutates the same module-level fixture objects `mockNewsletters.ts`
// serves (there's no separate engagement data store), so — like
// `mockCandidates.test.ts` — tests are written to be either symmetric
// (restoring what they changed) or additive in a way that doesn't affect
// other tests' assertions.

type S = components["schemas"];

const BASE = `/groups/g_trail/newsletters/${PUBLISHED_CYCLE_ID}/questions/q_trail_hike/responses/pub_r1`;

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

describe("mockEngagementRoute", () => {
  it("returns NO_MOCK_ROUTE for an unrelated path", () => {
    expect(mockEngagementRoute(req("GET", "/groups/g_trail/candidate-questions"))).toBe(
      NO_MOCK_ROUTE,
    );
  });

  it("404s for an unknown group and a non-published cycle", () => {
    expect(() =>
      mockEngagementRoute(req("GET", `${BASE.replace("g_trail", "g_nope")}/comments`)),
    ).toThrow(expect.objectContaining({ status: 403 }));

    expect(() =>
      mockEngagementRoute(
        req(
          "GET",
          `/groups/g_trail/newsletters/${OPEN_CYCLE_ID}/questions/q_trail_hike/responses/pub_r1/comments`,
        ),
      ),
    ).toThrow(expect.objectContaining({ status: 409, code: "CYCLE_NOT_PUBLISHED" }));

    expect(() =>
      mockEngagementRoute(
        req(
          "GET",
          `/groups/g_trail/newsletters/${PUBLISHED_CYCLE_ID}/questions/q_trail_hike/responses/no_such_response/comments`,
        ),
      ),
    ).toThrow(expect.objectContaining({ status: 404, code: "NOT_FOUND" }));
  });

  describe("comments", () => {
    it("lists the seeded comments oldest-first, including the deleted placeholder", () => {
      const result = mockEngagementRoute(
        req("GET", `${BASE}/comments`),
      ) as S["CommentListResponse"];

      expect(result.items.map((c) => c.commentId)).toEqual(["cmt_seed_1", "cmt_seed_2"]);
      const deleted = result.items.find((c) => c.commentId === "cmt_seed_2")!;
      expect(deleted.deletedAt).toBeTruthy();
      expect(deleted.body).toBe("");
      const edited = result.items.find((c) => c.commentId === "cmt_seed_1")!;
      expect(edited.editedAt).toBeTruthy();
    });

    it("rejects a whitespace-only body with no image as 422 VALIDATION_FAILED", () => {
      expect(() => mockEngagementRoute(req("POST", `${BASE}/comments`, { body: "   " }))).toThrow(
        expect.objectContaining({ status: 422, code: "VALIDATION_FAILED" }),
      );
    });

    it("creates a comment authored by the caller, then lets the author edit and delete it", () => {
      const created = mockEngagementRoute(
        req("POST", `${BASE}/comments`, { body: "Nice shot!" }),
      ) as S["CommentResponse"];
      expect(created.authorUserId).toBe("u_quinn");
      expect(created.body).toBe("Nice shot!");
      expect(created.editedAt).toBeNull();
      expect(created.deletedAt).toBeNull();

      const edited = mockEngagementRoute(
        req("PATCH", `${BASE}/comments/${created.commentId}`, { body: "Nice shot! Updated." }),
      ) as S["CommentResponse"];
      expect(edited.body).toBe("Nice shot! Updated.");
      expect(edited.editedAt).toBeTruthy();

      expect(
        mockEngagementRoute(req("DELETE", `${BASE}/comments/${created.commentId}`)),
      ).toBeUndefined();

      const list = mockEngagementRoute(req("GET", `${BASE}/comments`)) as S["CommentListResponse"];
      const afterDelete = list.items.find((c) => c.commentId === created.commentId)!;
      expect(afterDelete.deletedAt).toBeTruthy();
      expect(afterDelete.body).toBe("");

      // DELETE is idempotent.
      expect(
        mockEngagementRoute(req("DELETE", `${BASE}/comments/${created.commentId}`)),
      ).toBeUndefined();
    });

    it("403s a PATCH from a non-author even when the caller is a group admin", () => {
      // The caller ("u_quinn") is an admin in g_trail, but editing is
      // author-only regardless of role — unlike delete, admin doesn't bypass it.
      const answer = findPublishedAnswer("g_trail", PUBLISHED_CYCLE_ID, "q_trail_hike", "pub_r1")!;
      answer.comments.push({
        commentId: "cmt_patch_forbidden_test",
        authorUserId: "u_someone_else",
        displayName: "Someone Else",
        body: "original",
        image: null,
        createdAt: new Date().toISOString(),
        editedAt: null,
        deletedAt: null,
      });

      expect(() =>
        mockEngagementRoute(
          req("PATCH", `${BASE}/comments/cmt_patch_forbidden_test`, { body: "hijacked" }),
        ),
      ).toThrow(expect.objectContaining({ status: 403 }));

      answer.comments = answer.comments.filter((c) => c.commentId !== "cmt_patch_forbidden_test");
    });

    it("lets a group admin delete another member's comment", () => {
      const answer = findPublishedAnswer("g_trail", PUBLISHED_CYCLE_ID, "q_trail_hike", "pub_r1")!;
      answer.comments.push({
        commentId: "cmt_admin_delete_test",
        authorUserId: "u_someone_else",
        displayName: "Someone Else",
        body: "temp",
        image: null,
        createdAt: new Date().toISOString(),
        editedAt: null,
        deletedAt: null,
      });

      expect(
        mockEngagementRoute(req("DELETE", `${BASE}/comments/cmt_admin_delete_test`)),
      ).toBeUndefined();

      const comment = answer.comments.find((c) => c.commentId === "cmt_admin_delete_test")!;
      expect(comment.deletedAt).toBeTruthy();
      answer.comments = answer.comments.filter((c) => c.commentId !== "cmt_admin_delete_test");
    });

    it("403s a DELETE from a non-author, non-admin caller", () => {
      // g_meeple: the caller is a plain member there (Tara is the admin).
      const meepleAnswer = findPublishedAnswer(
        "g_meeple",
        PUBLISHED_CYCLE_ID,
        "q_g_meeple_archive_1",
        "pub_g_meeple_other",
      )!;
      meepleAnswer.comments.push({
        commentId: "cmt_forbidden_delete_test",
        authorUserId: "u_someone_else",
        displayName: "Someone Else",
        body: "temp",
        image: null,
        createdAt: new Date().toISOString(),
        editedAt: null,
        deletedAt: null,
      });

      expect(() =>
        mockEngagementRoute(
          req(
            "DELETE",
            "/groups/g_meeple/newsletters/" +
              `${PUBLISHED_CYCLE_ID}/questions/q_g_meeple_archive_1/responses/pub_g_meeple_other/comments/cmt_forbidden_delete_test`,
          ),
        ),
      ).toThrow(expect.objectContaining({ status: 403 }));

      meepleAnswer.comments = meepleAnswer.comments.filter(
        (c) => c.commentId !== "cmt_forbidden_delete_test",
      );
    });
  });

  describe("reactions", () => {
    it("rejects an invalid emoji with 422 VALIDATION_FAILED", () => {
      expect(() => mockEngagementRoute(req("PUT", `${BASE}/reactions/abc`))).toThrow(
        expect.objectContaining({ status: 422, code: "VALIDATION_FAILED" }),
      );
    });

    it("adds, idempotently re-adds, then removes a reaction", () => {
      const emoji = encodeURIComponent("🎈");

      const afterPut = mockEngagementRoute(
        req("PUT", `${BASE}/reactions/${emoji}`),
      ) as S["ReactionsResponse"];
      const group = afterPut.reactionGroups.find((g) => g.emoji === "🎈")!;
      expect(group).toMatchObject({ count: 1, reactedByMe: true });
      expect(afterPut.myReactions).toContain("🎈");

      const afterSecondPut = mockEngagementRoute(
        req("PUT", `${BASE}/reactions/${emoji}`),
      ) as S["ReactionsResponse"];
      expect(afterSecondPut.reactionGroups.find((g) => g.emoji === "🎈")).toMatchObject({
        count: 1,
      });

      const afterDelete = mockEngagementRoute(
        req("DELETE", `${BASE}/reactions/${emoji}`),
      ) as S["ReactionsResponse"];
      expect(afterDelete.reactionGroups.find((g) => g.emoji === "🎈")).toBeUndefined();
      expect(afterDelete.myReactions).not.toContain("🎈");

      // DELETE is idempotent.
      expect(() => mockEngagementRoute(req("DELETE", `${BASE}/reactions/${emoji}`))).not.toThrow();
    });

    it("lists reactions ordered by count desc", () => {
      const result = mockEngagementRoute(req("GET", `${BASE}/reactions`)) as S["ReactionsResponse"];
      expect(result.reactionGroups[0]).toMatchObject({ emoji: "🔥", count: 2 });
    });
  });
});
