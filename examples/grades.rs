//! List every subject with its grade summary for the current semester.
//!
//! Run with: `cargo run --example grades`
//! Needs `SCHOOL_STUDENT_ID` in addition to the login variables
//! (see `examples/common/mod.rs`).

mod common;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let session = common::login().await?;
    let student_id = common::student_id()?;

    let report = session.all_subjects(student_id).await?;

    for subject in &report.subjects {
        println!(
            "{}  (Ø semester {:.2}, year {:.2})",
            subject.subject.name, subject.average_semester, subject.average_year,
        );
        for grade in &subject.grades {
            let cancelled = if grade.cancelled { "  (cancelled)" } else { "" };
            println!(
                "  {}  {:>6}  weight {:>3}%  {}{}",
                grade.date, grade.grade, grade.weight, grade.type_name, cancelled,
            );
        }
    }

    Ok(())
}