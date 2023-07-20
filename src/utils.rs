use rand::Rng;
use regex::Regex;
use reqwest;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, COOKIE, ORIGIN, REFERER};
use serde::Serialize;
use serde_json::Value;
use tracing::{debug, error, info, warn};
use tungstenite::protocol::Message;

// Define two regular expressions for cleaning and splitting the message
lazy_static::lazy_static! {
    static ref CLEANER_RGX: Regex = Regex::new(r"~h~").unwrap();
    static ref SPLITTER_RGX: Regex = Regex::new(r"~m~[0-9]{1,}~m~").unwrap();
}

pub async fn get_request(
    url: &str,
    cookies: Option<String>,
) -> Result<reqwest::Response, reqwest::Error> {
    info!("Sending request to: {}", url);
    let client = create_client()?;
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

fn create_client() -> Result<reqwest::Client, reqwest::Error> {
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
    let mut rng = rand::thread_rng();
    let characters: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let result: String = (0..12)
        .map(|_| {
            let random_index = rng.gen_range(0..characters.len());
            characters[random_index] as char
        })
        .collect();

    format!("{}_{}", session_type, result)
}

// Parse the packet from the message
pub fn parse_packet(message: &str) -> Vec<Value> {
    if message.is_empty() {
        vec![Value::Null];
    }
    let cleaned_message = CLEANER_RGX.replace_all(message, "");
    let packets: Vec<Value> = SPLITTER_RGX
        .split(&cleaned_message)
        .filter(|x| !x.is_empty())
        .map(|x| {
            let packet: Value = match serde_json::from_str(x) {
                Ok(packet) => packet,
                Err(e) => {
                    debug!("Error parsing packet: {}", e);
                    Value::Null
                }
            };
            packet
        })
        .filter(|x| x != &Value::Null)
        .collect();
    packets
}

// Format the packet into a message
pub fn format_packet<T>(packet: T) -> Message
where
    T: Serialize,
{
    let msg = match serde_json::to_value(&packet) {
        Ok(msg) => match serde_json::to_string(&msg) {
            Ok(msg) => msg,
            Err(e) => {
                warn!("Error formatting packet: {}", e);
                String::from("")
            }
        },
        Err(e) => {
            warn!("Error formatting packet: {}", e);
            String::from("")
        }
    };
    let formatted_msg = format!("~m~{}~m~{}", msg.len(), msg.as_str());
    debug!("Formatted packet: {}", formatted_msg);
    Message::Text(formatted_msg)
}
