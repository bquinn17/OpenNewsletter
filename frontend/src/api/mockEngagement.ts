import type { components } from "../types/api";
import { isValidEmoji, normalizeEmoji } from "../utils/emoji";
import { ApiError } from "./client";
import { findPublishedAnswer } from "./mockNewsletters";
import {
  CALLER_ID,
  MOCK_IMAGE_DATA_URL,
  fail,
  newsletters,
  NO_MOCK_ROUTE,
  type MockRequest,
} from "./mockShared";

type S = components["schemas"];
type PublishedAnswer = S["PublishedAnswerResponse"];

/**
 * Mock routes for `/groups/{g}/newsletters/{c}/questions/{q}/responses/{r}/{comments,reactions}*`
 * (`03-api-contract.md` §8, `09-engagement.md`). Mutates the same
 * `PublishedAnswerResponse` objects `mockNewsletters.ts` serves on
 * `GET .../newsletters/{c}` — there's no separate engagement data store.
 */

/** `CALLER_ID` is a group admin only in `g_trail` (mirrors the memberships fixture in `mockTransport.ts`). */
const ADMIN_GROUP_IDS = new Set(["g_trail"]);

let nextCommentId = 1;

function validationFailed(field: string, message: string): never {
  const problem: S["ProblemDetails"] = {
    type: "https://api.opennewsletter.example.com/errors/validation-failed",
    title: "Validation failed",
    status: 422,
    detail: message,
    code: "VALIDATION_FAILED",
    correlationId: "mock-correlation-id",
    fieldErrors: [{ field, code: "INVALID", message }],
  };
  throw new ApiError(422, "VALIDATION_FAILED", problem);
}

function requirePublishedAnswer(
  groupId: string,
  cycleId: string,
  questionId: string,
  responseId: string,
): PublishedAnswer {
  const all = newsletters[groupId];
  if (!all) fail(403, "FORBIDDEN", `not a member of ${groupId}`);
  const summary = all.find((n) => n.cycleId === cycleId);
  if (!summary) fail(404, "NOT_FOUND", `cycle ${cycleId} not found`);
  if (summary.status !== "published") {
    fail(409, "CYCLE_NOT_PUBLISHED", `cycle ${cycleId} is not published`);
  }
  const answer = findPublishedAnswer(groupId, cycleId, questionId, responseId);
  if (!answer) fail(404, "NOT_FOUND", `response ${responseId} not found in cycle ${cycleId}`);
  return answer;
}

/** A generic stand-in for whatever image the caller uploaded (mirrors `mockTransport.ts`'s own placeholder). */
function mockCommentImage(imageId: string): S["CommentImageResponse"] {
  return {
    imageId,
    displayUrl: MOCK_IMAGE_DATA_URL,
    thumbUrl: MOCK_IMAGE_DATA_URL,
    width: 800,
    height: 600,
    caption: null,
  };
}

function listComments(answer: PublishedAnswer, query: URLSearchParams): S["CommentListResponse"] {
  const limit = query.get("limit");
  let items = [...answer.comments];
  if (limit) items = items.slice(0, Number(limit));
  return { items, nextCursor: null };
}

function createComment(answer: PublishedAnswer, body: unknown): S["CommentResponse"] {
  // Trusted internal shape — the only caller is our own typed `api.engagement.createComment`.
  const request = (body ?? {}) as S["CreateCommentRequest"];
  const text = (request.body ?? "").trim();
  const imageMediaId = request.imageMediaId ?? null;
  if (!text && !imageMediaId) {
    validationFailed("body", "a comment needs either text or an image");
  }

  const comment: S["CommentResponse"] = {
    commentId: `cmt_${nextCommentId++}`,
    authorUserId: CALLER_ID,
    displayName: "Quinn",
    body: text,
    image: imageMediaId ? mockCommentImage(imageMediaId) : null,
    createdAt: new Date().toISOString(),
    editedAt: null,
    deletedAt: null,
  };
  answer.comments.push(comment);
  return { ...comment };
}

function patchComment(
  answer: PublishedAnswer,
  commentId: string,
  body: unknown,
): S["CommentResponse"] {
  const comment = answer.comments.find((c) => c.commentId === commentId && !c.deletedAt);
  if (!comment) fail(404, "NOT_FOUND", `comment ${commentId} not found`);
  if (comment.authorUserId !== CALLER_ID) {
    fail(403, "FORBIDDEN", "only the author can edit this comment");
  }

  // Trusted internal shape — the only caller is our own typed `api.engagement.patchComment`.
  const request = (body ?? {}) as S["PatchCommentRequest"];
  const nextBody = request.body !== undefined ? request.body.trim() : comment.body;
  const nextImage =
    "imageMediaId" in request
      ? request.imageMediaId === null || request.imageMediaId === undefined
        ? null
        : mockCommentImage(request.imageMediaId)
      : comment.image;
  if (!nextBody && !nextImage) {
    validationFailed("body", "a comment needs either text or an image");
  }

  comment.body = nextBody;
  comment.image = nextImage;
  comment.editedAt = new Date().toISOString();
  return { ...comment };
}

