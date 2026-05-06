// Mock API client. Mirrors the route shapes documented in plans/03-api-contract.md.
// Each function returns a promise that resolves after a small simulated latency,
// mutating the in-memory store from `mockData.ts` so the UI feels live.
//
// Swap this file's implementations for `fetch` against the real backend without
// changing any callsites in `queries.ts` / `mutations.ts`.

import {
  config as mockConfig,
  groups,
  invites,
  newsletters,
  newsletterSummariesByGroup,
  notificationPrefs,
  pushDevices,
  trailCrewGradient,
} from "./mockData";
import type {
  AppConfig,
  CandidateQuestion,
  Group,
  GroupGradient,
  Invite,
  Membership,
  MyResponse,
  NewsletterDetail,
  NewsletterSummary,
  NotificationPref,
  PublishedNewsletter,
  PushDevice,
  Role,
  VotingNewsletter,
} from "./types";

function delay<T>(value: T, ms = 250): Promise<T> {
  return new Promise((resolve) => setTimeout(() => resolve(value), ms));
}

// --- Bootstrap ---------------------------------------------------------------

export const api = {
  async getConfig(): Promise<AppConfig> {
    return delay(structuredClone(mockConfig));
  },

  // --- Groups ---------------------------------------------------------------
  async getGroup(groupId: string): Promise<Group> {
    const g = groups[groupId];
    if (!g) throw new Error("group not found");
    return delay(structuredClone(g));
  },

  async patchGroup(groupId: string, patch: Partial<Group>): Promise<Group> {
    const g = groups[groupId];
    if (!g) throw new Error("group not found");
    Object.assign(g, patch);
    return delay(structuredClone(g));
  },

  async listMemberships(): Promise<Membership[]> {
    return delay(structuredClone(mockConfig.memberships));
  },

  // --- Newsletters ---------------------------------------------------------
  async listNewsletters(groupId: string): Promise<NewsletterSummary[]> {
    const list = newsletterSummariesByGroup[groupId] ?? [];
    return delay(structuredClone(list));
  },

  async getNewsletter(groupId: string, cycleId: string): Promise<NewsletterDetail> {
    const key = `${groupId}/${cycleId}`;
    const nl = newsletters[key];
    if (!nl) throw new Error("newsletter not found");
    return delay(structuredClone(nl));
  },

  // --- Candidate questions -------------------------------------------------
  async listCandidates(groupId: string): Promise<VotingNewsletter> {
    const nl = Object.values(newsletters).find(
      (n) => n.groupId === groupId && n.status === "voting",
    ) as VotingNewsletter | undefined;
    if (!nl) throw new Error("no voting cycle");
    return delay(structuredClone(nl));
  },

  async castVote(groupId: string, questionId: string): Promise<VotingNewsletter> {
    const nl = Object.values(newsletters).find(
      (n) => n.groupId === groupId && n.status === "voting",
    ) as VotingNewsletter | undefined;
    if (!nl) throw new Error("no voting cycle");
    if (nl.myVoteCount >= nl.votesPerUserPerCycle) {
      const e = new Error("vote cap reached") as Error & { code?: string };
      e.code = "VOTE_CAP_REACHED";
      throw e;
    }
    const c = nl.candidates.find((x) => x.questionId === questionId);
    if (!c) throw new Error("candidate not found");
    if (c.votedByMe) return delay(structuredClone(nl));
    c.votedByMe = true;
    c.voteCount += 1;
    nl.myVoteCount += 1;
    nl.candidates.sort((a, b) => b.voteCount - a.voteCount);
    return delay(structuredClone(nl));
  },

  async withdrawVote(groupId: string, questionId: string): Promise<VotingNewsletter> {
    const nl = Object.values(newsletters).find(
      (n) => n.groupId === groupId && n.status === "voting",
    ) as VotingNewsletter | undefined;
    if (!nl) throw new Error("no voting cycle");
    const c = nl.candidates.find((x) => x.questionId === questionId);
    if (!c) throw new Error("candidate not found");
    if (!c.votedByMe) return delay(structuredClone(nl));
    c.votedByMe = false;
    c.voteCount = Math.max(0, c.voteCount - 1);
    nl.myVoteCount = Math.max(0, nl.myVoteCount - 1);
    nl.candidates.sort((a, b) => b.voteCount - a.voteCount);
    return delay(structuredClone(nl));
  },

  async suggestCandidate(groupId: string, payload: NewCandidatePayload): Promise<CandidateQuestion> {
    const nl = Object.values(newsletters).find(
      (n) => n.groupId === groupId && n.status === "voting",
    ) as VotingNewsletter | undefined;
    if (!nl) throw new Error("no voting cycle");
    const newCandidate: CandidateQuestion = {
      questionId: `qc_${Date.now()}`,
      kind: payload.kind,
      prompt: payload.prompt,
      pollOptions:
        payload.kind === "poll"
          ? payload.pollOptions.map((label, i) => ({ optionId: `po_${Date.now()}_${i}`, label }))
          : null,
      voteCount: 0,
      votedByMe: false,
      submittedAt: new Date().toISOString(),
    };
    nl.candidates.unshift(newCandidate);
    return delay(structuredClone(newCandidate));
  },

  // --- Responses (drafts + publish) ---------------------------------------
  async saveResponse(
    groupId: string,
    cycleId: string,
    questionId: string,
    body: SaveResponseBody,
  ): Promise<MyResponse> {
    const nl = newsletters[`${groupId}/${cycleId}`];
    if (!nl || nl.status !== "open") throw new Error("cycle not open");
    let existing = nl.myResponses.find((r) => r.questionId === questionId);
    if (!existing) {
      existing = {
        responseId: `r_${questionId}_${Date.now()}`,
        questionId,
        status: "draft",
        kind: body.kind,
        version: 0,
        updatedAt: new Date().toISOString(),
      };
      nl.myResponses.push(existing);
    }
    if (body.kind === "text") {
      existing.body = body.body;
      existing.imageMediaIds = body.imageMediaIds;
      existing.wordCount = body.body.trim() ? body.body.trim().split(/\s+/).length : 0;
    } else {
      existing.pollOptionId = body.pollOptionId;
    }
    existing.version += 1;
    existing.updatedAt = new Date().toISOString();
    if (body.publish) {
      existing.status = "published";
      existing.publishedAt = new Date().toISOString();
    }
    return delay(structuredClone(existing), 350);
  },

  // --- Engagement ---------------------------------------------------------
  async toggleReaction(
    groupId: string,
    cycleId: string,
    questionId: string,
    responseId: string,
    emoji: string,
  ): Promise<{ emoji: string; count: number; reactedByMe: boolean }> {
    const nl = newsletters[`${groupId}/${cycleId}`];
    if (!nl || nl.status !== "published") throw new Error("not published");
    const q = nl.questions.find((x) => x.questionId === questionId);
    if (!q || q.kind !== "text") throw new Error("question not found");
    const ans = q.answers.find((a) => a.responseId === responseId);
    if (!ans) throw new Error("answer not found");
    let group = ans.reactionGroups.find((g) => g.emoji === emoji);
    if (!group) {
      group = { emoji, count: 1, reactedByMe: true };
      ans.reactionGroups.push(group);
    } else if (group.reactedByMe) {
      group.count -= 1;
      group.reactedByMe = false;
      if (group.count === 0) ans.reactionGroups = ans.reactionGroups.filter((g) => g !== group);
    } else {
      group.count += 1;
      group.reactedByMe = true;
    }
    return delay({ ...group });
  },

  async addComment(
    groupId: string,
    cycleId: string,
    questionId: string,
    responseId: string,
    body: string,
  ): Promise<PublishedNewsletter> {
    const nl = newsletters[`${groupId}/${cycleId}`];
    if (!nl || nl.status !== "published") throw new Error("not published");
    const q = nl.questions.find((x) => x.questionId === questionId);
    if (!q || q.kind !== "text") throw new Error("not a text question");
    const ans = q.answers.find((a) => a.responseId === responseId);
    if (!ans) throw new Error("answer not found");
    ans.comments.push({
      commentId: `c_${Date.now()}`,
      authorUserId: mockConfig.user.userId,
      authorDisplayName: mockConfig.user.displayName,
      authorAvatarColor: mockConfig.user.avatarColor,
      body,
      createdAt: new Date().toISOString(),
      editedAt: null,
    });
    return delay(structuredClone(nl));
  },

  async castPollVote(
    groupId: string,
    cycleId: string,
    questionId: string,
    optionId: string,
  ): Promise<PublishedNewsletter> {
    const nl = newsletters[`${groupId}/${cycleId}`];
    if (!nl || nl.status !== "published") throw new Error("not published");
    const q = nl.questions.find((x) => x.questionId === questionId);
    if (!q || q.kind !== "poll") throw new Error("not a poll");
    if (q.myVoteOptionId) {
      const prev = q.options.find((o) => o.optionId === q.myVoteOptionId);
      if (prev) prev.voteCount = Math.max(0, prev.voteCount - 1);
    } else {
      q.totalVotes += 1;
    }
    const next = q.options.find((o) => o.optionId === optionId);
    if (!next) throw new Error("option not found");
    next.voteCount += 1;
    q.myVoteOptionId = optionId;
    return delay(structuredClone(nl));
  },

  // --- Invites ------------------------------------------------------------
  async listInvites(groupId: string): Promise<Invite[]> {
    return delay(structuredClone(invites.filter((i) => i.groupId === groupId)));
  },

  async createInvite(groupId: string, role: Role): Promise<Invite> {
    const code = chunked(crypto.randomUUID().replace(/-/g, "").slice(0, 16).toUpperCase(), 4).join("-");
    const inv: Invite = {
      code,
      groupId,
      groupName: groups[groupId]?.name ?? "Unknown",
      status: "pending",
      expiresAt: new Date(Date.now() + 7 * 24 * 60 * 60 * 1000).toISOString(),
      roleOnRedeem: role,
    };
    invites.unshift(inv);
    return delay(structuredClone(inv));
  },

  async revokeInvite(code: string): Promise<void> {
    const inv = invites.find((i) => i.code === code);
    if (!inv) throw new Error("invite not found");
    inv.status = "revoked";
    return delay(undefined);
  },

  async previewInvite(code: string): Promise<Invite | null> {
    const inv = invites.find((i) => i.code === code && i.status === "pending");
    return delay(inv ? structuredClone(inv) : null, 400);
  },

  async redeemInvite(code: string): Promise<{ groupId: string; role: Role; groupName: string; gradient: GroupGradient }> {
    const inv = invites.find((i) => i.code === code && i.status === "pending");
    if (!inv) throw new Error("invite invalid");
    inv.status = "consumed";
    inv.consumedBy = mockConfig.user.userId;
    inv.consumedByName = mockConfig.user.displayName;
    inv.consumedAt = new Date().toISOString();
    if (!mockConfig.memberships.some((m) => m.groupId === inv.groupId)) {
      mockConfig.memberships.push({
        groupId: inv.groupId,
        role: inv.roleOnRedeem,
        groupName: inv.groupName,
        gradient: trailCrewGradient,
      });
    }
    return delay({ groupId: inv.groupId, role: inv.roleOnRedeem, groupName: inv.groupName, gradient: trailCrewGradient });
  },

  // --- Push ---------------------------------------------------------------
  async listPushDevices(): Promise<PushDevice[]> {
    return delay(structuredClone(pushDevices));
  },

  async removePushDevice(subscriptionId: string): Promise<void> {
    const idx = pushDevices.findIndex((d) => d.subscriptionId === subscriptionId);
    if (idx >= 0) pushDevices.splice(idx, 1);
    return delay(undefined);
  },

  async sendTestPush(): Promise<{ devices: number }> {
    return delay({ devices: pushDevices.length }, 600);
  },

  // --- Notification preferences ------------------------------------------
  async listNotificationPrefs(): Promise<NotificationPref[]> {
    return delay(structuredClone(notificationPrefs));
  },

  async updateNotificationPref(groupId: string, patch: Partial<NotificationPref>): Promise<NotificationPref> {
    let pref = notificationPrefs.find((p) => p.groupId === groupId);
    if (!pref) {
      pref = { groupId, cycleOpen: true, deadlineReminders: true };
      notificationPrefs.push(pref);
    }
    Object.assign(pref, patch);
    return delay(structuredClone(pref));
  },
};

export interface NewCandidatePayload {
  kind: "text" | "poll";
  prompt: string;
  pollOptions: string[];
}

export type SaveResponseBody =
  | {
      kind: "text";
      body: string;
      imageMediaIds: string[];
      publish?: boolean;
    }
  | {
      kind: "poll";
      pollOptionId: string;
      publish?: boolean;
    };

function chunked(str: string, size: number): string[] {
  const out: string[] = [];
  for (let i = 0; i < str.length; i += size) out.push(str.slice(i, i + size));
  return out;
}
