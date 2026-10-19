//! Business logic for the media routes (`docs/design/api.md` §9).

use crate::presign;
use crate::state::AppState;
use crate::validation;
use chrono::Utc;
use domain::api::{
    AvatarMediaResponse, CreateAvatarRequest, CreateAvatarResponse, CreateUploadRequest,
    CreateUploadResponse, ImageMediaResponse, PatchUploadRequest,
};
use domain::{
    ApiError, ApiErrorCode, AvatarId, AvatarMedia, CycleId, GroupId, ImageId, ImageMedia,
    ImagePurpose, MediaStatus, NewsletterStatus, QuestionId, ResponseStatus, UserId,
};
use persistence::{auth, media, newsletters, questions, responses, users, RepoError};
use shared::config::{MAX_AVATAR_BYTES, MAX_IMAGES_PER_RESPONSE, MAX_IMAGE_BYTES};

/// `POST /uploads` (§9.1). Validation order follows
/// `docs/design/media.md` §3.1: membership, mime type, byte size, sha256
/// shape, newsletter existence + cycle status for `purpose`, the question
/// being locked into this cycle, then (for `purpose=response`) the
/// per-answer image cap.
pub async fn create_upload(
    state: &AppState,
    caller: &UserId,
    request: CreateUploadRequest,
) -> Result<CreateUploadResponse, ApiError> {
    auth::require_membership(&state.repo, caller, &request.group_id, false).await?;

    let mime = validation::parse_image_mime_type(&request.mime_type)?;
    validation::validate_byte_size(request.byte_size, MAX_IMAGE_BYTES)?;
    validation::validate_sha256(request.sha256.as_deref())?;
    let purpose = request.purpose.unwrap_or(ImagePurpose::Response);

    let newsletter = newsletters::get_newsletter(&state.repo, &request.group_id, &request.cycle_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %request.group_id, cycle_id = %request.cycle_id, "newsletter lookup failed");
            ApiError::internal("failed to load newsletter")
        })?
        .ok_or_else(|| ApiError::not_found("newsletter not found"))?;

    if newsletter.status == NewsletterStatus::Archived {
        return Err(ApiError::new(
            ApiErrorCode::NewsletterArchived,
            "this edition has been archived",
        ));
    }
    match purpose {
        ImagePurpose::Response if newsletter.status != NewsletterStatus::Open => {
            return Err(ApiError::new(
                ApiErrorCode::CycleNotOpen,
                "this cycle is not open for responses",
            ));
        }
        ImagePurpose::Comment if newsletter.status != NewsletterStatus::Published => {
            return Err(ApiError::new(
                ApiErrorCode::CycleNotPublished,
                "this cycle is not published",
            ));
        }
        _ => {}
    }

    questions::get_locked_question(
        &state.repo,
        &request.group_id,
        &request.cycle_id,
        &request.question_id,
    )
    .await
    .map_err(|e| {
        tracing::error!(error = ?e, group_id = %request.group_id, cycle_id = %request.cycle_id, "locked question lookup failed");
        ApiError::internal("failed to load question")
    })?
    .ok_or_else(|| {
        ApiError::invalid_field(
            "questionId",
            format!("questionId `{}` is not a locked question in this cycle", request.question_id),
        )
    })?;

    if purpose == ImagePurpose::Response {
        let count = media::count_active_response_images(
            &state.repo,
            &request.group_id,
            &request.cycle_id,
            caller,
            &request.question_id,
        )
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %request.group_id, cycle_id = %request.cycle_id, "response image count failed");
            ApiError::internal("failed to count your images")
        })?;
        if count >= MAX_IMAGES_PER_RESPONSE {
            return Err(ApiError::new(
                ApiErrorCode::ImageLimitExceeded,
                format!("at most {MAX_IMAGES_PER_RESPONSE} images may be attached to a response"),
            ));
        }
    }

    let image_id = ImageId::generate();
    let ext = validation::extension_for(mime);
    let original_key = format!(
        "uploads/{}/{}/{}/{}/{}.{}",
        request.group_id, request.cycle_id, request.question_id, caller, image_id, ext
    );
    let now = Utc::now();
    let image = ImageMedia {
        image_id: image_id.clone(),
        user_id: caller.clone(),
        group_id: request.group_id.clone(),
        cycle_id: request.cycle_id.clone(),
        question_id: Some(request.question_id.clone()),
        purpose,
        mime_type: mime,
        original_key: original_key.clone(),
        display_key: None,
        thumb_key: None,
        status: MediaStatus::Pending,
        bytes: request.byte_size,
        width: None,
        height: None,
        caption: None,
        uploaded_at: now,
        processed_at: None,
        error_message: None,
        attached_comment_id: None,
    };
    media::put_image(&state.repo, &image).await.map_err(|e| {
        tracing::error!(error = ?e, group_id = %request.group_id, cycle_id = %request.cycle_id, "image row write failed");
        ApiError::internal("failed to create image")
    })?;

    let presigned = presign::presign_put(
        &state.s3,
        &state.media_originals_bucket,
        &original_key,
        &request.mime_type,
        request.sha256.as_deref(),
    )
    .await?;

    Ok(CreateUploadResponse {
        image_id,
        upload_url: presigned.upload_url,
        headers: presigned.headers,
        expires_in_seconds: presign::EXPIRES_IN_SECONDS,
    })
}

