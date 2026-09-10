# digreg-api

[![CI](https://github.com/Bishops-exe/digreg-api/actions/workflows/ci.yml/badge.svg)](https://github.com/Bishops-exe/digreg-api/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/digreg-api.svg)](https://crates.io/crates/digreg-api)
[![docs.rs](https://img.shields.io/docsrs/digreg-api)](https://docs.rs/digreg-api)
[![license](https://img.shields.io/crates/l/digreg-api.svg)](#license)

An async Rust client for the **Digitales Register** (`digitalesregister.it`)
school platform — the API behind the student/parent web and mobile apps.

This is an unofficial client. There is no public API specification; the request
and response shapes were reverse-engineered from the official web client and are
documented in [`docs/API.md`](docs/API.md). The upstream service can change
without notice.

## Installation

```toml
[dependencies]
digreg-api = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Usage

```rust,no_run
use digreg_api::School;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let session = School::create("myschool".to_owned())
        .await?
        .login(
            "my.user".to_owned(),
            "my-password".to_owned(),
            None, // two-factor code, if the account has 2FA enabled
        )
        .await?;

    for (date, lessons) in session.calendar_this_week().await?.iter() {
        println!("{date}");
        for lesson in lessons {
            println!("  {:>2}. {}", lesson.span.start.hour, lesson.subject.name);
        }
    }

    Ok(())
}
```

`School::create` resolves `<subdomain>.digitalesregister.it` and verifies it is
a real school; `login` establishes a cookie-backed [`Session`]. Every request
method lives on `Session`.

## What's covered

| Area | `Session` methods |
| --- | --- |
| Timetable | `calendar`, `calendar_this_week` |
| Dashboard | `dashboard`, `dashboard_future`, `dashboard_past`, `push_reminder` |
| Grades | `all_subjects` |
| Absences | `absences` |
| Messages | `messages` (+ `Message::mark_read`) |
| Notifications | `unread_notifications`, `mark_all_notifications_read` (+ `Notification::mark_read`) |
| Session | `extend` (keep-alive) |

Login handles two-factor auth and reports `password_expired` / `password_wrong`
/ `user_not_found` distinctly. Expired sessions surface as `FetchError::Logout`.

The messy wire formats are flattened on the way in — most notably the calendar,
which arrives buried under several meaningless nested maps and is exposed here
as a plain `BTreeMap<NaiveDate, Vec<Lesson>>`.

## WebAssembly

The crate builds for `wasm32-unknown-unknown` (CI checks this). On wasm,
`reqwest` uses the host's Fetch implementation and the host owns the cookie
store: the session cookie set at login is persisted by the browser / React
Native runtime and replayed automatically (`credentials: "include"`), so the
native cookie-jar code is compiled out.

Note that a browser context is still subject to CORS, which the upstream server
does not grant to third-party origins — a same-origin proxy is required there.

## Minimum supported Rust version

`1.88`. This is the floor of the dependency tree and may be raised in a minor
release.

## Running the examples and integration tests

Both read credentials from a `.env` file in the crate root (see
[`.env.example`](.env.example)) or from real environment variables:

```text
SCHOOL_SUBDOMAIN=myschool
SCHOOL_USERNAME=my.user
SCHOOL_PASSWORD=my-password
SCHOOL_TWO_FACTOR=          # optional
SCHOOL_STUDENT_ID=          # only the grades example / test
```

```sh
cargo run --example calendar
cargo test                  # network tests skip themselves when unset
```

## License

Licensed under the GNU Lesser General Public License v3.0 only
([`LICENSE`](LICENSE)).

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project shall be licensed as above, without any additional
terms or conditions.