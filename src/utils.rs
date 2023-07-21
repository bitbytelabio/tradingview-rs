use rand::Rng;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, COOKIE, ORIGIN, REFERER};
use serde::Serialize;
use serde_json::Value;
use tracing::{debug, error, info, warn};
use tungstenite::protocol::Message;

lazy_static::lazy_static! {
    static ref CLEANER_RGX: Regex = Regex::new(r"~h~").unwrap();
    static ref SPLITTER_RGX: Regex = Regex::new(r"~m~[0-9]{1,}~m~").unwrap();
}

pub async fn get_request(
    url: &str,
    cookies: Option<String>,
) -> Result<reqwest::Response, reqwest::Error> {
    info!("Sending request to: {}", url);
    let client = build_client()?;
    let mut request = client.get(url);
    if let Some(cookies) = cookies {
        let mut headers = HeaderMap::new();
        headers.insert(
            COOKIE,
            match HeaderValue::from_str(&cookies) {
                Ok(header_value) => header_value,
                Err(e) => {
                    error!("Error parsing cookies: {}", e);
                    HeaderValue::from_static("")
                }
            },
        );
        request = request.headers(headers);
    }
    debug!("Sending request: {:?}", request);
    let response = request.send().await?;
    Ok(response)
}

fn build_client() -> Result<reqwest::Client, reqwest::Error> {
    Ok(reqwest::Client::builder()
        .use_rustls_tls()
        .default_headers({
            let mut headers = HeaderMap::new();
            headers.insert(
                ACCEPT,
                HeaderValue::from_static("application/json, text/plain, */*"),
            );
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
        .https_only(true)
        .user_agent(crate::UA)
        .gzip(true)
        .build()?)
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

/// Parses a message string into a vector of JSON values.
///
/// The message string may contain one or more JSON packets, each of which is
/// preceded by a "~m~" prefix, followed by the length of the packet, and another
/// "~m~" prefix. The function uses the `serde_json` crate to parse each packet
/// into a JSON value, and returns a vector containing all the successful parses.
///
/// If the message string is empty, the function returns an error.
pub fn parse_packet(message: &str) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    if message.is_empty() {
        return Err("Empty message".into());
    }

    let cleaned_message = CLEANER_RGX.replace_all(message, "");
    let packets: Vec<Value> = SPLITTER_RGX
        .split(&cleaned_message)
        .filter(|x| !x.is_empty())
        .map(|x| match serde_json::from_str(x) {
            Ok(packet) => packet,
            Err(e) => {
                if e.is_syntax() {
                    error!("Error parsing packet: invalid JSON: {}", e);
                } else if e.is_eof() {
                    error!("Error parsing packet: incomplete JSON: {}", e);
                } else if e.is_io() {
                    error!("Error parsing packet: I/O error: {}", e);
                } else {
                    error!("Error parsing packet: {}", e);
                }
                Value::Null
            }
        })
        .filter(|x| x != &Value::Null)
        .collect();

    Ok(packets)
}

pub fn format_packet<T>(packet: T) -> Result<Message, Box<dyn std::error::Error>>
where
    T: Serialize,
{
    let msg = serde_json::to_value(&packet)?;
    let msg_str = serde_json::to_string(&msg)?;
    let formatted_msg = format!("~m~{}~m~{}", msg_str.len(), msg_str);
    debug!("Formatted packet: {}", formatted_msg);
    Ok(Message::Text(formatted_msg))
}
