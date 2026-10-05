import { describe, expect, it } from "vitest";
import type { components } from "../types/api";
import { ApiError } from "./client";
import { mockNewsletterRoute } from "./mockNewsletters";
import {
  NO_MOCK_ROUTE,
  OPEN_CYCLE_ID,
  PUBLISHED_CYCLE_ID,
  VOTING_CYCLE_ID,
  newsletters,
} from "./mockShared";

type S = components["schemas"];

function req(
  method: "GET" | "PUT",
  rawPath: string,
  body: unknown = undefined,
): {
  method: "GET" | "PUT";
  rawPath: string;
  segments: string[];
  query: URLSearchParams;
  body: unknown;
} {
  return {
    method,
    rawPath,
    segments: rawPath.split("/").filter(Boolean),
    query: new URLSearchParams(),
    body,
  };
}

function summaryFor(groupId: string, cycleId: string): S["NewsletterSummary"] {
  const summary = newsletters[groupId]?.find((n) => n.cycleId === cycleId);
  if (!summary) throw new Error(`fixture missing ${groupId}/${cycleId}`);
  return summary;
}

describe("mockNewsletterRoute", () => {
  it("returns NO_MOCK_ROUTE for an unrelated path", () => {
    expect(mockNewsletterRoute(req("GET", "/groups/g_trail/candidate-questions"))).toBe(
      NO_MOCK_ROUTE,
    );
  });

  it("GET detail on a voting cycle returns an empty candidate list", () => {
    const result = mockNewsletterRoute(
      req("GET", `/groups/g_trail/newsletters/${VOTING_CYCLE_ID}`),
    ) as S["NewsletterDetailResponse"];
    expect(result).toEqual({ status: "voting", candidates: [] });
  });

  it("GET detail on the open cycle hydrates myResponse per question from the response store", () => {
    const result = mockNewsletterRoute(
      req("GET", `/groups/g_trail/newsletters/${OPEN_CYCLE_ID}`),
    ) as Extract<S["NewsletterDetailResponse"], { status: "open" }>;
    expect(result.status).toBe("open");
    const hike = result.questions.find((q) => q.questionId === "q_trail_hike");
    const photo = result.questions.find((q) => q.questionId === "q_trail_photo");
    const mind = result.questions.find((q) => q.questionId === "q_trail_mind");
    expect(hike?.myResponse?.status).toBe("published");
    expect(photo?.myResponse?.status).toBe("draft");
    expect(mind?.myResponse).toBeNull();
  });

  it("GET detail on a published cycle returns hydrated answers and a poll with myVoteOptionId", () => {
    const result = mockNewsletterRoute(
      req("GET", `/groups/g_trail/newsletters/${PUBLISHED_CYCLE_ID}`),
    ) as Extract<S["NewsletterDetailResponse"], { status: "published" }>;
    const poll = result.questions.find((q) => q.kind === "poll");
    expect(poll?.myVoteOptionId).toBeTruthy();
    expect(poll?.options?.some((o) => o.optionId === poll.myVoteOptionId)).toBe(true);
    const textQuestion = result.questions.find((q) => q.kind === "text");
    expect(textQuestion?.answers?.length).toBeGreaterThan(0);
  });

  it("GET detail 404s for an unknown cycle and 403s for an unknown group", () => {
    expect(() => mockNewsletterRoute(req("GET", "/groups/g_trail/newsletters/999999"))).toThrow(
      ApiError,
    );
    expect(() =>
      mockNewsletterRoute(req("GET", `/groups/g_nope/newsletters/${OPEN_CYCLE_ID}`)),
    ).toThrow(ApiError);
  });

  it("first PUT on a fresh question assigns a responseId and bumps myDraftCount", () => {
    const before = summaryFor("g_trail", OPEN_CYCLE_ID).myDraftCount;
    const result = mockNewsletterRoute(
      req(
        "PUT",
        `/groups/g_trail/newsletters/${OPEN_CYCLE_ID}/questions/q_trail_checkout/my-response`,
        {
          kind: "text",
          body: "A new draft",
          imageMediaIds: [],
          publish: false,
        },
      ),
    ) as S["ResponseDto"];

    expect(result.responseId).toBeTruthy();
    expect(result.status).toBe("draft");
    expect(summaryFor("g_trail", OPEN_CYCLE_ID).myDraftCount).toBe(before + 1);

    const resaved = mockNewsletterRoute(
      req(
        "PUT",
        `/groups/g_trail/newsletters/${OPEN_CYCLE_ID}/questions/q_trail_checkout/my-response`,
        {
          kind: "text",
          body: "An edited draft",
          imageMediaIds: [],
          publish: false,
        },
      ),
    ) as S["ResponseDto"];
    expect(resaved.responseId).toBe(result.responseId);
    expect(summaryFor("g_trail", OPEN_CYCLE_ID).myDraftCount).toBe(before + 1);
  });

  it("publishing is sticky: a later publish:false save keeps status=published and the original publishedAt", async () => {
    const draftCountBefore = summaryFor("g_trail", OPEN_CYCLE_ID).myDraftCount;
    const publishedCountBefore = summaryFor("g_trail", OPEN_CYCLE_ID).myPublishedCount;

    const published = mockNewsletterRoute(
      req(
        "PUT",
        `/groups/g_trail/newsletters/${OPEN_CYCLE_ID}/questions/q_trail_mind/my-response`,
        {
          kind: "text",
          body: "Thinking about the holidays.",
          imageMediaIds: [],
          publish: true,
        },
      ),
    ) as S["ResponseDto"];
    expect(published.status).toBe("published");
    expect(summaryFor("g_trail", OPEN_CYCLE_ID).myDraftCount).toBe(draftCountBefore);
    expect(summaryFor("g_trail", OPEN_CYCLE_ID).myPublishedCount).toBe(publishedCountBefore + 1);

    await new Promise((resolve) => setTimeout(resolve, 2));
    const resaved = mockNewsletterRoute(
      req(
        "PUT",
        `/groups/g_trail/newsletters/${OPEN_CYCLE_ID}/questions/q_trail_mind/my-response`,
        {
          kind: "text",
          body: "Thinking about the holidays — updated.",
          imageMediaIds: [],
          publish: false,
        },
      ),
    ) as S["ResponseDto"];

    expect(resaved.status).toBe("published");
    expect(resaved.publishedAt).toBe(published.publishedAt);
    expect(resaved.body).toBe("Thinking about the holidays — updated.");
    // Sticky publish must not double-count on a later re-save.
    expect(summaryFor("g_trail", OPEN_CYCLE_ID).myPublishedCount).toBe(publishedCountBefore + 1);
  });

  it("refuses a PUT on a non-open cycle with 409 CYCLE_NOT_OPEN", () => {
    expect(() =>
      mockNewsletterRoute(
        req(
          "PUT",
          `/groups/g_trail/newsletters/${PUBLISHED_CYCLE_ID}/questions/q_trail_hike/my-response`,
          { kind: "text", body: "too late", imageMediaIds: [], publish: false },
        ),
      ),
    ).toThrow(expect.objectContaining({ status: 409, code: "CYCLE_NOT_OPEN" }));
  });

  it("rejects a kind mismatch with 422 VALIDATION_FAILED", () => {
    expect(() =>
      mockNewsletterRoute(
        req(
          "PUT",
          `/groups/g_trail/newsletters/${OPEN_CYCLE_ID}/questions/q_trail_poll/my-response`,
          {
            kind: "text",
            body: "wrong kind",
            imageMediaIds: [],
            publish: false,
          },
        ),
      ),
    ).toThrow(expect.objectContaining({ status: 422, code: "VALIDATION_FAILED" }));
  });
});
