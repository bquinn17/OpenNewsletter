//! Wire shapes for `plans/03-api-contract.md` §3.

use chrono::{DateTime, Utc};
use domain::{GroupId, Invite, InviteStatus, Role, UserId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateInviteRequest {
    pub group_id: GroupId,
    pub ttl_days: Option<u32>,
    pub role_on_redeem: Role,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInviteResponse {
    pub code: String,
    pub group_id: GroupId,
    pub expires_at: DateTime<Utc>,
    pub role_on_redeem: Role,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InviteSummary {
    pub code: String,
    pub group_id: GroupId,
    pub status: InviteStatus,
    pub role_on_redeem: Role,
    pub created_by: UserId,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub consumed_by: Option<UserId>,
    pub consumed_at: Option<DateTime<Utc>>,
}

impl InviteSummary {
    pub fn new(invite: Invite, display_code: String) -> Self {
        Self {
            code: display_code,
            group_id: invite.group_id,
            status: invite.status,
            role_on_redeem: invite.role_on_redeem,
            created_by: invite.created_by,
            created_at: invite.created_at,
            expires_at: invite.expires_at,
            consumed_by: invite.consumed_by,
            consumed_at: invite.consumed_at,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InviteListResponse {
    pub items: Vec<InviteSummary>,
    /// Always `null` in M4 — the invite list is bounded by the member soft cap.
    pub next_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RedeemRequest {
    pub code: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedeemResponse {
    pub group_id: GroupId,
    pub group_name: String,
    pub role: Role,
}
