use crate::routes::login_route::SchoolLoginErrorInner;
use crate::routes::{FetchError, Route, login_route::LoginRoute};
use crate::session::Session;
use reqwest::header::HeaderMap;
use reqwest::Client;
use std::fmt::{self, Display, Formatter};
use thiserror::Error;
use url::{ParseError, Url};

// On wasm the host (browser / React Native) owns the cookie store, so the
// manual jar is compiled out — see `build_session_client`.
#[cfg(not(target_arch = "wasm32"))]
use {reqwest::cookie::Jar, reqwest::header::SET_COOKIE, std::sync::Arc};

pub const DOMAIN_SUFFIX: &str = ".digitalesregister.it";

#[derive(Debug, Error)]
pub enum SchoolCreationError {
    #[error("subdomain is empty")]
    EmptySubdomain,
    #[error("subdomain is reserved")]
    ReservedSubdomain,
    #[error("constructed url is invalid")]
    InvalidUrl(#[from] ParseError),
    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("invalid school")]
    InvalidSchool,
}

#[derive(Debug, Error)]
pub enum SchoolLoginError {
    #[error("request failed: {0}")]
    Request(#[from] FetchError),
    #[error("constructed url is invalid")]
    InvalidUrl(#[from] ParseError),

    #[error("login error: {0}")]
    Login(#[from] SchoolLoginErrorInner),

    #[error("client builder failed: {0}")]
    ClientBuilder(#[from] reqwest::Error),
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct School {
    subdomain: String,
}

impl School {
    pub async fn create(subdomain: String) -> Result<School, SchoolCreationError> {
        let school = School {
            subdomain,
        };

        if school.is_valid_school().await? {
            Ok(school)
        } else {
            Err(SchoolCreationError::InvalidSchool)
        }
    }

    async fn is_valid_school(&self) -> Result<bool, SchoolCreationError> {
        let url = self.get_full_url();

        if self.subdomain.is_empty() {
            return Err(SchoolCreationError::EmptySubdomain);
        }

        if self.subdomain == "www" {
            return Err(SchoolCreationError::ReservedSubdomain);
        }

        let parsed_url = Url::parse(&url)?;
        let requested_host = parsed_url.host_str().map(str::to_owned);

        let client = Client::builder().build()?;
        let resp = client.get(parsed_url).send().await?;

        Ok(resp.url().host_str() == requested_host.as_deref())
    }

    pub async fn login(
        self,
        username: String,
        password: String,
        two_factor: Option<String>,
    ) -> Result<Session, SchoolLoginError> {
        let route = LoginRoute {
            username,
            password,
            two_factor,
        };

        let (_, headers, response) = route.fetch(&self, &Client::builder().build()?).await?;
        Into::<Result<(), SchoolLoginErrorInner>>::into(response)?;

        let client = self.build_session_client(&headers)?;
        Ok(Session::from(client, self))
    }

    /// Build the client that carries the authenticated session.
    ///
    /// On native targets the session cookie from the login response is copied
    /// into a [`Jar`] attached to the client. On wasm the host runtime owns
    /// the cookie store (the browser, or React Native's native networking):
    /// it persists the `Set-Cookie` from the login response automatically and
    /// replays it on later requests, which [`Route::fetch`] opts into with
    /// `fetch(..., { credentials: "include" })`.
    #[cfg(not(target_arch = "wasm32"))]
    fn build_session_client(&self, headers: &HeaderMap) -> Result<Client, SchoolLoginError> {
        let jar = Jar::default();
        let url = Url::parse(&self.get_full_url())?;

        for value in headers.get_all(SET_COOKIE) {
            if let Ok(cookie_str) = value.to_str() {
                jar.add_cookie_str(cookie_str, &url);
            }
        }

        Ok(Client::builder().cookie_provider(Arc::new(jar)).build()?)
    }

    #[cfg(target_arch = "wasm32")]
    fn build_session_client(&self, _headers: &HeaderMap) -> Result<Client, SchoolLoginError> {
        Ok(Client::builder().build()?)
    }

    pub fn get_subdomain(&self) -> &str {
        &self.subdomain
    }

    fn get_full_url(&self) -> String {
        format!("https://{}{}/v2/", self.subdomain, DOMAIN_SUFFIX)
    }

    pub(crate) fn extend_base_url(&self, extension: &str) -> String {
        format!("{}{}", self.get_full_url(), extension)
    }
}

impl Display for School {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        self.get_full_url().fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn school(subdomain: &str) -> School {
        School { subdomain: subdomain.to_owned() }
    }

    #[test]
    fn builds_the_v2_base_url_from_the_subdomain() {
        assert_eq!(
            school("vinzentinum").get_full_url(),
            "https://vinzentinum.digitalesregister.it/v2/",
        );
    }

    #[test]
    fn extends_the_base_url_with_a_route() {
        assert_eq!(
            school("myschool").extend_base_url("api/auth/login"),
            "https://myschool.digitalesregister.it/v2/api/auth/login",
        );
    }

    #[test]
    fn display_renders_the_full_url() {
        assert_eq!(
            school("myschool").to_string(),
            "https://myschool.digitalesregister.it/v2/",
        );
    }

    #[test]
    fn get_subdomain_returns_the_configured_value() {
        assert_eq!(school("myschool").get_subdomain(), "myschool");
    }

    #[tokio::test]
    async fn create_rejects_an_empty_subdomain_without_a_request() {
        let error = School::create(String::new()).await.unwrap_err();
        assert!(matches!(error, SchoolCreationError::EmptySubdomain));
    }

    #[tokio::test]
    async fn create_rejects_the_reserved_www_subdomain_without_a_request() {
        let error = School::create("www".to_owned()).await.unwrap_err();
        assert!(matches!(error, SchoolCreationError::ReservedSubdomain));
    }
}
