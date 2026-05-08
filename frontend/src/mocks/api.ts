// MOCK API IMPLEMENTATION.
// Everything in `src/mocks/` is mocked — no network calls, no backend.
// Each method returns a promise that resolves after a small simulated latency,
// mutating the in-memory store from `./data.ts` so the UI feels live within a session.
//
// To swap a real backend in: replace the `mockApi` export below with a fetch-based
// client that matches the same shape, then update `src/api/client.ts` to point at it.

import {
  config as mockConfig,
  groups,
  invites,
  newsletters,
  newsletterSummariesByGroup,
  notificationPrefs,
  pushDevices,
  trailCrewGradient,
} from "./data";
import type {
  AppConfig,
  CandidateQuestion,
  Comment,
  Group,
  GroupGradient,
  ImageUploadStatus,
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
} from "../api/types";

function delay<T>(value: T, ms = 250): Promise<T> {
  return new Promise((resolve) => setTimeout(() => resolve(value), ms));
}

const pushState = { globalEnabled: true };

// MOCK: in-memory upload state map keyed by imageId. Real impl replaces this
// with persistent storage (DDB row + S3 trigger updating it).
const uploadStates = new Map<string, ImageUploadStatus>();

// --- Bootstrap ---------------------------------------------------------------

export const mockApi = {
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
      existing.imageCaptions = body.imageCaptions;
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
    if (!q) throw new Error("question not found");
    const newComment: Comment = {
      commentId: `c_${Date.now()}`,
      authorUserId: mockConfig.user.userId,
      authorDisplayName: mockConfig.user.displayName,
      authorAvatarColor: mockConfig.user.avatarColor,
      authorAvatarUrl: mockConfig.user.avatarUrl ?? null,
      body,
      createdAt: new Date().toISOString(),
      editedAt: null,
    };
    if (q.kind === "poll") {
      // Poll comments hang off the question itself; the client passes
      // responseId === questionId as a sentinel.
      if (responseId !== q.questionId) throw new Error("poll comment responseId must match questionId");
      q.comments.push(newComment);
    } else {
      const ans = q.answers.find((a) => a.responseId === responseId);
      if (!ans) throw new Error("answer not found");
      ans.comments.push(newComment);
    }
    return delay(structuredClone(nl));
  },

  async editComment(
    groupId: string,
    cycleId: string,
    questionId: string,
    responseId: string,
    commentId: string,
    body: string,
  ): Promise<PublishedNewsletter> {
    const nl = newsletters[`${groupId}/${cycleId}`];
    if (!nl || nl.status !== "published") throw new Error("not published");
    const q = nl.questions.find((x) => x.questionId === questionId);
    if (!q) throw new Error("question not found");
    const list = q.kind === "poll" ? q.comments : q.answers.find((a) => a.responseId === responseId)?.comments;
    if (!list) throw new Error("comments not found");
    const c = list.find((cc) => cc.commentId === commentId);
    if (!c) throw new Error("comment not found");
    if (c.authorUserId !== mockConfig.user.userId) throw new Error("only the author can edit this comment");
    c.body = body;
    c.editedAt = new Date().toISOString();
    return delay(structuredClone(nl));
  },

  async deleteComment(
    groupId: string,
    cycleId: string,
    questionId: string,
    responseId: string,
    commentId: string,
  ): Promise<PublishedNewsletter> {
    const nl = newsletters[`${groupId}/${cycleId}`];
    if (!nl || nl.status !== "published") throw new Error("not published");
    const q = nl.questions.find((x) => x.questionId === questionId);
    if (!q) throw new Error("question not found");
    const list = q.kind === "poll" ? q.comments : q.answers.find((a) => a.responseId === responseId)?.comments;
    if (!list) throw new Error("comments not found");
    const idx = list.findIndex((cc) => cc.commentId === commentId);
    if (idx < 0) throw new Error("comment not found");
    const c = list[idx]!;
    const isAuthor = c.authorUserId === mockConfig.user.userId;
    const isAdmin = (groups[groupId]?.members.find((m) => m.userId === mockConfig.user.userId)?.role === "admin");
    if (!isAuthor && !isAdmin) throw new Error("only the author or a group admin can delete this comment");
    // Hard delete — no soft-delete tombstones in v1.
    list.splice(idx, 1);
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

  // --- Current user --------------------------------------------------------
  async patchUser(patch: Partial<Pick<AppConfig["user"], "displayName" | "avatarColor">>): Promise<AppConfig["user"]> {
    Object.assign(mockConfig.user, patch);
    // Mirror display name + avatar color into the user's row inside each group
    // so the admin/member views see the rename immediately.
    for (const g of Object.values(groups)) {
      const m = g.members.find((mem) => mem.userId === mockConfig.user.userId);
      if (m) {
        if (patch.displayName) m.displayName = patch.displayName;
        if (patch.avatarColor) m.avatarColor = patch.avatarColor;
      }
    }
    return delay(structuredClone(mockConfig.user));
  },

  // --- Membership management (admin) --------------------------------------
  async updateMemberRole(groupId: string, userId: string, role: Role): Promise<Group> {
    const g = groups[groupId];
    if (!g) throw new Error("group not found");
    const m = g.members.find((mem) => mem.userId === userId);
    if (!m) throw new Error("member not found");
    if (m.role === "admin" && role === "member") {
      const otherAdmins = g.members.filter((x) => x.userId !== userId && x.role === "admin");
      if (otherAdmins.length === 0) throw new Error("can't demote the last admin");
    }
    m.role = role;
    return delay(structuredClone(g));
  },

  async removeMember(groupId: string, userId: string): Promise<Group> {
    const g = groups[groupId];
    if (!g) throw new Error("group not found");
    if (userId === mockConfig.user.userId) throw new Error("can't remove yourself — leave the group from settings instead");
    const idx = g.members.findIndex((mem) => mem.userId === userId);
    if (idx < 0) throw new Error("member not found");
    g.members.splice(idx, 1);
    return delay(structuredClone(g));
  },

  async leaveGroup(groupId: string): Promise<{ groupId: string }> {
    const g = groups[groupId];
    if (!g) throw new Error("group not found");
    const me = g.members.find((m) => m.userId === mockConfig.user.userId);
    if (!me) throw new Error("you aren't in this group");
    if (me.role === "admin") {
      const otherAdmins = g.members.filter((m) => m.userId !== mockConfig.user.userId && m.role === "admin");
      if (otherAdmins.length === 0) {
        throw new Error("you're the only admin — promote someone else first");
      }
    }
    g.members = g.members.filter((m) => m.userId !== mockConfig.user.userId);
    mockConfig.memberships = mockConfig.memberships.filter((mb) => mb.groupId !== groupId);
    return delay({ groupId }, 300);
  },

  // --- Media (image upload lifecycle) -------------------------------------
  // MOCK: simulates the presign → upload → process pipeline by ticking through
  // states locally on a timer. For the real impl, replace these methods with
  // POST /uploads (presign), a direct PUT to S3, and polling GET /uploads/{id}
  // — and DELETE the simulated timers/state below entirely.
  async startImageUpload(_input: {
    groupId: string;
    cycleId: string;
    questionId: string;
    fileName: string;
    fileSize: number;
    mimeType: string;
    dataUrl: string; // mock-only: client passes a dataURL so the UI has something to render
  }): Promise<{ imageId: string }> {
    const imageId = `i_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`;
    const status: ImageUploadStatus = {
      imageId,
      status: "uploading",
      progress: 0,
      thumbUrl: null,
      displayUrl: null,
    };
    uploadStates.set(imageId, status);

    // MOCK delay schedule. Remove this whole block in the real implementation.
    const tick = (next: Partial<ImageUploadStatus>, after: number) =>
      setTimeout(() => {
        const cur = uploadStates.get(imageId);
        if (!cur) return;
        Object.assign(cur, next);
      }, after);
    tick({ progress: 0.25 }, 200);
    tick({ progress: 0.6 }, 500);
    tick({ progress: 1, status: "processing" }, 900);
    tick({ status: "ready", progress: 1, thumbUrl: _input.dataUrl, displayUrl: _input.dataUrl }, 1600);

    return delay({ imageId }, 200);
  },

  async getImageUploadStatus(imageId: string): Promise<ImageUploadStatus> {
    const s = uploadStates.get(imageId);
    if (!s) throw new Error("upload not found");
    return delay(structuredClone(s), 80);
  },

  async cancelImageUpload(imageId: string): Promise<void> {
    uploadStates.delete(imageId);
    return delay(undefined, 80);
  },

  async uploadUserAvatar(dataUrl: string): Promise<AppConfig["user"]> {
    // MOCK: pretend we PUT the avatar to S3 + ran the resize Lambda. The real
    // impl uploads via the same /uploads pipeline above and stores the resulting
    // CDN URL on the user row.
    mockConfig.user.avatarUrl = dataUrl;
    for (const g of Object.values(groups)) {
      const m = g.members.find((mem) => mem.userId === mockConfig.user.userId);
      if (m) m.avatarUrl = dataUrl;
    }
    return delay(structuredClone(mockConfig.user), 500);
  },

  async removeUserAvatar(): Promise<AppConfig["user"]> {
    mockConfig.user.avatarUrl = null;
    for (const g of Object.values(groups)) {
      const m = g.members.find((mem) => mem.userId === mockConfig.user.userId);
      if (m) m.avatarUrl = null;
    }
    return delay(structuredClone(mockConfig.user), 200);
  },

  // --- Push ---------------------------------------------------------------
  async getPushEnabled(): Promise<{ enabled: boolean }> {
    return delay({ enabled: pushState.globalEnabled });
  },

  async setPushEnabled(enabled: boolean): Promise<{ enabled: boolean }> {
    pushState.globalEnabled = enabled;
    return delay({ enabled }, 200);
  },

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
      pref = { groupId, cycleOpen: true, deadlineReminders: true, publication: true };
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
      imageCaptions?: Record<string, string>;
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