/// `GET /uploads/{imageId}` (§9.3). Any member of `groupId` may read; there
/// is no ownership check on reads, only on writes.
pub async fn get_upload(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    cycle_id: &CycleId,
    image_id: &ImageId,
) -> Result<ImageMediaResponse, ApiError> {
    auth::require_membership(&state.repo, caller, group_id, false).await?;
    let image = load_image(state, group_id, cycle_id, image_id).await?;
    to_image_response(state, image).await
}

/// `POST /uploads/{imageId}/complete` (§9.2) — a convenience alias for
/// `GET /uploads/{imageId}`; processing itself is always driven by the S3
/// trigger, never by this call.
pub async fn complete_upload(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    cycle_id: &CycleId,
    image_id: &ImageId,
) -> Result<ImageMediaResponse, ApiError> {
    get_upload(state, caller, group_id, cycle_id, image_id).await
}

/// `PATCH /uploads/{imageId}` (§9.6). Caller must own the image; allowed
/// regardless of the image's processing status.
pub async fn patch_upload(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    cycle_id: &CycleId,
    image_id: &ImageId,
    request: PatchUploadRequest,
) -> Result<ImageMediaResponse, ApiError> {
    auth::require_membership(&state.repo, caller, group_id, false).await?;
    let image = load_image(state, group_id, cycle_id, image_id).await?;
    require_owner(caller, &image)?;

    let caption = validation::validate_caption(request.caption)?;
    media::set_image_caption(&state.repo, group_id, cycle_id, image_id, caption.as_deref())
        .await
        .map_err(|e| match e {
            // Deleted between the load above and this write.
            RepoError::ConditionalCheckFailed => ApiError::not_found("image not found"),
            e => {
                tracing::error!(error = ?e, group_id = %group_id, cycle_id = %cycle_id, "caption update failed");
                ApiError::internal("failed to update caption")
            }
        })?;

    let updated = load_image(state, group_id, cycle_id, image_id).await?;
    to_image_response(state, updated).await
}

