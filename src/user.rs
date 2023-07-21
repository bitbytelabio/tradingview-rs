use futures_util::future::ErrInto;
use google_authenticator::get_code;
use google_authenticator::GA_AUTH;
use http::Error;
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use tracing::{debug, error, info, warn};

use crate::errors::LoginError;

#[derive(Debug, Deserialize)]
pub struct LoginUserResponse {
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
        let client: reqwest::Client = crate::utils::build_client(None)?;

        let response = client
            .post("https://www.tradingview.com/accounts/signin/")
            .multipart(
                reqwest::multipart::Form::new()
                    .text("username", username.clone())
                    .text("password", password.clone())
                    .text("remember", "true"),
            )
            .send()
            .await?;

        let (session, session_signature) =
            response
                .cookies()
                .fold((None, None), |session_cookies, cookie| {
                    match cookie.name() {
                        "sessionid" => (Some(cookie.value().to_string()), session_cookies.1),
                        "sessionid_sign" => (session_cookies.0, Some(cookie.value().to_string())),
                        _ => session_cookies,
                    }
                });

        if session.is_none() || session_signature.is_none() {
            let error_msg = "Wrong username or password".to_string();
            error!("{}", error_msg);
            return Err(Box::new(LoginError::LoginFailed(error_msg)));
        }

        let response: Value = response.json().await?;

        let mut user: User;

        if response["error"] == "".to_owned() {
            debug!("User data: {:#?}", response);
            warn!("2FA is not enabled for this account");
            info!("User is logged in successfully");
            let login_resp: LoginUserResponse = serde_json::from_value(response)?;

            user = login_resp.user;
        } else if response["error"] == "2FA_required".to_owned() {
            if totp_secret.is_none() {
                let error_msg =
                    "2FA is enabled for this account, but no secret was provided".to_string();
                error!("{}", error_msg);
                return Err(Box::new(LoginError::TOTPError(error_msg)));
            }
            let response: Value = Self::handle_2fa(
                &totp_secret.clone().unwrap(),
                session.clone().unwrap().as_str(),
                session_signature.clone().unwrap().as_str(),
            )
            .await?
            .json()
            .await?;
            debug!("User data: {:#?}", response);
            info!("User is logged in successfully");
            let login_resp: LoginUserResponse = serde_json::from_value(response)?;

            user = login_resp.user;
        } else {
            let error_msg = "Wrong username or password".to_string();
            error!("{}", error_msg);
            return Err(Box::new(LoginError::LoginFailed(error_msg)));
        }

        user.session = session.unwrap();
        user.session_signature = session_signature.unwrap();
        user.password = password;
        user.totp_secret = totp_secret.unwrap_or_default();

        Ok(user)
    }

    async fn handle_2fa(
        totp_secret: &str,
        session: &str,
        session_signature: &str,
    ) -> Result<reqwest::Response, Box<dyn std::error::Error>> {
        use reqwest::multipart::Form;

        let client: reqwest::Client = crate::utils::build_client(Some(&format!(
            "sessionid={}; sessionid_sign={};",
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
            return Err(Box::new(LoginError::TOTPError(
                "TOTP authentication failed, bad request".to_string(),
            )));
        }
    }

    pub async fn get_user(
        session: String,
        session_signature: String,
        url: Option<String>,
    ) -> Result<User, Box<dyn std::error::Error>> {
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
                    return Err(Box::new(LoginError::ParseError(
                        "Error parsing user id".to_string(),
                    )));
                }
            };

            let username_regex = Regex::new(r#""username":"(.*?)""#)?;
            let username = match username_regex.captures(&resp_body) {
                Some(captures) => captures[1].to_string(),
                None => {
                    error!("Error parsing username");
                    return Err(Box::new(LoginError::ParseError(
                        "Error parsing username".to_string(),
                    )));
                }
            };

            let session_hash_regex = Regex::new(r#""session_hash":"(.*?)""#)?;
            let session_hash = match session_hash_regex.captures(&resp_body) {
                Some(captures) => captures[1].to_string(),
                None => {
                    error!("Error parsing session_hash");
                    return Err(Box::new(LoginError::ParseError(
                        "Error parsing session_hash".to_string(),
                    )));
                }
            };

            let private_channel_regex = Regex::new(r#""private_channel":"(.*?)""#)?;
            let private_channel = match private_channel_regex.captures(&resp_body) {
                Some(captures) => captures[1].to_string(),
                None => {
                    error!("Error parsing private_channel");
                    return Err(Box::new(LoginError::ParseError(
                        "Error parsing private_channel".to_string(),
                    )));
                }
            };

            let auth_token_regex = Regex::new(r#""auth_token":"(.*?)""#)?;
            let auth_token = match auth_token_regex.captures(&resp_body) {
                Some(captures) => captures[1].to_string(),
                None => {
                    error!("Error parsing auth token");
                    return Err(Box::new(LoginError::ParseError(
                        "Error parsing auth token".to_string(),
                    )));
                }
            };

            Ok(User {
                auth_token: auth_token,
                id: id,
                username: username,
                session: session,
                session_signature: session_signature,
                session_hash: session_hash,
                private_channel: private_channel,
                password: "".to_string(),
                totp_secret: "".to_string(),
                is_pro: false,
            })
        } else {
            Err(Box::new(LoginError::SessionExpired))
        }
    }

    pub async fn update_token(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let client = crate::utils::build_client(Some(&format!(
            "sessionid={}; sessionid_sign={}",
            self.session, self.session_signature
        )))?;

        let resp_body = client
            .get("https://www.tradingview.com/")
            .send()
            .await?
            .text()
            .await?;

        Ok(if resp_body.contains("auth_token") {
            self.auth_token = match Regex::new(r#""auth_token":"(.*?)""#)?.captures(&resp_body) {
                Some(captures) => captures[1].to_string(),
                None => {
                    error!("Error parsing auth token");
                    return Err(Box::new(LoginError::ParseError(
                        "Error parsing auth token".to_string(),
                    )));
                }
            };
        })
    }
}
