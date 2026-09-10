//! Fetch the current week's timetable and print it period by period.
//!
//! Run with: `cargo run --example calendar`
//! (see `examples/common/mod.rs` for the required environment variables)

mod common;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let session = common::login().await?;

    let week = session.calendar_this_week().await?;

    for (date, lessons) in week.iter() {
        println!("{date}");
        for lesson in lessons {
            let room = lesson.rooms.first().map(|r| r.name.as_str()).unwrap_or("—");
            let teacher = lesson
                .teachers
                .first()
                .map(|t| t.last_name.as_str())
                .unwrap_or("—");
            println!(
                "  {:>2}. {:<24} {:<8} {}",
                lesson.span.start.hour, lesson.subject.name, room, teacher,
            );
        }
    }

    Ok(())
}