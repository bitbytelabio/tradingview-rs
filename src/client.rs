use crate::utils::get_request;
use crate::UA;
use google_authenticator::get_code;
use google_authenticator::GA_AUTH;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, COOKIE, ORIGIN, REFERER};
use serde_json::Value;
use tracing::{debug, error, info, warn};

pub struct Client {
    user_id: u32,
    username: String,
    session: String,
    signature: String,
    session_hash: String,
    private_channel: String,
    auth_token: String,
}

impl Client {
    async fn new(username: &str, password: &str, opt_secret: Option<String>) {}
    async fn login() {}
    async fn get_user(
        session: String,
        signature: String,
        url: Option<String>,
    ) -> Result<Self, std::io::Error> {
        let response = get_request(
            &url.unwrap_or_else(|| "https://www.tradingview.com/".to_string()),
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
            let user_id = id_regex.captures(&response).unwrap()[1]
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
            return Ok(Client {
                user_id,
                username,
                session: session.to_string(),
                signature: signature.to_string(),
                session_hash,
                private_channel,
                auth_token,
            });
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "User is not logged in",
        ))
    }
    fn error_handler() {}
}
