//! Business logic for the newsletter list + detail routes
//! (`plans/03-api-contract.md` §5).

use crate::state::AppState;
use domain::api::{
    AskedBy, CandidateItemResponse, MyResponseSummary, NewsletterDetailResponse,
    NewsletterListResponse, NewsletterSummary, OpenQuestionResponse, PollOptionResponse,
    PublishedAnswerResponse, PublishedCommentResponse, PublishedImageResponse,
    PublishedPollOptionResponse, PublishedQuestionResponse, ReactionGroupResponse,
};
use domain::{
    ApiError, ApiErrorCode, CandidateQuestion, CycleId, GroupId, GroupMembership, ImageId,
    LockedQuestion, MediaStatus, Newsletter, NewsletterStatus, PollOptionId, QuestionId,
    QuestionKind, ResponseStatus, Role, User, UserId,
};
use persistence::{auth, engagement, media, newsletters, questions, responses, users};
use shared::config::MAX_NEWSLETTER_LIST_LIMIT;
use std::collections::{HashMap, HashSet};

/// `GET /groups/{groupId}/newsletters` (§5.1).
pub async fn list_newsletters(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    status_filter: Option<NewsletterStatus>,
    limit: u32,
    cursor: Option<&str>,
) -> Result<NewsletterListResponse, ApiError> {
    auth::require_membership(&state.repo, caller, group_id, false).await?;

    let limit = limit.clamp(1, MAX_NEWSLETTER_LIST_LIMIT) as i32;
    let (page, next_cursor) = newsletters::list_recent(&state.repo, group_id, limit, cursor)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "newsletter listing failed");
            ApiError::internal("failed to list newsletters")
        })?;

    // A status filter is applied to this page's contents rather than pushed into
    // the underlying query (AP7 has no status attribute to filter on), so a
    // filtered page can return fewer than `limit` items even when more exist —
    // acceptable given how few cycles a group ever accumulates.
    let filtered: Vec<Newsletter> = match status_filter {
        Some(status) => page.into_iter().filter(|nl| nl.status == status).collect(),
        None => page,
    };

    let mut items = Vec::with_capacity(filtered.len());
    for nl in filtered {
        let my_responses =
            responses::list_my_responses_in_cycle(&state.repo, caller, &nl.cycle_id)
                .await
                .map_err(|e| {
                    tracing::error!(error = ?e, group_id = %group_id, cycle_id = %nl.cycle_id, "my-response listing failed");
                    ApiError::internal("failed to load your responses")
                })?;
        let my_draft_count = my_responses
            .iter()
            .filter(|r| r.status == ResponseStatus::Draft)
            .count() as u32;
        let my_published_count = my_responses
            .iter()
            .filter(|r| r.status == ResponseStatus::Published)
            .count() as u32;

        items.push(NewsletterSummary {
            question_count: nl.locked_question_ids.len() as u32,
            cycle_id: nl.cycle_id,
            status: nl.status,
            response_open_at: nl.response_open_at,
            response_close_at: nl.response_close_at,
            published_at: nl.published_at,
            my_draft_count,
            my_published_count,
        });
    }

    Ok(NewsletterListResponse { items, next_cursor })
}

/// `GET /groups/{groupId}/newsletters/{cycleId}` (§5.2).
pub async fn get_newsletter_detail(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    cycle_id: &CycleId,
) -> Result<NewsletterDetailResponse, ApiError> {
    let membership = auth::require_membership(&state.repo, caller, group_id, false).await?;
    let nl = newsletters::get_newsletter(&state.repo, group_id, cycle_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "newsletter lookup failed");
            ApiError::internal("failed to load newsletter")
        })?
        .ok_or_else(|| ApiError::not_found("newsletter not found"))?;

    match nl.status {
        NewsletterStatus::Voting => {
            voting_detail(state, caller, &membership, group_id, cycle_id).await
        }
        NewsletterStatus::Open => open_detail(state, caller, &membership, group_id, &nl).await,
        NewsletterStatus::Published => {
            published_detail(state, caller, &membership, group_id, &nl).await
        }
        NewsletterStatus::Archived => Err(ApiError::new(
            ApiErrorCode::NewsletterArchived,
            "this edition has been archived",
        )),
    }
}

