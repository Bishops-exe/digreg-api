//! Summarise the student's absences and upcoming absences.
//!
//! Run with: `cargo run --example absences`
//! (see `examples/common/mod.rs` for the required environment variables)

mod common;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let session = common::login().await?;

    let report = session.absences().await?;

    let stats = &report.statistics;
    println!(
        "{} absences total — {} justified, {} not justified, {} delayed",
        stats.counter, stats.justified, stats.not_justified, stats.delayed,
    );

    println!("\nPast:");
    for absence in &report.absences {
        let day = absence.group.first().map(|h| h.date.to_string()).unwrap_or_default();
        println!(
            "  {day}  {:?}  {} min over {} hour(s)",
            absence.justified,
            absence.total_minutes(),
            absence.group.len(),
        );
    }

    println!("\nUpcoming:");
    for future in &report.future_absences {
        println!(
            "  {} – {}  periods {}-{}  {:?}",
            future.start_date, future.end_date, future.start_time, future.end_time, future.justified,
        );
    }

    Ok(())
}