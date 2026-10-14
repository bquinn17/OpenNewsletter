# Future ideas

Features the owner wants kept but that aren't built. Nothing here is scheduled. Each entry says what the feature is, why it's deferred and what it would touch, so it can be picked up later without re-deriving the design. Deferring the first three was decided in [D11](decisions.md#d11) and [D12](decisions.md#d12).

---

## 1. Threaded (nested) comments

The original spec asked for "nested comments". Comments are flat ([`engagement.md` §1](design/engagement.md#1-comments)) because a small group's threads are short and flat lists are simpler to render on mobile.

If added:
- `Comment` gains `parent_comment_id` (null = top level). Keep one level of nesting (replies to a top-level comment), not arbitrary depth.
- Keys can stay as they are (`C#{created_at}#{comment_id}` in the answer's partition). The client groups replies under their parent, since a thread is at most a few dozen rows.
- Soft-deleting a parent keeps its `[deleted]` placeholder so replies stay anchored. That works because soft delete already exists.
- API: optional `parentCommentId` on `POST …/comments`; validate that the parent is in the same answer and is top-level.

## 2. Comments and reactions on poll questions

Poll questions show only tallies, and engagement attaches to text answers only ([`engagement.md` §3](design/engagement.md#3-polls), [`api.md` §8](design/api.md#8-engagement-routes-comments--reactions)).

If added:
- **Poll-level discussion.** A poll has no per-user answer worth commenting on, so comments and reactions would attach to the *question*. A new partition `GROUP#{g}#NL#{c}#Q#{q}#POLL` would mirror the per-answer comment/reaction rows.
- **Routes** nested as `…/questions/{questionId}/comments` and `…/questions/{questionId}/reactions/{emoji}`, with the same check order as [`api.md` §8](design/api.md#8-engagement-routes-comments--reactions) (published, not archived, the question is a poll).
- **Optional "show who voted"**, per poll and opted into by the suggester at suggestion time. It's off by default, because [`engagement.md` §3.5](design/engagement.md#35-anonymity) promises aggregate-only tallies.

## 3. Ephemeral per-PR CI stacks

Today `e2e.yml` runs against the long-lived `dev` stack ([`environments.md` §11](development/environments.md#11-ephemeral-ci-stacks)), and `config.py` knows only `dev` and `prod`.

The design, if per-PR isolation becomes worth the cost:
1. A PR opens (or `e2e` is run manually), and the workflow assumes the deploy role via OIDC.
2. `cdk deploy --context env=ci-pr-{n} --all` creates the stacks. They reuse the persistent `dev` `AuthStack`, so Cognito isn't recreated.
3. Playwright runs against the new stack.
4. When the workflow finishes, pass or fail, `cdk destroy --context env=ci-pr-{n} --all --force` removes it.

Safety nets:
- **Sweeper Lambda**: runs daily and destroys any `ci-pr-*` stack older than 24h.
- **Budget**: the account-wide AWS Budget ([`infrastructure.md` §9](design/infrastructure.md#9-monitoringstack)) catches stranded resources.

Prerequisites: `config.py` must accept `ci-pr-*` envs, and `ApiStack` and the media stacks must be able to point at an existing user pool. Each stack costs about $0.50, mostly CloudFront creation and deletion time rather than money.

## 4. Other deferred items (pointers)

Already designed elsewhere, listed here so this file is the one place to look:
- **Archival** of old editions: [`archival.md`](design/archival.md).
- **Inactive Cognito account cleanup**: [`archival.md` §6](design/archival.md#6-inactive-account-cleanup-separate-but-related).
- **Comment notifications** to the answer's author: [`engagement.md` §1.6](design/engagement.md#16-notifications-on-comments).
- **Unread tracking** on the newsletter list: [`notifications.md` §14](design/notifications.md#14-failure-modes--ux).
- **Offline draft background sync**, periodic prefetch and `share_target`: [`frontend.md` §14.9](design/frontend.md#149-not-built).
- **Self-hosted fonts**, for offline use and to drop the Google Fonts CSP origins: [`frontend.md` §12.1](design/frontend.md#121-content-security-policy).
- **"Hype check" banner** and **recurring-question helper text**: [`frontend.md` §14a](design/frontend.md#14a-client-derived-display-data-not-api-fields).
- **Markdown formatting toolbar** in the response editor. The editor has Write and Preview tabs only; the gap is tracked in [`PROGRESS.md`](../plans/PROGRESS.md) under Known gaps.
