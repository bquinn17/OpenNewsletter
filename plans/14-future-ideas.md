# 14 — Future Ideas

Ideas the owner wants kept but not built in v1. Nothing here is scheduled. Each entry says what it is, why it's deferred, and what it would touch, so a later milestone can pick it up without re-deriving the design. Owner decisions D11/D12 in `PROGRESS.md`.

---

## 1. Threaded (nested) comments

The original spec asked for "nested comments". v1 ships flat comments (`09-engagement.md` §1) because a small group's threads are short and flat lists are simpler to render on mobile.

If added:
- `Comment` gains `parent_comment_id` (null = top level). Keep one level of nesting (replies to a top-level comment), not arbitrary depth.
- Keys can stay as they are (`C#{created_at}#{comment_id}` in the answer's partition). The client groups replies under their parent, since a thread is at most a few dozen rows.
- Soft-deleting a parent keeps its `[deleted]` placeholder so replies stay anchored. That works because soft delete already exists.
- API: optional `parentCommentId` on `POST …/comments`; validate that the parent is in the same answer and is top-level.

## 2. Comments and reactions on poll questions

In v1, poll questions show only tallies, and engagement attaches to text answers only (`09-engagement.md` §3, `03-api-contract.md` §8).

If added:
- **Poll-level discussion.** A poll has no per-user answer worth commenting on, so comments and reactions would attach to the *question*. A new partition `GROUP#{g}#NL#{c}#Q#{q}#POLL` would mirror the per-answer comment/reaction rows.
- **Routes** nested as `…/questions/{questionId}/comments` and `…/questions/{questionId}/reactions/{emoji}`, with the same check order as `03` §8 (published, not archived, the question is a poll).
- **Optional "show who voted"**, per poll and opted into by the suggester at suggestion time. It's off by default, because `09` §3.5 promises aggregate-only tallies.

## 3. Ephemeral per-PR CI stacks

Moved here from `13-dev-environments.md` §11. Today `e2e.yml` runs against the long-lived `dev` stack, and `config.py` knows only `dev`/`prod`.

The design, if per-PR isolation becomes worth the cost:
1. A PR opens (or `e2e` is run manually), and the workflow assumes the deploy role via OIDC.
2. `cdk deploy --context env=ci-pr-{n} --all` creates the stacks. They reuse the persistent `dev` `AuthStack`, so Cognito isn't recreated.
3. Playwright runs against the new stack.
4. When the workflow finishes, pass or fail, `cdk destroy --context env=ci-pr-{n} --all --force` removes it.

Safety nets:
- **Sweeper Lambda**: runs daily and destroys any `ci-pr-*` stack older than 24h.
- **Budget**: the account-wide AWS Budget (`01-infrastructure-cdk.md` §9) catches stranded resources.

Prerequisites: `config.py` must accept `ci-pr-*` envs, and `ApiStack` and the media stacks must be able to point at an existing user pool. Each stack costs about $0.50, mostly CloudFront creation and deletion time rather than money.

## 4. Other deferred items (pointers)

Already designed elsewhere, listed here so this file is the one place to look:
- **Archival** of old editions: `10-archival.md`.
- **Inactive Cognito account cleanup**: `10-archival.md` §6.
- **Comment notifications** to the answer's author: `09-engagement.md` §1.6.
- **Unread tracking** on the newsletter list: `07-notifications.md` §14.
- **Offline draft background sync**, periodic prefetch and `share_target`: `04-frontend-architecture.md` §14.9.
- **Self-hosted fonts**, for offline use and to drop the Google Fonts CSP origins: `04-frontend-architecture.md` §12.1.
- **"Hype check" banner** and **recurring-question helper text**: `04-frontend-architecture.md` §14a.
- **Markdown formatting toolbar** in the response editor: `PROGRESS.md` Known gaps.
