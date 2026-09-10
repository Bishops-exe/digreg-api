//! Integration tests for the dashboard endpoint against a real Digitales
//! Register instance. See `tests/common/mod.rs` for how credentials are
//! supplied. These tests only read — they never create reminders on the
//! account.

mod common;

#[tokio::test]
async fn fetches_the_past_and_future_dashboards() {
    let Some(session) = common::session_or_skip().await else {
        return;
    };

    let past = session
        .dashboard_past()
        .await
        .expect("the past dashboard should load");
    let future = session
        .dashboard_future()
        .await
        .expect("the future dashboard should load");

    // Every day carries an ISO date; every item a stable id. Just walking the
    // structure proves it deserialized into the documented shape.
    for day in past.iter().chain(future.iter()) {
        assert_eq!(day.date.len(), "2022-10-19".len(), "date is `yyyy-MM-dd`");
        for item in &day.items {
            let _ = (item.id, item.checkable, item.checked, item.is_deletable());
        }
    }
}