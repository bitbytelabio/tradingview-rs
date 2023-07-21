use rand::Rng;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, COOKIE, ORIGIN, REFERER};
use serde::Serialize;
use serde_json::Value;
use tracing::{debug, error, info, warn};
use tungstenite::protocol::Message;

lazy_static::lazy_static! {
    static ref CLEANER_REGEX: Regex = Regex::new(r"~h~").unwrap();
    static ref SPLITTER_REGEX: Regex = Regex::new(r"~m~[0-9]{1,}~m~").unwrap();
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

pub fn build_client() -> Result<reqwest::Client, reqwest::Error> {
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

/// Formats a packet into a message string.
///
/// The function serializes the given packet to a JSON string using the `serde_json`
/// crate, and then formats the JSON string into a message string that can be sent
/// over the network. The message string consists of a "~m~" prefix, followed by
/// the length of the JSON string, and another "~m~" prefix, followed by the JSON
/// string itself. The function returns a `Message` enum variant containing the
/// formatted message string.
pub fn format_packet<T>(packet: T) -> Result<Message, Box<dyn std::error::Error>>
where
    T: Serialize,
{
    let json_string = serde_json::to_string(&packet)?;
    let formatted_message = format!("~m~{}~m~{}", json_string.len(), json_string);
    debug!("Formatted packet: {}", formatted_message);
    Ok(Message::Text(formatted_message))
}
