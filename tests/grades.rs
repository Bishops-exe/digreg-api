//! Integration test for the grades endpoint against a real Digitales Register
//! instance. Needs `SCHOOL_STUDENT_ID` on top of the login credentials — see
//! `tests/common/mod.rs`.

mod common;

#[tokio::test]
async fn fetches_every_subject_with_its_grade_summary() {
    let Some(session) = common::session_or_skip().await else {
        return;
    };
    let Some(student_id) = common::student_id_or_skip() else {
        return;
    };

    let response = session
        .all_subjects(student_id)
        .await
        .expect("the all_subjects request should succeed");

    for subject in &response.subjects {
        assert_eq!(
            subject.student.id, student_id,
            "every subject is scoped to the requested student",
        );
        for entry in &subject.grades {
            // The centi-grade round-trips through its string form.
            assert_eq!(
                entry.grade.to_string().parse().ok(),
                Some(entry.grade),
            );
        }
    }
}