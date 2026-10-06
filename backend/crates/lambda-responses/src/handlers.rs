//! Business logic for the response draft/publish routes
//! (`plans/03-api-contract.md` §7).

use crate::state::AppState;
use crate::validation;
use chrono::Utc;
use domain::api::{MyResponsesList, ResponseDto, SaveResponseRequest};
use domain::{
    ApiError, ApiErrorCode, CycleId, GroupId, ImageId, ImagePurpose, LockedQuestion, MediaStatus,
    NewsletterStatus, QuestionId, QuestionKind, ResponseStatus, UserId,
};
use persistence::responses::ResponseSave;
use persistence::{auth, media, newsletters, questions, responses};

/// `GET /groups/{groupId}/newsletters/{cycleId}/my-responses` (§7.1). AP15.
pub async fn list_my_responses(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    cycle_id: &CycleId,
) -> Result<MyResponsesList, ApiError> {
    auth::require_membership(&state.repo, caller, group_id, false).await?;
    require_newsletter_not_archived(state, group_id, cycle_id).await?;

    let items = responses::list_my_responses_in_cycle(&state.repo, caller, group_id, cycle_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, cycle_id = %cycle_id, "my-response listing failed");
            ApiError::internal("failed to list your responses")
        })?
        .into_iter()
        .map(ResponseDto::from)
        .collect();

    Ok(MyResponsesList { items })
}

/// `GET .../questions/{questionId}/my-response` (§7.2). AP16. 404 if the
/// caller has no draft yet.
pub async fn get_my_response(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
) -> Result<ResponseDto, ApiError> {
    auth::require_membership(&state.repo, caller, group_id, false).await?;
    require_newsletter_not_archived(state, group_id, cycle_id).await?;

    let response = responses::get_my_response(&state.repo, group_id, cycle_id, question_id, caller)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, cycle_id = %cycle_id, "my-response lookup failed");
            ApiError::internal("failed to load your response")
        })?
        .ok_or_else(|| ApiError::not_found("no response found for this question"))?;

    Ok(response.into())
}

/// `PUT .../questions/{questionId}/my-response` (§7.3) — the autosave/save-draft
/// and publish endpoint. Validation order: membership, cycle exists and isn't
/// archived, the question is locked into this cycle, the request `kind`
/// matches the question's kind, then per-kind content validation. The cycle
/// being writable (`open` and before `responseCloseAt`) is enforced inside
/// [`persistence::responses::save_response`]'s transaction, not here — a lost
/// race there maps to 409 `CYCLE_NOT_OPEN`.
pub async fn save_response(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    request: SaveResponseRequest,
) -> Result<ResponseDto, ApiError> {
    auth::require_membership(&state.repo, caller, group_id, false).await?;
    require_newsletter_not_archived(state, group_id, cycle_id).await?;

    let question = questions::get_locked_question(&state.repo, group_id, cycle_id, question_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, cycle_id = %cycle_id, "locked question lookup failed");
            ApiError::internal("failed to load question")
        })?
        .ok_or_else(|| ApiError::not_found("question not found in this cycle"))?;

    let content = validate_content(state, caller, group_id, cycle_id, &question, request).await?;

    let my_responses = responses::list_my_responses_in_cycle(&state.repo, caller, group_id, cycle_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, cycle_id = %cycle_id, "my-response listing failed");
            ApiError::internal("failed to list your responses")
        })?;
    let already_published_in_cycle = my_responses
        .iter()
        .any(|r| r.status == ResponseStatus::Published);
    let this_question_already_published = my_responses
        .iter()
        .any(|r| r.question_id == *question_id && r.status == ResponseStatus::Published);
    let will_be_published = content.publish || this_question_already_published;
    let bump_editions_answered = will_be_published && !already_published_in_cycle;

    let save = ResponseSave {
        group_id,
        cycle_id,
        question_id,
        user_id: caller,
        kind: content.kind,
        body: content.body.as_deref(),
        poll_option_id: content.poll_option_id.as_ref(),
        image_media_ids: &content.image_media_ids,
        publish: content.publish,
    };
    responses::save_response(&state.repo, &save, Utc::now(), bump_editions_answered)
        .await
        .map_err(|e| {
            if e.is_lost_race() {
                ApiError::new(
                    ApiErrorCode::CycleNotOpen,
                    "this cycle is not open for responses",
                )
            } else {
                tracing::error!(error = ?e, group_id = %group_id, cycle_id = %cycle_id, "response save failed");
                ApiError::internal("failed to save response")
            }
        })?;

    let saved = responses::get_my_response(&state.repo, group_id, cycle_id, question_id, caller)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, cycle_id = %cycle_id, "post-save response lookup failed");
            ApiError::internal("failed to load your saved response")
        })?
        .ok_or_else(|| ApiError::internal("response missing immediately after save"))?;

    Ok(saved.into())
}