/// `DELETE /uploads/{imageId}` (§9.4). Caller must own the image. Refused
/// with `IMAGE_IN_USE` when the caller's own *published* response for the
/// image's question still references it, or (`purpose=comment`, added M10)
/// a live comment still has it claimed via `attachedCommentId`
/// (`docs/design/engagement.md` §1.5, `docs/design/api.md` §8.2). If the
/// caller's *draft* still lists it, the id is dropped from the draft first
/// (a stale tab or older PWA that didn't save the draft without it). Then
/// deletes the S3 objects and the row (`docs/design/media.md` §8.0).
/// Idempotent: an already-deleted image is a no-op success.
pub async fn delete_upload(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    cycle_id: &CycleId,
    image_id: &ImageId,
) -> Result<(), ApiError> {
    auth::require_membership(&state.repo, caller, group_id, false).await?;
    let Some(image) = find_image(state, group_id, cycle_id, image_id).await? else {
        return Ok(());
    };
    require_owner(caller, &image)?;

    if image.purpose == ImagePurpose::Comment && image.attached_comment_id.is_some() {
        return Err(ApiError::new(
            ApiErrorCode::ImageInUse,
            "image is attached to a comment",
        ));
    }

    if let Some(question_id) = &image.question_id {
        release_from_my_response(state, caller, group_id, cycle_id, question_id, image_id).await?;
    }

    media_delete::delete_image_media(&state.repo, &state.image_deleter, &image)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, cycle_id = %cycle_id, image_id = %image_id, "image delete failed");
            ApiError::internal("failed to delete image")
        })
}

/// Rounds of read-then-strip before [`release_from_my_response`] gives up on
/// a response that keeps changing under it.
const RELEASE_ATTEMPTS: usize = 3;

/// The response half of [`delete_upload`]: refuse if the caller's published
/// response lists the image, and drop it from their draft if that does.
/// Runs before any S3 delete, so a draft published mid-delete is refused
/// with nothing yet removed. The strip is conditioned on the draft being
/// unchanged; a concurrent save or publish fails it, and this rereads.
async fn release_from_my_response(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    image_id: &ImageId,
) -> Result<(), ApiError> {
    for _ in 0..RELEASE_ATTEMPTS {
        let response =
            responses::get_my_response(&state.repo, group_id, cycle_id, question_id, caller)
                .await
                .map_err(|e| {
                    tracing::error!(error = ?e, group_id = %group_id, cycle_id = %cycle_id, "response lookup failed");
                    ApiError::internal("failed to check image usage")
                })?;
        let Some(response) = response.filter(|r| r.image_media_ids.contains(image_id)) else {
            return Ok(());
        };
        if response.status == ResponseStatus::Published {
            return Err(ApiError::new(
                ApiErrorCode::ImageInUse,
                "image is attached to your published response",
            ));
        }
        match responses::remove_image_from_draft(&state.repo, &response, image_id, Utc::now()).await
        {
            Ok(()) => return Ok(()),
            Err(RepoError::ConditionalCheckFailed) => continue,
            Err(e) => {
                tracing::error!(error = ?e, group_id = %group_id, cycle_id = %cycle_id, "removing the image from the draft failed");
                return Err(ApiError::internal(
                    "failed to remove the image from your draft",
                ));
            }
        }
    }
    tracing::warn!(group_id = %group_id, cycle_id = %cycle_id, image_id = %image_id, "draft kept changing while releasing an image");
    Err(ApiError::internal("your draft kept changing; try again"))
}

/// `POST /avatars` (§9.7). No group/membership check — avatars cross group
/// boundaries by design (`docs/design/media.md` §11).
pub async fn create_avatar(
    state: &AppState,
    caller: &UserId,
    request: CreateAvatarRequest,
) -> Result<CreateAvatarResponse, ApiError> {
    let mime = validation::parse_avatar_mime_type(&request.mime_type)?;
    validation::validate_byte_size(request.byte_size, MAX_AVATAR_BYTES)?;
    validation::validate_sha256(request.sha256.as_deref())?;

    let avatar_id = AvatarId::generate();
    let ext = validation::extension_for(mime);
    let original_key = format!("uploads/{caller}/{avatar_id}.{ext}");
    let avatar = AvatarMedia {
        avatar_id: avatar_id.clone(),
        user_id: caller.clone(),
        mime_type: mime,
        original_key: original_key.clone(),
        display_key: None,
        status: MediaStatus::Pending,
        bytes: request.byte_size,
        uploaded_at: Utc::now(),
        processed_at: None,
        error_message: None,
    };
    media::put_avatar(&state.repo, &avatar).await.map_err(|e| {
        tracing::error!(error = ?e, user_id = %caller, "avatar row write failed");
        ApiError::internal("failed to create avatar")
    })?;

    let presigned = presign::presign_put(
        &state.s3,
        &state.avatars_originals_bucket,
        &original_key,
        &request.mime_type,
        request.sha256.as_deref(),
    )
    .await?;

    Ok(CreateAvatarResponse {
        avatar_id,
        upload_url: presigned.upload_url,
        headers: presigned.headers,
        expires_in_seconds: presign::EXPIRES_IN_SECONDS,
    })
}

