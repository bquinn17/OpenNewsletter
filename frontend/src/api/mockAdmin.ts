import type { components } from "../types/api";
import { ApiError } from "./client";
import {
  CALLER_ID,
  fail,
  groups,
  hasOtherAdmin,
  NO_MOCK_ROUTE,
  type MockRequest,
} from "./mockShared";

type S = components["schemas"];

/**
 * Mock routes for the M12 admin surface: invite CRUD (`03-api-contract.md`
 * §3) and `PATCH /groups/{g}` / `PATCH /groups/{g}/members/{u}` (§4).
 * Mirrors `backend/crates/lambda-groups/src/validation.rs` for the settings
 * bounds and `group_routes.rs::admin_witness` for the LAST_ADMIN guard.
 */

const MIN_QUESTIONS_PER_CYCLE = 1;
const MAX_QUESTIONS_PER_CYCLE = 20;
const MIN_RESPONSE_WINDOW_DAYS = 1;
const MAX_RESPONSE_WINDOW_DAYS = 28;
const MAX_REMINDER_OFFSET_HOURS = 168;
const MAX_DISPLAY_NAME_CHARS = 40;
const INVITE_MAX_TTL_DAYS = 90;
const DAY_MS = 24 * 60 * 60 * 1000;

interface InviteRow {
  code: string;
  groupId: string;
  status: S["InviteStatus"];
  roleOnRedeem: S["Role"];
  createdBy: string;
  createdAt: string;
  expiresAt: string;
  consumedBy: string | null;
  consumedAt: string | null;
}

let invites: InviteRow[] = [];

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

function requireGroup(groupId: string): S["GroupResponse"] {
  const group = groups[groupId];
  if (!group) fail(404, "NOT_FOUND", `group ${groupId} not found`);
  return group;
}

/** The mock's stand-in for `auth::require_membership(.., admin_only: true)`. */
function requireAdmin(groupId: string): S["GroupResponse"] {
  const group = requireGroup(groupId);
  const caller = group.members.find((m) => m.userId === CALLER_ID);
  if (!caller || caller.role !== "admin") fail(403, "FORBIDDEN", `not an admin of ${groupId}`);
  return group;
}

const CODE_CHARS = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789"; // no ambiguous 0/O/1/I

function randomSegment(): string {
  let out = "";
  for (let i = 0; i < 4; i++) out += CODE_CHARS[Math.floor(Math.random() * CODE_CHARS.length)];
  return out;
}

function generateInviteCode(): string {
  let code: string;
  do {
    code = `${randomSegment()}-${randomSegment()}-${randomSegment()}-${randomSegment()}`;
  } while (invites.some((i) => i.code === code));
  return code;
}

function handleCreateInvite(body: unknown): S["CreateInviteResponse"] {
  // Trusted internal shape — the only caller is our own typed `api.createInvite`.
  const request = (body ?? {}) as Partial<S["CreateInviteRequest"]>;
  if (!request.groupId) validationFailed("groupId", "groupId is required");
  if (!request.roleOnRedeem) validationFailed("roleOnRedeem", "roleOnRedeem is required");
  requireAdmin(request.groupId);

  const ttlDays = request.ttlDays ?? 7;
  if (!Number.isInteger(ttlDays) || ttlDays < 1 || ttlDays > INVITE_MAX_TTL_DAYS) {
    validationFailed("ttlDays", `ttlDays must be between 1 and ${INVITE_MAX_TTL_DAYS}`);
  }

  const now = new Date();
  const row: InviteRow = {
    code: generateInviteCode(),
    groupId: request.groupId,
    status: "pending",
    roleOnRedeem: request.roleOnRedeem,
    createdBy: CALLER_ID,
    createdAt: now.toISOString(),
    expiresAt: new Date(now.getTime() + ttlDays * DAY_MS).toISOString(),
    consumedBy: null,
    consumedAt: null,
  };
  invites = [...invites, row];
  return {
    code: row.code,
    groupId: row.groupId,
    expiresAt: row.expiresAt,
    roleOnRedeem: row.roleOnRedeem,
  };
}

function handleListInvites(groupId: string): S["InviteListResponse"] {
  requireAdmin(groupId);
  const items = invites
    .filter((i) => i.groupId === groupId)
    .sort((a, b) => b.createdAt.localeCompare(a.createdAt))
    .map((i) => ({ ...i }));
  return { items, nextCursor: null };
}

function handleRevokeInvite(code: string): undefined {
  const invite = invites.find((i) => i.code === code);
  if (!invite) fail(404, "NOT_FOUND", `invite ${code} not found`);
  requireAdmin(invite.groupId);

  if (invite.status === "consumed") {
    fail(409, "INVITE_CONSUMED", "a consumed invite can't be revoked");
  }
  invite.status = "revoked";
  return undefined;
}

function handlePatchMember(groupId: string, userId: string, body: unknown): S["MemberResponse"] {
  const group = requireAdmin(groupId);
  // Trusted internal shape — the only caller is our own typed `api.patchMember`.
  const request = (body ?? {}) as Partial<S["PatchMemberRequest"]>;
  if (request.role !== "admin" && request.role !== "member") {
    validationFailed("role", "role must be admin or member");
  }

  const member = group.members.find((m) => m.userId === userId);
  if (!member) fail(404, "NOT_FOUND", `member ${userId} not found in group ${groupId}`);

  const isDemotion = member.role === "admin" && request.role === "member";
  // Mirrors `group_routes.rs::admin_witness`: an admin demoting someone ELSE
  // is their own witness, so only a self-demotion of the sole admin 409s.
  if (isDemotion && userId === CALLER_ID && !hasOtherAdmin(group, userId)) {
    fail(409, "LAST_ADMIN", "a group must keep at least one admin");
  }

  member.role = request.role;
  return { ...member };
}

