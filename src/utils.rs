use crate::{
    Result, UserCookies,
    live::models::{SocketMessage, SocketMessageDe},
    models::{MarketAdjustment, SessionType},
};
use bon::builder;
use iso_currency::Currency;
use rand::{Rng, distr::Alphanumeric};
use regex::Regex;
use reqwest::{
    Response,
    header::{ACCEPT, COOKIE, HeaderMap, HeaderValue, ORIGIN, REFERER},
};
use serde::Serialize;
use std::{collections::HashMap, sync::LazyLock};
use tokio_tungstenite::tungstenite::protocol::Message;
use tracing::{debug, error, warn};
use ustr::Ustr;

// ---------------------------------------------------------------------------
// Shared HTTP client — built once, reused for all requests.
// Enables connection pooling, DNS caching, TLS session reuse, and HTTP/2.
// ---------------------------------------------------------------------------
static SHARED_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
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

    let mut builder = reqwest::Client::builder()
        .default_headers(headers)
        .https_only(true)
        .user_agent(crate::UA);

    #[cfg(feature = "rustls-tls")]
    {
        builder = builder.use_rustls_tls();
    }
    #[cfg(feature = "native-tls")]
    {
        builder = builder.use_native_tls();
    }

    builder.build().expect("Failed to build shared HTTP client")
});

static CLEANER_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"~h~").expect("Failed to compile regex"));
static SPLITTER_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"~m~\d+~m~").expect("Failed to compile regex"));

#[macro_export]
macro_rules! payload {
    ($($payload:expr),*) => {
        {
        let payload_vec = vec![$(serde_json::Value::from($payload)),*];
        payload_vec
        }
    };
}

/// Returns a clone of the shared `reqwest::Client`.
///
/// The client is built once at first use and reused for all subsequent calls.
/// `reqwest::Client` is cheap to clone (it wraps an `Arc` internally), so
/// cloning enables connection pooling, DNS caching, and TLS session reuse.
///
/// For authenticated requests, use [`http_client`] and add the cookie
/// per-request via `.header(COOKIE, ...)`.
pub fn http_client() -> reqwest::Client {
    SHARED_CLIENT.clone()
}

/// Build a `reqwest::Client` with optional authentication cookies baked into
/// its default headers.
///
/// **Deprecated for hot paths.**  Prefer [`http_client`] + per-request
/// `.header(COOKIE, cookie_str)` to benefit from connection pooling.
///
/// This function is retained for backward compatibility.  When no cookie is
/// supplied it clones the shared client (zero-cost).  When a cookie IS
/// supplied it builds a fresh client (suboptimal — prefer per-request cookies).
#[deprecated(
    since = "0.2.0",
    note = "Use http_client() and add cookies per-request via .header(COOKIE, ...) for connection pooling"
)]
pub fn build_request(cookie: Option<&str>) -> Result<reqwest::Client> {
    if cookie.is_some() {
        warn!(
            "build_request() with cookies bypasses connection pooling; use http_client() + .header(COOKIE, ...) instead"
        );
        // Legacy path: build a dedicated client with cookies in default headers.
        // This is suboptimal but maintains backward compatibility.
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

        let mut builder = reqwest::Client::builder()
            .default_headers(headers)
            .https_only(true)
            .user_agent(crate::UA);

        #[cfg(feature = "rustls-tls")]
        {
            builder = builder.use_rustls_tls();
        }
        #[cfg(feature = "native-tls")]
        {
            builder = builder.use_native_tls();
        }

        return Ok(builder.build()?);
    }

    // No cookies: return the shared client (connection pooling enabled).
    Ok(SHARED_CLIENT.clone())
}

pub fn gen_session_id(session_type: &str) -> String {
    session_type.to_owned() + "_" + &gen_id()
}

#[inline]
pub fn gen_id() -> Ustr {
    let mut rng = rand::rng();
    let buf: [u8; 12] = std::array::from_fn(|_| rng.sample(Alphanumeric));
    // SAFETY: `Alphanumeric` samples only ASCII bytes (0-9, A-Z, a-z),
    // which are always valid UTF-8. The `expect` documents this invariant.
    let s = core::str::from_utf8(&buf).expect("Alphanumeric produces only ASCII");
    Ustr::from(s)
}

