//! Wire shapes for `plans/03-api-contract.md` §2 and §4.
//!
//! These are deliberately separate from the `domain` entities: DynamoDB stores
//! snake_case attributes, the HTTP contract is camelCase, and the two are allowed
//! to drift (the group row carries `created_by`, the API does not expose it).

use chrono::{DateTime, Utc};
use domain::{AvatarId, CycleSettings, Group, GroupId, NotificationSettings, Role, User, UserId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MembershipSummary {
    pub group_id: GroupId,
    pub role: Role,
    pub group_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigResponse {
    /// `null` until the caller redeems their first invite.
    pub user_id: Option<UserId>,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub vapid_public_key: String,
    pub group_defaults: serde_json::Value,
    pub memberships: Vec<MembershipSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserResponse {
    pub user_id: UserId,
    pub email: String,
    pub display_name: String,
    pub avatar_color: String,
    pub avatar_media_id: Option<AvatarId>,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl UserResponse {
    pub fn new(user: User, avatar_url: Option<String>) -> Self {
        Self {
            user_id: user.user_id,
            email: user.email,
            display_name: user.display_name,
            avatar_color: user.avatar_color,
            avatar_media_id: user.avatar_media_id,
            avatar_url,
            created_at: user.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PatchMeRequest {
    pub display_name: Option<String>,
    pub avatar_color: Option<String>,
    /// Absent leaves the photo alone; an explicit `null` clears it.
    #[serde(default, deserialize_with = "double_option")]
    pub avatar_media_id: Option<Option<AvatarId>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberResponse {
    pub user_id: UserId,
    pub display_name: String,
    pub role: Role,
    pub avatar_color: String,
    pub avatar_url: Option<String>,
    pub joined_at: DateTime<Utc>,
    pub editions_answered: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupResponse {
    pub group_id: GroupId,
    pub name: String,
    pub timezone: String,
    pub gradient: String,
    pub cycle_settings: CycleSettings,
    pub notification_settings: NotificationSettings,
    pub member_count: u32,
    pub member_soft_cap: u32,
    pub created_at: DateTime<Utc>,
    pub members: Vec<MemberResponse>,
}

impl GroupResponse {
    pub fn new(group: Group, members: Vec<MemberResponse>) -> Self {
        Self {
            group_id: group.group_id,
            name: group.name,
            timezone: group.timezone,
            gradient: group.gradient,
            cycle_settings: group.cycle_settings,
            notification_settings: group.notification_settings,
            member_count: group.member_count,
            member_soft_cap: group.member_soft_cap,
            created_at: group.created_at,
            members,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CycleSettingsPatch {
    pub questions_per_cycle: Option<u32>,
    pub votes_per_user_per_cycle: Option<u32>,
    pub response_window_days: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NotificationSettingsPatch {
    pub offsets_hours_before_close: Option<Vec<u32>>,
    pub on_cycle_open: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PatchGroupRequest {
    pub name: Option<String>,
    pub timezone: Option<String>,
    pub gradient: Option<String>,
    pub cycle_settings: Option<CycleSettingsPatch>,
    pub notification_settings: Option<NotificationSettingsPatch>,
    pub member_soft_cap: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PatchMemberRequest {
    pub role: Role,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MembershipListResponse {
    pub memberships: Vec<MembershipSummary>,
}

/// Distinguishes an absent JSON field from an explicit `null`.
fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    serde::Deserialize::deserialize(deserializer).map(Some)
}
