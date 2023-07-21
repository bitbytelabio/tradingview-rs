use crate::utils::get_request;
use crate::UA;
use google_authenticator::get_code;
use google_authenticator::GA_AUTH;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, COOKIE, ORIGIN, REFERER};
use reqwest::Client;
use serde_json::Value;
use tracing::{debug, error, info, warn};

#[derive(Debug)]
pub struct UserData {
    pub id: u32,
    pub username: String,
    pub session: String,
    pub signature: String,
    pub session_hash: String,
    pub private_channel: String,
    pub auth_token: String,
}

pub async fn get_user(
    session: &str,
    signature: &str,
    url: Option<&str>,
) -> Result<UserData, Box<dyn std::error::Error>> {
    let response = get_request(
        url.unwrap_or_else(|| "https://www.tradingview.com/"),
        Some(format!(
            "sessionid={}; sessionid_sign={};",
            session, signature
        )),
    )
    .await
    .unwrap_or_else(|e| {
        // !TODO: Handle this error
        error!("Error sending request: {:?}", e);
        panic!("Error sending request");
    })
    .text()
    .await
    .unwrap_or_else(|e| {
        // !TODO: Handle this error
        error!("Error getting response: {:?}", e);
        panic!("Error getting response");
    });

    if response.contains("auth_token") {
        info!("User is logged in");
        info!("Loading auth token and user info");
        let id_regex = Regex::new(r#""id":([0-9]{1,10}),"#).unwrap();
        let id = id_regex.captures(&response).unwrap()[1]
            .to_string()
            .parse()
            .unwrap_or_default();

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
        // !TODO: Handle this error
        Err("Wrong or expired sessionid/signature".into())
    }
}

pub async fn login_user(
    username: String,
    password: String,
    opt_secret: Option<String>,
) -> Result<UserData, Box<dyn std::error::Error>> {
    let response: reqwest::Response = Client::builder()
        .use_rustls_tls()
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
                .text("username", username)
                .text("password", password)
                .text("remember", "true".to_owned()),
        )
        .send()
        .await?;

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

    let response_data: Value = response.json().await?;

    debug!("Response message: {:#?}", response_data);

    if response_data["error"] == "".to_owned() {
        debug!("User data: {:#?}", response_data["user"]);
        warn!("2FA is not enabled for this account");
        info!("User is logged in successfully");
        let data = UserData {
            id: response_data["user"]["id"].as_u64().unwrap_or_default() as u32,
            username: response_data["user"]["username"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            session: session.unwrap_or_default(),
            signature: signature.unwrap_or_default(),
            session_hash: response_data["user"]["session_hash"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            private_channel: response_data["user"]["private_channel"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            auth_token: response_data["user"]["auth_token"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        };
        debug!("User data: {:#?}", data);
        return Ok(data);
    } else if response_data["error"] == "2FA_required".to_string() {
        match opt_secret {
            Some(opt) => {
                let session = &session.unwrap();
                let signature = &signature.unwrap();
                let response = Client::builder()
                    .use_rustls_tls()
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
                    .build()
                    .unwrap()
                    .post("https://www.tradingview.com/accounts/two-factor/signin/totp/")
                    .multipart(
                        reqwest::multipart::Form::new().text("code", get_code!(&opt).unwrap()),
                    )
                    .send()
                    .await?;
                if response.status() != 200 {
                    error!("Invalid TOTP secret");
                    return Err("Invalid TOTP secret".into());
                } else {
                    let response_data: Value = response.json().await.unwrap();
                    info!("User is logged in successfully");
                    let data = UserData {
                        id: response_data["user"]["id"].as_u64().unwrap() as u32,
                        username: response_data["user"]["username"]
                            .as_str()
                            .unwrap_or_default()
                            .to_string(),
                        session: session.to_string(),
                        signature: signature.to_string(),
                        session_hash: response_data["user"]["session_hash"]
                            .as_str()
                            .unwrap_or_default()
                            .to_string(),
                        private_channel: response_data["user"]["private_channel"]
                            .as_str()
                            .unwrap_or_default()
                            .to_string(),
                        auth_token: response_data["user"]["auth_token"]
                            .as_str()
                            .unwrap_or_default()
                            .to_string(),
                    };
                    debug!("User data: {:#?}", data);
                    return Ok(data);
                }
            }
            None => {
                error!("TOTP Secret is required");
                Err("TOTP Secret is required".into())
            }
        }
    } else {
        return {
            error!("Wrong username or password");
            Err("Wrong username or password".into())
        };
    }
}