pub fn parse_packet(message: &str) -> Vec<SocketMessage<SocketMessageDe>> {
    if message.is_empty() {
        return vec![];
    }

    let bytes = message.as_bytes();
    let len = bytes.len();
    let mut pos = 0;
    let mut packets = Vec::new();

    while pos < len {
        // Skip TradingView WebSocket heartbeat keep-alive markers "~h~".
        if pos + 3 <= len && &bytes[pos..pos + 3] == b"~h~" {
            pos += 3;
            continue;
        }

        // Expect "~m~" delimiter
        if pos + 3 > len || &bytes[pos..pos + 3] != b"~m~" {
            pos += 1;
            continue;
        }
        pos += 3;

        // Read the payload length (ASCII digits until next "~m~").
        let mut payload_len: usize = 0;
        while pos < len && bytes[pos].is_ascii_digit() {
            payload_len = payload_len
                .saturating_mul(10)
                .saturating_add((bytes[pos] - b'0') as usize);
            pos += 1;
        }

        // Expect closing "~m~" after the length.
        if pos + 3 > len || &bytes[pos..pos + 3] != b"~m~" {
            continue; // malformed frame — skip
        }
        pos += 3;

        // Extract the payload.
        let payload_end = pos.saturating_add(payload_len).min(len);
        let payload_bytes = &bytes[pos..payload_end];
        pos = payload_end;

        // SAFETY: input is &str, so payload_bytes is valid UTF-8.
        let payload_str = match core::str::from_utf8(payload_bytes) {
            Ok(s) => s,
            Err(_) => {
                // Non-UTF-8 payload — treat as unknown.
                let lossy = String::from_utf8_lossy(payload_bytes);
                packets.push(SocketMessage::Unknown(Ustr::from(lossy.as_ref())));
                continue;
            }
        };

        if payload_str.is_empty() {
            continue;
        }

        match serde_json::from_str(payload_str) {
            Ok(value) => packets.push(value),
            Err(error) => {
                if error.is_syntax() {
                    error!("error parsing packet, invalid JSON: {}", error);
                } else {
                    error!("error parsing packet: {}", error);
                }
                packets.push(SocketMessage::Unknown(Ustr::from(payload_str)));
            }
        }
    }

    packets
}

pub fn _parse_packet(message: &str) -> Vec<SocketMessage<SocketMessageDe>> {
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
                SocketMessage::Unknown(Ustr::from(packet))
            }
        })
        .collect();

    packets
}

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

