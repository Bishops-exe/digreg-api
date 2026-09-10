//! Print the dashboard — both what is due now and what is coming up.
//!
//! Run with: `cargo run --example dashboard`
//! (see `examples/common/mod.rs` for the required environment variables)

mod common;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let session = common::login().await?;

    for (heading, view_future) in [("Due now", false), ("Upcoming", true)] {
        println!("== {heading} ==");
        for day in session.dashboard(view_future).await?.iter() {
            if day.items.is_empty() {
                continue;
            }
            println!("{}", day.date);
            for item in &day.items {
                let check = if item.checkable {
                    if item.checked { "[x] " } else { "[ ] " }
                } else {
                    ""
                };
                let label = item.label.as_deref().unwrap_or("—");
                println!("  {check}{label} — {}", item.title);
            }
        }
        println!();
    }

    Ok(())
}