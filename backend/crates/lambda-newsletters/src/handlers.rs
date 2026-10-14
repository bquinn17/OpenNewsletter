//! Business logic for the newsletter list + detail routes
//! (`docs/design/api.md` §5).

use crate::state::AppState;
use domain::api::{
    AskedBy, CandidateItemResponse, CommentImageResponse, CommentResponse, MyResponseSummary,
    NewsletterDetailResponse, NewsletterListResponse, NewsletterSummary, OpenQuestionResponse,
    PollOptionResponse, PublishedAnswerResponse, PublishedImageResponse,
    PublishedPollOptionResponse, PublishedQuestionResponse,
};
use domain::{
    ApiError, ApiErrorCode, CandidateQuestion, Comment, CycleId, GroupId, GroupMembership, ImageId,
    ImageMedia, LockedQuestion, MediaStatus, Newsletter, NewsletterStatus, PollOptionId,
    QuestionId, QuestionKind, Reaction, ResponseStatus, Role, User, UserId,
};
use futures::future::try_join_all;
use futures::{stream, StreamExt, TryStreamExt};
use persistence::{auth, engagement, media, newsletters, questions, responses, users};
use shared::config::{EDITION_READ_CONCURRENCY, MAX_NEWSLETTER_LIST_LIMIT};
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

    // One AP15 read per cycle on the page; independent, so run together.
    let counts = try_join_all(filtered.iter().map(|nl| async move {
        let my_responses =
            responses::list_my_responses_in_cycle(&state.repo, caller, group_id, &nl.cycle_id)
                .await
                .map_err(|e| {
                    tracing::error!(error = ?e, group_id = %group_id, cycle_id = %nl.cycle_id, "my-response listing failed");
                    ApiError::internal("failed to load your responses")
                })?;
        let count = |status| my_responses.iter().filter(|r| r.status == status).count() as u32;
        Ok::<_, ApiError>((
            count(ResponseStatus::Draft),
            count(ResponseStatus::Published),
        ))
    }))
    .await?;

    let items = filtered
        .into_iter()
        .zip(counts)
        .map(
            |(nl, (my_draft_count, my_published_count))| NewsletterSummary {
                question_count: nl.locked_question_ids.len() as u32,
                cycle_id: nl.cycle_id,
                status: nl.status,
                response_open_at: nl.response_open_at,
                response_close_at: nl.response_close_at,
                published_at: nl.published_at,
                my_draft_count,
                my_published_count,
            },
        )
        .collect();

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

    let my_responses =
        responses::list_my_responses_in_cycle(&state.repo, caller, group_id, &nl.cycle_id)
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