function listReactions(answer: PublishedAnswer): S["ReactionsResponse"] {
  return {
    reactionGroups: answer.reactionGroups.map((g) => ({ ...g })),
    myReactions: answer.reactionGroups.filter((g) => g.reactedByMe).map((g) => g.emoji),
  };
}

/**
 * Server ordering (`03-api-contract.md` §8.5): count desc, then earliest
 * reaction, then emoji. This mock doesn't track per-reaction timestamps, so
 * it ties on emoji instead — good enough for a stable, deterministic demo.
 */
function sortReactionGroups(groups: S["ReactionGroupResponse"][]): void {
  groups.sort((a, b) => b.count - a.count || a.emoji.localeCompare(b.emoji));
}

function putReaction(answer: PublishedAnswer, rawEmoji: string): S["ReactionsResponse"] {
  if (!isValidEmoji(rawEmoji)) validationFailed("emoji", "not a valid emoji");
  const emoji = normalizeEmoji(rawEmoji);
  const group = answer.reactionGroups.find((g) => g.emoji === emoji);
  if (!group) {
    answer.reactionGroups.push({ emoji, count: 1, reactedByMe: true });
  } else if (!group.reactedByMe) {
    group.count += 1;
    group.reactedByMe = true;
  }
  sortReactionGroups(answer.reactionGroups);
  return listReactions(answer);
}

function deleteReaction(answer: PublishedAnswer, rawEmoji: string): S["ReactionsResponse"] {
  if (!isValidEmoji(rawEmoji)) validationFailed("emoji", "not a valid emoji");
  const emoji = normalizeEmoji(rawEmoji);
  const group = answer.reactionGroups.find((g) => g.emoji === emoji);
  if (group?.reactedByMe) {
    group.count -= 1;
    group.reactedByMe = false;
    if (group.count <= 0) {
      answer.reactionGroups = answer.reactionGroups.filter((g) => g !== group);
    }
  }
  return listReactions(answer);
}

export function mockEngagementRoute(request: MockRequest): unknown {
  const { method, segments, query, body } = request;
  if (
    segments[0] !== "groups" ||
    segments[2] !== "newsletters" ||
    segments[4] !== "questions" ||
    segments[6] !== "responses"
  ) {
    return NO_MOCK_ROUTE;
  }

  const groupId = decodeURIComponent(segments[1]!);
  const cycleId = decodeURIComponent(segments[3]!);
  const questionId = decodeURIComponent(segments[5]!);
  const responseId = decodeURIComponent(segments[7]!);
  const sub = segments[8];

  if (sub === "comments") {
    if (segments.length === 9) {
      const answer = requirePublishedAnswer(groupId, cycleId, questionId, responseId);
      if (method === "GET") return listComments(answer, query);
      if (method === "POST") return createComment(answer, body);
    }
    if (segments.length === 10) {
      const answer = requirePublishedAnswer(groupId, cycleId, questionId, responseId);
      const commentId = decodeURIComponent(segments[9]!);
      if (method === "PATCH") return patchComment(answer, commentId, body);
      if (method === "DELETE") {
        const comment = answer.comments.find((c) => c.commentId === commentId);
        if (!comment) fail(404, "NOT_FOUND", `comment ${commentId} not found`);
        if (!comment.deletedAt) {
          const isAdmin = ADMIN_GROUP_IDS.has(groupId);
          if (comment.authorUserId !== CALLER_ID && !isAdmin) {
            fail(403, "FORBIDDEN", "only the author or a group admin can delete this comment");
          }
          comment.deletedAt = new Date().toISOString();
          comment.body = "";
          comment.image = null;
        }
        return undefined;
      }
    }
  }

  if (sub === "reactions") {
    if (segments.length === 9 && method === "GET") {
      return listReactions(requirePublishedAnswer(groupId, cycleId, questionId, responseId));
    }
    if (segments.length === 10) {
      const emoji = decodeURIComponent(segments[9]!);
      const answer = requirePublishedAnswer(groupId, cycleId, questionId, responseId);
      if (method === "PUT") return putReaction(answer, emoji);
      if (method === "DELETE") return deleteReaction(answer, emoji);
    }
  }

  return NO_MOCK_ROUTE;
}
