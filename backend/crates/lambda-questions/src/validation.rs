//! Request validation for candidate questions (`plans/03-api-contract.md` §6.2).

use domain::api::CreatePollOptionRequest;
use domain::{ApiError, PollOption, PollOptionId, QuestionKind};
use shared::config::{
    MAX_POLL_OPTIONS, MAX_POLL_OPTION_LABEL_CHARS, MAX_PROMPT_CHARS, MIN_POLL_OPTIONS,
    MIN_PROMPT_CHARS,
};
use std::collections::HashSet;

pub fn prompt(raw: &str) -> Result<String, ApiError> {
    let trimmed = raw.trim();
    let len = trimmed.chars().count();
    if !(MIN_PROMPT_CHARS..=MAX_PROMPT_CHARS).contains(&len) {
        return Err(ApiError::validation(format!(
            "prompt must be between {MIN_PROMPT_CHARS} and {MAX_PROMPT_CHARS} characters"
        )));
    }
    Ok(trimmed.to_owned())
}

/// Validates `pollOptions` against `kind` and returns the poll options to store
/// (`None` for `kind=text`), each assigned a fresh `optionId`.
pub fn poll_options(
    kind: QuestionKind,
    raw: Option<&[CreatePollOptionRequest]>,
) -> Result<Option<Vec<PollOption>>, ApiError> {
    match kind {
        QuestionKind::Text => {
            if raw.is_some_and(|opts| !opts.is_empty()) {
                return Err(ApiError::validation(
                    "pollOptions must be omitted for kind=text",
                ));
            }
            Ok(None)
        }
        QuestionKind::Poll => {
            let opts = raw
                .filter(|opts| !opts.is_empty())
                .ok_or_else(|| ApiError::validation("pollOptions is required for kind=poll"))?;
            if !(MIN_POLL_OPTIONS..=MAX_POLL_OPTIONS).contains(&opts.len()) {
                return Err(ApiError::validation(format!(
                    "pollOptions must have between {MIN_POLL_OPTIONS} and {MAX_POLL_OPTIONS} options"
                )));
            }

            let mut seen = HashSet::with_capacity(opts.len());
            let mut result = Vec::with_capacity(opts.len());
            for opt in opts {
                let label = opt.label.trim();
                if label.is_empty() || label.chars().count() > MAX_POLL_OPTION_LABEL_CHARS {
                    return Err(ApiError::validation(format!(
                        "pollOptions labels must be 1 to {MAX_POLL_OPTION_LABEL_CHARS} characters"
                    )));
                }
                if !seen.insert(label.to_owned()) {
                    return Err(ApiError::validation(format!(
                        "pollOptions labels must be unique; \"{label}\" is duplicated"
                    )));
                }
                result.push(PollOption {
                    option_id: PollOptionId::generate(),
                    label: label.to_owned(),
                });
            }
            Ok(Some(result))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::ApiErrorCode;

    fn opt(label: &str) -> CreatePollOptionRequest {
        CreatePollOptionRequest {
            label: label.to_owned(),
        }
    }

    #[test]
    fn prompt_is_trimmed() {
        assert_eq!(prompt("  Hello?  ").unwrap(), "Hello?");
    }

    #[test]
    fn short_prompt_is_rejected() {
        let err = prompt("hi").expect_err("too short");
        assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    }

    #[test]
    fn overlong_prompt_is_rejected() {
        assert!(prompt(&"a".repeat(MAX_PROMPT_CHARS + 1)).is_err());
    }

    #[test]
    fn text_kind_rejects_poll_options() {
        let err = poll_options(QuestionKind::Text, Some(&[opt("Mt Si")]))
            .expect_err("text must not carry pollOptions");
        assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    }

    #[test]
    fn text_kind_allows_absent_poll_options() {
        assert_eq!(poll_options(QuestionKind::Text, None).unwrap(), None);
    }

    #[test]
    fn poll_kind_requires_at_least_two_options() {
        assert!(poll_options(QuestionKind::Poll, Some(&[opt("Mt Si")])).is_err());
    }

    #[test]
    fn poll_kind_rejects_duplicate_labels() {
        let err = poll_options(QuestionKind::Poll, Some(&[opt("Mt Si"), opt("Mt Si")]))
            .expect_err("duplicate labels");
        assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    }

    #[test]
    fn poll_kind_accepts_valid_options() {
        let result = poll_options(QuestionKind::Poll, Some(&[opt("Mt Si"), opt("Lake 22")]))
            .expect("valid poll");
        let options = result.expect("Some for poll kind");
        assert_eq!(options.len(), 2);
        assert_eq!(options[0].label, "Mt Si");
        assert_eq!(options[1].label, "Lake 22");
    }

    #[test]
    fn poll_kind_rejects_too_many_options() {
        let opts: Vec<_> = (0..MAX_POLL_OPTIONS + 1)
            .map(|i| opt(&format!("Option {i}")))
            .collect();
        assert!(poll_options(QuestionKind::Poll, Some(&opts)).is_err());
    }
}
