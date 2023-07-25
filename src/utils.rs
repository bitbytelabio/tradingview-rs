use rand::Rng;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, COOKIE, ORIGIN, REFERER};
use serde::Serialize;
use serde_json::Value;
use tokio_tungstenite::tungstenite::protocol::Message;
use tracing::{debug, error};

lazy_static::lazy_static! {
    static ref CLEANER_REGEX: Regex = Regex::new(r"~h~").unwrap();
    static ref SPLITTER_REGEX: Regex = Regex::new(r"~m~[0-9]{1,}~m~").unwrap();
}

pub fn build_request(cookie: Option<&str>) -> Result<reqwest::Client, Box<dyn std::error::Error>> {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(
        ORIGIN,
        HeaderValue::from_static("https://www.tradingview.com"),
    );
    headers.insert(
        REFERER,
        HeaderValue::from_static("https://www.tradingview.com/"),
    );
    if let Some(cookie) = cookie {
        headers.insert(COOKIE, HeaderValue::from_str(cookie)?);
    }

    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .default_headers(headers)
        .https_only(true)
        .user_agent(crate::UA)
        .gzip(true)
        .build()?;
    Ok(client)
}

pub fn gen_session_id(session_type: &str) -> String {
    let rng = rand::thread_rng();
    let result: String = rng
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(12)
        .map(char::from)
        .collect();
    session_type.to_owned() + "_" + &result
}

pub fn parse_packet(message: &str) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    if message.is_empty() {
        return Ok(vec![]);
    }

    let cleaned_message = CLEANER_REGEX.replace_all(message, "");
    let packets: Vec<Value> = SPLITTER_REGEX
        .split(&cleaned_message)
        .filter(|packet| !packet.is_empty())
        .map(|packet| match serde_json::from_str(packet) {
            Ok(value) => value,
            Err(error) => {
                if error.is_syntax() {
                    error!("Error parsing packet: invalid JSON: {}", error);
                } else {
                    error!("Error parsing packet: {}", error);
                }
                Value::Null
            }
        })
        .filter(|value| value != &Value::Null)
        .collect();

    Ok(packets)
}

pub fn format_packet<T>(packet: T) -> Result<Message, Box<dyn std::error::Error>>
where
    T: Serialize,
{
    let json_string = serde_json::to_string(&packet)?;
    let formatted_message = format!("~m~{}~m~{}", json_string.len(), json_string);
    debug!("Formatted packet: {}", formatted_message);
    Ok(Message::Text(formatted_message))
}
