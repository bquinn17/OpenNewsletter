//! Business logic for candidate-question and vote routes
//! (`plans/03-api-contract.md` §6).

use crate::state::AppState;
use crate::validation;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use chrono::Utc;
use domain::api::{
    AskedBy, CandidateItemResponse, CandidateListResponse, CandidateVoteResponse,
    CreateCandidateRequest, PollOptionResponse,
};
use domain::{
    ApiError, ApiErrorCode, CandidateQuestion, CandidateVote, CycleId, GroupId, QuestionId, Role,
    User, UserId,
};
use persistence::{auth, groups, newsletters, questions, users};
use shared::config::MAX_LIST_LIMIT;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    Top,
    Recent,
}

impl Sort {
    /// Default (when `sort` is omitted) is `top` (`03-api-contract.md` §6.1).
    pub fn parse(raw: Option<&str>) -> Result<Self, ApiError> {
        match raw {
            None | Some("top") => Ok(Sort::Top),
            Some("recent") => Ok(Sort::Recent),
            Some(other) => Err(ApiError::validation(format!(
                "sort must be `top` or `recent`, got `{other}`"
            ))),
        }
    }
}

/// `GET /groups/{groupId}/candidate-questions` (§6.1). The candidate pool is
/// bounded by the group's member count, so this fetches it in full via AP10 and
/// sorts/pages it in memory rather than threading AP11's GSI1 cursor.
pub async fn list_candidates(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    sort: Sort,
    limit: u32,
    cursor: Option<&str>,
) -> Result<CandidateListResponse, ApiError> {
    let membership = auth::require_membership(&state.repo, caller, group_id, false).await?;
    let group = load_group(state, group_id).await?;
    let next_cycle = require_voting_cycle(state, group_id).await?;

    let mut pool = questions::list_candidates(&state.repo, group_id, &next_cycle.cycle_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "candidate listing failed");
            ApiError::internal("failed to list candidate questions")
        })?;

    match sort {
        Sort::Top => pool.sort_by(|a, b| {
            b.vote_count
                .cmp(&a.vote_count)
                .then_with(|| a.submitted_at.cmp(&b.submitted_at))
        }),
        Sort::Recent => pool.sort_by_key(|c| std::cmp::Reverse(c.submitted_at)),
    }

    let offset = cursor.map(decode_offset).transpose()?.unwrap_or(0);
    let limit = limit.clamp(1, MAX_LIST_LIMIT) as usize;
    let total = pool.len();
    let page: Vec<CandidateQuestion> = pool.into_iter().skip(offset).take(limit).collect();
    let next_cursor = if offset + page.len() < total {
        Some(encode_offset(offset + page.len()))
    } else {
        None
    };

    let my_votes = list_my_votes(state, group_id, &next_cycle.cycle_id, caller).await?;
    let voted_ids: HashSet<QuestionId> = my_votes.iter().map(|v| v.question_id.clone()).collect();
    let my_vote_count = my_votes.len() as u32;

    let is_admin = membership.role == Role::Admin;
    let profiles = load_submitter_profiles(state, &page).await?;

    let items = page
        .into_iter()
        .map(|c| {
            let voted_by_me = voted_ids.contains(&c.question_id);
            to_candidate_item(state, caller, is_admin, &profiles, c, voted_by_me)
        })
        .collect();

    Ok(CandidateListResponse {
        next_cycle_id: next_cycle.cycle_id,
        votes_per_user_per_cycle: group.cycle_settings.votes_per_user_per_cycle,
        my_vote_count,
        items,
        next_cursor,
    })
}

/// `POST /groups/{groupId}/candidate-questions` (§6.2).
pub async fn create_candidate(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    request: CreateCandidateRequest,
) -> Result<CandidateItemResponse, ApiError> {
    auth::require_membership(&state.repo, caller, group_id, false).await?;
    let next_cycle = require_voting_cycle(state, group_id).await?;

    let prompt = validation::prompt(&request.prompt)?;
    let poll_options = validation::poll_options(request.kind, request.poll_options.as_deref())?;

    let candidate = CandidateQuestion {
        question_id: QuestionId::generate(),
        group_id: group_id.clone(),
        next_cycle_id: next_cycle.cycle_id,
        kind: request.kind,
        prompt,
        poll_options,
        vote_count: 0,
        submitted_by: caller.clone(),
        is_anonymous: request.is_anonymous,
        submitted_at: Utc::now(),
    };

    questions::put_candidate(&state.repo, &candidate)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "candidate creation failed");
            ApiError::internal("failed to create candidate question")
        })?;

    // The submitter always sees their own attribution, even when isAnonymous=true (§6.2).
    let submitter = load_user(state, caller).await?;
    let mut profiles = HashMap::with_capacity(1);
    profiles.insert(caller.clone(), submitter);
    Ok(to_candidate_item(
        state, caller, false, &profiles, candidate, false,
    ))
}

