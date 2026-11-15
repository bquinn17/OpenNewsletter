#!/usr/bin/env python3
"""Recompute and rewrite every group's MemberStats rows from scratch.

One-off, rerunnable repair tool (`docs/design/data-model.md` §2.17 MemberStats;
write semantics in §4 #3, #6, #8, #9 and `docs/design/lifecycle.md` §5.3). It writes ABSOLUTE
values with plain `SET` (never `ADD`), so running it twice — or a hundred
times — is harmless: it always lands on the same numbers computed fresh from
the source rows (responses, reactions, comments, locked questions).

What it recomputes, per group, walking that group's `published` cycles in
chronological (`cycle_id`) order:
  - `reaction_count` (+ the `gsi2pk`/`gsi2sk` pair) on every published TEXT
    Response row, excluding self-reactions (reactor == author).
  - `reactions_received` / `reactions_given` per user, same self-exclusion.
  - `comments_written` per user, excluding soft-deleted comments.
  - `editions_answered` per user (editions with at least one published
    answer of theirs, text or poll) and the `last_answered_cycle` marker.
  - `questions_asked` per user, excluding anonymous candidates (and any
    locked question with no `submitted_by`).

An open cycle counts toward `editions_answered` and `questions_asked` too,
since the live code counts both before publication (at an answer's first
publish, and at promotion).
  - `current_streak` / `longest_streak` / `streak_cycle`, walking published
    cycles in order and skipping cycles with no locked questions (an empty
    edition neither extends nor breaks a streak).

Rows are only written for "every user ever seen in the group": current
members, plus anyone who already has a MemberStats row (so a departed
member's historical row still gets corrected, but no new row is created for
a departed member with no existing row — the leaderboard read never shows
them anyway).

Usage:
    python3 scripts/backfill_member_stats.py [--env dev]
        [--group-id GROUP_ID]    # limit to one group; default: all groups
        [--dry-run]              # print what would be written, write nothing

Requires: boto3 installed, AWS credentials configured for the target account.
Reads:    infra/cdk.out/{env}-outputs.json  (written by 'cdk deploy --outputs-file').
"""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Iterable, Iterator

REPO_ROOT = Path(__file__).parent.parent

# Mirrors `persistence::keys::candidate_gsi1sk`'s padding
# (`backend/crates/shared/src/config.rs::VOTE_COUNT_PAD_WIDTH`), reused for
# `Response.gsi2sk` per `docs/design/data-model.md` §2.9.
REACTION_COUNT_PAD_WIDTH = 6

STATS_ENTITY = "MemberStats"


# ---------------------------------------------------------------------------
# Key builders — mirror `persistence::keys` / `persistence::member_stats`
# ---------------------------------------------------------------------------


def member_stats_pk(group_id: str) -> str:
    return f"GROUP#{group_id}#STATS"


MEMBER_STATS_SK_PREFIX = "MEMBER#"


def member_stats_sk(user_id: str) -> str:
    return f"{MEMBER_STATS_SK_PREFIX}{user_id}"


def group_pk(group_id: str) -> str:
    return f"GROUP#{group_id}"


def newsletter_sk(cycle_id: str) -> str:
    return f"NL#{cycle_id}"


def locked_pk(group_id: str, cycle_id: str) -> str:
    return f"GROUP#{group_id}#NL#{cycle_id}"


def response_pk(group_id: str, cycle_id: str, question_id: str) -> str:
    return f"GROUP#{group_id}#NL#{cycle_id}#Q#{question_id}"


def response_sk(user_id: str) -> str:
    return f"A#{user_id}"


def engagement_pk(
    group_id: str, cycle_id: str, question_id: str, answer_user_id: str
) -> str:
    return f"GROUP#{group_id}#NL#{cycle_id}#Q#{question_id}#A#{answer_user_id}"


def reacted_gsi2pk(group_id: str) -> str:
    return f"GROUP#{group_id}#REACTED"


