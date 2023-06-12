use crate::UA;
use google_authenticator::get_code;
use google_authenticator::GA_AUTH;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, COOKIE, ORIGIN, REFERER};
use reqwest::{Client, Error};
use serde::Deserialize;
use tracing::{error, info, warn};

#[derive(Debug)]
pub struct UserData {
    id: u32,
    username: String,
    session: String,
    signature: String,
    session_hash: String,
    private_channel: String,
    auth_token: String,
}

#[derive(Debug, Deserialize)]
struct LoginResponse {
    user: LoginUserData,
}

#[derive(Debug, Deserialize)]
struct LoginUserData {
    id: u32,
    username: String,
    session_hash: String,
    private_channel: String,
    auth_token: String,
}

pub async fn get_user(
    session: &str,
    signature: &str,
    url: Option<&str>,
) -> Result<UserData, Box<dyn std::error::Error>> {
    let response = crate::utils::client::get_request(
        url.unwrap_or_else(|| "https://www.tradingview.com/"),
        Some(format!(
            "sessionid={}; sessionid_sign={};",
            session, signature
        )),
    )
    .await
    .unwrap()
    .text()
    .await
    .unwrap();
    if response.contains("auth_token") {
        info!("User is logged in");
        info!("Loading auth token and user info");
        let id_regex = Regex::new(r#""id":([0-9]{1,10}),"#).unwrap();
        let id = id_regex.captures(&response).unwrap()[1]
            .to_string()
            .parse()
            .unwrap();

        let username_regex = Regex::new(r#""username":"(.*?)""#).unwrap();
        let username = username_regex.captures(&response).unwrap()[1].to_string();

        let session = session; // Assuming session is already defined
        let signature = signature; // Assuming signature is already defined

        let session_hash_regex = Regex::new(r#""session_hash":"(.*?)""#).unwrap();
        let session_hash = session_hash_regex.captures(&response).unwrap()[1].to_string();

        let private_channel_regex = Regex::new(r#""private_channel":"(.*?)""#).unwrap();
        let private_channel = private_channel_regex.captures(&response).unwrap()[1].to_string();

        let auth_token_regex = Regex::new(r#""auth_token":"(.*?)""#).unwrap();
        let auth_token = auth_token_regex.captures(&response).unwrap()[1].to_string();

        let user_data = UserData {
            id,
            username,
            session: session.to_string(),
            signature: signature.to_string(),
            session_hash,
            private_channel,
            auth_token,
        };
        Ok(user_data)
    } else {
        Err("Wrong or expired sessionid/signature")?
    }
}

pub async fn login_user(
    username: &str,
    password: &str,
    opt_secret: Option<String>,
) -> Result<UserData, Error> {
    let response = Client::builder()
        .default_headers({
            let mut headers = HeaderMap::new();
            headers.insert(
                ORIGIN,
                HeaderValue::from_static("https://www.tradingview.com"),
            );
            headers.insert(
                REFERER,
                HeaderValue::from_static("https://www.tradingview.com/"),
            );
            headers
        })
        .user_agent(UA)
        .https_only(true)
        .gzip(true)
        .build()?
        .post("https://www.tradingview.com/accounts/signin/")
        .multipart(
            reqwest::multipart::Form::new()
                .text("username", username.to_string())
                .text("password", password.to_string())
                .text("remember", "true".to_string()),
        )
        .send()
        .await
        .unwrap();

    let (session, signature) = response
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

    match opt_secret {
        Some(opt) => {
            let session = &session.unwrap();
            let signature = &signature.unwrap();
            if response.status().is_success() {
                let response = Client::builder()
                    .default_headers({
                        let mut headers = HeaderMap::new();
                        headers.insert(
                            ORIGIN,
                            HeaderValue::from_static("https://www.tradingview.com"),
                        );
                        headers.insert(
                            REFERER,
                            HeaderValue::from_static("https://www.tradingview.com/"),
                        );
                        headers.insert(
                            COOKIE,
                            format!("sessionid={}; sessionid_sign={};", &session, &signature)
                                .parse()
                                .unwrap(),
                        );
                        headers
                    })
                    .user_agent(UA)
                    .https_only(true)
                    .gzip(true)
                    .build()?
                    .post("https://www.tradingview.com/accounts/two-factor/signin/totp/")
                    .multipart(
                        reqwest::multipart::Form::new().text("code", get_code!(&opt).unwrap()),
                    )
                    .send()
                    .await
                    .unwrap()
                    .json::<LoginResponse>()
                    .await
                    .unwrap();
                return Ok(UserData {
                    id: response.user.id,
                    username: response.user.username.to_string(),
                    session: session.to_string(),
                    signature: signature.to_string(),
                    session_hash: response.user.session_hash.to_string(),
                    private_channel: response.user.private_channel.to_string(),
                    auth_token: response.user.auth_token.to_string(),
                });
            } else {
                error!("Wrong username or password");
                Err("Wrong username or password").unwrap()
            }
        }
        None => {
            if response.status().is_success() {
                warn!("No OTP secret provided");
                info!("Trying to login without OTP");
                let response_user_data = response.json::<LoginResponse>().await.unwrap();
                let user_data = UserData {
                    id: response_user_data.user.id,
                    username: response_user_data.user.username.to_string(),
                    session: session.unwrap().to_string(),
                    signature: signature.unwrap().to_string(),
                    session_hash: response_user_data.user.session_hash.to_string(),
                    private_channel: response_user_data.user.private_channel.to_string(),
                    auth_token: response_user_data.user.auth_token.to_string(),
                };
                Ok(user_data)
            } else {
                error!("Wrong username or password");
                Err("Wrong username or password").unwrap()
            }
        }
    }
}