/// `POST .../votes` (§6.3). Idempotent — re-voting returns the current state.
pub async fn cast_vote(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    question_id: &QuestionId,
) -> Result<CandidateVoteResponse, ApiError> {
    auth::require_membership(&state.repo, caller, group_id, false).await?;
    let group = load_group(state, group_id).await?;
    let next_cycle = require_voting_cycle(state, group_id).await?;
    let candidate = load_candidate(state, group_id, &next_cycle.cycle_id, question_id).await?;

    let my_votes = list_my_votes(state, group_id, &next_cycle.cycle_id, caller).await?;
    if my_votes.iter().any(|v| v.question_id == *question_id) {
        return Ok(CandidateVoteResponse {
            question_id: question_id.clone(),
            vote_count: candidate.vote_count,
            voted_by_me: true,
            my_vote_count: my_votes.len() as u32,
        });
    }

    if my_votes.len() as u32 >= group.cycle_settings.votes_per_user_per_cycle {
        let already: Vec<String> = my_votes.iter().map(|v| v.question_id.to_string()).collect();
        return Err(ApiError::new(
            ApiErrorCode::VoteCapReached,
            format!(
                "no votes remaining this cycle; already voted for: {}",
                already.join(", ")
            ),
        ));
    }

    let vote = CandidateVote {
        user_id: caller.clone(),
        group_id: group_id.clone(),
        cycle_id: next_cycle.cycle_id.clone(),
        question_id: question_id.clone(),
        voted_at: Utc::now(),
    };
    let vote_count =
        cast_vote_with_retry(state, group_id, &next_cycle.cycle_id, &vote, candidate).await?;

    Ok(CandidateVoteResponse {
        question_id: question_id.clone(),
        vote_count,
        voted_by_me: true,
        my_vote_count: my_votes.len() as u32 + 1,
    })
}

/// One retry against a freshly-reloaded vote count: `cast_vote_tx`'s condition
/// fails when another request changed `voteCount` between our read and the
/// write, which — at this app's scale — is a rare double-click, not a hot path
/// worth more than a single retry.
async fn cast_vote_with_retry(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
    vote: &CandidateVote,
    mut candidate: CandidateQuestion,
) -> Result<u32, ApiError> {
    for attempt in 0..2 {
        let new_count = candidate.vote_count + 1;
        match questions::cast_vote_tx(&state.repo, vote, new_count).await {
            Ok(()) => return Ok(new_count),
            Err(e) if e.is_lost_race() && attempt == 0 => {
                candidate = load_candidate(state, group_id, cycle_id, &vote.question_id).await?;
            }
            Err(e) => {
                tracing::error!(error = ?e, group_id = %group_id, question_id = %vote.question_id, "cast vote failed");
                return Err(ApiError::internal("failed to cast vote"));
            }
        }
    }
    unreachable!("the loop above always returns within two attempts")
}

/// `DELETE .../votes` (§6.4). Idempotent — withdrawing a nonexistent vote
/// returns the current state rather than an error.
pub async fn withdraw_vote(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    question_id: &QuestionId,
) -> Result<CandidateVoteResponse, ApiError> {
    auth::require_membership(&state.repo, caller, group_id, false).await?;
    let next_cycle = require_voting_cycle(state, group_id).await?;
    let candidate = load_candidate(state, group_id, &next_cycle.cycle_id, question_id).await?;

    let my_votes = list_my_votes(state, group_id, &next_cycle.cycle_id, caller).await?;
    if !my_votes.iter().any(|v| v.question_id == *question_id) {
        return Ok(CandidateVoteResponse {
            question_id: question_id.clone(),
            vote_count: candidate.vote_count,
            voted_by_me: false,
            my_vote_count: my_votes.len() as u32,
        });
    }

    let vote = CandidateVote {
        user_id: caller.clone(),
        group_id: group_id.clone(),
        cycle_id: next_cycle.cycle_id.clone(),
        question_id: question_id.clone(),
        voted_at: Utc::now(),
    };
    let vote_count =
        withdraw_vote_with_retry(state, group_id, &next_cycle.cycle_id, &vote, candidate).await?;

    Ok(CandidateVoteResponse {
        question_id: question_id.clone(),
        vote_count,
        voted_by_me: false,
        my_vote_count: my_votes.len() as u32 - 1,
    })
}

