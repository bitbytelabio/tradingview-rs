use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, ORIGIN, REFERER};
use reqwest::{Client, Error};
use serde::Deserialize;
use tracing::{debug, error, info};

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
    error: String,
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

#[tracing::instrument]
pub async fn login_user(username: &str, password: &str) -> Result<UserData, Error> {
    let client = Client::builder()
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
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36 uacq")
        .https_only(true)
        .gzip(true)
        .build()?
        .post("https://www.tradingview.com/accounts/signin/")
        .multipart(reqwest::multipart::Form::new()
            .text("username", username.to_string())
            .text("password", password.to_string())
            .text("remember", "true".to_string())
        );
    let response = client.send().await?;
    let (session, signature) = response
        .cookies()
        .fold((None, None), |session_cookies, cookies| {
            if cookies.name() == "sessionid" {
                (Some(cookies.value().to_string()), session_cookies.1)
            } else if cookies.name() == "sessionid_sign" {
                (session_cookies.0, Some(cookies.value().to_string()))
            } else {
                session_cookies
            }
        });

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
}