struct ValidatedContent {
    kind: QuestionKind,
    body: Option<String>,
    poll_option_id: Option<domain::PollOptionId>,
    image_media_ids: Vec<ImageId>,
    publish: bool,
}

async fn validate_content(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question: &LockedQuestion,
    request: SaveResponseRequest,
) -> Result<ValidatedContent, ApiError> {
    match request {
        SaveResponseRequest::Text {
            body,
            image_media_ids,
            publish,
        } => {
            if question.kind != QuestionKind::Text {
                return Err(ApiError::invalid_field(
                    "kind",
                    "kind must be `text` to match this question",
                ));
            }
            let body = validation::body(&body)?;
            let image_media_ids = validation::dedupe_image_ids(image_media_ids)?;
            for image_id in &image_media_ids {
                validate_response_image(state, caller, group_id, cycle_id, image_id).await?;
            }
            Ok(ValidatedContent {
                kind: QuestionKind::Text,
                body: Some(body),
                poll_option_id: None,
                image_media_ids,
                publish,
            })
        }
        SaveResponseRequest::Poll {
            poll_option_id,
            publish,
        } => {
            if question.kind != QuestionKind::Poll {
                return Err(ApiError::invalid_field(
                    "kind",
                    "kind must be `poll` to match this question",
                ));
            }
            validation::poll_option_belongs_to_question(question, &poll_option_id)?;
            Ok(ValidatedContent {
                kind: QuestionKind::Poll,
                body: None,
                poll_option_id: Some(poll_option_id),
                image_media_ids: Vec::new(),
                publish,
            })
        }
    }
}

/// An attached image must exist, belong to the caller, be `ready`, have
/// `purpose=response`, and reference this group/cycle (`03-api-contract.md`
/// §7.3). `media::get_image` is keyed by `(groupId, cycleId, imageId)`, so a
/// successful lookup already guarantees the group/cycle match.
async fn validate_response_image(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    cycle_id: &CycleId,
    image_id: &ImageId,
) -> Result<(), ApiError> {
    let image = media::get_image(&state.repo, group_id, cycle_id, image_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, cycle_id = %cycle_id, "image lookup failed");
            ApiError::internal("failed to load image")
        })?
        .ok_or_else(|| ApiError::invalid_field("imageMediaIds", format!("image `{image_id}` does not exist")))?;

    let valid = image.user_id == *caller
        && image.status == MediaStatus::Ready
        && image.purpose == ImagePurpose::Response;
    if valid {
        Ok(())
    } else {
        Err(ApiError::invalid_field(
            "imageMediaIds",
            format!("image `{image_id}` is not a ready response image owned by you in this cycle"),
        ))
    }
}

async fn require_newsletter_not_archived(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
) -> Result<(), ApiError> {
    let nl = newsletters::get_newsletter(&state.repo, group_id, cycle_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, cycle_id = %cycle_id, "newsletter lookup failed");
            ApiError::internal("failed to load newsletter")
        })?
        .ok_or_else(|| ApiError::not_found("newsletter not found"))?;

    if nl.status == NewsletterStatus::Archived {
        return Err(ApiError::new(
            ApiErrorCode::NewsletterArchived,
            "this edition has been archived",
        ));
    }
    Ok(())
}
