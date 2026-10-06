//! Contract test per `plans/11-testing-ci-cd.md` §2.3: loads `shared/openapi.yaml`
//! and round-trips every request/response example against the matching
//! `domain::api` type via the `ROUTE_TABLE` below.
//!
//! To add a route: give its operation a `request` and/or response `example` in the
//! YAML, then add a `(METHOD, "path", "request"|"<status>", round_trip::<YourType>)`
//! row to `ROUTE_TABLE`. Either side missing its counterpart fails the test — that's
//! what makes updating the YAML and `domain::api` happen in the same PR.
//!
//! No Docker needed: this only reads a file on disk and calls serde. Runs under
//! plain `cargo test`.

use domain::api::{
    AvatarMediaResponse, CandidateItemResponse, CandidateListResponse, CandidateVoteResponse,
    CommentListResponse, CommentResponse, ConfigResponse, CreateAvatarRequest,
    CreateAvatarResponse, CreateCandidateRequest, CreateCommentRequest, CreateInviteRequest,
    CreateInviteResponse, CreateUploadRequest, CreateUploadResponse, GroupResponse, HealthResponse,
    ImageMediaResponse, InviteListResponse, MediaCookieResponse, MemberResponse,
    MembershipListResponse, MyResponsesList, NewsletterDetailResponse, NewsletterListResponse,
    PatchCommentRequest, PatchGroupRequest, PatchMeRequest, PatchMemberRequest, PatchUploadRequest,
    ProblemDetails, PushPreferenceListResponse, PushPreferenceResponse, PushSubscribeRequest,
    PushSubscribeResponse, PushSubscriptionListResponse, PushTestResponse, PushUnsubscribeRequest,
    PutPushPreferenceRequest, ReactionsResponse, RedeemRequest, RedeemResponse, ResponseDto,
    SaveResponseRequest, UserResponse,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;

type Checker = fn(&Value) -> Result<(), String>;

/// Deserialize `example` into `T`, reserialize it, and require byte-for-byte JSON
/// equality with the original. A mismatch means either the YAML schema/example or
/// the Rust struct's serde attributes have drifted from the other.
fn round_trip<T>(example: &Value) -> Result<(), String>
where
    T: DeserializeOwned + Serialize,
{
    let parsed: T = serde_json::from_value(example.clone()).map_err(|e| {
        format!(
            "failed to deserialize into {}: {e}",
            std::any::type_name::<T>()
        )
    })?;
    let reserialized = serde_json::to_value(&parsed)
        .map_err(|e| format!("failed to reserialize {}: {e}", std::any::type_name::<T>()))?;
    if &reserialized != example {
        return Err(format!(
            "round trip mismatch for {}\n  original:     {example}\n  reserialized: {reserialized}",
            std::any::type_name::<T>()
        ));
    }
    Ok(())
}

/// `(method, path, "request" | status code) -> domain::api type`. The path string
/// must match `shared/openapi.yaml` exactly (including `{param}` braces).
const ROUTE_TABLE: &[(&str, &str, &str, Checker)] = &[
    ("GET", "/config", "200", round_trip::<ConfigResponse>),
    ("GET", "/me", "200", round_trip::<UserResponse>),
    ("PATCH", "/me", "request", round_trip::<PatchMeRequest>),
    ("PATCH", "/me", "200", round_trip::<UserResponse>),
    ("PATCH", "/me", "422", round_trip::<ProblemDetails>),
    (
        "POST",
        "/admin/invites",
        "request",
        round_trip::<CreateInviteRequest>,
    ),
    (
        "POST",
        "/admin/invites",
        "201",
        round_trip::<CreateInviteResponse>,
    ),
    (
        "POST",
        "/admin/invites",
        "422",
        round_trip::<ProblemDetails>,
    ),
    (
        "GET",
        "/admin/groups/{groupId}/invites",
        "200",
        round_trip::<InviteListResponse>,
    ),
    (
        "POST",
        "/invites/redeem",
        "request",
        round_trip::<RedeemRequest>,
    ),
    (
        "POST",
        "/invites/redeem",
        "200",
        round_trip::<RedeemResponse>,
    ),
    (
        "GET",
        "/groups",
        "200",
        round_trip::<MembershipListResponse>,
    ),
    (
        "GET",
        "/groups/{groupId}",
        "200",
        round_trip::<GroupResponse>,
    ),
    (
        "GET",
        "/groups/{groupId}",
        "404",
        round_trip::<ProblemDetails>,
    ),
    (
        "PATCH",
        "/groups/{groupId}",
        "request",
        round_trip::<PatchGroupRequest>,
    ),
    (
        "PATCH",
        "/groups/{groupId}",
        "200",
        round_trip::<GroupResponse>,
    ),
    (
        "PATCH",
        "/groups/{groupId}",
        "422",
        round_trip::<ProblemDetails>,
    ),
    (
        "PATCH",
        "/groups/{groupId}/members/{userId}",
        "request",
        round_trip::<PatchMemberRequest>,
    ),
    (
        "PATCH",
        "/groups/{groupId}/members/{userId}",
        "200",
        round_trip::<MemberResponse>,
    ),
    (
        "PATCH",
        "/groups/{groupId}/members/{userId}",
        "404",
        round_trip::<ProblemDetails>,
    ),
    (
        "DELETE",
        "/groups/{groupId}/members/{userId}",
        "404",
        round_trip::<ProblemDetails>,
    ),
    ("GET", "/healthz", "200", round_trip::<HealthResponse>),
    (
        "GET",
        "/groups/{groupId}/newsletters",
        "200",
        round_trip::<NewsletterListResponse>,
    ),
    (
        "GET",
        "/groups/{groupId}/newsletters/{cycleId}",
        "200",
        round_trip::<NewsletterDetailResponse>,
    ),
    (
        "GET",
        "/groups/{groupId}/newsletters/{cycleId}",
        "404",
        round_trip::<ProblemDetails>,
    ),
    (
        "GET",
        "/groups/{groupId}/candidate-questions",
        "200",
        round_trip::<CandidateListResponse>,
    ),
    (
        "POST",
        "/groups/{groupId}/candidate-questions",
        "request",
        round_trip::<CreateCandidateRequest>,
    ),
    (
        "POST",
        "/groups/{groupId}/candidate-questions",
        "201",
        round_trip::<CandidateItemResponse>,
    ),
    (
        "POST",
        "/groups/{groupId}/candidate-questions",
        "422",
        round_trip::<ProblemDetails>,
    ),
    (
        "POST",
        "/groups/{groupId}/candidate-questions/{questionId}/votes",
        "200",
        round_trip::<CandidateVoteResponse>,
    ),
    (
        "POST",
        "/groups/{groupId}/candidate-questions/{questionId}/votes",
        "404",
        round_trip::<ProblemDetails>,
    ),
    (
        "DELETE",
        "/groups/{groupId}/candidate-questions/{questionId}/votes",
        "200",
        round_trip::<CandidateVoteResponse>,
    ),
    (
        "DELETE",
        "/groups/{groupId}/candidate-questions/{questionId}/votes",
        "404",
        round_trip::<ProblemDetails>,
    ),
    (
        "DELETE",
        "/admin/groups/{groupId}/candidate-questions/{questionId}",
        "404",
        round_trip::<ProblemDetails>,
    ),
    (
        "GET",
        "/groups/{groupId}/newsletters/{cycleId}/my-responses",
        "200",
        round_trip::<MyResponsesList>,
    ),
    (
        "GET",
        "/groups/{groupId}/newsletters/{cycleId}/my-responses",
        "404",
        round_trip::<ProblemDetails>,
    ),
    (
        "GET",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/my-response",
        "200",
        round_trip::<ResponseDto>,
    ),
    (
        "GET",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/my-response",
        "404",
        round_trip::<ProblemDetails>,
    ),
    (
        "PUT",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/my-response",
        "request",
        round_trip::<SaveResponseRequest>,
    ),
    (
        "PUT",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/my-response",
        "200",
        round_trip::<ResponseDto>,
    ),
    (
        "PUT",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/my-response",
        "404",
        round_trip::<ProblemDetails>,
    ),
    (
        "PUT",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/my-response",
        "422",
        round_trip::<ProblemDetails>,
    ),
    // ---- `03-api-contract.md` §8 — Engagement routes (comments + reactions) ----
    (
        "GET",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/comments",
        "200",
        round_trip::<CommentListResponse>,
    ),
    (
        "GET",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/comments",
        "404",
        round_trip::<ProblemDetails>,
    ),
    (
        "GET",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/comments",
        "422",
        round_trip::<ProblemDetails>,
    ),
    (
        "POST",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/comments",
        "request",
        round_trip::<CreateCommentRequest>,
    ),
    (
        "POST",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/comments",
        "201",
        round_trip::<CommentResponse>,
    ),
    (
        "POST",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/comments",
        "404",
        round_trip::<ProblemDetails>,
    ),
    (
        "POST",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/comments",
        "422",
        round_trip::<ProblemDetails>,
    ),
    (
        "PATCH",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/comments/{commentId}",
        "request",
        round_trip::<PatchCommentRequest>,
    ),
    (
        "PATCH",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/comments/{commentId}",
        "200",
        round_trip::<CommentResponse>,
    ),
    (
        "PATCH",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/comments/{commentId}",
        "404",
        round_trip::<ProblemDetails>,
    ),
    (
        "PATCH",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/comments/{commentId}",
        "422",
        round_trip::<ProblemDetails>,
    ),
    (
        "DELETE",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/comments/{commentId}",
        "404",
        round_trip::<ProblemDetails>,
    ),
    (
        "GET",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/reactions",
        "200",
        round_trip::<ReactionsResponse>,
    ),
    (
        "GET",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/reactions",
        "404",
        round_trip::<ProblemDetails>,
    ),
    (
        "PUT",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/reactions/{emoji}",
        "200",
        round_trip::<ReactionsResponse>,
    ),
    (
        "PUT",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/reactions/{emoji}",
        "404",
        round_trip::<ProblemDetails>,
    ),
    (
        "PUT",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/reactions/{emoji}",
        "422",
        round_trip::<ProblemDetails>,
    ),
    (
        "DELETE",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/reactions/{emoji}",
        "200",
        round_trip::<ReactionsResponse>,
    ),
    (
        "DELETE",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/reactions/{emoji}",
        "404",
        round_trip::<ProblemDetails>,
    ),
    (
        "DELETE",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/reactions/{emoji}",
        "422",
        round_trip::<ProblemDetails>,
    ),
    // ---- `03-api-contract.md` §9 — Media routes ----
    (
        "POST",
        "/uploads",
        "request",
        round_trip::<CreateUploadRequest>,
    ),
    (
        "POST",
        "/uploads",
        "201",
        round_trip::<CreateUploadResponse>,
    ),
    ("POST", "/uploads", "422", round_trip::<ProblemDetails>),
    (
        "GET",
        "/uploads/{imageId}",
        "200",
        round_trip::<ImageMediaResponse>,
    ),
    (
        "GET",
        "/uploads/{imageId}",
        "404",
        round_trip::<ProblemDetails>,
    ),
    (
        "PATCH",
        "/uploads/{imageId}",
        "request",
        round_trip::<PatchUploadRequest>,
    ),
    (
        "PATCH",
        "/uploads/{imageId}",
        "200",
        round_trip::<ImageMediaResponse>,
    ),
    (
        "PATCH",
        "/uploads/{imageId}",
        "404",
        round_trip::<ProblemDetails>,
    ),
    (
        "PATCH",
        "/uploads/{imageId}",
        "422",
        round_trip::<ProblemDetails>,
    ),
    (
        "DELETE",
        "/uploads/{imageId}",
        "404",
        round_trip::<ProblemDetails>,
    ),
    (
        "POST",
        "/uploads/{imageId}/complete",
        "200",
        round_trip::<ImageMediaResponse>,
    ),
    (
        "POST",
        "/uploads/{imageId}/complete",
        "404",
        round_trip::<ProblemDetails>,
    ),
    (
        "GET",
        "/media-cookie",
        "200",
        round_trip::<MediaCookieResponse>,
    ),
    ("GET", "/media-cookie", "422", round_trip::<ProblemDetails>),
    (
        "POST",
        "/avatars",
        "request",
        round_trip::<CreateAvatarRequest>,
    ),
    (
        "POST",
        "/avatars",
        "201",
        round_trip::<CreateAvatarResponse>,
    ),
    ("POST", "/avatars", "422", round_trip::<ProblemDetails>),
    (
        "GET",
        "/avatars/{avatarId}",
        "200",
        round_trip::<AvatarMediaResponse>,
    ),
    (
        "GET",
        "/avatars/{avatarId}",
        "404",
        round_trip::<ProblemDetails>,
    ),
    (
        "DELETE",
        "/avatars/{avatarId}",
        "404",
        round_trip::<ProblemDetails>,
    ),
    // ---- `03-api-contract.md` §10 — Push routes ----
    (
        "POST",
        "/push/subscribe",
        "request",
        round_trip::<PushSubscribeRequest>,
    ),
    (
        "POST",
        "/push/subscribe",
        "201",
        round_trip::<PushSubscribeResponse>,
    ),
    (
        "POST",
        "/push/subscribe",
        "422",
        round_trip::<ProblemDetails>,
    ),
    (
        "POST",
        "/push/unsubscribe",
        "request",
        round_trip::<PushUnsubscribeRequest>,
    ),
    (
        "POST",
        "/push/unsubscribe",
        "422",
        round_trip::<ProblemDetails>,
    ),
    (
        "GET",
        "/push/subscriptions",
        "200",
        round_trip::<PushSubscriptionListResponse>,
    ),
    (
        "POST",
        "/push/test",
        "200",
        round_trip::<PushTestResponse>,
    ),
    (
        "GET",
        "/push/preferences",
        "200",
        round_trip::<PushPreferenceListResponse>,
    ),
    (
        "PUT",
        "/push/preferences/{groupId}",
        "request",
        round_trip::<PutPushPreferenceRequest>,
    ),
    (
        "PUT",
        "/push/preferences/{groupId}",
        "200",
        round_trip::<PushPreferenceResponse>,
    ),
    (
        "PUT",
        "/push/preferences/{groupId}",
        "422",
        round_trip::<ProblemDetails>,
    ),
];

const HTTP_METHODS: &[&str] = &["get", "post", "put", "patch", "delete"];

fn load_spec() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../shared/openapi.yaml");
    let content = fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"));
    serde_yaml::from_str(&content).unwrap_or_else(|e| panic!("failed to parse {path} as YAML: {e}"))
}

