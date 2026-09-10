//! Integration tests that talk to a real Digitales Register instance.
//! See `tests/common/mod.rs` for how credentials are supplied.

mod common;

use digreg_api::School;

#[tokio::test]
async fn create_succeeds_for_configured_subdomain() {
    let Some(credentials) = common::credentials_or_skip() else {
        return;
    };

    let school = School::create(credentials.subdomain.clone())
        .await
        .expect("configured subdomain should resolve to a valid school");

    assert_eq!(school.get_subdomain(), credentials.subdomain);
}

#[tokio::test]
async fn create_rejects_unknown_subdomain() {
    // Only needs network access, but stays gated so `cargo test` is offline-safe
    // and never hits the network unless the user opted in with credentials.
    if common::credentials_or_skip().is_none() {
        return;
    }

    let result = School::create("digreg-api-nonexistent-subdomain-zzzz".to_owned()).await;

    assert!(result.is_err(), "unknown subdomain should not create a School");
}

#[tokio::test]
async fn login_with_valid_credentials_succeeds() {
    let Some(credentials) = common::credentials_or_skip() else {
        return;
    };

    let school = School::create(credentials.subdomain.clone())
        .await
        .expect("configured subdomain should resolve to a valid school");

    school
        .login(
            credentials.username,
            credentials.password,
            credentials.two_factor,
        )
        .await
        .expect("login with the configured credentials should succeed");
}

#[tokio::test]
async fn login_with_wrong_password_fails() {
    let Some(credentials) = common::credentials_or_skip() else {
        return;
    };

    let school = School::create(credentials.subdomain.clone())
        .await
        .expect("configured subdomain should resolve to a valid school");

    let result = school
        .login(
            credentials.username,
            "definitely-not-the-password".to_owned(),
            None,
        )
        .await;

    assert!(result.is_err(), "wrong password should not log in");
}