async fn withdraw_vote_with_retry(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
    vote: &CandidateVote,
    mut candidate: CandidateQuestion,
) -> Result<u32, ApiError> {
    for attempt in 0..2 {
        let new_count = candidate.vote_count.saturating_sub(1);
        match questions::withdraw_vote_tx(&state.repo, vote, new_count).await {
            Ok(()) => return Ok(new_count),
            Err(e) if e.is_lost_race() && attempt == 0 => {
                candidate = load_candidate(state, group_id, cycle_id, &vote.question_id).await?;
            }
            Err(e) => {
                tracing::error!(error = ?e, group_id = %group_id, question_id = %vote.question_id, "withdraw vote failed");
                return Err(ApiError::internal("failed to withdraw vote"));
            }
        }
    }
    unreachable!("the loop above always returns within two attempts")
}

/// `DELETE /admin/groups/{groupId}/candidate-questions/{questionId}` (§6.5). The
/// route carries no `cycleId`, so this resolves which cycle the candidate lives
/// in by checking the two cycles that can ever hold a still-relevant row: the
/// current `voting` cycle (still deletable) and the current `open` cycle (which
/// just promoted from `voting` — a match among its locked questions means the
/// promotion race won, so this returns `CANDIDATE_PROMOTED`).
pub async fn admin_delete_candidate(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    question_id: &QuestionId,
) -> Result<(), ApiError> {
    auth::require_membership(&state.repo, caller, group_id, true).await?;

    if let Some(voting) = newsletters::find_voting_cycle(&state.repo, group_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "voting cycle lookup failed");
            ApiError::internal("failed to resolve the voting cycle")
        })?
    {
        if load_candidate(state, group_id, &voting.cycle_id, question_id)
            .await
            .is_ok()
        {
            return delete_candidate_and_votes(state, group_id, &voting.cycle_id, question_id)
                .await;
        }
    }

    if let Some(open) = newsletters::find_open_cycle(&state.repo, group_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "open cycle lookup failed");
            ApiError::internal("failed to resolve the open cycle")
        })?
    {
        let locked = questions::list_locked_questions(&state.repo, group_id, &open.cycle_id)
            .await
            .map_err(|e| {
                tracing::error!(error = ?e, group_id = %group_id, "locked question lookup failed");
                ApiError::internal("failed to load locked questions")
            })?;
        if locked.iter().any(|l| l.question_id == *question_id) {
            return Err(ApiError::new(
                ApiErrorCode::CandidatePromoted,
                "this question has already been locked into a cycle",
            ));
        }

        if load_candidate(state, group_id, &open.cycle_id, question_id)
            .await
            .is_ok()
        {
            return delete_candidate_and_votes(state, group_id, &open.cycle_id, question_id).await;
        }
    }

    Err(ApiError::not_found("candidate question not found"))
}

async fn delete_candidate_and_votes(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
) -> Result<(), ApiError> {
    cascade_delete_votes(state, group_id, cycle_id, question_id).await?;
    questions::delete_candidate(&state.repo, group_id, cycle_id, question_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "candidate delete failed");
            ApiError::internal("failed to delete candidate question")
        })
}

/// `02-data-model-dynamodb.md` has no access pattern for "list voters of
/// question X" (`CandidateVote` is keyed by voter, not by question), so this
/// deletes speculatively for every group member instead of scanning the table
/// (`03-api-contract.md` §6.5 implementation note).
async fn cascade_delete_votes(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
) -> Result<(), ApiError> {
    let members = groups::list_members_for_group(&state.repo, group_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "member listing failed");
            ApiError::internal("failed to list group members")
        })?;
    for member in members {
        questions::delete_vote_if_present(
            &state.repo,
            group_id,
            cycle_id,
            &member.user_id,
            question_id,
        )
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, user_id = %member.user_id, "vote cascade delete failed");
            ApiError::internal("failed to remove votes for this question")
        })?;
    }
    Ok(())
}

