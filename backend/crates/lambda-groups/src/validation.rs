//! Request validation for the group and profile routes
//! (`plans/03-api-contract.md` §2.3 and §4.3).

use crate::dto::{CycleSettingsPatch, NotificationSettingsPatch, PatchGroupRequest};
use domain::{ApiError, CycleSettings, Group, NotificationSettings};
use persistence::groups::GroupPatch;
use shared::config::{
    AVATAR_COLOR_SLUGS, GRADIENT_SLUGS, MAX_DISPLAY_NAME_CHARS, MAX_QUESTIONS_PER_CYCLE,
    MAX_REMINDER_OFFSET_HOURS, MAX_RESPONSE_WINDOW_DAYS, MIN_QUESTIONS_PER_CYCLE,
    MIN_RESPONSE_WINDOW_DAYS,
};

pub fn display_name(raw: &str) -> Result<String, ApiError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ApiError::validation("displayName must not be blank"));
    }
    if trimmed.chars().count() > MAX_DISPLAY_NAME_CHARS {
        return Err(ApiError::validation(format!(
            "displayName must be at most {MAX_DISPLAY_NAME_CHARS} characters"
        )));
    }
    Ok(trimmed.to_owned())
}

pub fn avatar_color(raw: &str) -> Result<(), ApiError> {
    if AVATAR_COLOR_SLUGS.contains(&raw) {
        return Ok(());
    }
    Err(ApiError::validation(format!(
        "avatarColor must be one of: {}",
        AVATAR_COLOR_SLUGS.join(", ")
    )))
}

fn gradient(raw: &str) -> Result<(), ApiError> {
    if GRADIENT_SLUGS.contains(&raw) {
        return Ok(());
    }
    Err(ApiError::validation(format!(
        "gradient must be one of: {}",
        GRADIENT_SLUGS.join(", ")
    )))
}

fn timezone(raw: &str) -> Result<(), ApiError> {
    if raw.parse::<chrono_tz::Tz>().is_ok() {
        return Ok(());
    }
    Err(ApiError::validation(format!(
        "timezone `{raw}` is not a valid IANA time zone"
    )))
}

/// Merge a patch onto the group's current settings and validate the result, so a
/// partial patch can't produce an invalid combination (e.g. raising the vote cap
/// above a `questionsPerCycle` the request didn't mention).
fn cycle_settings(
    current: &CycleSettings,
    patch: &CycleSettingsPatch,
) -> Result<CycleSettings, ApiError> {
    let merged = CycleSettings {
        questions_per_cycle: patch
            .questions_per_cycle
            .unwrap_or(current.questions_per_cycle),
        votes_per_user_per_cycle: patch
            .votes_per_user_per_cycle
            .unwrap_or(current.votes_per_user_per_cycle),
        response_window_days: patch
            .response_window_days
            .unwrap_or(current.response_window_days),
        auto_publish: current.auto_publish,
    };

    if !(MIN_QUESTIONS_PER_CYCLE..=MAX_QUESTIONS_PER_CYCLE).contains(&merged.questions_per_cycle) {
        return Err(ApiError::validation(format!(
            "questionsPerCycle must be between {MIN_QUESTIONS_PER_CYCLE} and {MAX_QUESTIONS_PER_CYCLE}"
        )));
    }
    if !(MIN_RESPONSE_WINDOW_DAYS..=MAX_RESPONSE_WINDOW_DAYS).contains(&merged.response_window_days)
    {
        return Err(ApiError::validation(format!(
            "responseWindowDays must be between {MIN_RESPONSE_WINDOW_DAYS} and {MAX_RESPONSE_WINDOW_DAYS}"
        )));
    }
    // Each (voter, question) pair is unique, so a member can never cast more
    // upvotes than there are candidate questions to spend them on.
    if !(1..=merged.questions_per_cycle).contains(&merged.votes_per_user_per_cycle) {
        return Err(ApiError::validation(
            "votesPerUserPerCycle must be between 1 and questionsPerCycle",
        ));
    }
    Ok(merged)
}

