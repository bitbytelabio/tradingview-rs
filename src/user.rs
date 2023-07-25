use async_trait::async_trait;
use google_authenticator::get_code;
use google_authenticator::GA_AUTH;
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use tracing::{debug, error, info, warn};

use crate::errors::LoginError;

#[derive(Debug, Deserialize)]
pub struct LoginUserResponse {
    user: User,
}

#[derive(Default, Debug, Deserialize)]
pub struct User {
    pub id: u32,
    pub username: String,
    pub is_pro: bool,
    pub auth_token: String,
    pub session_hash: String,
    pub private_channel: String,

    #[serde(skip_deserializing)]
    password: String,
    #[serde(skip_deserializing)]
    totp_secret: String,
    #[serde(skip_deserializing)]
    pub session: String,
    #[serde(skip_deserializing)]
    pub signature: String,
}

#[async_trait]
pub trait UserBuilder {}

impl User {
    pub async fn new() -> Self {
        return User {
            auth_token: "unauthorized_user_token".to_owned(),
            ..User::default()
        };
    }

    pub async fn login(
        &mut self,
        username: String,
        password: String,
        totp_secret: Option<String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let client: reqwest::Client = crate::utils::reqwest_build_client(None)?;

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

        let (session, signature) =
            response
                .cookies()
                .fold((None, None), |session_cookies, cookie| {
                    match cookie.name() {
                        "sessionid" => (Some(cookie.value().to_string()), session_cookies.1),
                        "sessionid_sign" => (session_cookies.0, Some(cookie.value().to_string())),
                        _ => session_cookies,
                    }
                });

        if session.is_none() || signature.is_none() {
            let error_msg = "Wrong username or password".to_string();
            error!("{}", error_msg);
            return Err(Box::new(LoginError::LoginFailed(error_msg)));
        }

        let response: Value = response.json().await?;

        let user: User;

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
                signature.clone().unwrap().as_str(),
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

        Ok(User {
            session: session.unwrap_or_default(),
            signature: signature.unwrap_or_default(),
            password: password,
            totp_secret: totp_secret.unwrap_or_default(),
            ..user
        })
    }

    async fn handle_2fa(
        totp_secret: &str,
        session: &str,
        signature: &str,
    ) -> Result<reqwest::Response, Box<dyn std::error::Error>> {
        use reqwest::multipart::Form;

        let client: reqwest::Client = crate::utils::reqwest_build_client(Some(&format!(
            "sessionid={}; sessionid_sign={};",
            session, signature
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
        signature: String,
        url: Option<String>,
    ) -> Result<User, Box<dyn std::error::Error>> {
        use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, COOKIE};
        lazy_static::lazy_static! {
            static ref ID_REGEX: Regex = Regex::new(r#""id":([0-9]{1,10}),"#).unwrap();
            static ref USERNAME_REGEX: Regex =  Regex::new(r#""username":"(.*?)""#).unwrap();
            static ref SESSION_HASH_REGEX: Regex = Regex::new(r#""session_hash":"(.*?)""#).unwrap();
            static ref PRIVATE_CHANNEL_REGEX: Regex =  Regex::new(r#""private_channel":"(.*?)""#).unwrap();
            static ref AUTH_TOKEN_REGEX: Regex =  Regex::new(r#""auth_token":"(.*?)""#).unwrap();
            static ref IS_PRO_REGEX: Regex = Regex::new(r#""is_pro":"(.*?)""#).unwrap();
        }

        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            COOKIE,
            HeaderValue::from_str(&format!(
                "sessionid={}; sessionid_sign={}",
                session, signature
            ))?,
        );

        let resp = reqwest::Client::builder()
            .use_rustls_tls()
            .default_headers(headers)
            .https_only(true)
            .user_agent(crate::UA)
            .gzip(true)
            .build()?
            .get(url.unwrap_or("https://www.tradingview.com/".to_owned()))
            .send()
            .await?
            .text()
            .await?;

        if resp.contains("auth_token") {
            info!("User is logged in, loading auth token and user info");
            let id: u32 = match ID_REGEX.captures(&resp) {
                Some(captures) => captures[1].to_string().parse()?,
                None => {
                    error!("Error parsing user id");
                    return Err(Box::new(LoginError::ParseError(
                        "Error parsing user id".to_string(),
                    )));
                }
            };
            let username = match USERNAME_REGEX.captures(&resp) {
                Some(captures) => captures[1].to_string(),
                None => {
                    error!("Error parsing username");
                    return Err(Box::new(LoginError::ParseError(
                        "Error parsing username".to_string(),
                    )));
                }
            };
            let session_hash = match SESSION_HASH_REGEX.captures(&resp) {
                Some(captures) => captures[1].to_string(),
                None => {
                    error!("Error parsing session_hash");
                    return Err(Box::new(LoginError::ParseError(
                        "Error parsing session_hash".to_string(),
                    )));
                }
            };
            let private_channel = match PRIVATE_CHANNEL_REGEX.captures(&resp) {
                Some(captures) => captures[1].to_string(),
                None => {
                    error!("Error parsing private_channel");
                    return Err(Box::new(LoginError::ParseError(
                        "Error parsing private_channel".to_string(),
                    )));
                }
            };
            let auth_token = match AUTH_TOKEN_REGEX.captures(&resp) {
                Some(captures) => captures[1].to_string(),
                None => {
                    error!("Error parsing auth token");
                    return Err(Box::new(LoginError::ParseError(
                        "Error parsing auth token".to_string(),
                    )));
                }
            };
            let is_pro: bool = match IS_PRO_REGEX.captures(&resp) {
                Some(captures) => captures[1].to_string().parse()?,
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
                signature: signature,
                session_hash: session_hash,
                private_channel: private_channel,
                is_pro: is_pro,
                ..User::default()
            })
        } else {
            Err(Box::new(LoginError::SessionExpired))
        }
    }
}
