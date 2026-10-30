//! `PATCH /me` deletes the avatar that a new one replaces, or that clearing the
//! photo removes (`docs/design/api.md` §2.3/§9.7, `docs/design/media.md` §11.4).

mod common;

use chrono::Utc;
use domain::api::PatchMeRequest;
use domain::{AvatarId, AvatarMedia, ImageMimeType, MediaStatus, UserId};
use groups::me;
use persistence::{media, test_factories, users, Repo};
use pretty_assertions::assert_eq;

async fn seed_avatar(repo: &Repo, user_id: &str, avatar_id: &str) -> AvatarId {
    let avatar = AvatarMedia {
        avatar_id: AvatarId::new(avatar_id),
        user_id: UserId::new(user_id),
        mime_type: ImageMimeType::Jpeg,
        original_key: format!("uploads/{user_id}/{avatar_id}.jpg"),
        display_key: Some(format!("avatar/{avatar_id}/display.webp")),
        status: MediaStatus::Ready,
        bytes: 65_536,
        uploaded_at: Utc::now(),
        processed_at: Some(Utc::now()),
        error_message: None,
    };
    media::put_avatar(repo, &avatar).await.unwrap();
    avatar.avatar_id
}

/// `u1`, currently showing `av-old`, with `av-new` uploaded and ready.
async fn seed_user_with_two_avatars(repo: &Repo) -> (AvatarId, AvatarId) {
    let old = seed_avatar(repo, "u1", "av-old").await;
    let new = seed_avatar(repo, "u1", "av-new").await;
    let mut user = test_factories::user("u1");
    user.avatar_media_id = Some(old.clone());
    users::put_user(repo, &user).await.unwrap();
    (old, new)
}

fn set_avatar(avatar_id: &AvatarId) -> PatchMeRequest {
    PatchMeRequest {
        display_name: None,
        avatar_color: None,
        avatar_media_id: Some(Some(avatar_id.clone())),
    }
}

#[tokio::test]
async fn replacing_an_avatar_deletes_the_old_one() {
    let (_c, repo) = test_factories::make_test_repo().await;
    let (old, new) = seed_user_with_two_avatars(&repo).await;
    let (state, store) = common::state_with_fakes(repo);
    let user_id = UserId::new("u1");

    let me = me::patch_me(&state, &user_id, set_avatar(&new))
        .await
        .unwrap();

    assert_eq!(me.avatar_media_id, Some(new.clone()));
    assert_eq!(
        store.deleted().await,
        vec![
            (
                common::AVATARS_ORIGINALS.to_owned(),
                "uploads/u1/av-old.jpg".to_owned()
            ),
            (
                common::PROCESSED.to_owned(),
                "avatar/av-old/display.webp".to_owned()
            ),
        ]
    );
    assert_eq!(
        media::get_avatar(&state.repo, &user_id, &old)
            .await
            .unwrap(),
        None
    );
    assert!(media::get_avatar(&state.repo, &user_id, &new)
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn clearing_the_avatar_deletes_it() {
    let (_c, repo) = test_factories::make_test_repo().await;
    let (old, _new) = seed_user_with_two_avatars(&repo).await;
    let (state, store) = common::state_with_fakes(repo);
    let user_id = UserId::new("u1");
    let clear = PatchMeRequest {
        display_name: None,
        avatar_color: None,
        avatar_media_id: Some(None),
    };

    let me = me::patch_me(&state, &user_id, clear).await.unwrap();

    assert_eq!(me.avatar_media_id, None);
    assert_eq!(store.deleted().await.len(), 2);
    assert_eq!(
        media::get_avatar(&state.repo, &user_id, &old)
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn a_patch_that_leaves_the_avatar_alone_deletes_nothing() {
    let (_c, repo) = test_factories::make_test_repo().await;
    let (old, _new) = seed_user_with_two_avatars(&repo).await;
    let (state, store) = common::state_with_fakes(repo);
    let user_id = UserId::new("u1");
    let rename = PatchMeRequest {
        display_name: Some("Quinn".into()),
        avatar_color: None,
        avatar_media_id: None,
    };

    let me = me::patch_me(&state, &user_id, rename).await.unwrap();

    assert_eq!(me.avatar_media_id, Some(old));
    assert!(store.deleted().await.is_empty());
}

#[tokio::test]
async fn a_failed_delete_of_the_replaced_avatar_still_saves_the_profile() {
    let (_c, repo) = test_factories::make_test_repo().await;
    let (old, new) = seed_user_with_two_avatars(&repo).await;
    let (state, store) = common::state_with_fakes(repo);
    store.set_failing(true);
    let user_id = UserId::new("u1");

    let me = me::patch_me(&state, &user_id, set_avatar(&new))
        .await
        .expect("the delete is best effort");

    assert_eq!(me.avatar_media_id, Some(new));
    assert!(media::get_avatar(&state.repo, &user_id, &old)
        .await
        .unwrap()
        .is_some());
}