function handlePatchGroup(groupId: string, body: unknown): S["GroupResponse"] {
  const group = requireAdmin(groupId);
  // Trusted internal shape — the only caller is our own typed `api.patchGroup`.
  const request = (body ?? {}) as S["PatchGroupRequest"];

  // Validate the whole patch before writing anything, like the backend, so a
  // 422 never leaves a partial update in the shared fixture.
  let name = group.name;
  if (request.name !== undefined) {
    const trimmed = request.name.trim();
    if (trimmed.length < 1 || trimmed.length > MAX_DISPLAY_NAME_CHARS) {
      validationFailed("name", `name must be 1 to ${MAX_DISPLAY_NAME_CHARS} characters`);
    }
    name = trimmed;
  }

  let cycleSettings = group.cycleSettings;
  if (request.cycleSettings) {
    const next = { ...group.cycleSettings, ...request.cycleSettings };
    if (
      next.questionsPerCycle < MIN_QUESTIONS_PER_CYCLE ||
      next.questionsPerCycle > MAX_QUESTIONS_PER_CYCLE
    ) {
      validationFailed(
        "questionsPerCycle",
        `questionsPerCycle must be between ${MIN_QUESTIONS_PER_CYCLE} and ${MAX_QUESTIONS_PER_CYCLE}`,
      );
    }
    if (
      next.responseWindowDays < MIN_RESPONSE_WINDOW_DAYS ||
      next.responseWindowDays > MAX_RESPONSE_WINDOW_DAYS
    ) {
      validationFailed(
        "responseWindowDays",
        `responseWindowDays must be between ${MIN_RESPONSE_WINDOW_DAYS} and ${MAX_RESPONSE_WINDOW_DAYS}`,
      );
    }
    if (next.votesPerUserPerCycle < 1 || next.votesPerUserPerCycle > next.questionsPerCycle) {
      validationFailed(
        "votesPerUserPerCycle",
        "votesPerUserPerCycle must be between 1 and questionsPerCycle",
      );
    }
    cycleSettings = next;
  }

  let notificationSettings = group.notificationSettings;
  if (request.notificationSettings) {
    const next = { ...group.notificationSettings, ...request.notificationSettings };
    const offsets = next.offsetsHoursBeforeClose;
    if (offsets.length === 0) {
      validationFailed("offsetsHoursBeforeClose", "offsetsHoursBeforeClose must not be empty");
    }
    if (offsets.some((h) => h < 1 || h > MAX_REMINDER_OFFSET_HOURS)) {
      validationFailed(
        "offsetsHoursBeforeClose",
        `offsetsHoursBeforeClose entries must be between 1 and ${MAX_REMINDER_OFFSET_HOURS}`,
      );
    }
    if (offsets.some((h, i) => i > 0 && h >= offsets[i - 1]!)) {
      validationFailed(
        "offsetsHoursBeforeClose",
        "offsetsHoursBeforeClose must be strictly descending",
      );
    }
    notificationSettings = next;
  }

  if (request.memberSoftCap !== undefined) {
    if (request.memberSoftCap < group.memberCount) {
      validationFailed(
        "memberSoftCap",
        `memberSoftCap cannot be below the current member count of ${group.memberCount}`,
      );
    }
  }

  group.name = name;
  if (request.timezone !== undefined) group.timezone = request.timezone;
  if (request.gradient !== undefined) group.gradient = request.gradient;
  group.cycleSettings = cycleSettings;
  group.notificationSettings = notificationSettings;
  if (request.memberSoftCap !== undefined) group.memberSoftCap = request.memberSoftCap;

  return { ...group, members: [...group.members] };
}

export function mockAdminRoute(request: MockRequest): unknown {
  const { method, segments, body } = request;

  if (
    segments.length === 2 &&
    segments[0] === "admin" &&
    segments[1] === "invites" &&
    method === "POST"
  ) {
    return handleCreateInvite(body);
  }
  if (
    segments.length === 4 &&
    segments[0] === "admin" &&
    segments[1] === "groups" &&
    segments[3] === "invites" &&
    method === "GET"
  ) {
    return handleListInvites(decodeURIComponent(segments[2]!));
  }
  if (
    segments.length === 4 &&
    segments[0] === "admin" &&
    segments[1] === "invites" &&
    segments[3] === "revoke" &&
    method === "POST"
  ) {
    return handleRevokeInvite(decodeURIComponent(segments[2]!));
  }
  if (
    segments.length === 4 &&
    segments[0] === "groups" &&
    segments[2] === "members" &&
    method === "PATCH"
  ) {
    return handlePatchMember(
      decodeURIComponent(segments[1]!),
      decodeURIComponent(segments[3]!),
      body,
    );
  }
  if (segments.length === 2 && segments[0] === "groups" && method === "PATCH") {
    return handlePatchGroup(decodeURIComponent(segments[1]!), body);
  }

  return NO_MOCK_ROUTE;
}
