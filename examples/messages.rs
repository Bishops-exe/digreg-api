//! List every message addressed to the user, newest field first.
//!
//! Run with: `cargo run --example messages`
//! (see `examples/common/mod.rs` for the required environment variables)

mod common;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let session = common::login().await?;

    for message in session.messages().await?.iter() {
        let mark = if message.is_unread() { "•" } else { " " };
        println!(
            "{mark} {}  {}  (from {})",
            message.time_sent, message.subject, message.from_name,
        );
        for file in message.downloadable_files() {
            println!(
                "     attachment: {}",
                file.original_name.as_deref().unwrap_or("?"),
            );
        }
    }

    Ok(())
}