def reacted_gsi2sk(reaction_count: int, response_id: str) -> str:
    """Pads `reaction_count` the same way `candidate_gsi1sk` pads vote counts."""
    return f"{reaction_count:0{REACTION_COUNT_PAD_WIDTH}d}#{response_id}"


def membership_gsi1pk(group_id: str) -> str:
    return f"GROUP#{group_id}"


MEMBERSHIP_GSI1SK_PREFIX = "MEMBER#"


# ---------------------------------------------------------------------------
# Pure fact types + computation (no boto3 here — unit-tested directly)
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class LockedQuestionFact:
    question_id: str
    submitted_by: str | None
    is_anonymous: bool


@dataclass(frozen=True)
class ReactionFact:
    reactor_user_id: str


@dataclass(frozen=True)
class CommentFact:
    author_user_id: str
    is_deleted: bool


@dataclass(frozen=True)
class ResponseFact:
    question_id: str
    response_id: str
    author_user_id: str
    status: str  # "draft" | "published"
    kind: str  # "text" | "poll"
    reactions: tuple[ReactionFact, ...] = field(default_factory=tuple)
    comments: tuple[CommentFact, ...] = field(default_factory=tuple)
    # Whether the Response row, as currently stored, already has a
    # `reaction_count` attribute (so a count that recomputes to 0 still gets
    # corrected down rather than left stale).
    had_reaction_count_attr: bool = False


