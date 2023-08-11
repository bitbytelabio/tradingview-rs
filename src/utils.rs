use crate::{
    prelude::*,
    socket::{SocketMessage, SocketMessageDe},
    MarketAdjustment, SessionType,
};
use base64::engine::{general_purpose::STANDARD as BASE64, Engine as _};
use iso_currency::Currency;
use rand::Rng;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, COOKIE, ORIGIN, REFERER};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::HashMap,
    io::{prelude::*, Cursor},
};
use tokio_tungstenite::tungstenite::protocol::Message;
use tracing::{debug, error};
use zip::ZipArchive;

lazy_static::lazy_static! {
    static ref CLEANER_REGEX: Regex = Regex::new(r"~h~").unwrap();
    static ref SPLITTER_REGEX: Regex = Regex::new(r"~m~[0-9]{1,}~m~").unwrap();
}

pub fn build_request(cookie: Option<&str>) -> Result<reqwest::Client> {
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
    session_type.to_owned() + "_" + &gen_id()
}

pub fn gen_id() -> String {
    let rng = rand::thread_rng();
    let result: String = rng
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(12)
        .map(char::from)
        .collect();
    result
}

pub fn parse_packet(message: &str) -> Result<Vec<SocketMessage<SocketMessageDe>>> {
    if message.is_empty() {
        return Ok(vec![]);
    }

    let cleaned_message = CLEANER_REGEX.replace_all(message, "");
    let packets: Vec<SocketMessage<SocketMessageDe>> = SPLITTER_REGEX
        .split(&cleaned_message)
        .filter(|packet| !packet.is_empty())
        .map(|packet| match serde_json::from_str(packet) {
            Ok(value) => value,
            Err(error) => {
                if error.is_syntax() {
                    error!("error parsing packet: invalid JSON: {}", error);
                } else {
                    error!("error parsing packet: {}", error);
                }
                SocketMessage::Unknown(packet.to_string())
            }
        })
        .collect();

    Ok(packets)
}

pub fn format_packet<T>(packet: T) -> Result<Message>
where
    T: Serialize,
{
    let json_string = serde_json::to_string(&packet)?;
    let formatted_message = format!("~m~{}~m~{}", json_string.len(), json_string);
    debug!("Formatted packet: {}", formatted_message);
    Ok(Message::Text(formatted_message))
}

pub fn clean_em_tags(text: &str) -> Result<String> {
    let regex = Regex::new(r"<[^>]*>")?;
    let cleaned_text = regex.replace_all(text, "");
    Ok(cleaned_text.to_string())
}

pub fn symbol_init(
    symbol: &str,
    adjustment: Option<MarketAdjustment>,
    currency: Option<Currency>,
    session_type: Option<SessionType>,
) -> Result<String> {
    let mut symbol_init: HashMap<String, String> = HashMap::new();
    symbol_init.insert(
        "adjustment".to_string(),
        adjustment.unwrap_or_default().to_string(),
    );
    symbol_init.insert("symbol".to_string(), symbol.to_string());
    if let Some(c) = currency {
        symbol_init.insert("currency-id".to_string(), c.code().to_string());
    }
    if let Some(s) = session_type {
        symbol_init.insert("session".to_string(), s.to_string());
    }
    let symbol_init_json = serde_json::to_value(&symbol_init)?;
    Ok(format!("={}", symbol_init_json))
}

pub fn parse_compressed(data: &str) -> Result<Value> {
    let decoded_data = BASE64.decode(data)?;
    let mut zip = ZipArchive::new(Cursor::new(decoded_data))?;
    let mut file = zip.by_index(0)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let parsed_data: Value = serde_json::from_str(&contents)?;
    Ok(parsed_data)
}