/// Follows a single `$ref` (this spec only uses local, one-level refs into
/// `components.responses`) so shared error responses' examples are found too.
fn resolve_ref<'a>(root: &'a Value, node: &'a Value) -> &'a Value {
    let mut current = node;
    while let Some(reference) = current.get("$ref").and_then(Value::as_str) {
        let pointer = reference
            .strip_prefix('#')
            .unwrap_or_else(|| panic!("only local $ref values are supported, got {reference}"));
        current = root
            .pointer(pointer)
            .unwrap_or_else(|| panic!("dangling $ref: {reference}"));
    }
    current
}

/// Every `(method, path, "request" | status, example)` found in the spec.
fn discover_examples(spec: &Value) -> Vec<(String, String, String, Value)> {
    let mut found = Vec::new();
    let paths = spec["paths"].as_object().expect("spec.paths is an object");

    for (path, path_item) in paths {
        let operations = path_item
            .as_object()
            .unwrap_or_else(|| panic!("path item for {path} is not an object"));

        for (method, operation) in operations {
            if !HTTP_METHODS.contains(&method.as_str()) {
                continue;
            }
            let method_upper = method.to_uppercase();

            if let Some(example) =
                operation.pointer("/requestBody/content/application~1json/example")
            {
                found.push((
                    method_upper.clone(),
                    path.clone(),
                    "request".to_owned(),
                    example.clone(),
                ));
            }

            if let Some(responses) = operation.get("responses").and_then(Value::as_object) {
                for (status, response) in responses {
                    let resolved = resolve_ref(spec, response);
                    if let Some(example) = resolved.pointer("/content/application~1json/example") {
                        found.push((
                            method_upper.clone(),
                            path.clone(),
                            status.clone(),
                            example.clone(),
                        ));
                    }
                }
            }
        }
    }

    found
}

