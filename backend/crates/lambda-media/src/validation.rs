//! Request validation for the media routes (`plans/03-api-contract.md` §9).

use base64::Engine;
use domain::{ApiError, ApiErrorCode, ImageMimeType};
use shared::config::MAX_IMAGE_CAPTION_CHARS;

/// `mimeType` for `POST /uploads` — jpeg/png/webp/gif. An unrecognized
/// string (including a type that's simply not one we accept, like an SVG)
/// is `IMAGE_BAD_TYPE` (415), never a generic 422 — which is why the DTO
/// field is a bare `String` rather than an enum that would fail JSON
/// deserialization first (`08-media-uploads.md` §1).
pub fn parse_image_mime_type(raw: &str) -> Result<ImageMimeType, ApiError> {
    match raw {
        "image/jpeg" => Ok(ImageMimeType::Jpeg),
        "image/png" => Ok(ImageMimeType::Png),
        "image/webp" => Ok(ImageMimeType::Webp),
        "image/gif" => Ok(ImageMimeType::Gif),
        other => Err(bad_type(other)),
    }
}

/// `mimeType` for `POST /avatars` — jpeg/png/webp only; `image/gif` (valid
/// for a response image) and anything else is `IMAGE_BAD_TYPE`.
pub fn parse_avatar_mime_type(raw: &str) -> Result<ImageMimeType, ApiError> {
    match raw {
        "image/jpeg" => Ok(ImageMimeType::Jpeg),
        "image/png" => Ok(ImageMimeType::Png),
        "image/webp" => Ok(ImageMimeType::Webp),
        other => Err(bad_type(other)),
    }
}

fn bad_type(mime: &str) -> ApiError {
    ApiError::new(
        ApiErrorCode::ImageBadType,
        format!("mimeType `{mime}` is not a supported image type"),
    )
}

/// The file extension used in S3 keys for a given mime type
/// (`08-media-uploads.md` §2).
pub fn extension_for(mime: ImageMimeType) -> &'static str {
    match mime {
        ImageMimeType::Jpeg => "jpg",
        ImageMimeType::Png => "png",
        ImageMimeType::Webp => "webp",
        ImageMimeType::Gif => "gif",
    }
}

/// `byteSize` must be nonzero and no larger than `max_bytes`.
pub fn validate_byte_size(byte_size: u64, max_bytes: u64) -> Result<(), ApiError> {
    if byte_size == 0 {
        return Err(ApiError::invalid_field(
            "byteSize",
            "byteSize must be greater than zero",
        ));
    }
    if byte_size > max_bytes {
        return Err(ApiError::new(
            ApiErrorCode::ImageTooLarge,
            format!("byteSize must be at most {max_bytes} bytes"),
        ));
    }
    Ok(())
}

/// `sha256`, when present, must be a standard-base64 encoding of exactly 32
/// bytes (44 chars, per `03-api-contract.md` §9.1's `CreateUploadRequest`).
pub fn validate_sha256(sha256: Option<&str>) -> Result<(), ApiError> {
    let Some(sha256) = sha256 else {
        return Ok(());
    };
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(sha256)
        .map_err(|_| invalid_sha256())?;
    if decoded.len() != 32 {
        return Err(invalid_sha256());
    }
    Ok(())
}

fn invalid_sha256() -> ApiError {
    ApiError::invalid_field(
        "sha256",
        "sha256 must be a standard-base64 encoding of 32 bytes",
    )
}

/// `caption` for `PATCH /uploads/{imageId}` (§9.6). Trims whitespace; an
/// empty or all-whitespace string clears the caption, same as `null`.
pub fn validate_caption(raw: Option<String>) -> Result<Option<String>, ApiError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > MAX_IMAGE_CAPTION_CHARS {
        return Err(ApiError::invalid_field(
            "caption",
            format!("caption must be at most {MAX_IMAGE_CAPTION_CHARS} characters"),
        ));
    }
    Ok(Some(trimmed.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_mime_type_accepts_the_allowed_set() {
        assert_eq!(
            parse_image_mime_type("image/jpeg").unwrap(),
            ImageMimeType::Jpeg
        );
        assert_eq!(
            parse_image_mime_type("image/png").unwrap(),
            ImageMimeType::Png
        );
        assert_eq!(
            parse_image_mime_type("image/webp").unwrap(),
            ImageMimeType::Webp
        );
        assert_eq!(
            parse_image_mime_type("image/gif").unwrap(),
            ImageMimeType::Gif
        );
    }

    #[test]
    fn image_mime_type_rejects_svg_as_bad_type_not_validation_failed() {
        let err = parse_image_mime_type("image/svg+xml").expect_err("svg is not allowed");
        assert_eq!(err.code, ApiErrorCode::ImageBadType);
    }

    #[test]
    fn avatar_mime_type_rejects_gif() {
        let err = parse_avatar_mime_type("image/gif").expect_err("gif is not allowed for avatars");
        assert_eq!(err.code, ApiErrorCode::ImageBadType);
    }

    #[test]
    fn byte_size_rejects_zero() {
        let err = validate_byte_size(0, 1_000).expect_err("zero is invalid");
        assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    }

    #[test]
    fn byte_size_rejects_over_the_cap() {
        let err = validate_byte_size(1_001, 1_000).expect_err("over the cap");
        assert_eq!(err.code, ApiErrorCode::ImageTooLarge);
    }

    #[test]
    fn byte_size_accepts_within_bounds() {
        validate_byte_size(1, 1_000).unwrap();
        validate_byte_size(1_000, 1_000).unwrap();
    }

    #[test]
    fn sha256_accepts_absent() {
        validate_sha256(None).unwrap();
    }

    #[test]
    fn sha256_accepts_a_valid_32_byte_encoding() {
        let encoded = base64::engine::general_purpose::STANDARD.encode([0u8; 32]);
        validate_sha256(Some(&encoded)).unwrap();
    }

    #[test]
    fn sha256_rejects_the_wrong_length() {
        let encoded = base64::engine::general_purpose::STANDARD.encode([0u8; 16]);
        let err = validate_sha256(Some(&encoded)).expect_err("16 bytes is not 32");
        assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    }

    #[test]
    fn sha256_rejects_non_base64() {
        let err = validate_sha256(Some("not base64!!")).expect_err("not valid base64");
        assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    }

    #[test]
    fn caption_trims_and_clears_on_blank() {
        assert_eq!(
            validate_caption(Some("  hi  ".into())).unwrap(),
            Some("hi".into())
        );
        assert_eq!(validate_caption(Some("   ".into())).unwrap(), None);
        assert_eq!(validate_caption(None).unwrap(), None);
    }

    #[test]
    fn caption_rejects_overlong() {
        let err =
            validate_caption(Some("a".repeat(MAX_IMAGE_CAPTION_CHARS + 1))).expect_err("too long");
        assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    }
}
