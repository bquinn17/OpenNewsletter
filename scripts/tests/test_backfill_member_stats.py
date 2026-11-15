"""Unit tests for `scripts/backfill_member_stats.py`'s pure computation.

Nothing here touches AWS; boto3 is imported only inside the script's I/O
helpers (`boto3_resource`), so these tests exercise the pure fact types and
computation functions directly.

Run with: pytest scripts/tests
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))

from backfill_member_stats import (
    CommentFact,
    CycleFacts,
    LockedQuestionFact,
    ReactionFact,
    ResponseFact,
    accumulate_comment_counters,
    accumulate_question_counters,
    accumulate_reaction_counters,
    compute_member_stats,
    compute_response_updates,
    compute_streaks,
    reacted_gsi2sk,
    response_reaction_count,
)

U1, U2, U3, U4 = "user-1", "user-2", "user-3", "user-4"


def _text_response(
    *,
    question_id: str = "q1",
    response_id: str = "r1",
    author: str = U1,
    status: str = "published",
    reactions: tuple[ReactionFact, ...] = (),
    comments: tuple[CommentFact, ...] = (),
    had_reaction_count_attr: bool = False,
) -> ResponseFact:
    return ResponseFact(
        question_id=question_id,
        response_id=response_id,
        author_user_id=author,
        status=status,
        kind="text",
        reactions=reactions,
        comments=comments,
        had_reaction_count_attr=had_reaction_count_attr,
    )


# ---------------------------------------------------------------------------
# Self-reactions excluded
# ---------------------------------------------------------------------------


def test_response_reaction_count_excludes_self_reactions() -> None:
    resp = _text_response(
        author=U1,
        reactions=(
            ReactionFact(reactor_user_id=U1),  # self: excluded
            ReactionFact(reactor_user_id=U2),
            ReactionFact(reactor_user_id=U3),
        ),
    )
    assert response_reaction_count(resp) == 2


def test_accumulate_reaction_counters_excludes_self_reactions_both_sides() -> None:
    resp = _text_response(
        author=U1,
        reactions=(ReactionFact(reactor_user_id=U1), ReactionFact(reactor_user_id=U2)),
    )
    cycles = [CycleFacts(cycle_id="202601", responses=(resp,))]
    received, given = accumulate_reaction_counters(cycles)
    assert received == {U1: 1}
    assert given == {U2: 1}
    # The self-reactor (U1) gets no "given" credit for reacting to themselves.
    assert U1 not in given


def test_accumulate_reaction_counters_only_counts_published_text() -> None:
    draft = _text_response(
        status="draft", reactions=(ReactionFact(reactor_user_id=U2),)
    )
    poll = ResponseFact(
        question_id="q2",
        response_id="r2",
        author_user_id=U1,
        status="published",
        kind="poll",
        reactions=(ReactionFact(reactor_user_id=U2),),
    )
    cycles = [CycleFacts(cycle_id="202601", responses=(draft, poll))]
    received, given = accumulate_reaction_counters(cycles)
    assert received == {}
    assert given == {}


# ---------------------------------------------------------------------------
# Deleted comments excluded
# ---------------------------------------------------------------------------


def test_accumulate_comment_counters_excludes_deleted() -> None:
    resp = _text_response(
        comments=(
            CommentFact(author_user_id=U2, is_deleted=False),
            CommentFact(author_user_id=U2, is_deleted=True),
            CommentFact(author_user_id=U3, is_deleted=False),
        )
    )
    cycles = [CycleFacts(cycle_id="202601", responses=(resp,))]
    assert accumulate_comment_counters(cycles) == {U2: 1, U3: 1}


# ---------------------------------------------------------------------------
# Anonymous questions excluded
# ---------------------------------------------------------------------------


def test_accumulate_question_counters_excludes_anonymous_and_missing_submitter() -> (
    None
):
    cycles = [
        CycleFacts(
            cycle_id="202601",
            locked_questions=(
                LockedQuestionFact(
                    question_id="q1", submitted_by=U1, is_anonymous=False
                ),
                LockedQuestionFact(
                    question_id="q2", submitted_by=U1, is_anonymous=True
                ),
                LockedQuestionFact(
                    question_id="q3", submitted_by=None, is_anonymous=False
                ),
                LockedQuestionFact(
                    question_id="q4", submitted_by=U2, is_anonymous=False
                ),
            ),
        )
    ]
    assert accumulate_question_counters(cycles) == {U1: 1, U2: 1}


def test_an_open_cycle_counts_toward_questions_and_editions_only() -> None:
    # The live code counts promoted questions and an answer's first publish
    # before the cycle publishes, so a backfill run while a cycle is open
    # must count them too, or it drops them for good.
    published = [
        CycleFacts(
            cycle_id="202601",
            locked_questions=(
                LockedQuestionFact(
                    question_id="q1", submitted_by=U1, is_anonymous=False
                ),
            ),
            responses=(_text_response(author=U1),),
        )
    ]
    open_cycle = CycleFacts(
        cycle_id="202602",
        locked_questions=(
            LockedQuestionFact(question_id="q2", submitted_by=U1, is_anonymous=False),
            LockedQuestionFact(question_id="q3", submitted_by=U2, is_anonymous=True),
        ),
        responses=(
            _text_response(question_id="q2", author=U1),
            _text_response(
                question_id="q2", response_id="r2", author=U2, status="draft"
            ),
        ),
    )
    results = {
        r.user_id: r
        for r in compute_member_stats(published, [U1, U2], [U1, U2], [open_cycle])
    }
    assert results[U1].questions_asked == 2
    assert results[U2].questions_asked == 0
    assert results[U1].editions_answered == 2
    assert results[U1].last_answered_cycle == "202602"
    assert results[U2].editions_answered == 0, "a draft is not an answer"
    assert results[U2].last_answered_cycle is None
    # The open cycle settles no streak: U1's streak is the published one alone.
    assert results[U1].current_streak == 1
    assert results[U1].streak_cycle == "202601"


def test_editions_answered_counts_each_cycle_once_and_keeps_the_latest_marker() -> None:
    cycles = [
        CycleFacts(
            cycle_id="202601",
            responses=(
                _text_response(question_id="q1", author=U1),
                _text_response(question_id="q2", response_id="r2", author=U1),
            ),
        ),
        CycleFacts(cycle_id="202602", responses=()),
        CycleFacts(
            cycle_id="202603",
            responses=(
                ResponseFact(
                    question_id="q1",
                    response_id="r3",
                    author_user_id=U1,
                    status="published",
                    kind="poll",
                ),
            ),
        ),
    ]
    results = {r.user_id: r for r in compute_member_stats(cycles, [U1], [U1])}
    assert results[U1].editions_answered == 2
    assert results[U1].last_answered_cycle == "202603"


# ---------------------------------------------------------------------------
# Response row updates: write only when nonzero or correcting a stale count
# ---------------------------------------------------------------------------


def test_compute_response_updates_skips_zero_with_no_existing_attribute() -> None:
    resp = _text_response(reactions=())  # no reactions at all, no stale attribute
    cycles = [CycleFacts(cycle_id="202601", responses=(resp,))]
    assert compute_response_updates(cycles) == []


def test_compute_response_updates_corrects_stale_nonzero_down_to_zero() -> None:
    resp = _text_response(reactions=(), had_reaction_count_attr=True)
    cycles = [CycleFacts(cycle_id="202601", responses=(resp,))]
    updates = compute_response_updates(cycles)
    assert len(updates) == 1
    assert updates[0].reaction_count == 0


def test_compute_response_updates_writes_nonzero_counts() -> None:
    resp = _text_response(reactions=(ReactionFact(reactor_user_id=U2),))
    cycles = [CycleFacts(cycle_id="202601", responses=(resp,))]
    updates = compute_response_updates(cycles)
    assert len(updates) == 1
    assert updates[0].reaction_count == 1


def test_reacted_gsi2sk_padding_matches_candidate_gsi1sk_width() -> None:
    assert reacted_gsi2sk(7, "r1") == "000007#r1"
    assert reacted_gsi2sk(0, "r1") == "000000#r1"
    assert reacted_gsi2sk(9, "r1") < reacted_gsi2sk(10, "r1")
    assert reacted_gsi2sk(99, "r1") < reacted_gsi2sk(100, "r1")


# ---------------------------------------------------------------------------
# Streak walk
# ---------------------------------------------------------------------------


def _cycle(
    cycle_id: str, answered_by: tuple[str, ...], has_questions: bool = True
) -> CycleFacts:
    locked = (
        (LockedQuestionFact(question_id="q1", submitted_by=None, is_anonymous=False),)
        if (has_questions)
        else ()
    )
    responses = tuple(
        ResponseFact(
            question_id="q1",
            response_id=f"r-{cycle_id}-{u}",
            author_user_id=u,
            status="published",
            kind="text",
        )
        for u in answered_by
    )
    return CycleFacts(cycle_id=cycle_id, locked_questions=locked, responses=responses)


def test_streak_extends_while_answering_each_cycle() -> None:
    cycles = [
        _cycle("202601", answered_by=(U1,)),
        _cycle("202602", answered_by=(U1,)),
        _cycle("202603", answered_by=(U1,)),
    ]
    results = compute_streaks(cycles, universe_user_ids=[U1], current_member_ids=[U1])
    assert results[U1].current_streak == 3
    assert results[U1].longest_streak == 3
    assert results[U1].streak_cycle == "202603"


def test_streak_resets_on_a_cycle_with_no_answer() -> None:
    cycles = [
        _cycle("202601", answered_by=(U1,)),
        _cycle("202602", answered_by=()),  # has questions, U1 didn't answer
        _cycle("202603", answered_by=(U1,)),
    ]
    results = compute_streaks(cycles, universe_user_ids=[U1], current_member_ids=[U1])
    # Reset at 202602, then one fresh answer at 202603.
    assert results[U1].current_streak == 1
    assert results[U1].longest_streak == 1


def test_streak_skips_empty_editions_without_resetting_or_extending() -> None:
    cycles = [
        _cycle("202601", answered_by=(U1,)),
        _cycle("202602", answered_by=(), has_questions=False),  # empty edition: skipped
        _cycle("202603", answered_by=(U1,)),
    ]
    results = compute_streaks(cycles, universe_user_ids=[U1], current_member_ids=[U1])
    # The empty cycle is invisible to the walk, so the streak just keeps extending.
    assert results[U1].current_streak == 2
    assert results[U1].longest_streak == 2
    assert results[U1].streak_cycle == "202603"


def test_streak_longest_persists_through_a_later_reset() -> None:
    cycles = [
        _cycle("202601", answered_by=(U1,)),
        _cycle("202602", answered_by=(U1,)),  # current=2, longest=2
        _cycle("202603", answered_by=()),  # reset to 0
        _cycle("202604", answered_by=(U1,)),  # current=1, longest stays 2
    ]
    results = compute_streaks(cycles, universe_user_ids=[U1], current_member_ids=[U1])
    assert results[U1].current_streak == 1
    assert results[U1].longest_streak == 2


def test_non_current_member_gets_current_streak_zeroed_but_keeps_longest() -> None:
    cycles = [
        _cycle("202601", answered_by=(U1,)),
        _cycle("202602", answered_by=(U1,)),
    ]
    # U1 left the group: not in current_member_ids, but still in the universe
    # because they have an existing stats row.
    results = compute_streaks(cycles, universe_user_ids=[U1], current_member_ids=[])
    assert results[U1].current_streak == 0
    assert results[U1].longest_streak == 2
    assert results[U1].streak_cycle is None


def test_streak_with_no_published_cycles_is_all_zero() -> None:
    results = compute_streaks([], universe_user_ids=[U1], current_member_ids=[U1])
    assert results[U1].current_streak == 0
    assert results[U1].longest_streak == 0
    assert results[U1].streak_cycle is None


# ---------------------------------------------------------------------------
# Absolute values: every computed counter is a plain non-negative count
# ---------------------------------------------------------------------------


def test_all_computed_values_are_non_negative() -> None:
    resp = _text_response(
        author=U1,
        reactions=(ReactionFact(reactor_user_id=U1), ReactionFact(reactor_user_id=U2)),
        comments=(CommentFact(author_user_id=U2, is_deleted=False),),
    )
    cycles = [
        CycleFacts(
            cycle_id="202601",
            locked_questions=(
                LockedQuestionFact(
                    question_id="q1", submitted_by=U1, is_anonymous=False
                ),
            ),
            responses=(resp,),
        )
    ]
    results = compute_member_stats(
        cycles, universe_user_ids=[U1, U2, U4], current_member_ids=[U1, U2, U4]
    )
    for r in results:
        assert r.reactions_received >= 0
        assert r.reactions_given >= 0
        assert r.comments_written >= 0
        assert r.questions_asked >= 0
        assert r.current_streak >= 0
        assert r.longest_streak >= 0


def test_compute_member_stats_end_to_end_small_group() -> None:
    cycle1 = CycleFacts(
        cycle_id="202601",
        locked_questions=(
            LockedQuestionFact(question_id="q1", submitted_by=U1, is_anonymous=False),
        ),
        responses=(
            _text_response(
                question_id="q1",
                response_id="r1",
                author=U2,
                reactions=(
                    ReactionFact(reactor_user_id=U1),
                    ReactionFact(reactor_user_id=U2),
                ),
                comments=(CommentFact(author_user_id=U1, is_deleted=False),),
            ),
        ),
    )
    results = {
        r.user_id: r
        for r in compute_member_stats(
            [cycle1], universe_user_ids=[U1, U2], current_member_ids=[U1, U2]
        )
    }
    assert results[U2].reactions_received == 1  # U2's self-reaction excluded
    assert results[U1].reactions_given == 1
    assert results[U1].comments_written == 1
    assert results[U1].questions_asked == 1
    assert results[U2].questions_asked == 0
