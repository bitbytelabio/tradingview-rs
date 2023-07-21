use google_authenticator::get_code;
use google_authenticator::GA_AUTH;
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use tracing::{debug, error, info, warn};

use crate::errors::LoginError;

#[derive(Debug, Deserialize)]
pub struct LoginUserResponse {
    error: String,
    user: User,
}

#[derive(Debug, Deserialize)]
pub struct User {
    id: u32,
    username: String,
    is_pro: bool,
    auth_token: String,
    session_hash: String,
    private_channel: String,

    #[serde(skip_deserializing)]
    password: String,
    #[serde(skip_deserializing)]
    totp_secret: String,
    #[serde(skip_deserializing)]
    session: String,
    #[serde(skip_deserializing)]
    session_signature: String,
}

impl User {
    pub async fn new(
        username: Option<String>,
        password: Option<String>,
        totp_secret: Option<String>,
    ) -> Self {
        if username.is_none() || password.is_none() {
            warn!("Username or password is empty, using the free user, data would be limited");
            return User {
                id: 0,
                username: "".to_owned(),
                is_pro: false,
                auth_token: "unauthorized_user_token".to_owned(),
                session_hash: "".to_owned(),
                private_channel: "".to_owned(),
                password: "".to_owned(),
                totp_secret: "".to_owned(),
                session: "".to_owned(),
                session_signature: "".to_owned(),
            };
        }

        Self::login(username.unwrap(), password.unwrap(), totp_secret)
            .await
            .unwrap()
    }

    async fn login(
        username: String,
        password: String,
        totp_secret: Option<String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        use reqwest::multipart::Form;

        let client: reqwest::Client = crate::utils::build_client(None)?;

        let response = client
            .post("https://www.tradingview.com/accounts/signin/")
            .multipart(
                Form::new()
                    .text("username", username)
                    .text("password", password)
                    .text("remember", "true".to_owned()),
            )
            .send()
            .await
            .unwrap();

        let (session, session_signature) =
            response
                .cookies()
                .fold((None, None), |session_cookies, cookie| {
                    if cookie.name() == "sessionid" {
                        (Some(cookie.value().to_string()), session_cookies.1)
                    } else if cookie.name() == "sessionid_sign" {
                        (session_cookies.0, Some(cookie.value().to_string()))
                    } else {
                        session_cookies
                    }
                });

        if session.is_none() || session_signature.is_none() {
            error!("Wrong username or password");
            return Err(Box::new(LoginError::LoginFailed));
        }

        let response: Value = response.json().await?;

        if response["error"] == "".to_owned() {
            debug!("User data: {:#?}", response);
            warn!("2FA is not enabled for this account");
            info!("User is logged in successfully");
            let login_resp: LoginUserResponse = serde_json::from_value(response)?;
            Ok(login_resp.user)
        } else if response["error"] == "2FA_required".to_owned() {
            if totp_secret.is_none() {
                error!("2FA is enabled for this account, but no secret was provided");
                return Err(Box::new(LoginError::InvalidTOTP));
            }
            let response: Value = Self::handle_2fa(
                &totp_secret.unwrap(),
                session.unwrap().as_str(),
                session_signature.unwrap().as_str(),
            )
            .await?
            .json()
            .await?;
            if response["error"] == "".to_owned() {
                debug!("User data: {:#?}", response);
                info!("User is logged in successfully");
                let login_resp: LoginUserResponse = serde_json::from_value(response)?;
                Ok(login_resp.user)
            } else {
                error!("2FA authentication failed, something went wrong");
                Err(Box::new(LoginError::MFALoginFailed))
            }
        } else {
            Err(Box::new(LoginError::LoginFailed))
        }
    }

    async fn handle_2fa(
        totp_secret: &str,
        session: &str,
        session_signature: &str,
    ) -> Result<reqwest::Response, Box<dyn std::error::Error>> {
        use reqwest::multipart::Form;

        let client: reqwest::Client = crate::utils::build_client(Some(&format!(
            "sessionid={}; sessionid_sign={}",
            session, session_signature
        )))?;

        let response = client
            .post("https://www.tradingview.com/accounts/two-factor/signin/totp/")
            .multipart(Form::new().text("code", get_code!(totp_secret)?))
            .send()
            .await?;

        if response.status().is_success() {
            return Ok(response);
        } else {
            return Err(Box::new(LoginError::MFALoginFailed));
        }
    }

    pub async fn get_auth_token(
        self,
        session: String,
        session_signature: String,
        url: Option<String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let client = crate::utils::build_client(Some(&format!(
            "sessionid={}; sessionid_sign={}",
            session, session_signature
        )))?;

        let resp_body = client
            .get(url.unwrap_or_else(|| "https://www.tradingview.com/".to_string()))
            .send()
            .await?
            .text()
            .await?;

        if resp_body.contains("auth_token") {
            info!("User is logged in");
            info!("Loading auth token and user info");
            let id_regex = Regex::new(r#""id":([0-9]{1,10}),"#)?;
            let id: u32 = match id_regex.captures(&resp_body) {
                Some(captures) => captures[1].to_string().parse()?,
                None => {
                    error!("Error parsing user id");
                    return Err(Box::new(LoginError::ParseUserDataError));
                }
            };

            let username_regex = Regex::new(r#""username":"(.*?)""#)?;
            let username = match username_regex.captures(&resp_body) {
                Some(captures) => captures[1].to_string(),
                None => {
                    error!("Error parsing username");
                    return Err(Box::new(LoginError::ParseUserDataError));
                }
            };

            let session_hash_regex = Regex::new(r#""session_hash":"(.*?)""#)?;
            let session_hash = match session_hash_regex.captures(&resp_body) {
                Some(captures) => captures[1].to_string(),
                None => {
                    error!("Error parsing username");
                    return Err(Box::new(LoginError::ParseUserDataError));
                }
            };

            let private_channel_regex = Regex::new(r#""private_channel":"(.*?)""#)?;
            let private_channel = match private_channel_regex.captures(&resp_body) {
                Some(captures) => captures[1].to_string(),
                None => {
                    error!("Error parsing username");
                    return Err(Box::new(LoginError::ParseUserDataError));
                }
            };

            let auth_token_regex = Regex::new(r#""auth_token":"(.*?)""#)?;
            let auth_token = match auth_token_regex.captures(&resp_body) {
                Some(captures) => captures[1].to_string(),
                None => {
                    error!("Error parsing auth token");
                    return Err(Box::new(LoginError::ParseAuthTokenError));
                }
            };

            return Ok(User {
                id: id,
                username,
                session_hash,
                private_channel,
                auth_token,
                ..self
            });
        }
        Err(Box::new(LoginError::SessionExpired))
    }
}
