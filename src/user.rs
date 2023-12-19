pub use crate::models::UserCookies;
use crate::{error::Error, error::LoginError, Result, UA};
use google_authenticator::{get_code, GA_AUTH};
use reqwest::{
    header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE, COOKIE, ORIGIN, REFERER},
    Client, Response,
};
use serde::Deserialize;
use serde_json::Value;
use tracing::{debug, error, info, warn};

impl UserCookies {
    pub fn new() -> Self {
        UserCookies::default()
    }

    pub async fn login(
        &mut self,
        username: &str,
        password: &str,
        totp_secret: Option<&str>,
    ) -> Result<Self> {
        let mut headers = HeaderMap::new();

        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            ORIGIN,
            HeaderValue::from_static("https://www.tradingview.com"),
        );
        headers.insert(
            REFERER,
            HeaderValue::from_static("https://www.tradingview.com/"),
        );
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/x-www-form-urlencoded"),
        );

        let client = Client::builder()
            .use_rustls_tls()
            .default_headers(headers)
            .https_only(true)
            .user_agent(UA)
            .build()?;

        let response = client
            .post("https://www.tradingview.com/accounts/signin/")
            .body(format!(
                "username={}&password={}&remember=true",
                username, password
            ))
            .send()
            .await?;

        let (session, signature, device_token) =
            response
                .cookies()
                .fold((None, None, None), |session_cookies, cookie| {
                    match cookie.name() {
                        "sessionid" => (
                            Some(cookie.value().to_string()),
                            session_cookies.1,
                            session_cookies.2,
                        ),
                        "sessionid_sign" => (
                            session_cookies.0,
                            Some(cookie.value().to_string()),
                            session_cookies.2,
                        ),
                        "device_t" => (
                            session_cookies.0,
                            session_cookies.1,
                            Some(cookie.value().to_string()),
                        ),
                        _ => session_cookies,
                    }
                });
        if session.is_none() || signature.is_none() {
            error!("unable to login, username or password is invalid");
            return Err(Error::LoginError(LoginError::InvalidCredentials));
        }

        #[derive(Debug, Deserialize)]
        struct LoginUserResponse {
            user: UserCookies,
        }

        let response: Value = response.json().await?;

        let user: UserCookies;

        if response["error"] == *"" {
            debug!("User data: {:#?}", response);
            warn!("2FA is not enabled for this account");
            info!("User is logged in successfully");
            let login_resp: LoginUserResponse = serde_json::from_value(response)?;

            user = login_resp.user;
        } else if response["error"] == *"2FA_required" {
            if totp_secret.is_none() {
                error!("2FA is enabled for this account, but no TOTP secret was provided");
                return Err(Error::LoginError(LoginError::OTPSecretNotFound));
            }

            let response = Self::handle_mfa(
                totp_secret.unwrap(),
                session.clone().unwrap_or_default().as_str(),
                signature.clone().unwrap_or_default().as_str(),
            )
            .await?;

            let (session, signature, device_token) =
                response
                    .cookies()
                    .fold((None, None, None), |session_cookies, cookie| {
                        match cookie.name() {
                            "sessionid" => (
                                Some(cookie.value().to_string()),
                                session_cookies.1,
                                session_cookies.2,
                            ),
                            "sessionid_sign" => (
                                session_cookies.0,
                                Some(cookie.value().to_string()),
                                session_cookies.2,
                            ),
                            "device_t" => (
                                session_cookies.0,
                                session_cookies.1,
                                Some(cookie.value().to_string()),
                            ),
                            _ => session_cookies,
                        }
                    });

            let body = response.json().await?;
            debug!("2FA login response: {:#?}", body);
            info!("User is logged in successfully");
            let login_resp: LoginUserResponse = serde_json::from_value(body)?;

            user = login_resp.user;

            return Ok(UserCookies {
                session: session.unwrap_or_default(),
                session_signature: signature.unwrap_or_default(),
                device_token: device_token.unwrap_or_default(),
                ..user
            });
        } else {
            error!("unable to login, username or password is invalid");
            return Err(Error::LoginError(LoginError::InvalidCredentials));
        }

        Ok(UserCookies {
            session: session.unwrap_or_default(),
            session_signature: signature.unwrap_or_default(),
            device_token: device_token.unwrap_or_default(),
            ..user
        })
    }

    async fn handle_mfa(totp_secret: &str, session: &str, signature: &str) -> Result<Response> {
        if totp_secret.is_empty() {
            return Err(Error::LoginError(LoginError::OTPSecretNotFound));
        }

        let mut headers = HeaderMap::new();

        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            ORIGIN,
            HeaderValue::from_static("https://www.tradingview.com"),
        );
        headers.insert(
            REFERER,
            HeaderValue::from_static("https://www.tradingview.com/"),
        );
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/x-www-form-urlencoded"),
        );
        headers.insert(
            COOKIE,
            HeaderValue::from_str(&format!("sessionid={session}; sessionid_sign={signature};"))?,
        );

        let client = Client::builder()
            .use_rustls_tls()
            .default_headers(headers)
            .https_only(true)
            .user_agent(UA)
            .build()?;

        let response = client
            .post("https://www.tradingview.com/accounts/two-factor/signin/totp/")
            .body(format!(
                "code={}",
                match get_code!(totp_secret) {
                    Ok(code) => code,
                    Err(e) => {
                        error!("Error generating TOTP code: {}", e);
                        return Err(Error::LoginError(LoginError::InvalidOTPSecret));
                    }
                }
            ))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response)
        } else {
            Err(Error::LoginError(LoginError::InvalidOTPSecret))
        }
    }
}
