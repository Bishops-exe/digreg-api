//! Integration test for session keep-alive against a real Digitales Register
//! instance. See `tests/common/mod.rs` for how credentials are supplied.

mod common;

#[tokio::test]
async fn extends_a_freshly_authenticated_session() {
    let Some(session) = common::session_or_skip().await else {
        return;
    };

    session
        .extend()
        .await
        .expect("extendSession should succeed for a fresh session");
}