use reqwest::header::HeaderMap;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub mod login_route;
pub mod dashboard_route;
pub mod extend_session;
pub mod calendar_route;
pub mod grades_route;
pub mod absences_route;
pub mod message_route;
pub mod notification_route;

use crate::school::School;
use reqwest::{Client, Method, StatusCode};
use thiserror::Error;
use crate::Session;

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("error: {0}")]
    Request(#[from] reqwest::Error),

    #[error("login error: {0}")]
    ParseJson(#[from] serde_json::Error),

    #[error("the client has been logged out")]
    Logout,

    #[error("the server did not report success")]
    Unsuccessful,

    #[error("status code error {0}")]
    StatusError(StatusCode),
}

/// Probe for the bare `{"forceLogout":true,"noSession":true}` body that some
/// endpoints return in place of the redirect-script logout marker.
#[derive(Deserialize)]
struct LogoutSignal {
    #[serde(default, rename = "noSession")]
    no_session: bool,
}

pub(crate) type FetchResult<T> = Result<T, FetchError>;
pub(crate) type PackedFetch<T> = (StatusCode, HeaderMap, T);
pub(crate) type PackedFetchResult<T> = FetchResult<PackedFetch<T>>;

pub(crate) trait Route: Serialize + Sized {
    type Response: DeserializeOwned;

    /// The HTTP method. Almost every endpoint is `POST` — even the ones with
    /// an empty (`{}`) body — so that is the default; only a couple of
    /// endpoints (`student/certificate`, `?semesterWechsel=N`) override it to
    /// `GET`.
    const METHOD: Method = Method::POST;

    fn get_route() -> &'static str;

    async fn fetch(
        &self,
        school: &School,
        client: &Client,
    ) -> PackedFetchResult<Self::Response> {
        let url = school.extend_base_url(Self::get_route());

        let fetch = match Self::METHOD {
            // GET endpoints carry no body; every other method sends the route
            // serialized as JSON, including an empty struct as `{}`.
            Method::GET => client.get(url),
            method => client.request(method, url).json(self),
        };

        // The browser / React Native cookie store holds the session cookie;
        // ask it to attach cookies to this cross-origin request.
        #[cfg(target_arch = "wasm32")]
        let fetch = fetch.fetch_credentials_include();

        let resp = fetch.send().await?;

        let status = resp.status();
        let headers = resp.headers().clone();
        let text = resp.text().await?;
        let trimmed = text.trim();

        if trimmed.starts_with("<script type=\"text/javascript\">")
            && trimmed.ends_with("</script>")
            && trimmed.contains("window.location = \"https://")
            && trimmed.contains(".digitalesregister.it/v2/login\";")
        {
            return Err(FetchError::Logout)
        }

        // An expired session is also signalled, on some endpoints, as a plain
        // JSON body `{"forceLogout":true,"noSession":true}` rather than the
        // redirect script above. Without this check it surfaces as a confusing
        // deserialization error. (A healthy `extendSession` reply also carries
        // `noSession`, but set to `false`.)
        if let Ok(LogoutSignal { no_session: true }) = serde_json::from_str(trimmed) {
            return Err(FetchError::Logout);
        }

        let data =  serde_json::from_str::<Self::Response>(text.as_str())?;

        Ok((status, headers, data))
    }

    async fn fetch_client(&self, session: &Session) -> PackedFetchResult<Self::Response> {
        self.fetch(&session.school, &session.client).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_logout(body: &str) -> bool {
        matches!(
            serde_json::from_str(body.trim()),
            Ok(LogoutSignal { no_session: true }),
        )
    }

    #[test]
    fn recognises_the_json_logout_body() {
        assert!(is_logout(r#"{"forceLogout":true,"noSession":true}"#));
    }

    #[test]
    fn a_healthy_extend_session_reply_is_not_a_logout() {
        assert!(!is_logout(
            r#"{"forceLogout":false,"newExpiration":1700000600,"noSession":false,"serverTime":1700000000}"#
        ));
    }

    #[test]
    fn ordinary_payloads_are_not_mistaken_for_a_logout() {
        assert!(!is_logout(r#"{"loggedIn":true}"#));
        assert!(!is_logout(r#"[{"date":"2022-10-19","items":[]}]"#));
        assert!(!is_logout(r#"{"subjects":[]}"#));
    }
}
