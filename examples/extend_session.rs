//! Log in and immediately extend the session (the keep-alive call the app
//! makes shortly before the server would expire it).
//!
//! Run with: `cargo run --example extend_session`
//! (see `examples/common/mod.rs` for the required environment variables)

mod common;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let session = common::login().await?;

    let response = session.extend().await?;
    println!("{response:#?}");

    Ok(())
}