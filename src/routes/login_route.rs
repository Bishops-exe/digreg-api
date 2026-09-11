use monostate::MustBe;
use serde::{Deserialize, Serialize};
use thiserror::Error;
pub(crate) use crate::routes::Route;

#[derive(Serialize)]
pub struct LoginRoute {
    pub username: String,
    pub password: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub two_factor: Option<String>
}

impl Route for LoginRoute {
    type Response = LoginResponse;

    fn get_route() -> &'static str {
        "api/auth/login"
    }
}


#[derive(Deserialize, Debug, PartialEq)]
#[serde(untagged)]
pub enum LoginResponse {
    Success {
        #[serde(rename = "loggedIn")]
        logged_in: MustBe!(true),
    },

    #[serde(rename_all = "camelCase")]
    PasswordExpired {
        error: MustBe!("password_expired"),
        email_for_messages_enabled: bool,
        email_for_substitutions_enabled: bool,
        show_switch_notification_substitution: bool
    },

    InvalidPassword {
        error: MustBe!("password_wrong")
    },

    InvalidUser {
        error: MustBe!("user_not_found")
    },

    TwoFactorNeeded {
        error: MustBe!("two_factor_needed")
    },

    Failure {
        #[serde(rename = "loggedIn")]
        logged_in: MustBe!(false),
        error: String,
        message: Option<String>,
    },
}

/// The reason a login attempt did not produce a session.
#[derive(Debug, PartialEq, Error)]
pub enum SchoolLoginErrorInner {
    #[error("the password has expired")]
    PasswordExpired,

    #[error("the password is wrong")]
    InvalidPassword,

    #[error("the user was not found")]
    InvalidUser,

    #[error("2fa needed")]
    TwoFactorNeeded,

    #[error("[{error}] {message}")]
    OtherError { error: String, message: String },
}

impl Into<Result<(), SchoolLoginErrorInner>> for LoginResponse {
    fn into(self) -> Result<(), SchoolLoginErrorInner> {
        match self {
            LoginResponse::Success { .. } => Ok(()),
            LoginResponse::PasswordExpired { .. } => Err(SchoolLoginErrorInner::PasswordExpired),
            LoginResponse::InvalidPassword { .. } => Err(SchoolLoginErrorInner::InvalidPassword),
            LoginResponse::InvalidUser { .. } => Err(SchoolLoginErrorInner::InvalidUser),
            LoginResponse::TwoFactorNeeded { .. } => Err(SchoolLoginErrorInner::TwoFactorNeeded),
            LoginResponse::Failure { error, message, .. } => {
                Err(SchoolLoginErrorInner::OtherError { error: error.clone(), message: message.unwrap_or(error) })
            }

        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(json: &str) -> Result<(), SchoolLoginErrorInner> {
        serde_json::from_str::<LoginResponse>(json).unwrap().into()
    }

    #[test]
    fn omits_the_two_factor_key_when_absent() {
        let route = LoginRoute {
            username: "mmustermann".to_owned(),
            password: "hunter2".to_owned(),
            two_factor: None,
        };
        assert_eq!(
            serde_json::to_value(&route).unwrap(),
            serde_json::json!({ "username": "mmustermann", "password": "hunter2" }),
        );
    }

    #[test]
    fn includes_the_two_factor_key_when_present() {
        let route = LoginRoute {
            username: "mmustermann".to_owned(),
            password: "hunter2".to_owned(),
            two_factor: Some("123456".to_owned()),
        };
        assert_eq!(
            serde_json::to_value(&route).unwrap(),
            serde_json::json!({
                "username": "mmustermann",
                "password": "hunter2",
                "two_factor": "123456",
            }),
        );
    }

    #[test]
    fn success_response_maps_to_ok() {
        let response: LoginResponse =
            serde_json::from_str(r#"{ "loggedIn": true }"#).unwrap();
        assert!(matches!(response, LoginResponse::Success { .. }));
        assert_eq!(outcome(r#"{ "loggedIn": true }"#), Ok(()));
    }

    #[test]
    fn wrong_password_response_maps_to_invalid_password() {
        assert_eq!(
            outcome(r#"{ "error": "password_wrong" }"#),
            Err(SchoolLoginErrorInner::InvalidPassword),
        );
    }

    #[test]
    fn unknown_user_response_maps_to_invalid_user() {
        assert_eq!(
            outcome(r#"{ "error": "user_not_found" }"#),
            Err(SchoolLoginErrorInner::InvalidUser),
        );
    }

    #[test]
    fn password_expired_response_maps_to_password_expired() {
        let json = r#"{
            "error": "password_expired",
            "emailForMessagesEnabled": false,
            "emailForSubstitutionsEnabled": true,
            "showSwitchNotificationSubstitution": false
        }"#;
        assert_eq!(outcome(json), Err(SchoolLoginErrorInner::PasswordExpired));
    }

    #[test]
    fn generic_failure_response_keeps_error_and_message() {
        let json = r#"{
            "loggedIn": false,
            "error": "wrong_credentials",
            "message": "Benutzername oder Passwort falsch."
        }"#;
        assert_eq!(
            outcome(json),
            Err(SchoolLoginErrorInner::OtherError {
                error: "wrong_credentials".to_owned(),
                message: "Benutzername oder Passwort falsch.".to_owned(),
            }),
        );
    }
}