#[test]
fn every_yaml_example_round_trips_through_its_mapped_domain_type() {
    let spec = load_spec();
    let discovered = discover_examples(&spec);
    assert!(
        !discovered.is_empty(),
        "found no request/response examples in shared/openapi.yaml — is the spec well-formed?"
    );

    let mut failures = Vec::new();

    for (method, path, kind, example) in &discovered {
        match ROUTE_TABLE
            .iter()
            .find(|(m, p, k, _)| m == method && p == path && k == kind)
        {
            None => failures.push(format!(
                "{method} {path} [{kind}] has an example in shared/openapi.yaml but no \
                 matching entry in ROUTE_TABLE (backend/crates/domain/tests/openapi_contract.rs)"
            )),
            Some((_, _, _, checker)) => {
                if let Err(e) = checker(example) {
                    failures.push(format!("{method} {path} [{kind}]: {e}"));
                }
            }
        }
    }

    // Catches the opposite drift: a stale or mistyped ROUTE_TABLE entry that no
    // longer (or never did) match anything the YAML actually defines.
    let discovered_keys: BTreeSet<(&str, &str, &str)> = discovered
        .iter()
        .map(|(m, p, k, _)| (m.as_str(), p.as_str(), k.as_str()))
        .collect();
    for (method, path, kind, _) in ROUTE_TABLE {
        if !discovered_keys.contains(&(method, path, kind)) {
            failures.push(format!(
                "ROUTE_TABLE entry {method} {path} [{kind}] has no matching example in \
                 shared/openapi.yaml"
            ));
        }
    }

    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn gradient_and_avatar_color_enums_match_shared_config() {
    let spec = load_spec();
    assert_eq!(
        enum_values(&spec, "GradientSlug"),
        shared::config::GRADIENT_SLUGS.to_vec(),
        "shared/openapi.yaml's GradientSlug enum has drifted from shared::config::GRADIENT_SLUGS"
    );
    assert_eq!(
        enum_values(&spec, "AvatarColorSlug"),
        shared::config::AVATAR_COLOR_SLUGS.to_vec(),
        "shared/openapi.yaml's AvatarColorSlug enum has drifted from shared::config::AVATAR_COLOR_SLUGS"
    );
}

fn enum_values<'a>(spec: &'a Value, schema: &str) -> Vec<&'a str> {
    spec.pointer(&format!("/components/schemas/{schema}/enum"))
        .unwrap_or_else(|| panic!("components.schemas.{schema}.enum is missing"))
        .as_array()
        .unwrap_or_else(|| panic!("components.schemas.{schema}.enum is not an array"))
        .iter()
        .map(|v| v.as_str().expect("enum values are strings"))
        .collect()
}
