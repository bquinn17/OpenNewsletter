//! Business logic for the comment and reaction routes
//! (`plans/03-api-contract.md` §8).

use crate::state::AppState;
use crate::validation;
use chrono::Utc;
use domain::api::{
    CommentImageResponse, CommentListResponse, CommentResponse, CreateCommentRequest,
    PatchCommentRequest, ReactionsResponse,
};
use domain::{
    ApiError, ApiErrorCode, Comment, CommentId, CycleId, GroupId, GroupMembership, ImageId,
    ImagePurpose, MediaStatus, NewsletterStatus, QuestionId, QuestionKind, Reaction, ResponseId,
    ResponseStatus, Role, User, UserId,
};
use persistence::engagement::{self, CommentLocation, NewComment};
use persistence::{auth, media, newsletters, responses, users};
use std::collections::HashMap;

/// `GET .../comments` (§8.1). AP18.
// Args mirror the route's own path + query parameters; bundling them into a
// struct wouldn't reduce real duplication since each is independently
// required by the caller (same reasoning as `persistence::responses::ResponseSave`).
#[allow(clippy::too_many_arguments)]
pub async fn list_comments(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    response_id: &ResponseId,
    limit: i32,
    cursor: Option<&str>,
) -> Result<CommentListResponse, ApiError> {
    let (answer_user_id, _membership) =
        require_published_text_answer(state, caller, group_id, cycle_id, question_id, response_id)
            .await?;

    let exclusive_start_key = match cursor {
        Some(c) => Some(
            persistence::cursor::decode(c)
                .map_err(|_| ApiError::invalid_field("cursor", "invalid cursor"))?,
        ),
        None => None,
    };

    let (comments, next_key) = engagement::list_comments_page(
        &state.repo,
        group_id,
        cycle_id,
        question_id,
        &answer_user_id,
        limit,
        exclusive_start_key,
    )
    .await
    .map_err(|e| {
        tracing::error!(error = ?e, group_id = %group_id, "comment listing failed");
        ApiError::internal("failed to load comments")
    })?;

    let items = hydrate_comments(state, group_id, cycle_id, comments).await?;
    let next_cursor = match next_key {
        Some(key) => Some(persistence::cursor::encode(&key).map_err(|e| {
            tracing::error!(error = ?e, "cursor encode failed");
            ApiError::internal("failed to encode cursor")
        })?),
        None => None,
    };

    Ok(CommentListResponse { items, next_cursor })
}

/// `POST .../comments` (§8.2).
pub async fn create_comment(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    response_id: &ResponseId,
    request: CreateCommentRequest,
) -> Result<CommentResponse, ApiError> {
    let (answer_user_id, _membership) =
        require_published_text_answer(state, caller, group_id, cycle_id, question_id, response_id)
            .await?;

    let body = match &request.body {
        Some(raw) => Some(validation::body(raw)?),
        None => None,
    };
    validation::require_body_or_image(body.as_deref(), request.image_media_id.as_ref())?;

    if let Some(image_id) = &request.image_media_id {
        validate_comment_image(
            state,
            caller,
            group_id,
            cycle_id,
            question_id,
            image_id,
            None,
        )
        .await?;
    }

    let comment_id = CommentId::generate();
    let now = Utc::now();
    let new = NewComment {
        group_id,
        cycle_id,
        question_id,
        answer_user_id: &answer_user_id,
        comment_id: &comment_id,
        author_user_id: caller,
        body: body.as_deref().unwrap_or(""),
        image_media_id: request.image_media_id.as_ref(),
        created_at: now,
    };

    let comment = engagement::create_comment(&state.repo, &new)
        .await
        .map_err(|e| {
            if e.is_lost_race() {
                ApiError::invalid_field(
                    "imageMediaId",
                    "image is already attached to another comment",
                )
            } else {
                tracing::error!(error = ?e, group_id = %group_id, "comment create failed");
                ApiError::internal("failed to create comment")
            }
        })?;

    let authors = load_authors(state, std::slice::from_ref(caller)).await?;
    hydrate_comment(state, group_id, cycle_id, &authors, comment).await
}

