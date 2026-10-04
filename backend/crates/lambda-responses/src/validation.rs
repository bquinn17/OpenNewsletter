//! Request validation for response saves (`plans/03-api-contract.md` §7.3).

use domain::{ApiError, ApiErrorCode, ImageId, LockedQuestion, PollOptionId};
use shared::config::{MAX_IMAGES_PER_RESPONSE, MAX_RESPONSE_BODY_CHARS};
use std::collections::HashSet;

/// Body length, counted in chars (not bytes) — 0 to `MAX_RESPONSE_BODY_CHARS`.
pub fn body(raw: &str) -> Result<String, ApiError> {
    if raw.chars().count() > MAX_RESPONSE_BODY_CHARS {
        return Err(ApiError::invalid_field(
            "body",
            format!("body must be at most {MAX_RESPONSE_BODY_CHARS} characters"),
        ));
    }
    Ok(raw.to_owned())
}

/// Deduplicates `imageMediaIds` (preserving first-seen order) and caps the
/// result at `MAX_IMAGES_PER_RESPONSE`.
pub fn dedupe_image_ids(ids: Vec<ImageId>) -> Result<Vec<ImageId>, ApiError> {
    let mut seen = HashSet::with_capacity(ids.len());
    let mut deduped = Vec::with_capacity(ids.len());
    for id in ids {
        if seen.insert(id.clone()) {
            deduped.push(id);
        }
    }
    if deduped.len() > MAX_IMAGES_PER_RESPONSE {
        return Err(ApiError::new(
            ApiErrorCode::ImageLimitExceeded,
            format!("at most {MAX_IMAGES_PER_RESPONSE} images may be attached to a response"),
        ));
    }
    Ok(deduped)
}

/// `pollOptionId` must be one of the locked question's options.
pub fn poll_option_belongs_to_question(
    question: &LockedQuestion,
    poll_option_id: &PollOptionId,
) -> Result<(), ApiError> {
    let belongs = question
        .poll_options
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .any(|o| &o.option_id == poll_option_id);
    if belongs {
        Ok(())
    } else {
        Err(ApiError::invalid_field(
            "pollOptionId",
            format!("pollOptionId `{poll_option_id}` is not one of this question's options"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{GroupId, PollOption, QuestionId, QuestionKind};

    fn text_question() -> LockedQuestion {
        LockedQuestion {
            question_id: QuestionId::new("q1"),
            group_id: GroupId::new("g1"),
            cycle_id: domain::CycleId::new("202606"),
            kind: QuestionKind::Poll,
            prompt: "Best trailhead?".into(),
            poll_options: Some(vec![
                PollOption {
                    option_id: PollOptionId::new("opt-a"),
                    label: "North".into(),
                },
                PollOption {
                    option_id: PollOptionId::new("opt-b"),
                    label: "South".into(),
                },
            ]),
            display_order: 0,
            submitted_by: domain::UserId::new("u1"),
            is_anonymous: false,
            locked_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn body_within_limit_is_accepted() {
        assert_eq!(body("hello").unwrap(), "hello");
        assert_eq!(body("").unwrap(), "");
    }

    #[test]
    fn overlong_body_is_rejected() {
        let err = body(&"a".repeat(MAX_RESPONSE_BODY_CHARS + 1)).expect_err("too long");
        assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    }

    #[test]
    fn duplicate_image_ids_are_removed() {
        let ids = vec![ImageId::new("i1"), ImageId::new("i1"), ImageId::new("i2")];
        let deduped = dedupe_image_ids(ids).unwrap();
        assert_eq!(deduped, vec![ImageId::new("i1"), ImageId::new("i2")]);
    }

    #[test]
    fn too_many_distinct_image_ids_is_rejected() {
        let ids: Vec<ImageId> = (0..MAX_IMAGES_PER_RESPONSE + 1)
            .map(|i| ImageId::new(format!("i{i}")))
            .collect();
        let err = dedupe_image_ids(ids).expect_err("over the cap");
        assert_eq!(err.code, ApiErrorCode::ImageLimitExceeded);
    }

    #[test]
    fn poll_option_must_belong_to_the_question() {
        let question = text_question();
        assert!(poll_option_belongs_to_question(&question, &PollOptionId::new("opt-a")).is_ok());
        let err = poll_option_belongs_to_question(&question, &PollOptionId::new("opt-z"))
            .expect_err("not one of the options");
        assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    }
}
