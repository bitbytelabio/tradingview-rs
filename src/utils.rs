use crate::{
    Result, UserCookies,
    live::models::{SocketMessage, SocketMessageDe},
    models::{MarketAdjustment, SessionType},
};
use base64::engine::{Engine as _, general_purpose::STANDARD as BASE64};
use bon::builder;
use iso_currency::Currency;
use rand::{Rng, distr::Alphanumeric};
use regex::Regex;
use reqwest::{
    Response,
    header::{ACCEPT, COOKIE, HeaderMap, HeaderValue, ORIGIN, REFERER},
};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::HashMap,
    io::{Cursor, prelude::*},
};
use tokio_tungstenite::tungstenite::protocol::Message;
use tracing::{debug, error};
use ustr::Ustr;
use zip::ZipArchive;

lazy_static::lazy_static! {
    static ref CLEANER_REGEX: Regex = Regex::new(r"~h~").expect("Failed to compile regex");
    static ref SPLITTER_REGEX: Regex = Regex::new(r"~m~\d+~m~").expect("Failed to compile regex");
}

#[macro_export]
macro_rules! payload {
    ($($payload:expr),*) => {
        {
        let payload_vec = vec![$(serde_json::Value::from($payload)),*];
        payload_vec
        }
    };
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

    let mut client = reqwest::Client::builder()
        .default_headers(headers)
        .https_only(true)
        .user_agent(crate::UA);
    #[cfg(feature = "rustls-tls")]
    {
        client = client.use_rustls_tls();
    }
    #[cfg(feature = "native-tls")]
    {
        client = client.use_native_tls();
    }
    let client = client.build()?;
    Ok(client)
}

pub fn gen_session_id(session_type: &str) -> String {
    session_type.to_owned() + "_" + &gen_id()
}

#[inline]
pub fn gen_id() -> String {
    let rng = rand::rng();
    let result: String = rng
        .sample_iter(&Alphanumeric)
        .take(12)
        .map(char::from)
        .collect();
    result
}

#[inline]
pub fn parse_packet(message: &str) -> Vec<SocketMessage<SocketMessageDe>> {
    if message.is_empty() {
        return vec![];
    }

    let cleaned_message = CLEANER_REGEX.replace_all(message, "");
    let packets: Vec<SocketMessage<SocketMessageDe>> = SPLITTER_REGEX
        .split(&cleaned_message)
        .filter(|packet| !packet.is_empty())
        .map(|packet| match serde_json::from_str(packet) {
            Ok(value) => value,
            Err(error) => {
                if error.is_syntax() {
                    error!("error parsing packet, invalid JSON: {}", error);
                } else {
                    error!("error parsing packet: {}", error);
                }
                SocketMessage::Unknown(packet.to_string())
            }
        })
        .collect();

    packets
}

#[inline]
pub fn format_packet<T: Serialize>(packet: T) -> Result<Message> {
    let json_string = serde_json::to_string(&packet)?;
    let formatted_message = format!("~m~{}~m~{}", json_string.len(), json_string);
    debug!("Formatted packet: {}", formatted_message);
    Ok(Message::Text(formatted_message.into()))
}

#[builder]
pub fn symbol_init(
    instrument: &str, // The instrument symbol, e.g., "HOSE:FPT"
    adjustment: Option<MarketAdjustment>,
    currency: Option<Currency>,
    session_type: Option<SessionType>,
    replay: Option<&str>,
) -> Result<String> {
    let mut symbol_init: HashMap<Ustr, Ustr> = HashMap::new();
    if let Some(s) = replay {
        symbol_init.insert(Ustr::from("replay"), Ustr::from(s));
    }
    if let Some(a) = adjustment {
        symbol_init.insert(Ustr::from("adjustment"), Ustr::from(&a.to_string()));
    }
    symbol_init.insert(Ustr::from("symbol"), Ustr::from(instrument));
    if let Some(c) = currency {
        symbol_init.insert(Ustr::from("currency-id"), Ustr::from(c.code()));
    }
    if let Some(s) = session_type {
        symbol_init.insert(Ustr::from("session"), Ustr::from(&s.to_string()));
    }
    let symbol_init_json = serde_json::to_value(&symbol_init)?;
    Ok(format!("={symbol_init_json}"))
}

pub fn _parse_compressed(data: &str) -> Result<Value> {
    let decoded_data = BASE64.decode(data)?;
    let mut zip = ZipArchive::new(Cursor::new(decoded_data))?;
    let mut file = zip.by_index(0)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let parsed_data: Value = serde_json::from_str(&contents)?;
    Ok(parsed_data)
}

pub async fn get(
    client: Option<&UserCookies>,
    url: &str,
    queries: &[(&str, &str)],
) -> Result<Response> {
    let c = match client {
        Some(client) => {
            let cookie = format!(
                "sessionid={}; sessionid_sign={}; device_t={};",
                client.session, client.session_signature, client.device_token
            );
            build_request(Some(&cookie))?
        }
        None => build_request(None)?,
    };

    let response = c.get(url).query(queries).send().await?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{
        models::{MarketAdjustment, SessionType},
        utils::*,
    };
    #[test]
    fn test_parse_packet() {
        let current_dir = std::env::current_dir().unwrap().display().to_string();
        println!("Current dir: {current_dir}");
        let messages =
            std::fs::read_to_string(format!("{current_dir}/tests/data/socket_messages.txt"))
                .unwrap();
        let result = parse_packet(messages.as_str());

        let data = result;
        assert_eq!(data.len(), 42);
    }

    #[test]
    fn test_gen_session_id() {
        let session_type = "qc";
        let session_id = gen_session_id(session_type);
        assert_eq!(session_id.len(), 15); // 2 (session_type) + 1 (_) + 12 (random characters)
        assert!(session_id.starts_with(session_type));
    }

    #[test]
    fn test_symbol_init() {
        let test1 = symbol_init().instrument("NSE:NIFTY").call();
        assert!(test1.is_ok());
        assert_eq!(test1.unwrap(), r#"={"symbol":"NSE:NIFTY"}"#.to_string());

        let test2 = symbol_init()
            .instrument("HOSE:FPT")
            .adjustment(MarketAdjustment::Dividends)
            .currency(Currency::USD)
            .session_type(SessionType::Extended)
            .replay("aaaaaaaaaaaa")
            .call();
        assert!(test2.is_ok());
        let test2_json: Value = serde_json::from_str(&test2.unwrap().replace('=', "")).unwrap();
        let expected2_json = json!({
            "adjustment": "dividends",
            "currency-id": "USD",
            "replay": "aaaaaaaaaaaa",
            "session": "extended",
            "symbol": "HOSE:FPT"
        });
        assert_eq!(test2_json, expected2_json);
    }
}
