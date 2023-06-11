use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, COOKIE, ORIGIN, REFERER};
use reqwest::{Client, Error, Response};
use tracing::{debug, error, info};

#[derive(Debug)]
pub struct UserData {
    id: String,
    username: String,
    session: String,
    signature: String,
    session_hash: String,
    private_channel: String,
    auth_token: String,
}

#[tracing::instrument]
pub async fn get_user(session: &str, signature: &str, url: Option<&str>) {
    let response = crate::utils::client::get_request(
        url.unwrap_or_else(|| "https://www.tradingview.com/"),
        Some(format!(
            "sessionid={}; sessionid_sign={}",
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
        let id = id_regex.captures(&response).unwrap()[1].to_string();

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
        debug!("User data: {:#?}", user_data);
    } else {
        error!("User is not logged in");
    }
}

#[tracing::instrument]
pub async fn login_user(username: &str, password: &str) -> Result<(), Error> {
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
    let mut session: &str;
    let mut signature: &str;
    let mut device_id: &str;
    let cookies = response.cookies();
    for cookie in cookies {
        if cookie.name() == "sessionid" {
            session = cookie.value();
            debug!("Session: {}", session);
        } else if cookie.name() == "sessionid_sign" {
            signature = cookie.value();
            debug!("Signature: {}", signature);
        } else if cookie.name() == "device_t" {
            device_id = cookie.value();
            debug!("Device ID: {}", device_id);
        }
    }
    let body: serde_json::Value = response.json().await?;
    debug!("Login response: {:#?}", body);

    // cookies.into_iter().for_each(|cookie| {
    //     debug!("Cookie: {:#?}", cookie);
    // });
    // let response: serde_json::Value = client.send().await?.json().await?;
    // debug!("Login response: {:#?}", response);

    Ok(())
}