/// The published edition is read in a fixed number of concurrent waves, not
/// per answer (`docs/design/api.md` §5.2): locked questions, then every
/// question's answers, then every text answer's engagement partition, then one
/// batch each for the user profiles and images the whole edition references.
async fn published_detail(
    state: &AppState,
    caller: &UserId,
    membership: &GroupMembership,
    group_id: &GroupId,
    nl: &Newsletter,
) -> Result<NewsletterDetailResponse, ApiError> {
    let cycle_id = &nl.cycle_id;
    let locked = load_locked_questions(state, group_id, cycle_id).await?;

    let answers_per_question = try_join_all(
        locked
            .iter()
            .map(|lq| load_published_answers(state, group_id, cycle_id, &lq.question_id)),
    )
    .await?;

    // One pass over the text questions' answers collects everything they
    // hydrate: engagement keys, answer authors and answer images.
    let mut user_ids: HashSet<UserId> = locked.iter().map(|l| l.submitted_by.clone()).collect();
    let mut image_ids: HashSet<ImageId> = HashSet::new();
    let mut text_answers: Vec<(QuestionId, UserId)> = Vec::new();
    for (lq, answers) in locked.iter().zip(&answers_per_question) {
        if lq.kind != QuestionKind::Text {
            continue;
        }
        for r in answers {
            text_answers.push((lq.question_id.clone(), r.user_id.clone()));
            user_ids.insert(r.user_id.clone());
            image_ids.extend(r.image_media_ids.iter().cloned());
        }
    }
    let engagement = load_engagement(state, group_id, cycle_id, text_answers).await?;
    for (comments, _) in engagement.values() {
        for c in comments {
            user_ids.insert(c.author_user_id.clone());
            if c.deleted_at.is_none() {
                image_ids.extend(c.image_media_id.iter().cloned());
            }
        }
    }
    let user_ids: Vec<UserId> = user_ids.into_iter().collect();
    let image_ids: Vec<ImageId> = image_ids.into_iter().collect();
    let (profiles, images) = futures::try_join!(
        load_profiles(state, &user_ids),
        load_images(state, group_id, cycle_id, &image_ids),
    )?;

    let is_admin = membership.role == Role::Admin;
    let mut questions_out = Vec::with_capacity(locked.len());
    for (lq, answers) in locked.into_iter().zip(answers_per_question) {
        let asked_by = locked_asked_by(state, caller, is_admin, &lq, &profiles);
        let question = match lq.kind {
            QuestionKind::Text => published_text_question(
                state,
                caller,
                lq,
                asked_by,
                answers,
                &engagement,
                &profiles,
                &images,
            )?,
            QuestionKind::Poll => published_poll_question(caller, lq, asked_by, answers),
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

#[allow(clippy::too_many_arguments)]
fn published_text_question(
    state: &AppState,
    caller: &UserId,
    lq: LockedQuestion,
    asked_by: Option<AskedBy>,
    answers: Vec<domain::Response>,
    engagement: &EngagementByAnswer,
    profiles: &HashMap<UserId, User>,
    images: &HashMap<ImageId, ImageMedia>,
) -> Result<PublishedQuestionResponse, ApiError> {
    let mut answers_out = Vec::with_capacity(answers.len());
    for r in answers {
        let author = profiles
            .get(&r.user_id)
            .ok_or_else(|| ApiError::internal("answer references a missing user profile"))?;
        let (comments, reactions) = engagement
            .get(&(lq.question_id.clone(), r.user_id.clone()))
            .map(|(c, r)| (c.as_slice(), r.as_slice()))
            .unwrap_or_default();
        let comments = comments
            .iter()
            .map(|c| comment_response(state, profiles, images, c))
            .collect::<Result<Vec<_>, _>>()?;

        answers_out.push(PublishedAnswerResponse {
            response_id: r.response_id,
            user_id: r.user_id,
            display_name: author.display_name.clone(),
            body: r.body,
            images: r
                .image_media_ids
                .iter()
                .filter_map(|id| images.get(id))
                .filter_map(|img| published_image(state, img))
                .collect(),
            published_at: r.published_at.unwrap_or(r.updated_at),
            comments,
            reaction_groups: domain::engagement::group_reactions(reactions, caller),
        });
    }

    Ok(PublishedQuestionResponse {
        question_id: lq.question_id,
        kind: lq.kind,
        prompt: lq.prompt,
        display_order: lq.display_order,
        asked_by,
        is_anonymous: lq.is_anonymous,
        answers: Some(answers_out),
        options: None,
        my_vote_option_id: None,
    })
}

fn published_poll_question(
    caller: &UserId,
    lq: LockedQuestion,
    asked_by: Option<AskedBy>,
    answers: Vec<domain::Response>,
) -> PublishedQuestionResponse {
    let mut tallies: HashMap<PollOptionId, u32> = HashMap::new();
    let mut my_vote_option_id = None;
    for r in answers {
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

    PublishedQuestionResponse {
        question_id: lq.question_id,
        kind: lq.kind,
        prompt: lq.prompt,
        display_order: lq.display_order,
        asked_by,
        is_anonymous: lq.is_anonymous,
        answers: None,
        options: Some(options),
        my_vote_option_id,
    }
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

/// A question's published answers, in AP14 order.
async fn load_published_answers(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
) -> Result<Vec<domain::Response>, ApiError> {
    let answers = responses::list_answers(&state.repo, group_id, cycle_id, question_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "answer listing failed");
            ApiError::internal("failed to load answers")
        })?;
    Ok(answers
        .into_iter()
        .filter(|r| r.status == ResponseStatus::Published)
        .collect())
}

/// Comments (oldest first) and reactions per `(question, answer author)`.
type EngagementByAnswer = HashMap<(QuestionId, UserId), (Vec<Comment>, Vec<Reaction>)>;

/// One engagement-partition read per text answer, [`EDITION_READ_CONCURRENCY`]
/// at a time.
async fn load_engagement(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
    answers: Vec<(QuestionId, UserId)>,
) -> Result<EngagementByAnswer, ApiError> {
    stream::iter(answers)
        .map(|(question_id, answer_user)| async move {
            let rows = engagement::list_engagement(
                &state.repo,
                group_id,
                cycle_id,
                &question_id,
                &answer_user,
            )
            .await
            .map_err(|e| {
                tracing::error!(error = ?e, group_id = %group_id, "engagement listing failed");
                ApiError::internal("failed to load comments and reactions")
            })?;
            Ok::<_, ApiError>(((question_id, answer_user), rows))
        })
        .buffer_unordered(EDITION_READ_CONCURRENCY)
        .try_collect()
        .await
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

async fn load_images(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
    ids: &[ImageId],
) -> Result<HashMap<ImageId, ImageMedia>, ApiError> {
    media::get_images_batch(&state.repo, group_id, cycle_id, ids)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "image batch lookup failed");
            ApiError::internal("failed to load images")
        })
}

/// The display/thumb pair for an image, or `None` until it's `ready` with
/// both variants and dimensions recorded.
fn ready_variants(state: &AppState, img: &ImageMedia) -> Option<(String, String, u32, u32)> {
    if img.status != MediaStatus::Ready {
        return None;
    }
    let (Some(display_key), Some(thumb_key), Some(width), Some(height)) =
        (&img.display_key, &img.thumb_key, img.width, img.height)
    else {
        return None;
    };
    Some((
        state.image_url(display_key)?,
        state.image_url(thumb_key)?,
        width,
        height,
    ))
}

fn published_image(state: &AppState, img: &ImageMedia) -> Option<PublishedImageResponse> {
    let (display_url, thumb_url, width, height) = ready_variants(state, img)?;
    Some(PublishedImageResponse {
        image_id: img.image_id.clone(),
        display_url,
        thumb_url,
        width,
        height,
    })
}

/// Builds the wire `CommentResponse` for one comment row, including a
/// soft-deleted one as a `[deleted]` placeholder (`docs/design/engagement.md` §1.1,
/// decided M10 — soft-deleted comments are returned here too, not filtered).
/// The attached image is hydrated only while it's `ready`.
fn comment_response(
    state: &AppState,
    profiles: &HashMap<UserId, User>,
    images: &HashMap<ImageId, ImageMedia>,
    comment: &Comment,
) -> Result<CommentResponse, ApiError> {
    let author = profiles
        .get(&comment.author_user_id)
        .ok_or_else(|| ApiError::internal("comment references a missing user profile"))?;
    let is_deleted = comment.deleted_at.is_some();
    let image = comment
        .image_media_id
        .as_ref()
        .filter(|_| !is_deleted)
        .and_then(|id| images.get(id))
        .and_then(|img| {
            let (display_url, thumb_url, width, height) = ready_variants(state, img)?;
            Some(CommentImageResponse {
                image_id: img.image_id.clone(),
                display_url,
                thumb_url,
                width,
                height,
                caption: img.caption.clone(),
            })
        });

    Ok(CommentResponse {
        comment_id: comment.comment_id.clone(),
        author_user_id: comment.author_user_id.clone(),
        display_name: author.display_name.clone(),
        body: if is_deleted {
            String::new()
        } else {
            comment.body.clone()
        },
        image,
        created_at: comment.created_at,
        edited_at: comment.edited_at,
        deleted_at: comment.deleted_at,
    })
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