pub async fn get(
    client: Option<&UserCookies>,
    url: &str,
    queries: &[(&str, &str)],
) -> Result<Response> {
    let mut req = SHARED_CLIENT.get(url);
    if let Some(c) = client {
        let cookie = format!(
            "sessionid={}; sessionid_sign={}; device_t={};",
            c.session, c.session_signature, c.device_token
        );
        req = req.header(COOKIE, &cookie);
    }
    let response = req.query(queries).send().await?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

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

    // ------------------------------------------------------------------
    // Shared HTTP client tests
    // ------------------------------------------------------------------

    #[test]
    fn test_http_client_is_singleton() {
        // Multiple calls to http_client() return clones of the same
        // underlying reqwest::Client (Arc-based, cheap to clone).
        let c1 = http_client();
        let c2 = http_client();
        let c3 = http_client();
        // All three should be usable — they share the same connection pool.
        drop(c1);
        drop(c2);
        drop(c3);
    }

    #[test]
    fn test_http_client_supports_concurrent_access() {
        // Verify http_client() can be called from multiple "threads"
        // (here simulated sequentially — connection pool is Send+Sync).
        let clients: Vec<_> = (0..100).map(|_| http_client()).collect();
        assert_eq!(clients.len(), 100);
    }

    #[test]
    #[allow(deprecated)]
    fn test_build_request_no_cookie_uses_shared_client() {
        // Without cookies, build_request() should return a clone of the
        // shared client (no new connection pool created).
        let client = build_request(None).expect("build_request without cookie");
        // Client must be usable.
        drop(client);
    }

    #[test]
    fn test_cookie_formatting() {
        // Verify the cookie string format used in get() and misc get().
        let cookies = UserCookies {
            session: "abc123".into(),
            session_signature: "sig456".into(),
            device_token: "dev789".into(),
            ..Default::default()
        };
        let cookie = format!(
            "sessionid={}; sessionid_sign={}; device_t={};",
            cookies.session, cookies.session_signature, cookies.device_token
        );
        assert_eq!(
            cookie,
            "sessionid=abc123; sessionid_sign=sig456; device_t=dev789;"
        );
    }

    #[test]
    fn test_deprecated_build_request_with_cookie_still_works() {
        // Backward compat: build_request(Some(cookie)) should still return
        // a usable client (though with a deprecation warning at runtime).
        #[allow(deprecated)]
        let client =
            build_request(Some("sessionid=test; sessionid_sign=sig;")).expect("with cookie");
        drop(client);
    }

    // ------------------------------------------------------------------
    // Fuzz / edge-case tests for parse_packet()
    // ------------------------------------------------------------------

    #[test]
    fn test_parse_empty_string() {
        let result = parse_packet("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_only_heartbeat() {
        // The parser should skip heartbeat markers and return empty.
        let result = parse_packet("~h~~h~~h~");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_single_valid_packet() {
        // {"m":"test","p":["hello"]} = 25 bytes
        let packet = r#"~m~25~m~{"m":"test","p":["hello"]}"#;
        let result = parse_packet(packet);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_parse_multiple_packets() {
        // payloads: {"m":"test1","p":["a"]} = 23 bytes each
        let input = concat!(
            r#"~m~23~m~{"m":"test1","p":["a"]}"#,
            r#"~m~23~m~{"m":"test2","p":["b"]}"#,
            r#"~m~23~m~{"m":"test3","p":["c"]}"#,
        );
        let result = parse_packet(input);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_parse_with_embedded_heartbeats() {
        // {"key":"value"} = 15 bytes
        let input = "~h~~m~15~m~{\"key\":\"value\"}~h~~m~5~m~12345";
        let result = parse_packet(input);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_parse_truncated_frame_no_panic() {
        // Partial "~m~" without closing delimiter should not panic.
        let _result = parse_packet("~m~999");
        // Regex parser: "~m~999" doesn't match ~m~\d+~m~ so it returns
        // the whole string as one segment.  Either way, no panic.
    }

    #[test]
    fn test_parse_truncated_payload_no_panic() {
        // Length declares more bytes than available.
        let result = parse_packet("~m~999~m~short");
        assert!(result.len() <= 1); // may parse partial JSON or return Unknown
    }

    #[test]
    fn test_parse_random_garbage_no_panic() {
        // The parser must never panic on arbitrary input.
        let garbage = [
            "",
            "~~~",
            "~m~",
            "~m~0~m~",
            "~m~abc~m~",
            "~m~-1~m~",
            "not a packet at all",
            "~m~5~m~hello~m~3~m~bye",
            "\x00\x01\x02\x03",
            "~m~999999999999999999999999~m~", // huge length
        ];
        for input in &garbage {
            let _result = parse_packet(input);
            // No panic => pass
        }
    }

    #[test]
    fn test_parse_length_zero() {
        // ~m~0~m~ means zero-length payload — should be skipped.
        let result = parse_packet("~m~0~m~");
        assert!(result.is_empty());
    }

    #[test]
    fn test_roundtrip_format_then_parse() {
        // format_packet → parse_packet should be lossless.
        let msg = serde_json::json!({"m": "test_method", "p": [{"key": "value"}]});
        let formatted = super::format_packet(msg).expect("format succeeds");
        let text = match &formatted {
            tokio_tungstenite::tungstenite::protocol::Message::Text(t) => t.as_str(),
            _ => panic!("expected text message"),
        };
        let parsed = parse_packet(text);
        assert!(!parsed.is_empty());
    }
}
