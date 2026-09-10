use crate::routes::Route;
use chrono::{DateTime, Utc};
use chrono::serde::ts_seconds;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtendSessionRoute {
    #[serde(with = "ts_seconds")]
    pub(crate) last_action: DateTime<Utc>,
}

impl Route for ExtendSessionRoute {
    type Response = ExtendSessionResponse;

    fn get_route() -> &'static str {
        "api/auth/extendSession"
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtendSessionResponse {
    pub force_logout: bool,
    #[serde(with = "ts_seconds")]
    pub new_expiration: DateTime<Utc>,
    pub no_session: bool,
    #[serde(with = "ts_seconds")]
    pub server_time: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn serializes_last_action_as_unix_seconds() {
        let route = ExtendSessionRoute {
            last_action: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        };
        assert_eq!(
            serde_json::to_value(&route).unwrap(),
            serde_json::json!({ "lastAction": 1_700_000_000 }),
        );
    }

    #[test]
    fn deserializes_the_response_timestamps() {
        let json = r#"{
            "forceLogout": false,
            "newExpiration": 1700000600,
            "noSession": false,
            "serverTime": 1700000000
        }"#;

        let response: ExtendSessionResponse = serde_json::from_str(json).unwrap();

        assert!(!response.force_logout);
        assert!(!response.no_session);
        assert_eq!(response.new_expiration.timestamp(), 1_700_000_600);
        assert_eq!(response.server_time.timestamp(), 1_700_000_000);
    }
}