@dataclass(frozen=True)
class CycleFacts:
    cycle_id: str
    locked_questions: tuple[LockedQuestionFact, ...] = field(default_factory=tuple)
    responses: tuple[ResponseFact, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class ResponseUpdate:
    """What to write back onto one Response row (only emitted when the
    recomputed count is nonzero, or the row already carried a nonzero/zero
    `reaction_count` attribute that needs correcting)."""

    cycle_id: str
    question_id: str
    author_user_id: str
    response_id: str
    reaction_count: int


@dataclass(frozen=True)
class StreakResult:
    current_streak: int
    longest_streak: int
    streak_cycle: str | None  # only ever set for a current member


@dataclass(frozen=True)
class MemberStatsResult:
    user_id: str
    editions_answered: int
    last_answered_cycle: str | None
    reactions_received: int
    reactions_given: int
    comments_written: int
    questions_asked: int
    current_streak: int
    longest_streak: int
    streak_cycle: str | None


def published_text_responses(responses: Iterable[ResponseFact]) -> list[ResponseFact]:
    return [r for r in responses if r.status == "published" and r.kind == "text"]


def response_reaction_count(response: ResponseFact) -> int:
    """Reactions on this answer excluding self-reactions (reactor == author)."""
    return sum(
        1 for r in response.reactions if r.reactor_user_id != response.author_user_id
    )


def compute_response_updates(cycles: Iterable[CycleFacts]) -> list[ResponseUpdate]:
    """One update per published TEXT response whose row needs a write: only
    when the recomputed count is > 0, or the row already has a
    `reaction_count` attribute (so a stale nonzero count gets corrected to 0)."""
    updates: list[ResponseUpdate] = []
    for cycle in cycles:
        for resp in published_text_responses(cycle.responses):
            count = response_reaction_count(resp)
            if count > 0 or resp.had_reaction_count_attr:
                updates.append(
                    ResponseUpdate(
                        cycle_id=cycle.cycle_id,
                        question_id=resp.question_id,
                        author_user_id=resp.author_user_id,
                        response_id=resp.response_id,
                        reaction_count=count,
                    )
                )
    return updates


def accumulate_reaction_counters(
    cycles: Iterable[CycleFacts],
) -> tuple[dict[str, int], dict[str, int]]:
    """`reactions_received` and `reactions_given` per user, self-reactions
    excluded, across every published TEXT response in the given cycles."""
    received: dict[str, int] = {}
    given: dict[str, int] = {}
    for cycle in cycles:
        for resp in published_text_responses(cycle.responses):
            for reaction in resp.reactions:
                if reaction.reactor_user_id == resp.author_user_id:
                    continue  # self-reactions count nowhere
                received[resp.author_user_id] = received.get(resp.author_user_id, 0) + 1
                given[reaction.reactor_user_id] = (
                    given.get(reaction.reactor_user_id, 0) + 1
                )
    return received, given


def accumulate_comment_counters(cycles: Iterable[CycleFacts]) -> dict[str, int]:
    """`comments_written` per user, excluding soft-deleted comments."""
    written: dict[str, int] = {}
    for cycle in cycles:
        for resp in cycle.responses:
            for comment in resp.comments:
                if comment.is_deleted:
                    continue
                written[comment.author_user_id] = (
                    written.get(comment.author_user_id, 0) + 1
                )
    return written


def accumulate_question_counters(cycles: Iterable[CycleFacts]) -> dict[str, int]:
    """`questions_asked` per user, excluding anonymous questions and any
    locked question missing `submitted_by`."""
    asked: dict[str, int] = {}
    for cycle in cycles:
        for q in cycle.locked_questions:
            if q.is_anonymous or not q.submitted_by:
                continue
            asked[q.submitted_by] = asked.get(q.submitted_by, 0) + 1
    return asked


def accumulate_editions_answered(
    cycles: Iterable[CycleFacts],
) -> tuple[dict[str, int], dict[str, str]]:
    """`editions_answered` per user (cycles in which they published at least
    one answer, text or poll) and `last_answered_cycle`, the latest such cycle.
    `cycles` must be chronological, as the live bump's marker is the most
    recent cycle it counted."""
    answered: dict[str, int] = {}
    last: dict[str, str] = {}
    for cycle in cycles:
        for user in {
            r.author_user_id for r in cycle.responses if r.status == "published"
        }:
            answered[user] = answered.get(user, 0) + 1
            last[user] = cycle.cycle_id
    return answered, last


def compute_streaks(
    cycles: list[CycleFacts],
    universe_user_ids: Iterable[str],
    current_member_ids: Iterable[str],
) -> dict[str, StreakResult]:
    """Walks `cycles` in the given (chronological) order, skipping any cycle
    with no locked questions — an empty edition neither extends nor breaks a
    streak. Every user in `universe_user_ids` either extends their streak
    (answered, text or poll, published) or resets to 0 (didn't)."""
    universe = list(universe_user_ids)
    members = set(current_member_ids)
    current = dict.fromkeys(universe, 0)
    longest = dict.fromkeys(universe, 0)
    last_nonempty_cycle_id: str | None = None

    for cycle in cycles:
        if not cycle.locked_questions:
            continue  # empty edition: skip entirely
        last_nonempty_cycle_id = cycle.cycle_id
        answered = {
            r.author_user_id for r in cycle.responses if r.status == "published"
        }
        for user in universe:
            if user in answered:
                current[user] += 1
                longest[user] = max(longest[user], current[user])
            else:
                current[user] = 0

    results: dict[str, StreakResult] = {}
    for user in universe:
        is_member = user in members
        results[user] = StreakResult(
            current_streak=current[user] if is_member else 0,
            longest_streak=longest[user],
            streak_cycle=last_nonempty_cycle_id if is_member else None,
        )
    return results


def compute_member_stats(
    cycles: list[CycleFacts],
    universe_user_ids: Iterable[str],
    current_member_ids: Iterable[str],
    open_cycles: Iterable[CycleFacts] = (),
) -> list[MemberStatsResult]:
    """The full per-user MemberStats recompute for one group: every counter
    plus the streak fields, absolute (not deltas). `cycles` are the published
    ones, chronological. `open_cycles` (open but not yet published) count only
    where the live code already counted them: `questions_asked`, bumped at
    promotion, and `editions_answered`, bumped at an answer's first publish.
    An open cycle has no reactions or comments yet and settles no streak."""
    received, given = accumulate_reaction_counters(cycles)
    comments_written = accumulate_comment_counters(cycles)
    counted = [*cycles, *open_cycles]
    questions_asked = accumulate_question_counters(counted)
    editions_answered, last_answered_cycle = accumulate_editions_answered(counted)
    streaks = compute_streaks(cycles, universe_user_ids, current_member_ids)

    results = []
    for user in universe_user_ids:
        streak = streaks[user]
        results.append(
            MemberStatsResult(
                user_id=user,
                editions_answered=editions_answered.get(user, 0),
                last_answered_cycle=last_answered_cycle.get(user),
                reactions_received=received.get(user, 0),
                reactions_given=given.get(user, 0),
                comments_written=comments_written.get(user, 0),
                questions_asked=questions_asked.get(user, 0),
                current_streak=streak.current_streak,
                longest_streak=streak.longest_streak,
                streak_cycle=streak.streak_cycle,
            )
        )
    return results


# ---------------------------------------------------------------------------
# DynamoDB I/O (kept separate from the pure computation above)
# ---------------------------------------------------------------------------


def load_outputs(env: str) -> dict[str, Any]:
    path = REPO_ROOT / "infra" / "cdk.out" / f"{env}-outputs.json"
    if not path.exists():
        sys.exit(
            f"Outputs file not found: {path}\n"
            f"Run 'cd infra && cdk deploy --context env={env} --all "
            f"--outputs-file cdk.out/{env}-outputs.json' first."
        )
    with path.open() as f:
        outputs: dict[str, Any] = json.load(f)
    return outputs


def get_table_name(outputs: dict[str, Any], env: str) -> str:
    val = outputs.get(f"DataStack-{env}", {}).get("TableName", "")
    if not val:
        sys.exit(
            f"Missing CDK output DataStack-{env}.TableName — run 'make deploy-dev' first."
        )
    return str(val)


def boto3_resource() -> Any:
    # Imported here (not at module top) so the pure functions above can be
    # unit-tested without boto3 installed, mirroring `scripts/bootstrap_admin.py`.
    try:
        import boto3  # type: ignore[import-untyped]
    except ImportError:
        sys.exit(
            "boto3 not installed. Run: infra/.venv/bin/pip install -r scripts/requirements.txt"
        )
    return boto3.resource("dynamodb", region_name="us-east-1")


def paginate_query(table: Any, **kwargs: Any) -> Iterator[dict[str, Any]]:
    while True:
        resp = table.query(**kwargs)
        yield from resp.get("Items", [])
        if "LastEvaluatedKey" not in resp:
            return
        kwargs["ExclusiveStartKey"] = resp["LastEvaluatedKey"]


def paginate_scan(table: Any, **kwargs: Any) -> Iterator[dict[str, Any]]:
    while True:
        resp = table.scan(**kwargs)
        yield from resp.get("Items", [])
        if "LastEvaluatedKey" not in resp:
            return
        kwargs["ExclusiveStartKey"] = resp["LastEvaluatedKey"]


def list_group_ids(table: Any) -> list[str]:
    """All `Group` rows in the table (a full-table Scan: there is no index
    that lists every group directly)."""
    items = paginate_scan(
        table,
        FilterExpression="entity = :e AND sk = :sk",
        ExpressionAttributeValues={":e": "Group", ":sk": "META"},
        ProjectionExpression="group_id",
    )
    return [str(item["group_id"]) for item in items]


def list_current_member_ids(table: Any, group_id: str) -> set[str]:
    items = paginate_query(
        table,
        IndexName="gsi1",
        KeyConditionExpression="gsi1pk = :pk AND begins_with(gsi1sk, :prefix)",
        ExpressionAttributeValues={
            ":pk": membership_gsi1pk(group_id),
            ":prefix": MEMBERSHIP_GSI1SK_PREFIX,
        },
        ProjectionExpression="user_id",
    )
    return {str(item["user_id"]) for item in items}


def list_existing_stats_user_ids(table: Any, group_id: str) -> set[str]:
    items = paginate_query(
        table,
        KeyConditionExpression="pk = :pk AND begins_with(sk, :prefix)",
        ExpressionAttributeValues={
            ":pk": member_stats_pk(group_id),
            ":prefix": MEMBER_STATS_SK_PREFIX,
        },
        ProjectionExpression="user_id",
    )
    return {str(item["user_id"]) for item in items}


def list_cycle_ids(table: Any, group_id: str, status: str) -> list[str]:
    """This group's cycles in `status`, chronological: monthly `yyyymm` and
    weekly `yyyymmdd` ids sort correctly together, so ascending sk order is
    already chronological (`docs/design/lifecycle.md` §13)."""
    items = paginate_query(
        table,
        KeyConditionExpression="pk = :pk AND begins_with(sk, :prefix)",
        FilterExpression="#s = :status",
        ExpressionAttributeNames={"#s": "status"},
        ExpressionAttributeValues={
            ":pk": group_pk(group_id),
            ":prefix": "NL#",
            ":status": status,
        },
        ProjectionExpression="cycle_id",
        ScanIndexForward=True,
    )
    return [str(item["cycle_id"]) for item in items]


def fetch_locked_questions(
    table: Any, group_id: str, cycle_id: str
) -> list[LockedQuestionFact]:
    items = paginate_query(
        table,
        KeyConditionExpression="pk = :pk AND begins_with(sk, :prefix)",
        ExpressionAttributeValues={
            ":pk": locked_pk(group_id, cycle_id),
            ":prefix": "Q#",
        },
    )
    return [
        LockedQuestionFact(
            question_id=str(item["question_id"]),
            submitted_by=(
                str(item["submitted_by"]) if item.get("submitted_by") else None
            ),
            is_anonymous=bool(item.get("is_anonymous", False)),
        )
        for item in items
    ]


def fetch_responses(
    table: Any, group_id: str, cycle_id: str, question_id: str
) -> list[dict[str, Any]]:
    return list(
        paginate_query(
            table,
            KeyConditionExpression="pk = :pk AND begins_with(sk, :prefix)",
            ExpressionAttributeValues={
                ":pk": response_pk(group_id, cycle_id, question_id),
                ":prefix": "A#",
            },
        )
    )


def fetch_engagement(
    table: Any, group_id: str, cycle_id: str, question_id: str, answer_user_id: str
) -> tuple[list[ReactionFact], list[CommentFact]]:
    items = list(
        paginate_query(
            table,
            KeyConditionExpression="pk = :pk",
            ExpressionAttributeValues={
                ":pk": engagement_pk(group_id, cycle_id, question_id, answer_user_id)
            },
        )
    )
    reactions = [
        ReactionFact(reactor_user_id=str(item["reactor_user_id"]))
        for item in items
        if item.get("entity") == "Reaction"
    ]
    comments = [
        CommentFact(
            author_user_id=str(item["author_user_id"]),
            is_deleted=item.get("deleted_at") is not None,
        )
        for item in items
        if item.get("entity") == "Comment"
    ]
    return reactions, comments


def fetch_group_cycles(
    table: Any, group_id: str, cycle_status: str
) -> list[CycleFacts]:
    """The group's cycles in `status`, chronological. Reactions and comments
    exist only on published cycles, so only those read engagement rows."""
    cycles: list[CycleFacts] = []
    for cycle_id in list_cycle_ids(table, group_id, cycle_status):
        locked_questions = fetch_locked_questions(table, group_id, cycle_id)
        responses: list[ResponseFact] = []
        for q in locked_questions:
            for raw in fetch_responses(table, group_id, cycle_id, q.question_id):
                status = str(raw.get("status", ""))
                kind = str(raw.get("kind", ""))
                reactions: list[ReactionFact] = []
                comments: list[CommentFact] = []
                if (
                    status == "published"
                    and kind == "text"
                    and cycle_status == "published"
                ):
                    reactions, comments = fetch_engagement(
                        table, group_id, cycle_id, q.question_id, str(raw["user_id"])
                    )
                responses.append(
                    ResponseFact(
                        question_id=q.question_id,
                        response_id=str(raw.get("response_id", "")),
                        author_user_id=str(raw["user_id"]),
                        status=status,
                        kind=kind,
                        reactions=tuple(reactions),
                        comments=tuple(comments),
                        had_reaction_count_attr="reaction_count" in raw,
                    )
                )
        cycles.append(
            CycleFacts(
                cycle_id=cycle_id,
                locked_questions=tuple(locked_questions),
                responses=tuple(responses),
            )
        )
    return cycles


def write_member_stats(
    table: Any, group_id: str, result: MemberStatsResult, *, dry_run: bool
) -> None:
    item = {
        "pk": member_stats_pk(group_id),
        "sk": member_stats_sk(result.user_id),
        "entity": STATS_ENTITY,
        "group_id": group_id,
        "user_id": result.user_id,
        "editions_answered": result.editions_answered,
        "reactions_received": result.reactions_received,
        "reactions_given": result.reactions_given,
        "comments_written": result.comments_written,
        "questions_asked": result.questions_asked,
        "current_streak": result.current_streak,
        "longest_streak": result.longest_streak,
    }
    if result.streak_cycle is not None:
        item["streak_cycle"] = result.streak_cycle
    # The live bump's "already counted this cycle" marker (§4 #4): without it,
    # a later publish in the cycle it names would count that cycle twice.
    if result.last_answered_cycle is not None:
        item["last_answered_cycle"] = result.last_answered_cycle
    if dry_run:
        print(f"  [dry-run] MemberStats {item['sk']}: {item}")
        return
    table.put_item(Item=item)


def write_response_update(
    table: Any, group_id: str, update: ResponseUpdate, *, dry_run: bool
) -> None:
    key = {
        "pk": response_pk(group_id, update.cycle_id, update.question_id),
        "sk": response_sk(update.author_user_id),
    }
    if dry_run:
        print(
            f"  [dry-run] Response {key['pk']}/{key['sk']}: "
            f"reaction_count={update.reaction_count}"
        )
        return
    update_expr = "SET reaction_count = :c, gsi2pk = :pk2, gsi2sk = :sk2"
    table.update_item(
        Key=key,
        UpdateExpression=update_expr,
        ExpressionAttributeValues={
            ":c": update.reaction_count,
            ":pk2": reacted_gsi2pk(group_id),
            ":sk2": reacted_gsi2sk(update.reaction_count, update.response_id),
        },
    )


def backfill_group(table: Any, group_id: str, *, dry_run: bool) -> None:
    print(f"\n=== Group {group_id} ===")
    current_members = list_current_member_ids(table, group_id)
    existing_stats_users = list_existing_stats_user_ids(table, group_id)
    universe = current_members | existing_stats_users

    cycles = fetch_group_cycles(table, group_id, "published")
    open_cycles = fetch_group_cycles(table, group_id, "open")
    print(f"  {len(cycles)} published cycle(s), {len(universe)} user(s) in scope")

    response_updates = compute_response_updates(cycles)
    for update in response_updates:
        write_response_update(table, group_id, update, dry_run=dry_run)
    print(f"  {len(response_updates)} Response row(s) updated")

    results = compute_member_stats(cycles, universe, current_members, open_cycles)
    for result in results:
        write_member_stats(table, group_id, result, dry_run=dry_run)
    print(f"  {len(results)} MemberStats row(s) written")


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------


def main() -> None:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "--env", default="dev", help="CDK environment name (default: dev)"
    )
    parser.add_argument(
        "--group-id", help="Limit the backfill to one group (default: every group)"
    )
    parser.add_argument(
        "--dry-run", action="store_true", help="Print planned writes; write nothing"
    )
    args = parser.parse_args()

    outputs = load_outputs(args.env)
    table_name = get_table_name(outputs, args.env)

    resource = boto3_resource()
    table = resource.Table(table_name)

    print(f"=== Backfilling MemberStats in '{args.env}' ({table_name}) ===")
    if args.dry_run:
        print("(dry run: no writes will be made)")

    group_ids = [args.group_id] if args.group_id else list_group_ids(table)
    if not group_ids:
        print("No groups found.")
        return

    for group_id in group_ids:
        backfill_group(table, group_id, dry_run=args.dry_run)

    print("\nDone.")


if __name__ == "__main__":
    main()
