//! Integration test for the notifications endpoint against a real Digitales
//! Register instance. See `tests/common/mod.rs` for how credentials are
//! supplied. This test only reads — it never marks notifications read.

mod common;

#[tokio::test]
async fn fetches_the_unread_notifications() {
    let Some(session) = common::session_or_skip().await else {
        return;
    };

    let notifications = session
        .unread_notifications()
        .await
        .expect("the notification/unread request should succeed");

    for notification in notifications.iter() {
        // `message_id` is `Some` exactly for `type == "message"`.
        assert_eq!(
            notification.message_id().is_some(),
            notification.kind == "message" && notification.object_id.is_some(),
        );
    }
}