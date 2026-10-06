//! Business logic for the media routes (`plans/03-api-contract.md` §9).

use crate::presign;
use crate::state::AppState;
use crate::{cloudfront, validation};
use chrono::{Duration, Utc};
use domain::api::{
    AvatarMediaResponse, CreateAvatarRequest, CreateAvatarResponse, CreateUploadRequest,
    CreateUploadResponse, ImageMediaResponse, MediaCookieResponse, PatchUploadRequest,
};
use domain::{
    ApiError, ApiErrorCode, AvatarId, AvatarMedia, CycleId, GroupId, ImageId, ImageMedia,
    ImagePurpose, MediaStatus, NewsletterStatus, UserId,
};
use persistence::{auth, media, newsletters, questions, responses, users};
use shared::config::{MAX_AVATAR_BYTES, MAX_IMAGES_PER_RESPONSE, MAX_IMAGE_BYTES};

/// `POST /uploads` (§9.1). Validation order follows
/// `08-media-uploads.md` §3.1: membership, mime type, byte size, sha256
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
    Ok(to_image_response(state, image))
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
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, cycle_id = %cycle_id, "caption update failed");
            ApiError::internal("failed to update caption")
        })?;

    let updated = load_image(state, group_id, cycle_id, image_id).await?;
    Ok(to_image_response(state, updated))
}

/// `DELETE /uploads/{imageId}` (§9.4). Caller must own the image. Refused
/// with `IMAGE_IN_USE` when the caller's own *published* response for the
/// image's question still references it, or (`purpose=comment`, added M10)
/// a live comment still has it claimed via `attachedCommentId`
/// (`09-engagement.md` §1.5, `03-api-contract.md` §8.2). Otherwise
/// idempotent: an already-`failed` image is a no-op success.
pub async fn delete_upload(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    cycle_id: &CycleId,
    image_id: &ImageId,
) -> Result<(), ApiError> {
    auth::require_membership(&state.repo, caller, group_id, false).await?;
    let image = load_image(state, group_id, cycle_id, image_id).await?;
    require_owner(caller, &image)?;

    if image.purpose == ImagePurpose::Comment && image.attached_comment_id.is_some() {
        return Err(ApiError::new(
            ApiErrorCode::ImageInUse,
            "image is attached to a comment",
        ));
    }

    if let Some(question_id) = &image.question_id {
        let response = responses::get_my_response(&state.repo, group_id, cycle_id, question_id, caller)
            .await
            .map_err(|e| {
                tracing::error!(error = ?e, group_id = %group_id, cycle_id = %cycle_id, "response lookup failed");
                ApiError::internal("failed to check image usage")
            })?;
        if let Some(response) = response {
            let in_use =
                response.published_at.is_some() && response.image_media_ids.contains(image_id);
            if in_use {
                return Err(ApiError::new(
                    ApiErrorCode::ImageInUse,
                    "image is attached to your published response",
                ));
            }
        }
    }

    media::mark_image_deleted(&state.repo, group_id, cycle_id, image_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, cycle_id = %cycle_id, "image delete failed");
            ApiError::internal("failed to delete image")
        })?;
    Ok(())
}

/// The caller's signed CloudFront cookie values, plus the `Set-Cookie`
/// headers to emit (empty when `MEDIA_COOKIE_DOMAIN` is unset).
#[derive(Debug)]
pub struct MediaCookie {
    pub body: MediaCookieResponse,
    pub set_cookie_headers: Vec<String>,
}

/// `GET /media-cookie` (§9.5, `08-media-uploads.md` §4.3).
pub async fn get_media_cookie(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
) -> Result<MediaCookie, ApiError> {
    auth::require_membership(&state.repo, caller, group_id, false).await?;

    let resource = format!(
        "{}/img/{}/*",
        state.cdn_base_url.trim_end_matches('/'),
        group_id
    );
    let expires_at = Utc::now() + Duration::hours(1);
    let policy = cloudfront::policy_json(&resource, expires_at.timestamp());

    let key = state.signing_key().await?;
    let signature = cloudfront::sign_policy(key, policy.as_bytes()).map_err(|e| {
        tracing::error!(error = ?e, "failed to sign cloudfront policy");
        ApiError::internal("failed to sign cloudfront policy")
    })?;

    let policy_b64 = cloudfront::modified_base64_encode(policy.as_bytes());
    let signature_b64 = cloudfront::modified_base64_encode(&signature);

    let mut set_cookie_headers = Vec::new();
    if !state.media_cookie_domain.is_empty() {
        let path = format!("/img/{group_id}/");
        let domain = &state.media_cookie_domain;
        set_cookie_headers.push(format!(
            "CloudFront-Policy={policy_b64}; Domain={domain}; Path={path}; Max-Age=3600; HttpOnly; Secure; SameSite=None"
        ));
        set_cookie_headers.push(format!(
            "CloudFront-Signature={signature_b64}; Domain={domain}; Path={path}; Max-Age=3600; HttpOnly; Secure; SameSite=None"
        ));
        set_cookie_headers.push(format!(
            "CloudFront-Key-Pair-Id={}; Domain={domain}; Path={path}; Max-Age=3600; HttpOnly; Secure; SameSite=None",
            state.cdn_key_pair_id
        ));
    }

    Ok(MediaCookie {
        body: MediaCookieResponse {
            policy: policy_b64,
            signature: signature_b64,
            key_pair_id: state.cdn_key_pair_id.clone(),
            expires_at,
        },
        set_cookie_headers,
    })
}

/// `POST /avatars` (§9.7). No group/membership check — avatars cross group
/// boundaries by design (`08-media-uploads.md` §11).
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
        state.avatar_url(&avatar.avatar_id)
    } else {
        None
    };
    Ok(AvatarMediaResponse::new(avatar, avatar_url))
}

/// `DELETE /avatars/{avatarId}` (§9.7). Idempotent; clears
/// `User.avatar_media_id` if it still points at this avatar.
pub async fn delete_avatar(
    state: &AppState,
    caller: &UserId,
    avatar_id: &AvatarId,
) -> Result<(), ApiError> {
    load_avatar(state, caller, avatar_id).await?;

    media::mark_avatar_deleted(&state.repo, caller, avatar_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, user_id = %caller, "avatar delete failed");
            ApiError::internal("failed to delete avatar")
        })?;
    users::clear_user_avatar_if(&state.repo, caller, avatar_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, user_id = %caller, "avatar unlink failed");
            ApiError::internal("failed to unlink avatar")
        })?;
    Ok(())
}

async fn load_image(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
    image_id: &ImageId,
) -> Result<ImageMedia, ApiError> {
    media::get_image(&state.repo, group_id, cycle_id, image_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, cycle_id = %cycle_id, "image lookup failed");
            ApiError::internal("failed to load image")
        })?
        .ok_or_else(|| ApiError::not_found("image not found"))
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

fn to_image_response(state: &AppState, image: ImageMedia) -> ImageMediaResponse {
    let (display_url, thumb_url) = if image.status == MediaStatus::Ready {
        (
            image
                .display_key
                .as_deref()
                .and_then(|k| state.image_url(k)),
            image.thumb_key.as_deref().and_then(|k| state.image_url(k)),
        )
    } else {
        (None, None)
    };
    ImageMediaResponse::new(image, display_url, thumb_url)
}
