//! Integration test that talks to a real Digitales Register instance.
//! See `tests/common/mod.rs` for how credentials are supplied.

mod common;

use chrono::{Datelike, Days, NaiveDate, Timelike, Weekday};

#[tokio::test]
async fn fetches_the_current_week() {
    let Some(session) = common::session_or_skip().await else {
        return;
    };

    let week = session
        .calendar_this_week()
        .await
        .expect("calendar request should succeed");

    assert!(!week.is_empty(), "the week should contain at least one day");

    for lesson in week.values().flatten() {
            assert!(
                lesson.span.start.time < lesson.span.end.time
                    || lesson.span.end.time.num_seconds_from_midnight() == 0,
                "lesson should start before it ends"
            );
    }
}

#[tokio::test]
async fn snaps_a_mid_week_date_back_to_its_monday() {
    let Some(session) = common::session_or_skip().await else {
        return;
    };

    // A Wednesday well inside a normal school term.
    let wednesday = NaiveDate::from_ymd_opt(2025, 11, 12).unwrap();
    assert_eq!(wednesday.weekday(), Weekday::Wed);
    let monday = wednesday - Days::new(2);

    let week = session
        .calendar(wednesday)
        .await
        .expect("calendar request should succeed");

    for day in week.keys() {
        assert!(
            (*day >= monday) && (*day <= monday + Days::new(6)),
            "every returned day ({day}) lies in the week starting {monday}",
        );
    }
}