/// `GET /avatars/{avatarId}` (§9.7). Looked up under the caller's own `USER#`
/// partition, so another user's avatar is a plain 404.
pub async fn get_avatar(
    state: &AppState,
    caller: &UserId,
    avatar_id: &AvatarId,
) -> Result<AvatarMediaResponse, ApiError> {
    let avatar = load_avatar(state, caller, avatar_id).await?;
    let avatar_url = if avatar.status == MediaStatus::Ready {
        Some(state.media_urls.avatar_url(&avatar.avatar_id).await?)
    } else {
        None
    };
    Ok(AvatarMediaResponse::new(avatar, avatar_url))
}

/// `DELETE /avatars/{avatarId}` (§9.7). Unlinks it from `User.avatar_media_id`
/// if it's the current one, then deletes its S3 objects and row
/// (`docs/design/media.md` §11.4). Unlinking first means a failed delete never
/// leaves the profile pointing at a half-deleted avatar. Idempotent: the
/// lookup is under the caller's own partition, so an already-deleted avatar
/// (or another user's id, which touches nothing of theirs) is a no-op success.
pub async fn delete_avatar(
    state: &AppState,
    caller: &UserId,
    avatar_id: &AvatarId,
) -> Result<(), ApiError> {
    let avatar = media::get_avatar(&state.repo, caller, avatar_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, user_id = %caller, "avatar lookup failed");
            ApiError::internal("failed to load avatar")
        })?;

    users::clear_user_avatar_if(&state.repo, caller, avatar_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, user_id = %caller, "avatar unlink failed");
            ApiError::internal("failed to unlink avatar")
        })?;

    let Some(avatar) = avatar else {
        return Ok(());
    };
    media_delete::delete_avatar_media(&state.repo, &state.avatar_deleter, &avatar)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, user_id = %caller, avatar_id = %avatar_id, "avatar delete failed");
            ApiError::internal("failed to delete avatar")
        })
}

async fn load_image(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
    image_id: &ImageId,
) -> Result<ImageMedia, ApiError> {
    find_image(state, group_id, cycle_id, image_id)
        .await?
        .ok_or_else(|| ApiError::not_found("image not found"))
}

async fn find_image(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
    image_id: &ImageId,
) -> Result<Option<ImageMedia>, ApiError> {
    media::get_image(&state.repo, group_id, cycle_id, image_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, cycle_id = %cycle_id, "image lookup failed");
            ApiError::internal("failed to load image")
        })
}

async fn load_avatar(
    state: &AppState,
    caller: &UserId,
    avatar_id: &AvatarId,
) -> Result<AvatarMedia, ApiError> {
    media::get_avatar(&state.repo, caller, avatar_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, user_id = %caller, "avatar lookup failed");
            ApiError::internal("failed to load avatar")
        })?
        .ok_or_else(|| ApiError::not_found("avatar not found"))
}

fn require_owner(caller: &UserId, image: &ImageMedia) -> Result<(), ApiError> {
    if image.user_id == *caller {
        Ok(())
    } else {
        Err(ApiError::forbidden("you do not own this image"))
    }
}

async fn to_image_response(
    state: &AppState,
    image: ImageMedia,
) -> Result<ImageMediaResponse, ApiError> {
    let (display_url, thumb_url) = match state.media_urls.ready_image_urls(&image).await? {
        Some(urls) => (Some(urls.display_url), Some(urls.thumb_url)),
        None => (None, None),
    };
    Ok(ImageMediaResponse::new(image, display_url, thumb_url))
}
