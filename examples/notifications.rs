//! Print the user's unread notifications.
//!
//! Run with: `cargo run --example notifications`
//! (see `examples/common/mod.rs` for the required environment variables)

mod common;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let session = common::login().await?;

    let notifications = session.unread_notifications().await?;
    println!("{} unread notification(s)", notifications.len());

    for notification in notifications.iter() {
        println!(
            "  {}  {} — {}",
            notification.time_sent, notification.title, notification.sub_title,
        );
        if let Some(message_id) = notification.message_id() {
            println!("     → message #{message_id}");
        }
    }

    Ok(())
}