//! Shared helpers for the integration tests.
//!
//! Credentials come from a `.env` file in the crate root (see `.env.example`)
//! or from real environment variables:
//!
//! ```text
//! SCHOOL_SUBDOMAIN=myschool
//! SCHOOL_USERNAME=my.user
//! SCHOOL_PASSWORD=my-password
//! SCHOOL_TWO_FACTOR=          # optional
//! ```
//!
//! When the variables are missing, [`credentials_or_skip`] prints a notice and
//! returns `None` so the caller can `return` early and let `cargo test` still
//! succeed on machines without credentials.

#![allow(dead_code)]

use std::sync::Arc;

use digreg_api::{School, Session};
use tokio::sync::OnceCell;

pub struct Credentials {
    pub subdomain: String,
    pub username: String,
    pub password: String,
    pub two_factor: Option<String>,
}

/// One shared session per test binary. The backend invalidates an account's
/// older sessions whenever it logs in again, so the tests in a file — which
/// `libtest` runs on parallel threads — must not each log in on their own.
static SESSION: OnceCell<Option<Arc<Session>>> = OnceCell::const_new();

/// Resolve the school, log in once per binary, and hand back the shared live
/// [`Session`] — or print a skip notice and return `None` when no credentials
/// are configured.
///
/// ```ignore
/// let Some(session) = common::session_or_skip().await else { return };
/// ```
pub async fn session_or_skip() -> Option<Arc<Session>> {
    SESSION
        .get_or_init(|| async {
            let credentials = credentials_or_skip()?;

            let school = School::create(credentials.subdomain)
                .await
                .expect("configured subdomain should resolve to a valid school");

            let session = school
                .login(
                    credentials.username,
                    credentials.password,
                    credentials.two_factor,
                )
                .await
                .expect("login with the configured credentials should succeed");

            Some(Arc::new(session))
        })
        .await
        .clone()
}

/// The student id for the grade endpoints (`SCHOOL_STUDENT_ID`). The library
/// does not scrape it from the session config yet, so tests that need it read
/// it from the environment and skip when it is absent.
pub fn student_id_or_skip() -> Option<i64> {
    match non_empty_var("SCHOOL_STUDENT_ID").and_then(|value| value.parse().ok()) {
        Some(id) => Some(id),
        None => {
            eprintln!("skipping: set SCHOOL_STUDENT_ID to run this test");
            None
        }
    }
}

/// Load credentials, or print a skip notice and return `None`.
///
/// ```ignore
/// let Some(credentials) = common::credentials_or_skip() else { return };
/// ```
pub fn credentials_or_skip() -> Option<Credentials> {
    match credentials() {
        Some(credentials) => Some(credentials),
        None => {
            eprintln!(
                "skipping: set SCHOOL_SUBDOMAIN / SCHOOL_USERNAME / SCHOOL_PASSWORD \
                 (e.g. in a .env file) to run this test"
            );
            None
        }
    }
}

pub fn credentials() -> Option<Credentials> {
    // Ignore the result: a missing `.env` is fine, real env vars may still be set.
    let _ = dotenvy::dotenv();

    Some(Credentials {
        subdomain: non_empty_var("SCHOOL_SUBDOMAIN")?,
        username: non_empty_var("SCHOOL_USERNAME")?,
        password: non_empty_var("SCHOOL_PASSWORD")?,
        two_factor: non_empty_var("SCHOOL_TWO_FACTOR"),
    })
}

fn non_empty_var(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|value| !value.is_empty())
}
