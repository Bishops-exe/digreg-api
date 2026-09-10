//! Shared credential loading for the examples.
//!
//! Reads a `.env` file in the crate root (or real environment variables); see
//! `.env.example`:
//!
//! ```text
//! SCHOOL_SUBDOMAIN=myschool
//! SCHOOL_USERNAME=my.user
//! SCHOOL_PASSWORD=my-password
//! SCHOOL_TWO_FACTOR=          # optional
//! SCHOOL_STUDENT_ID=          # only needed by the grades example
//! ```

#![allow(dead_code)]

use std::env;
use std::error::Error;

use digreg_api::{School, Session};

/// Resolve the school and log in, using credentials from the environment.
pub async fn login() -> Result<Session, Box<dyn Error>> {
    // A missing `.env` is fine as long as the vars are set some other way.
    let _ = dotenvy::dotenv();

    let subdomain = env::var("SCHOOL_SUBDOMAIN")?;
    let username = env::var("SCHOOL_USERNAME")?;
    let password = env::var("SCHOOL_PASSWORD")?;
    let two_factor = env::var("SCHOOL_TWO_FACTOR").ok().filter(|v| !v.is_empty());

    let session = School::create(subdomain)
        .await?
        .login(username, password, two_factor)
        .await?;

    Ok(session)
}

/// The student id for the grades endpoint (`SCHOOL_STUDENT_ID`). The library
/// does not scrape it from the session config yet, so it has to be supplied.
pub fn student_id() -> Result<i64, Box<dyn Error>> {
    let raw = env::var("SCHOOL_STUDENT_ID").map_err(|_| {
        "set SCHOOL_STUDENT_ID (the student's user id) to run this example"
    })?;
    Ok(raw.trim().parse()?)
}