/// `PATCH .../comments/{commentId}` (§8.3). Author only.
#[allow(clippy::too_many_arguments)]
pub async fn patch_comment(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    response_id: &ResponseId,
    comment_id: &CommentId,
    request: PatchCommentRequest,
) -> Result<CommentResponse, ApiError> {
    let (answer_user_id, _membership) =
        require_published_text_answer(state, caller, group_id, cycle_id, question_id, response_id)
            .await?;

    let existing = load_live_comment(
        state,
        group_id,
        cycle_id,
        question_id,
        &answer_user_id,
        comment_id,
    )
    .await?;
    if existing.author_user_id != *caller {
        return Err(ApiError::forbidden("only the comment's author can edit it"));
    }

    let new_body = match &request.body {
        Some(raw) => Some(validation::body(raw)?),
        None => None,
    };

    let resulting_body = new_body.clone().unwrap_or_else(|| existing.body.clone());
    let resulting_image_id: Option<ImageId> = match &request.image_media_id {
        Some(Some(id)) => Some(id.clone()),
        Some(None) => None,
        None => existing.image_media_id.clone(),
    };
    validation::require_body_or_image(Some(resulting_body.as_str()), resulting_image_id.as_ref())?;

    let mut attach_image_id: Option<ImageId> = None;
    let mut detach_image_id: Option<ImageId> = None;
    if let Some(image_change) = &request.image_media_id {
        match image_change {
            Some(new_id) if existing.image_media_id.as_ref() != Some(new_id) => {
                validate_comment_image(
                    state,
                    caller,
                    group_id,
                    cycle_id,
                    question_id,
                    new_id,
                    Some(comment_id),
                )
                .await?;
                attach_image_id = Some(new_id.clone());
                detach_image_id = existing.image_media_id.clone();
            }
            Some(_unchanged) => {}
            None => detach_image_id = existing.image_media_id.clone(),
        }
    }

    let now = Utc::now();
    let loc = CommentLocation {
        group_id,
        cycle_id,
        question_id,
        answer_user_id: &answer_user_id,
        comment_id,
        created_at: existing.created_at,
    };

    let update = engagement::update_comment(
        &state.repo,
        &loc,
        new_body.as_deref(),
        request.image_media_id.as_ref().map(Option::as_ref),
        now,
        detach_image_id.as_ref(),
        attach_image_id.as_ref(),
    )
    .await;
    match update {
        Ok(()) => {}
        Err(e) if e.is_lost_race() => {
            // Deleted meanwhile → 404 from `load_live_comment`; otherwise the
            // new image was claimed by another comment.
            load_live_comment(
                state,
                group_id,
                cycle_id,
                question_id,
                &answer_user_id,
                comment_id,
            )
            .await?;
            return Err(ApiError::invalid_field(
                "imageMediaId",
                "image is already attached to another comment",
            ));
        }
        Err(e) => {
            tracing::error!(error = ?e, group_id = %group_id, "comment update failed");
            return Err(ApiError::internal("failed to update comment"));
        }
    }

    let updated = engagement::get_comment_by_id(
        &state.repo,
        group_id,
        cycle_id,
        question_id,
        &answer_user_id,
        comment_id,
    )
    .await
    .map_err(|e| {
        tracing::error!(error = ?e, group_id = %group_id, "post-update comment lookup failed");
        ApiError::internal("failed to load updated comment")
    })?
    .ok_or_else(|| ApiError::internal("comment missing immediately after update"))?;

    let authors = load_authors(state, std::slice::from_ref(caller)).await?;
    hydrate_comment(state, group_id, cycle_id, &authors, updated).await
}

