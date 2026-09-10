//! Integration test for the absences endpoint against a real Digitales
//! Register instance. See `tests/common/mod.rs` for how credentials are
//! supplied.

mod common;

#[tokio::test]
async fn fetches_absences_and_statistics() {
    let Some(session) = common::session_or_skip().await else {
        return;
    };

    let response = session
        .absences()
        .await
        .expect("the absences request should succeed");

    for absence in &response.absences {
        assert_eq!(
            absence.total_minutes(),
            absence.group.iter().map(|hour| hour.minutes).sum::<u64>(),
            "total_minutes sums the group",
        );
    }

    for future in &response.future_absences {
        assert!(
            future.start_date <= future.end_date,
            "an upcoming absence starts on or before it ends",
        );
    }
}