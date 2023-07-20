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
    async fn get_user() {}
    fn error_handler() {}
}