/// `DELETE .../comments/{commentId}` (§8.4). Author or group admin.
/// Idempotent — an already-deleted comment is a no-op 204.
pub async fn delete_comment(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    response_id: &ResponseId,
    comment_id: &CommentId,
) -> Result<(), ApiError> {
    let (answer_user_id, membership) =
        require_published_text_answer(state, caller, group_id, cycle_id, question_id, response_id)
            .await?;

    let existing = engagement::get_comment_by_id(
        &state.repo,
        group_id,
        cycle_id,
        question_id,
        &answer_user_id,
        comment_id,
    )
    .await
    .map_err(|e| {
        tracing::error!(error = ?e, group_id = %group_id, "comment lookup failed");
        ApiError::internal("failed to load comment")
    })?
    .ok_or_else(|| ApiError::not_found("comment not found"))?;

    if existing.deleted_at.is_some() {
        return Ok(());
    }

    let is_admin = membership.role == Role::Admin;
    if existing.author_user_id != *caller && !is_admin {
        return Err(ApiError::forbidden(
            "only the comment's author or a group admin can delete it",
        ));
    }

    let loc = CommentLocation {
        group_id,
        cycle_id,
        question_id,
        answer_user_id: &answer_user_id,
        comment_id,
        created_at: existing.created_at,
    };
    engagement::soft_delete_comment(
        &state.repo,
        &loc,
        Utc::now(),
        existing.image_media_id.as_ref(),
    )
    .await
    .map_err(|e| {
        tracing::error!(error = ?e, group_id = %group_id, "comment soft-delete failed");
        ApiError::internal("failed to delete comment")
    })?;
    Ok(())
}

/// `GET .../reactions` (§8.5). AP19.
pub async fn get_reactions(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    response_id: &ResponseId,
) -> Result<ReactionsResponse, ApiError> {
    let (answer_user_id, _membership) =
        require_published_text_answer(state, caller, group_id, cycle_id, question_id, response_id)
            .await?;
    build_reactions_response(
        state,
        group_id,
        cycle_id,
        question_id,
        &answer_user_id,
        caller,
    )
    .await
}

/// `PUT .../reactions/{emoji}` (§8.6). Idempotent, last-write-wins.
#[allow(clippy::too_many_arguments)]
pub async fn put_reaction(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    response_id: &ResponseId,
    raw_emoji: &str,
) -> Result<ReactionsResponse, ApiError> {
    let (answer_user_id, _membership) =
        require_published_text_answer(state, caller, group_id, cycle_id, question_id, response_id)
            .await?;
    let emoji = validation::emoji(raw_emoji)?;

    let reaction = Reaction {
        group_id: group_id.clone(),
        cycle_id: cycle_id.clone(),
        question_id: question_id.clone(),
        answer_user_id: answer_user_id.clone(),
        reactor_user_id: caller.clone(),
        emoji,
        created_at: Utc::now(),
    };
    engagement::put_reaction(&state.repo, &reaction)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "reaction put failed");
            ApiError::internal("failed to save reaction")
        })?;

    build_reactions_response(
        state,
        group_id,
        cycle_id,
        question_id,
        &answer_user_id,
        caller,
    )
    .await
}

/// `DELETE .../reactions/{emoji}` (§8.7). Idempotent.
#[allow(clippy::too_many_arguments)]
pub async fn delete_reaction(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    response_id: &ResponseId,
    raw_emoji: &str,
) -> Result<ReactionsResponse, ApiError> {
    let (answer_user_id, _membership) =
        require_published_text_answer(state, caller, group_id, cycle_id, question_id, response_id)
            .await?;
    let emoji = validation::emoji(raw_emoji)?;

    engagement::delete_reaction(
        &state.repo,
        group_id,
        cycle_id,
        question_id,
        &answer_user_id,
        caller,
        &emoji,
    )
    .await
    .map_err(|e| {
        tracing::error!(error = ?e, group_id = %group_id, "reaction delete failed");
        ApiError::internal("failed to remove reaction")
    })?;

    build_reactions_response(
        state,
        group_id,
        cycle_id,
        question_id,
        &answer_user_id,
        caller,
    )
    .await
}

