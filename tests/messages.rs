//! Integration test for the messages endpoint against a real Digitales
//! Register instance. See `tests/common/mod.rs` for how credentials are
//! supplied. This test only reads — it never marks a message read.

mod common;

#[tokio::test]
async fn fetches_every_message_addressed_to_the_user() {
    let Some(session) = common::session_or_skip().await else {
        return;
    };

    let messages = session
        .messages()
        .await
        .expect("the getMyMessages request should succeed");

    for message in messages.iter() {
        assert_eq!(message.is_unread(), message.time_read.is_none());
        // Every surfaced attachment is a fully-populated downloadable file.
        for file in message.downloadable_files() {
            assert!(file.is_downloadable_file());
        }
    }
}