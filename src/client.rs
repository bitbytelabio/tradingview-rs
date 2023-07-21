use crate::client;
use crate::utils::get_request;
use crate::UA;
use google_authenticator::get_code;
use google_authenticator::GA_AUTH;
use regex::Regex;
use reqwest::cookie;
use reqwest::header::{HeaderMap, HeaderValue, COOKIE, ORIGIN, REFERER};
use serde_json::Value;
use tracing::{debug, error, info, warn};

pub struct Client {
    user_id: u32,
    username: String,
    password: String,
    opt_secret: String,
    session: String,
    signature: String,
    session_hash: String,
    private_channel: String,
    auth_token: String,
}

impl Client {
    async fn new(username: Option<String>, password: Option<String>, opt_secret: Option<String>) {}
    async fn login(&self) {
        use reqwest::multipart::Form;

        let client: reqwest::Client = match crate::utils::build_client() {
            Ok(c) => c,
            Err(e) => panic!("Error building client: {}", e),
        };

        let response = client
            .post("https://www.tradingview.com/accounts/signin/")
            .multipart(
                Form::new()
                    .text("username", self.username.to_owned())
                    .text("password", self.password.to_owned())
                    .text("remember", "true".to_owned()),
            )
            .send()
            .await
            .unwrap();

        let (session, signature) =
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

        let response_data: Value = response.json().await.unwrap();

        if response_data["error"] == "".to_owned() {
            debug!("User data: {:#?}", response_data["user"]);
            warn!("2FA is not enabled for this account");
            info!("User is logged in successfully");
            let data = Client {
                user_id: response_data["user"]["id"].as_u64().unwrap_or_default() as u32,
                password: self.password.to_owned(),
                username: self.username.to_owned(),
                opt_secret: self.opt_secret.to_owned(),

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
        } else if response_data["error"] == "2FA_required".to_owned() {
            
        }
    }
    async fn login_2fa(
        totp_secret: &str,
        session: &str,
        signature: &str,
    ) -> Result<reqwest::Response, Box<dyn std::error::Error>> {
        use reqwest::multipart::Form;

        let client: reqwest::Client = reqwest::Client::builder()
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
            .build()?;

        let response = client
            .post("https://www.tradingview.com/accounts/two-factor/signin/totp/")
            .multipart(Form::new().text("code", get_code!(totp_secret)?))
            .send()
            .await?;

        if response.status().is_success() {
            return Ok(response);
        } else {
            return Err("2FA authentication failed, something went wrong".into());
        }
    }
    async fn get_user(
        session: String,
        signature: String,
        url: Option<String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let response = get_request(
            &url.unwrap_or_else(|| "https://www.tradingview.com/".to_string()),
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
        Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            "User is not logged in",
        )))
    }
    fn error_handler() {}
}