// ---------- shared helpers ----------

/// Check order shared by all seven routes (`03-api-contract.md` §8, GETs
/// included): membership (403) -> cycle exists (404) -> not archived (410)
/// -> published (409) -> answer resolves to a published text response
/// (404). Returns the answer's `userId` (the engagement partition's
/// addressing component) and the caller's membership (for the admin check
/// on delete).
async fn require_published_text_answer(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    response_id: &ResponseId,
) -> Result<(UserId, GroupMembership), ApiError> {
    let membership = auth::require_membership(&state.repo, caller, group_id, false).await?;

    let nl = newsletters::get_newsletter(&state.repo, group_id, cycle_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "newsletter lookup failed");
            ApiError::internal("failed to load newsletter")
        })?
        .ok_or_else(|| ApiError::not_found("newsletter not found"))?;

    if nl.status == NewsletterStatus::Archived {
        return Err(ApiError::new(
            ApiErrorCode::NewsletterArchived,
            "this edition has been archived",
        ));
    }
    if nl.status != NewsletterStatus::Published {
        return Err(ApiError::new(
            ApiErrorCode::CycleNotPublished,
            "this cycle is not published",
        ));
    }

    let answers = responses::list_answers(&state.repo, group_id, cycle_id, question_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "answer listing failed");
            ApiError::internal("failed to load answers")
        })?;

    let answer = answers
        .into_iter()
        .find(|r| r.response_id == *response_id)
        .filter(|r| r.kind == QuestionKind::Text && r.status == ResponseStatus::Published)
        .ok_or_else(|| ApiError::not_found("response not found"))?;

    Ok((answer.user_id, membership))
}

/// Looks up a comment by id and 404s if it's missing or already
/// soft-deleted — the shape `PATCH` needs (§8.3: "A missing or soft-deleted
/// comment returns 404").
async fn load_live_comment(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    answer_user_id: &UserId,
    comment_id: &CommentId,
) -> Result<Comment, ApiError> {
    let comment = engagement::get_comment_by_id(
        &state.repo,
        group_id,
        cycle_id,
        question_id,
        answer_user_id,
        comment_id,
    )
    .await
    .map_err(|e| {
        tracing::error!(error = ?e, group_id = %group_id, "comment lookup failed");
        ApiError::internal("failed to load comment")
    })?
    .ok_or_else(|| ApiError::not_found("comment not found"))?;

    if comment.deleted_at.is_some() {
        return Err(ApiError::not_found("comment not found"));
    }
    Ok(comment)
}

/// An attached `imageMediaId` must be `ready`, owned by the caller, uploaded
/// for this group/cycle/question with `purpose=comment`, and not attached to
/// a *different* comment (`03-api-contract.md` §8.2). `existing_comment_id`
/// is `None` on create (any attachment is "someone else's") and
/// `Some(this comment)` on patch (re-claiming its own current image is
/// fine).
async fn validate_comment_image(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    image_id: &ImageId,
    existing_comment_id: Option<&CommentId>,
) -> Result<(), ApiError> {
    let image = media::get_image(&state.repo, group_id, cycle_id, image_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "image lookup failed");
            ApiError::internal("failed to load image")
        })?
        .ok_or_else(|| {
            ApiError::invalid_field("imageMediaId", format!("image `{image_id}` does not exist"))
        })?;

    let claimed_by_another_comment = image
        .attached_comment_id
        .as_ref()
        .is_some_and(|attached| Some(attached) != existing_comment_id);

    let valid = image.user_id == *caller
        && image.status == MediaStatus::Ready
        && image.purpose == ImagePurpose::Comment
        && image.question_id.as_ref() == Some(question_id)
        && !claimed_by_another_comment;

    if valid {
        Ok(())
    } else {
        Err(ApiError::invalid_field(
            "imageMediaId",
            format!(
                "image `{image_id}` is not a ready, unattached comment image owned by you for \
                 this question"
            ),
        ))
    }
}