async fn voting_detail(
    state: &AppState,
    caller: &UserId,
    membership: &GroupMembership,
    group_id: &GroupId,
    cycle_id: &CycleId,
) -> Result<NewsletterDetailResponse, ApiError> {
    let mut pool = questions::list_candidates(&state.repo, group_id, cycle_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "candidate listing failed");
            ApiError::internal("failed to list candidate questions")
        })?;
    pool.sort_by(|a, b| {
        b.vote_count
            .cmp(&a.vote_count)
            .then_with(|| a.submitted_at.cmp(&b.submitted_at))
    });

    let my_votes = questions::list_my_votes(&state.repo, group_id, cycle_id, caller)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "vote listing failed");
            ApiError::internal("failed to list your votes")
        })?;
    let voted_ids: HashSet<QuestionId> = my_votes.into_iter().map(|v| v.question_id).collect();

    let is_admin = membership.role == Role::Admin;
    let submitter_ids: Vec<UserId> = pool
        .iter()
        .map(|c| c.submitted_by.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let profiles = load_profiles(state, &submitter_ids).await?;

    let candidates: Vec<CandidateItemResponse> = pool
        .into_iter()
        .map(|c| {
            let voted_by_me = voted_ids.contains(&c.question_id);
            candidate_item(state, caller, is_admin, &profiles, c, voted_by_me)
        })
        .collect();

    Ok(NewsletterDetailResponse::Voting { candidates })
}

async fn open_detail(
    state: &AppState,
    caller: &UserId,
    membership: &GroupMembership,
    group_id: &GroupId,
    nl: &Newsletter,
) -> Result<NewsletterDetailResponse, ApiError> {
    let locked = load_locked_questions(state, group_id, &nl.cycle_id).await?;

    let my_responses = responses::list_my_responses_in_cycle(&state.repo, caller, &nl.cycle_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "my-response listing failed");
            ApiError::internal("failed to load your responses")
        })?;
    let mut by_question: HashMap<QuestionId, domain::Response> = my_responses
        .into_iter()
        .map(|r| (r.question_id.clone(), r))
        .collect();

    let is_admin = membership.role == Role::Admin;
    let profiles = load_profiles(
        state,
        &locked
            .iter()
            .map(|l| l.submitted_by.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>(),
    )
    .await?;

    let questions_out = locked
        .into_iter()
        .map(|lq| {
            let asked_by = locked_asked_by(state, caller, is_admin, &lq, &profiles);
            let poll_options = poll_option_responses(&lq.poll_options);
            let my_response = by_question
                .remove(&lq.question_id)
                .map(|r| MyResponseSummary {
                    response_id: r.response_id,
                    status: r.status,
                    body: r.body,
                    image_media_ids: r.image_media_ids,
                    poll_option_id: r.poll_option_id,
                    updated_at: r.updated_at,
                    published_at: r.published_at,
                });
            OpenQuestionResponse {
                question_id: lq.question_id,
                kind: lq.kind,
                prompt: lq.prompt,
                display_order: lq.display_order,
                asked_by,
                is_anonymous: lq.is_anonymous,
                poll_options,
                my_response,
            }
        })
        .collect();

    Ok(NewsletterDetailResponse::Open {
        cycle_id: nl.cycle_id.clone(),
        response_open_at: nl.response_open_at,
        response_close_at: nl.response_close_at,
        questions: questions_out,
    })
}

