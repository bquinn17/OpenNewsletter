//! End-to-end newsletter lifecycle test standing in for M5's manual "done-when"
//! flow (which needs real AWS): bootstrap a group, suggest and vote on
//! candidates through `lambda-questions`' own handlers, then drive the cycle
//! through `voting -> open -> published` via `lambda-cycle-tick`'s `run_tick`,
//! exactly as the real EventBridge schedule would (`06-newsletter-lifecycle.md` §5).

mod common;

use chrono::{Duration, Utc};
use cycle_tick::notify::{FanoutKind, Notifier};
use cycle_tick::tick::run_tick_with;
use domain::api::CreateCandidateRequest;
use domain::{CycleId, GroupId, Newsletter, NewsletterStatus, QuestionKind, Role, UserId};
use persistence::newsletters;
use pretty_assertions::assert_eq;
use questions::handlers;
use questions::state::AppState;
use std::sync::Mutex;

/// Records every fan-out the tick triggers, in order.
#[derive(Default)]
struct RecordingNotifier(Mutex<Vec<(FanoutKind, String, String)>>);

#[async_trait::async_trait]
impl Notifier for RecordingNotifier {
    async fn fanout(&self, kind: FanoutKind, group_id: &GroupId, cycle_id: &CycleId) {
        self.0
            .lock()
            .unwrap()
            .push((kind, group_id.to_string(), cycle_id.to_string()));
    }
}

#[tokio::test]
async fn a_cycle_runs_from_voting_through_open_to_published_and_seeds_the_next_cycle() {
    let (_c, repo) = common::make_repo().await;

    // 1. Bootstrap a group with a low questionsPerCycle so "top-N locked" is
    // easy to assert, and its first voting cycle.
    let mut group = common::group("g1", "u1");
    group.cycle_settings.questions_per_cycle = 2;
    group.cycle_settings.votes_per_user_per_cycle = 3;
    common::seed_group_and_membership(&repo, &group, "u1", Role::Admin).await;
    common::seed_group_and_membership(&repo, &group, "u2", Role::Member).await;
    for user_id in ["u1", "u2"] {
        persistence::users::put_user(&repo, &common::user(user_id))
            .await
            .unwrap();
    }

    // Dates relative to `Utc::now()` — `run_tick` reads the real wall clock, so
    // a fixed-past-date fixture (fine for `persistence`'s own tests, which pass
    // an explicit `now` to `list_cycles_due`) would already be "due" for every
    // transition at once and collapse both ticks below into a single call.
    let now = Utc::now();
    let voting = Newsletter {
        group_id: GroupId::new("g1"),
        cycle_id: CycleId::new("202606"),
        status: NewsletterStatus::Voting,
        vote_window_open_at: now,
        vote_window_close_at: now + Duration::days(10),
        response_open_at: now + Duration::days(10),
        response_close_at: now + Duration::days(14),
        published_at: None,
        next_transition_at: Some(now + Duration::days(10)),
        locked_question_ids: Vec::new(),
        notified_offsets_hours: Vec::new(),
        notified_on_open: false,
    };
    newsletters::write_status_transition(&repo, &voting)
        .await
        .unwrap();

    let state = AppState {
        repo,
        cdn_base_url: String::new(),
    };
    let group_id = GroupId::new("g1");

    // 2. Suggest three candidates and vote through the questions handler, so the
    // top 2 by vote count are unambiguous.
    let prompts = ["Best hike?", "Best campsite?", "Best trail snack?"];
    let mut question_ids = Vec::new();
    for prompt in prompts {
        let item = handlers::create_candidate(
            &state,
            &UserId::new("u1"),
            &group_id,
            CreateCandidateRequest {
                kind: QuestionKind::Text,
                prompt: prompt.to_owned(),
                poll_options: None,
                is_anonymous: false,
            },
        )
        .await
        .expect("member can submit a candidate");
        question_ids.push(item.question_id);
    }

    // u1 votes for [0], u2 votes for [0, 1] -> question 0 has 2 votes, question 1
    // has 1 vote, question 2 has 0 votes. Top 2 = questions 0 and 1.
    handlers::cast_vote(&state, &UserId::new("u1"), &group_id, &question_ids[0])
        .await
        .unwrap();
    handlers::cast_vote(&state, &UserId::new("u2"), &group_id, &question_ids[0])
        .await
        .unwrap();
    handlers::cast_vote(&state, &UserId::new("u2"), &group_id, &question_ids[1])
        .await
        .unwrap();

    // 3. Rewind the voting cycle's deadline so the tick treats it as due — but
    // only just: `responseCloseAt` (now+14d) must stay in the future, or the
    // tick's second query (open cycles due) would immediately re-publish it
    // within this same call.
    newsletters::rewind_active_cycle_deadline(&state.repo, &group_id, Duration::days(11))
        .await
        .unwrap()
        .expect("group has an active cycle to rewind");

    // 4. Run the tick — this is exactly what the EventBridge schedule invokes.
    let notifier = RecordingNotifier::default();
    let summary = run_tick_with(&state.repo, &notifier).await;
    assert!(
        summary
            .transitions
            .iter()
            .any(|t| t.cycle_id == "202606" && t.from == Some("voting") && t.to == "open"),
        "tick must promote the voting cycle to open: {:?}",
        summary.transitions
    );

    // 5. The cycle is now `open` with the top 2 (by vote count) locked in order.
    let cycle_id = CycleId::new("202606");
    let opened = newsletters::get_newsletter(&state.repo, &group_id, &cycle_id)
        .await
        .unwrap()
        .expect("cycle exists");
    assert_eq!(opened.status, NewsletterStatus::Open);
    assert_eq!(opened.locked_question_ids.len(), 2);
    assert_eq!(opened.locked_question_ids[0], question_ids[0]);
    assert_eq!(opened.locked_question_ids[1], question_ids[1]);

    // The tick also seeded the group's next `voting` cycle in the same pass.
    let next_voting = newsletters::find_voting_cycle(&state.repo, &group_id)
        .await
        .unwrap()
        .expect("tick creates the next voting cycle alongside promotion");
    assert_ne!(next_voting.cycle_id, cycle_id);

    // 6. Rewind the (now open) cycle's response deadline and tick again.
    newsletters::rewind_active_cycle_deadline(&state.repo, &group_id, Duration::days(15))
        .await
        .unwrap()
        .expect("group has an active (open) cycle to rewind");
    let summary = run_tick_with(&state.repo, &notifier).await;
    assert!(
        summary
            .transitions
            .iter()
            .any(|t| t.cycle_id == "202606" && t.from == Some("open") && t.to == "published"),
        "tick must publish the open cycle: {:?}",
        summary.transitions
    );

    // 7. The cycle is now `published`.
    let published = newsletters::get_newsletter(&state.repo, &group_id, &cycle_id)
        .await
        .unwrap()
        .expect("cycle exists");
    assert_eq!(published.status, NewsletterStatus::Published);
    assert!(published.published_at.is_some());

    // Each transition triggered exactly one fan-out, cycle-open then publication.
    assert_eq!(
        *notifier.0.lock().unwrap(),
        vec![
            (
                FanoutKind::CycleOpen,
                group_id.to_string(),
                "202606".to_string()
            ),
            (
                FanoutKind::Publication,
                group_id.to_string(),
                "202606".to_string()
            ),
        ]
    );

    // 8. The next voting cycle created back in step 5 is untouched and still voting.
    let still_voting = newsletters::find_voting_cycle(&state.repo, &group_id)
        .await
        .unwrap()
        .expect("the next voting cycle still exists");
    assert_eq!(still_voting.cycle_id, next_voting.cycle_id);
}