async fn load_authors(
    state: &AppState,
    author_ids: &[UserId],
) -> Result<HashMap<UserId, User>, ApiError> {
    users::get_users_batch(&state.repo, author_ids)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, "comment author batch lookup failed");
            ApiError::internal("failed to load comment authors")
        })
}

async fn hydrate_comments(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
    comments: Vec<Comment>,
) -> Result<Vec<CommentResponse>, ApiError> {
    let author_ids: Vec<UserId> = comments
        .iter()
        .map(|c| c.author_user_id.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let authors = load_authors(state, &author_ids).await?;

    let mut out = Vec::with_capacity(comments.len());
    for comment in comments {
        out.push(hydrate_comment(state, group_id, cycle_id, &authors, comment).await?);
    }
    Ok(out)
}

/// Builds the wire `CommentResponse`, including a soft-deleted comment as a
/// `[deleted]` placeholder (`09-engagement.md` §1.1).
async fn hydrate_comment(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
    authors: &HashMap<UserId, User>,
    comment: Comment,
) -> Result<CommentResponse, ApiError> {
    let author = authors
        .get(&comment.author_user_id)
        .ok_or_else(|| ApiError::internal("comment references a missing user profile"))?;
    let is_deleted = comment.deleted_at.is_some();
    let image = if is_deleted {
        None
    } else {
        match &comment.image_media_id {
            Some(image_id) => hydrate_comment_image(state, group_id, cycle_id, image_id).await?,
            None => None,
        }
    };

    Ok(CommentResponse {
        comment_id: comment.comment_id,
        author_user_id: comment.author_user_id,
        display_name: author.display_name.clone(),
        body: if is_deleted {
            String::new()
        } else {
            comment.body
        },
        image,
        created_at: comment.created_at,
        edited_at: comment.edited_at,
        deleted_at: comment.deleted_at,
    })
}

/// The comment's attached image, hydrated only while it's `ready`
/// (`09-engagement.md` §1.1).
async fn hydrate_comment_image(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
    image_id: &ImageId,
) -> Result<Option<CommentImageResponse>, ApiError> {
    let Some(img) = media::get_image(&state.repo, group_id, cycle_id, image_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "comment image lookup failed");
            ApiError::internal("failed to load comment image")
        })?
    else {
        return Ok(None);
    };
    if img.status != MediaStatus::Ready {
        return Ok(None);
    }
    let (Some(display_key), Some(thumb_key), Some(width), Some(height)) =
        (img.display_key, img.thumb_key, img.width, img.height)
    else {
        return Ok(None);
    };
    let (Some(display_url), Some(thumb_url)) =
        (state.image_url(&display_key), state.image_url(&thumb_key))
    else {
        return Ok(None);
    };
    Ok(Some(CommentImageResponse {
        image_id: img.image_id,
        display_url,
        thumb_url,
        width,
        height,
        caption: img.caption,
    }))
}

async fn build_reactions_response(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    answer_user_id: &UserId,
    caller: &UserId,
) -> Result<ReactionsResponse, ApiError> {
    let reactions =
        engagement::list_reactions(&state.repo, group_id, cycle_id, question_id, answer_user_id)
            .await
            .map_err(|e| {
                tracing::error!(error = ?e, group_id = %group_id, "reaction listing failed");
                ApiError::internal("failed to load reactions")
            })?;
    let groups = domain::engagement::group_reactions(&reactions, caller);
    let my_reactions = groups
        .iter()
        .filter(|g| g.reacted_by_me)
        .map(|g| g.emoji.clone())
        .collect();
    Ok(ReactionsResponse {
        reaction_groups: groups,
        my_reactions,
    })
}