fn notification_settings(
    current: &NotificationSettings,
    patch: &NotificationSettingsPatch,
) -> Result<NotificationSettings, ApiError> {
    let merged = NotificationSettings {
        offsets_hours_before_close: patch
            .offsets_hours_before_close
            .clone()
            .unwrap_or_else(|| current.offsets_hours_before_close.clone()),
        on_cycle_open: patch.on_cycle_open.unwrap_or(current.on_cycle_open),
    };

    let offsets = &merged.offsets_hours_before_close;
    if offsets.is_empty() {
        return Err(ApiError::validation(
            "offsetsHoursBeforeClose must not be empty",
        ));
    }
    if offsets
        .iter()
        .any(|h| *h < 1 || *h > MAX_REMINDER_OFFSET_HOURS)
    {
        return Err(ApiError::validation(format!(
            "offsetsHoursBeforeClose entries must be between 1 and {MAX_REMINDER_OFFSET_HOURS}"
        )));
    }
    if offsets.windows(2).any(|pair| pair[0] <= pair[1]) {
        return Err(ApiError::validation(
            "offsetsHoursBeforeClose must be strictly descending",
        ));
    }
    Ok(merged)
}

pub fn group_patch(current: &Group, request: PatchGroupRequest) -> Result<GroupPatch, ApiError> {
    let mut patch = GroupPatch::default();

    if let Some(name) = request.name {
        patch.name = Some(display_name(&name)?);
    }
    if let Some(tz) = request.timezone {
        timezone(&tz)?;
        patch.timezone = Some(tz);
    }
    if let Some(slug) = request.gradient {
        gradient(&slug)?;
        patch.gradient = Some(slug);
    }
    if let Some(settings) = request.cycle_settings {
        patch.cycle_settings = Some(cycle_settings(&current.cycle_settings, &settings)?);
    }
    if let Some(settings) = request.notification_settings {
        patch.notification_settings = Some(notification_settings(
            &current.notification_settings,
            &settings,
        )?);
    }
    if let Some(cap) = request.member_soft_cap {
        if cap < current.member_count {
            return Err(ApiError::validation(format!(
                "memberSoftCap cannot be below the current member count of {}",
                current.member_count
            )));
        }
        patch.member_soft_cap = Some(cap);
    }

    Ok(patch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::ApiErrorCode;

    fn settings() -> CycleSettings {
        CycleSettings {
            questions_per_cycle: 5,
            votes_per_user_per_cycle: 3,
            response_window_days: 4,
            auto_publish: true,
        }
    }

    #[test]
    fn display_name_is_trimmed() {
        assert_eq!(display_name("  Quinn  ").unwrap(), "Quinn");
    }

    #[test]
    fn whitespace_only_display_name_is_rejected() {
        let err = display_name("   ").expect_err("blank name");
        assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    }

    #[test]
    fn over_long_display_name_is_rejected() {
        let err = display_name(&"a".repeat(MAX_DISPLAY_NAME_CHARS + 1)).expect_err("too long");
        assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    }

    #[test]
    fn unknown_avatar_color_is_rejected() {
        assert!(avatar_color("chartreuse").is_err());
        assert!(avatar_color("teal").is_ok());
    }

    #[test]
    fn unknown_gradient_is_rejected() {
        assert!(gradient("neon-slime").is_err());
        assert!(gradient("ocean-dusk").is_ok());
    }

    #[test]
    fn unknown_timezone_is_rejected() {
        assert!(timezone("Mars/Olympus_Mons").is_err());
        assert!(timezone("America/New_York").is_ok());
    }

    #[test]
    fn vote_cap_above_the_patched_question_count_is_rejected() {
        let patch = CycleSettingsPatch {
            questions_per_cycle: Some(2),
            votes_per_user_per_cycle: None,
            response_window_days: None,
        };
        let err = cycle_settings(&settings(), &patch).expect_err("3 votes over 2 questions");
        assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    }

    #[test]
    fn response_window_beyond_the_cap_is_rejected() {
        let patch = CycleSettingsPatch {
            questions_per_cycle: None,
            votes_per_user_per_cycle: None,
            response_window_days: Some(MAX_RESPONSE_WINDOW_DAYS + 1),
        };
        assert!(cycle_settings(&settings(), &patch).is_err());
    }

    #[test]
    fn reminder_offsets_must_descend() {
        let current = NotificationSettings {
            offsets_hours_before_close: vec![96, 48, 24],
            on_cycle_open: true,
        };
        let patch = NotificationSettingsPatch {
            offsets_hours_before_close: Some(vec![24, 48]),
            on_cycle_open: None,
        };
        assert!(notification_settings(&current, &patch).is_err());
    }

    #[test]
    fn reminder_offsets_beyond_the_query_window_are_rejected() {
        let current = NotificationSettings {
            offsets_hours_before_close: vec![96],
            on_cycle_open: true,
        };
        let patch = NotificationSettingsPatch {
            offsets_hours_before_close: Some(vec![MAX_REMINDER_OFFSET_HOURS + 1]),
            on_cycle_open: None,
        };
        assert!(notification_settings(&current, &patch).is_err());
    }
}