async fn published_detail(
    state: &AppState,
    caller: &UserId,
    membership: &GroupMembership,
    group_id: &GroupId,
    nl: &Newsletter,
) -> Result<NewsletterDetailResponse, ApiError> {
    let locked = load_locked_questions(state, group_id, &nl.cycle_id).await?;
    let is_admin = membership.role == Role::Admin;
    let profiles = load_profiles(
        state,
        &locked
            .iter()
            .map(|l| l.submitted_by.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>(),
    )
    .await?;

    let mut questions_out = Vec::with_capacity(locked.len());
    for lq in locked {
        let asked_by = locked_asked_by(state, caller, is_admin, &lq, &profiles);
        let question = match lq.kind {
            QuestionKind::Text => {
                published_text_question(state, group_id, &nl.cycle_id, caller, lq, asked_by).await?
            }
            QuestionKind::Poll => {
                published_poll_question(state, group_id, &nl.cycle_id, caller, lq, asked_by).await?
            }
        };
        questions_out.push(question);
    }

    Ok(NewsletterDetailResponse::Published {
        cycle_id: nl.cycle_id.clone(),
        response_close_at: nl.response_close_at,
        published_at: nl.published_at.unwrap_or(nl.response_close_at),
        questions: questions_out,
    })
}

async fn published_text_question(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
    caller: &UserId,
    lq: LockedQuestion,
    asked_by: Option<AskedBy>,
) -> Result<PublishedQuestionResponse, ApiError> {
    let raw = responses::list_answers(&state.repo, group_id, cycle_id, &lq.question_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "answer listing failed");
            ApiError::internal("failed to load answers")
        })?;

    let mut answers = Vec::new();
    for r in raw
        .into_iter()
        .filter(|r| r.status == ResponseStatus::Published)
    {
        let author = load_user(state, &r.user_id).await?;
        let images = hydrate_images(state, group_id, cycle_id, &r.image_media_ids).await?;
        let comments =
            load_comments(state, group_id, cycle_id, &lq.question_id, &r.user_id).await?;
        let reaction_groups = load_reaction_groups(
            state,
            group_id,
            cycle_id,
            &lq.question_id,
            &r.user_id,
            caller,
        )
        .await?;
        answers.push(PublishedAnswerResponse {
            response_id: r.response_id,
            user_id: r.user_id,
            display_name: author.display_name,
            body: r.body,
            images,
            published_at: r.published_at.unwrap_or(r.updated_at),
            comments,
            reaction_groups,
        });
    }

    Ok(PublishedQuestionResponse {
        question_id: lq.question_id,
        kind: lq.kind,
        prompt: lq.prompt,
        display_order: lq.display_order,
        asked_by,
        is_anonymous: lq.is_anonymous,
        answers: Some(answers),
        options: None,
        my_vote_option_id: None,
    })
}

async fn published_poll_question(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
    caller: &UserId,
    lq: LockedQuestion,
    asked_by: Option<AskedBy>,
) -> Result<PublishedQuestionResponse, ApiError> {
    let raw = responses::list_answers(&state.repo, group_id, cycle_id, &lq.question_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "answer listing failed");
            ApiError::internal("failed to load poll answers")
        })?;

    let mut tallies: HashMap<PollOptionId, u32> = HashMap::new();
    let mut my_vote_option_id = None;
    for r in raw
        .into_iter()
        .filter(|r| r.status == ResponseStatus::Published)
    {
        let Some(option_id) = r.poll_option_id else {
            continue;
        };
        *tallies.entry(option_id.clone()).or_insert(0) += 1;
        if r.user_id == *caller {
            my_vote_option_id = Some(option_id);
        }
    }

    let options = lq
        .poll_options
        .unwrap_or_default()
        .into_iter()
        .map(|o| {
            let vote_count = tallies.get(&o.option_id).copied().unwrap_or(0);
            PublishedPollOptionResponse {
                option_id: o.option_id,
                label: o.label,
                vote_count,
            }
        })
        .collect();

    Ok(PublishedQuestionResponse {
        question_id: lq.question_id,
        kind: lq.kind,
        prompt: lq.prompt,
        display_order: lq.display_order,
        asked_by,
        is_anonymous: lq.is_anonymous,
        answers: None,
        options: Some(options),
        my_vote_option_id,
    })
}

// ---------- shared helpers ----------

async fn load_locked_questions(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
) -> Result<Vec<LockedQuestion>, ApiError> {
    let mut locked = questions::list_locked_questions(&state.repo, group_id, cycle_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "locked question listing failed");
            ApiError::internal("failed to load locked questions")
        })?;
    locked.sort_by_key(|l| l.display_order);
    Ok(locked)
}

async fn load_user(state: &AppState, user_id: &UserId) -> Result<User, ApiError> {
    users::get_user(&state.repo, user_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, user_id = %user_id, "user lookup failed");
            ApiError::internal("failed to load user profile")
        })?
        .ok_or_else(|| ApiError::internal("answer references a missing user profile"))
}

async fn load_profiles(
    state: &AppState,
    ids: &[UserId],
) -> Result<HashMap<UserId, User>, ApiError> {
    users::get_users_batch(&state.repo, ids).await.map_err(|e| {
        tracing::error!(error = ?e, "profile batch lookup failed");
        ApiError::internal("failed to load user profiles")
    })
}