// ---------- shared helpers ----------

async fn load_group(state: &AppState, group_id: &GroupId) -> Result<domain::Group, ApiError> {
    groups::get_group(&state.repo, group_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "group lookup failed");
            ApiError::internal("failed to load group")
        })?
        .ok_or_else(|| ApiError::not_found("group not found"))
}

async fn load_user(state: &AppState, user_id: &UserId) -> Result<User, ApiError> {
    users::get_user(&state.repo, user_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, user_id = %user_id, "user lookup failed");
            ApiError::internal("failed to load user profile")
        })?
        .ok_or_else(|| ApiError::internal("caller has no user profile"))
}

async fn require_voting_cycle(
    state: &AppState,
    group_id: &GroupId,
) -> Result<domain::Newsletter, ApiError> {
    newsletters::find_voting_cycle(&state.repo, group_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "voting cycle lookup failed");
            ApiError::internal("failed to resolve the voting cycle")
        })?
        .ok_or_else(|| {
            ApiError::new(
                ApiErrorCode::CycleNotVoting,
                "this group has no cycle currently accepting candidate questions",
            )
        })
}

async fn load_candidate(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
) -> Result<CandidateQuestion, ApiError> {
    questions::get_candidate(&state.repo, group_id, cycle_id, question_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "candidate lookup failed");
            ApiError::internal("failed to load candidate question")
        })?
        .ok_or_else(|| ApiError::not_found("candidate question not found"))
}

async fn list_my_votes(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
    caller: &UserId,
) -> Result<Vec<CandidateVote>, ApiError> {
    questions::list_my_votes(&state.repo, group_id, cycle_id, caller)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "vote listing failed");
            ApiError::internal("failed to list your votes")
        })
}

async fn load_submitter_profiles(
    state: &AppState,
    candidates: &[CandidateQuestion],
) -> Result<HashMap<UserId, User>, ApiError> {
    let ids: Vec<UserId> = candidates
        .iter()
        .map(|c| c.submitted_by.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    users::get_users_batch(&state.repo, &ids)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, "submitter profile batch lookup failed");
            ApiError::internal("failed to load submitter profiles")
        })
}

fn to_candidate_item(
    state: &AppState,
    caller: &UserId,
    is_admin: bool,
    profiles: &HashMap<UserId, User>,
    candidate: CandidateQuestion,
    voted_by_me: bool,
) -> CandidateItemResponse {
    let asked_by = asked_by(state, caller, is_admin, &candidate, profiles);
    let poll_options = candidate.poll_options.map(|opts| {
        opts.into_iter()
            .map(|o| PollOptionResponse {
                option_id: o.option_id,
                label: o.label,
            })
            .collect()
    });

    CandidateItemResponse {
        question_id: candidate.question_id,
        kind: candidate.kind,
        prompt: candidate.prompt,
        poll_options,
        asked_by,
        is_anonymous: candidate.is_anonymous,
        vote_count: candidate.vote_count,
        voted_by_me,
        submitted_at: candidate.submitted_at,
    }
}

/// §6.1 — `askedBy` is populated when `isAnonymous=false`, or the caller is the
/// submitter, or the caller is a group admin; otherwise `None`.
fn asked_by(
    state: &AppState,
    caller: &UserId,
    is_admin: bool,
    candidate: &CandidateQuestion,
    profiles: &HashMap<UserId, User>,
) -> Option<AskedBy> {
    let visible = !candidate.is_anonymous || is_admin || candidate.submitted_by == *caller;
    if !visible {
        return None;
    }
    let user = profiles.get(&candidate.submitted_by)?;
    Some(AskedBy {
        user_id: user.user_id.clone(),
        display_name: user.display_name.clone(),
        avatar_color: user.avatar_color.clone(),
        avatar_url: user
            .avatar_media_id
            .as_ref()
            .and_then(|id| state.avatar_url(id)),
    })
}

fn encode_offset(offset: usize) -> String {
    URL_SAFE_NO_PAD.encode(serde_json::json!({ "offset": offset }).to_string())
}

fn decode_offset(raw: &str) -> Result<usize, ApiError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(raw)
        .map_err(|_| ApiError::validation("invalid cursor"))?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| ApiError::validation("invalid cursor"))?;
    value
        .get("offset")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .ok_or_else(|| ApiError::validation("invalid cursor"))
}