async fn hydrate_images(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
    image_ids: &[ImageId],
) -> Result<Vec<PublishedImageResponse>, ApiError> {
    let mut images = Vec::with_capacity(image_ids.len());
    for image_id in image_ids {
        let Some(img) = media::get_image(&state.repo, group_id, cycle_id, image_id)
            .await
            .map_err(|e| {
                tracing::error!(error = ?e, group_id = %group_id, "image lookup failed");
                ApiError::internal("failed to load image")
            })?
        else {
            continue;
        };
        if img.status != MediaStatus::Ready {
            continue;
        }
        let (Some(display_key), Some(thumb_key), Some(width), Some(height)) =
            (img.display_key, img.thumb_key, img.width, img.height)
        else {
            continue;
        };
        let (Some(display_url), Some(thumb_url)) =
            (state.image_url(&display_key), state.image_url(&thumb_key))
        else {
            continue;
        };
        images.push(PublishedImageResponse {
            image_id: img.image_id,
            display_url,
            thumb_url,
            width,
            height,
        });
    }
    Ok(images)
}

async fn load_comments(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    answer_user_id: &UserId,
) -> Result<Vec<PublishedCommentResponse>, ApiError> {
    let comments =
        engagement::list_comments(&state.repo, group_id, cycle_id, question_id, answer_user_id)
            .await
            .map_err(|e| {
                tracing::error!(error = ?e, group_id = %group_id, "comment listing failed");
                ApiError::internal("failed to load comments")
            })?;

    let mut out = Vec::with_capacity(comments.len());
    for c in comments.into_iter().filter(|c| c.deleted_at.is_none()) {
        let author = load_user(state, &c.author_user_id).await?;
        out.push(PublishedCommentResponse {
            comment_id: c.comment_id,
            author_user_id: c.author_user_id,
            display_name: author.display_name,
            body: c.body,
            created_at: c.created_at,
        });
    }
    Ok(out)
}

async fn load_reaction_groups(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    answer_user_id: &UserId,
    caller: &UserId,
) -> Result<Vec<ReactionGroupResponse>, ApiError> {
    let reactions =
        engagement::list_reactions(&state.repo, group_id, cycle_id, question_id, answer_user_id)
            .await
            .map_err(|e| {
                tracing::error!(error = ?e, group_id = %group_id, "reaction listing failed");
                ApiError::internal("failed to load reactions")
            })?;

    let mut counts: HashMap<String, (u32, bool)> = HashMap::new();
    for r in reactions {
        let entry = counts.entry(r.emoji).or_insert((0, false));
        entry.0 += 1;
        if r.reactor_user_id == *caller {
            entry.1 = true;
        }
    }

    let mut groups: Vec<ReactionGroupResponse> = counts
        .into_iter()
        .map(|(emoji, (count, reacted_by_me))| ReactionGroupResponse {
            emoji,
            count,
            reacted_by_me,
        })
        .collect();
    groups.sort_by(|a, b| a.emoji.cmp(&b.emoji));
    Ok(groups)
}

fn poll_option_responses(
    poll_options: &Option<Vec<domain::PollOption>>,
) -> Option<Vec<PollOptionResponse>> {
    poll_options.as_ref().map(|opts| {
        opts.iter()
            .map(|o| PollOptionResponse {
                option_id: o.option_id.clone(),
                label: o.label.clone(),
            })
            .collect()
    })
}

fn candidate_item(
    state: &AppState,
    caller: &UserId,
    is_admin: bool,
    profiles: &HashMap<UserId, User>,
    candidate: CandidateQuestion,
    voted_by_me: bool,
) -> CandidateItemResponse {
    let visible = !candidate.is_anonymous || is_admin || candidate.submitted_by == *caller;
    let asked_by = visible
        .then(|| profiles.get(&candidate.submitted_by))
        .flatten()
        .map(|user| to_asked_by(state, user));
    let poll_options = poll_option_responses(&candidate.poll_options);

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

fn locked_asked_by(
    state: &AppState,
    caller: &UserId,
    is_admin: bool,
    lq: &LockedQuestion,
    profiles: &HashMap<UserId, User>,
) -> Option<AskedBy> {
    let visible = !lq.is_anonymous || is_admin || lq.submitted_by == *caller;
    if !visible {
        return None;
    }
    profiles
        .get(&lq.submitted_by)
        .map(|user| to_asked_by(state, user))
}

fn to_asked_by(state: &AppState, user: &User) -> AskedBy {
    AskedBy {
        user_id: user.user_id.clone(),
        display_name: user.display_name.clone(),
        avatar_color: user.avatar_color.clone(),
        avatar_url: user
            .avatar_media_id
            .as_ref()
            .and_then(|id| state.avatar_url(id)),
    }
}
