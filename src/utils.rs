use crate::{
    Result, UserCookies,
    live::models::{SocketMessage, SocketMessageDe},
    models::{MarketAdjustment, SessionType},
};
use bon::builder;
use iso_currency::Currency;
use rand::{Rng, distr::Alphanumeric};
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
pub fn gen_id() -> String {
    let mut rng = rand::rng();
    let buf: [u8; 12] = std::array::from_fn(|_| rng.sample(Alphanumeric));
    // SAFETY: `Alphanumeric` samples only ASCII bytes (0-9, A-Z, a-z),
    // which are always valid UTF-8.
    let s = core::str::from_utf8(&buf).expect("Alphanumeric produces only ASCII");
    s.to_owned()
}

/// Extract properly-framed `~m~<len>~m~~h~<counter>` heartbeat echoes from
/// a raw TradingView protocol text frame.
///
/// Each returned string is a complete, properly framed heartbeat echo ready
/// to be sent back to the server. The framing length is **computed from the
/// actual `~h~<counter>` payload size**, not blindly echoed from the server
/// frame. This guarantees correctness even if the server sends a malformed
/// length.
pub fn extract_heartbeat_echoes(raw: &str) -> Vec<String> {
    parse_packet(raw)
        .into_iter()
        .filter_map(|msg| msg.heartbeat_echo())
        .collect()
}

fn classify_json_value(value: serde_json::Value) -> SocketMessage<SocketMessageDe> {
    match value {
        serde_json::Value::Object(mut map) => {
            if matches!(map.get("m"), Some(serde_json::Value::String(_)))
                && matches!(map.get("p"), Some(serde_json::Value::Array(_)))
            {
                let m_str = match map.remove("m").unwrap() {
                    serde_json::Value::String(s) => s,
                    _ => unreachable!(),
                };
                let p_vec = match map.remove("p").unwrap() {
                    serde_json::Value::Array(a) => a,
                    _ => unreachable!(),
                };
                let t = map.get("t").and_then(|v| v.as_u64()).unwrap_or(0);
                let t_ms = map.get("t_ms").and_then(|v| v.as_u64()).unwrap_or(0);
                return SocketMessage::SocketMessage(SocketMessageDe {
                    m: Ustr::from(&m_str),
                    p: p_vec,
                    t,
                    t_ms,
                });
            }

            if map.contains_key("session_id") && map.contains_key("timestamp") {
                let obj = serde_json::Value::Object(map);
                if let Ok(info) =
                    serde_json::from_value::<crate::live::models::SocketServerInfo>(obj.clone())
                {
                    return SocketMessage::SocketServerInfo(info);
                }
                return SocketMessage::Other(obj);
            }

            SocketMessage::Other(serde_json::Value::Object(map))
        }
        other => SocketMessage::Other(other),
    }
}

pub fn parse_packet(message: &str) -> Vec<SocketMessage<SocketMessageDe>> {
    if message.is_empty() {
        return vec![];
    }

    let bytes = message.as_bytes();
    let mut pos = 0;
    let mut packets = Vec::new();

    while pos < bytes.len() {
        if bytes[pos..].starts_with(b"~m~") {
            let header_start = pos + 3;
            let mut len_end = header_start;
            while len_end < bytes.len() && bytes[len_end].is_ascii_digit() {
                len_end += 1;
            }

            if len_end > header_start && bytes[len_end..].starts_with(b"~m~") {
                let len_str = &message[header_start..len_end];
                if let Ok(payload_len) = len_str.parse::<usize>() {
                    let payload_start = len_end + 3;
                    let slice = &message[payload_start..];
                    let mut utf16_count = 0;
                    let mut actual_bytes = slice.len();
                    let mut found = false;

                    for (byte_offset, ch) in slice.char_indices() {
                        if utf16_count >= payload_len {
                            actual_bytes = byte_offset;
                            found = true;
                            break;
                        }
                        utf16_count += ch.len_utf16();
                    }

                    if !found && utf16_count <= payload_len {
                        actual_bytes = slice.len();
                    }

                    if actual_bytes > 0 {
                        let payload = &slice[..actual_bytes];
                        if payload.starts_with("~h~") {
                            let mut hb_len = 3;
                            while hb_len < payload.len()
                                && payload.as_bytes()[hb_len].is_ascii_digit()
                            {
                                hb_len += 1;
                            }
                            if hb_len > 3 {
                                if let Ok(counter) = payload[3..hb_len].parse::<u64>() {
                                    packets.push(SocketMessage::Heartbeat(counter));
                                }
                                pos = payload_start + hb_len;
                                continue;
                            }
                        }

                        match serde_json::from_str::<serde_json::Value>(payload) {
                            Ok(val) => {
                                packets.push(classify_json_value(val));
                            }
                            Err(err) => {
                                if err.is_syntax() {
                                    error!("error parsing packet, invalid JSON: {}", err);
                                } else {
                                    error!("error parsing packet: {}", err);
                                }
                                packets.push(SocketMessage::Unknown(payload.to_string()));
                            }
                        }
                    }

                    pos = payload_start + actual_bytes;
                    continue;
                }
            }

            pos += 3;
        } else if bytes[pos..].starts_with(b"~h~") {
            let counter_start = pos + 3;
            let mut counter_end = counter_start;
            while counter_end < bytes.len() && bytes[counter_end].is_ascii_digit() {
                counter_end += 1;
            }

            if counter_end > counter_start {
                if let Ok(counter) = message[counter_start..counter_end].parse::<u64>() {
                    packets.push(SocketMessage::Heartbeat(counter));
                }
                pos = counter_end;
            } else {
                pos += 3;
            }
        } else {
            pos += 1;
        }
    }

    packets
}

pub fn format_packet<T: Serialize>(packet: T) -> Result<Message> {
    let json_string = serde_json::to_string(&packet)?;
    let utf16_len = json_string.encode_utf16().count();
    let formatted_message = format!("~m~{}~m~{}", utf16_len, json_string);
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
        live::models,
        models::{MarketAdjustment, SessionType},
        utils::*,
    };

    // ──────────────────────────────────────────────────────────────────
    // parse_packet — basic smoke tests
    // ──────────────────────────────────────────────────────────────────

    #[test]
    fn parse_packet_from_file() {
        let current_dir = std::env::current_dir().unwrap().display().to_string();
        let messages =
            std::fs::read_to_string(format!("{current_dir}/tests/data/socket_messages.txt"))
                .unwrap();
        let result = parse_packet(messages.as_str());
        assert_eq!(result.len(), 42);
    }

    #[test]
    fn parse_packet_empty_returns_empty() {
        assert!(parse_packet("").is_empty());
    }

    #[test]
    fn parse_packet_only_heartbeats_returns_empty() {
        assert!(parse_packet("~h~~h~~h~").is_empty());
    }

    #[test]
    fn parse_packet_single_valid_message() {
        // {"m":"test","p":["hello"]} = 25 bytes
        let result = parse_packet(r#"~m~25~m~{"m":"test","p":["hello"]}"#);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn parse_packet_multiple_messages() {
        let input = concat!(
            r#"~m~23~m~{"m":"test1","p":["a"]}"#,
            r#"~m~23~m~{"m":"test2","p":["b"]}"#,
            r#"~m~23~m~{"m":"test3","p":["c"]}"#,
        );
        assert_eq!(parse_packet(input).len(), 3);
    }

    #[test]
    fn parse_packet_skips_interleaved_heartbeats() {
        let input = "~h~~m~15~m~{\"key\":\"value\"}~h~~m~5~m~12345";
        assert_eq!(parse_packet(input).len(), 2);
    }

    // ──────────────────────────────────────────────────────────────────
    // parse_packet — type verification (deserialization correctness)
    // ──────────────────────────────────────────────────────────────────

    #[test]
    fn parse_packet_deserializes_socket_message_de() {
        // A well-formed SocketMessageDe with m, p, t, t_ms fields.
        let payload = serde_json::json!({
            "m": "timescale_update",
            "p": [{"sds_5": {"s": [{"i": 0, "v": [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]}]}}],
            "t": 1685633880_u64,
            "t_ms": 1685633880000_u64,
        });
        let payload_str = payload.to_string();
        let packet = format!("~m~{}~m~{}", payload_str.len(), payload_str);
        let result = parse_packet(&packet);
        assert_eq!(result.len(), 1);

        match &result[0] {
            SocketMessage::SocketMessage(de) => {
                assert_eq!(de.m.as_str(), "timescale_update");
                assert_eq!(de.p.len(), 1);
                assert_eq!(de.t, 1685633880);
                assert_eq!(de.t_ms, 1685633880000);
            }
            other => panic!("expected SocketMessageDe, got {other:?}"),
        }
    }

    #[test]
    fn parse_packet_deserializes_socket_server_info() {
        // SocketServerInfo is matched by the untagged enum before SocketMessageDe.
        // `#[serde(rename_all = "camelCase")]` applies to most fields;
        // `session_id`, `studies_metadata_hash`, and `auth_scheme_vsn` have
        // explicit `#[serde(rename)]` overrides.
        let info = serde_json::json!({
            "session_id": "cs_abc123",
            "timestamp": 1685633880_i64,
            "timestampMs": 1685633880000_i64,
            "release": "v24.10",
            "studies_metadata_hash": "hash123",
            "auth_scheme_vsn": 2_i64,
            "protocol": "json",
            "via": "direct",
            "javastudies": ["study1", "study2"],
        });
        let payload_str = info.to_string();
        let packet = format!("~m~{}~m~{}", payload_str.len(), payload_str);
        let result = parse_packet(&packet);
        assert_eq!(result.len(), 1);

        match &result[0] {
            SocketMessage::SocketServerInfo(si) => {
                assert_eq!(si.session_id.as_str(), "cs_abc123");
                assert_eq!(si.timestamp, 1685633880);
                assert_eq!(si.release.as_str(), "v24.10");
                assert_eq!(si.sjavastudies.len(), 2);
            }
            other => panic!("expected SocketServerInfo, got {other:?}"),
        }
    }

    #[test]
    fn parse_packet_other_variant_for_unknown_json_structure() {
        // Valid JSON that doesn't match SocketServerInfo or SocketMessageDe
        // should fall into the Other(Value) variant.
        let payload = serde_json::json!({"unexpected_field": "strange", "count": 42});
        let payload_str = payload.to_string();
        let packet = format!("~m~{}~m~{}", payload_str.len(), payload_str);
        let result = parse_packet(&packet);
        assert_eq!(result.len(), 1);

        match &result[0] {
            SocketMessage::Other(v) => {
                assert_eq!(v["unexpected_field"], "strange");
                assert_eq!(v["count"], 42);
            }
            other => panic!("expected Other(Value), got {other:?}"),
        }
    }

    #[test]
    fn parse_packet_unknown_variant_for_invalid_json() {
        // Non-JSON text should produce an Unknown variant.
        let input = "~m~11~m~not_a_json!";
        let result = parse_packet(input);
        assert_eq!(result.len(), 1);

        match &result[0] {
            SocketMessage::Unknown(s) => {
                assert_eq!(s.as_str(), "not_a_json!");
            }
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    // ──────────────────────────────────────────────────────────────────
    // parse_packet — edge case: non-UTF8 payload
    // ──────────────────────────────────────────────────────────────────

    #[test]
    fn parse_packet_non_utf8_payload_becomes_unknown() {
        // Build a packet with non-UTF8 bytes.  The byte sequence 0xFF is
        // never valid UTF-8, so the parser falls back to lossy conversion.
        let non_utf8_payload = vec![0xFF, 0xFE, 0xFD, b'a', b'b', b'c'];
        let payload_len = non_utf8_payload.len();
        let mut packet = format!("~m~{}~m~", payload_len);
        packet.push_str(
            // SAFETY: we're constructing a string-like frame; the payload
            // bytes are appended directly for test purposes.
            core::str::from_utf8(&non_utf8_payload).unwrap_or(""),
        );
        // For a fully non-UTF8 payload, we construct raw bytes manually.
        let raw = [b"~m~6~m~" as &[u8], &[0xFF, 0xFE, 0xFD, b'a', b'b', b'c']].concat();
        let lossy = String::from_utf8_lossy(&raw);
        let result = parse_packet(&lossy);
        // Should produce an Unknown variant with the lossy text.
        assert_eq!(result.len(), 1);
        match &result[0] {
            SocketMessage::Unknown(_) => { /* expected */ }
            other => panic!("expected Unknown for non-UTF8 payload, got {other:?}"),
        }
    }

    // ──────────────────────────────────────────────────────────────────
    // parse_packet — edge case: length field variants
    // ──────────────────────────────────────────────────────────────────

    #[test]
    fn parse_packet_length_zero_is_skipped() {
        let result = parse_packet("~m~0~m~");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_packet_leading_zeros_in_length() {
        // "~m~005~m~hello" — length 5, payload "hello"
        let result = parse_packet("~m~005~m~hello");
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn parse_packet_negative_like_length_is_skipped() {
        // "~m~-1~m~xxx" — '-' is not a digit, so not a valid length header.
        // Protocol invariant: malformed frames never panic.
        let result = parse_packet("~m~-1~m~xxx");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_packet_truncated_before_length_delimiter_does_not_panic() {
        let _ = parse_packet("~m~999");
    }

    #[test]
    fn parse_packet_length_exceeds_remaining_bytes_clamped() {
        // Length declares 999 bytes but only "short" is available.
        // The parser clamps at the end of input and tries to parse "short".
        let result = parse_packet("~m~999~m~short");
        assert!(result.len() <= 1);
    }

    #[test]
    fn parse_packet_payload_contains_tilde_m_delimiter_substring() {
        // If the payload itself contains "~m~", it must be included in the
        // payload, not treated as a new delimiter (since we use length-based
        // extraction, not delimiter scanning).
        let json_payload = serde_json::json!({"note": "look for ~m~ in payload"});
        let payload_str = json_payload.to_string();
        let packet = format!("~m~{}~m~{}", payload_str.len(), payload_str);
        let result = parse_packet(&packet);
        assert_eq!(result.len(), 1);
        match &result[0] {
            SocketMessage::Other(v) => {
                assert_eq!(v["note"], "look for ~m~ in payload");
            }
            other => panic!("expected a parsed message, got {other:?}"),
        }
    }

    #[test]
    fn parse_packet_random_garbage_never_panics() {
        let garbage = [
            "",
            "~~~",
            "~m~",
            "~m~abc~m~",
            "~m~-1~m~",
            "not a packet at all",
            "~m~5~m~hello~m~3~m~bye",
            "\x00\x01\x02\x03",
            "~m~999999999999999999999999~m~", // huge length
            "~h~~h~~m~~m~~h~",
            "~m~5~m~",
            "~m~~m~5~m~hello",
        ];
        for input in &garbage {
            let _result = parse_packet(input);
            // No panic => pass
        }
    }

    // ──────────────────────────────────────────────────────────────────
    // parse_packet — heartbeat / ping edge cases
    // ──────────────────────────────────────────────────────────────────

    #[test]
    fn parse_packet_ping_digits_before_frame() {
        // ~h~9999999999~m~5~m~hello => heartbeat + frame
        let result = parse_packet("~h~9999999999~m~5~m~hello");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], SocketMessage::Heartbeat(9999999999));
        assert_eq!(result[1], SocketMessage::Unknown("hello".to_string()));
    }

    #[test]
    fn parse_packet_consecutive_pings_only() {
        // 10 repetitions of "~h~9999999999" — 10 typed Heartbeat packets.
        let input = "~h~9999999999".repeat(10);
        let result = parse_packet(&input);
        assert_eq!(result.len(), 10);
        for msg in result {
            assert_eq!(msg, SocketMessage::Heartbeat(9999999999));
        }
    }

    #[test]
    fn parse_packet_mixed_pings_and_frames() {
        let input = concat!(
            "~h~",                                     // bare heartbeat without counter (skipped)
            "~m~23~m~{\"m\":\"test1\",\"p\":[\"a\"]}", // valid frame
            "~h~9999999999",                           // ping digits
            "~m~23~m~{\"m\":\"test2\",\"p\":[\"b\"]}", // valid frame
            "~h~88888888",                             // more ping digits
        );
        let result = parse_packet(input);
        assert_eq!(result.len(), 4);
        assert!(matches!(&result[0], SocketMessage::SocketMessage(_)));
        assert_eq!(result[1], SocketMessage::Heartbeat(9999999999));
        assert!(matches!(&result[2], SocketMessage::SocketMessage(_)));
        assert_eq!(result[3], SocketMessage::Heartbeat(88888888));
    }

    #[test]
    fn parse_packet_ping_digits_resembling_frame_prefix() {
        let result = parse_packet("~h~10~m~hello");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], SocketMessage::Heartbeat(10));
    }

    #[test]
    fn parse_packet_large_ping_no_panic() {
        let ping = format!("~h~{}", "9".repeat(100));
        let _result = parse_packet(&ping);
    }

    // ──────────────────────────────────────────────────────────────────
    // format_packet → parse_packet round-trip
    // ──────────────────────────────────────────────────────────────────

    #[test]
    fn roundtrip_format_then_parse_basic() {
        let msg = serde_json::json!({"m": "test_method", "p": [{"key": "value"}]});
        let formatted = super::format_packet(&msg).expect("format succeeds");
        let text = match &formatted {
            tokio_tungstenite::tungstenite::protocol::Message::Text(t) => t.as_str(),
            _ => panic!("expected text message"),
        };
        let parsed = parse_packet(text);
        assert!(!parsed.is_empty());
    }

    #[test]
    fn roundtrip_format_then_parse_socket_message_ser() {
        let msg = models::SocketMessageSer::new(
            "qsd",
            serde_json::json!([{
                "n": "AAPL",
                "v": {"bid": 150.25, "ask": 150.30, "lp": 150.28}
            }]),
        );
        let formatted = msg.to_message().expect("format succeeds");
        let text = match &formatted {
            tokio_tungstenite::tungstenite::protocol::Message::Text(t) => t.as_str(),
            _ => panic!("expected text message"),
        };
        let parsed = parse_packet(text);
        assert_eq!(parsed.len(), 1);
        match &parsed[0] {
            SocketMessage::SocketMessage(de) => {
                assert_eq!(de.m.as_str(), "qsd");
                assert_eq!(de.p[0]["n"], "AAPL");
            }
            other => panic!(
                "expected SocketMessage(SocketMessageDe) for SocketMessageSer round-trip, got {other:?}"
            ),
        }
    }

    #[test]
    fn roundtrip_multiple_formatted_packets() {
        let msgs: Vec<models::SocketMessageSer> = (0..5)
            .map(|i| {
                models::SocketMessageSer::new(
                    format!("method_{i}"),
                    serde_json::json!([{"index": i}]),
                )
            })
            .collect();

        // Concatenate formatted packets.
        let mut combined = String::new();
        for msg in &msgs {
            let fmt = msg.to_message().expect("format succeeds");
            if let tokio_tungstenite::tungstenite::protocol::Message::Text(t) = &fmt {
                combined.push_str(t.as_str());
            }
        }

        let parsed = parse_packet(&combined);
        assert_eq!(parsed.len(), msgs.len());
        for (i, p) in parsed.iter().enumerate() {
            match p {
                SocketMessage::SocketMessage(de) => {
                    assert_eq!(de.m.as_str(), format!("method_{i}"));
                    assert_eq!(de.p[0]["index"], i);
                }
                other => {
                    panic!("expected SocketMessage(SocketMessageDe) at index {i}, got {other:?}")
                }
            }
        }
    }

    #[test]
    fn roundtrip_with_unicode_payload() {
        // Payload containing Unicode characters.
        let msg = serde_json::json!({
            "m": "study_data",
            "p": [{"name": "📈 Moving Average", "currency": "€"}]
        });
        let formatted = super::format_packet(&msg).expect("format succeeds");
        let text = match &formatted {
            tokio_tungstenite::tungstenite::protocol::Message::Text(t) => t.as_str(),
            _ => panic!("expected text message"),
        };
        let parsed = parse_packet(text);
        assert_eq!(parsed.len(), 1);
    }

    #[test]
    fn roundtrip_full_socket_message_de() {
        // A complete SocketMessageDe includes m, p, t, and t_ms.
        // The untagged enum should deserialize this as SocketMessage(SocketMessageDe).
        let payload = serde_json::json!({
            "m": "timescale_update",
            "p": [{"sds_5": {"s": [{"i": 0, "v": [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]}]}}],
            "t": 1685633880_u64,
            "t_ms": 1685633880000_u64,
        });
        let formatted = super::format_packet(&payload).expect("format succeeds");
        let text = match &formatted {
            tokio_tungstenite::tungstenite::protocol::Message::Text(t) => t.as_str(),
            _ => panic!("expected text message"),
        };
        let parsed = parse_packet(text);
        assert_eq!(parsed.len(), 1);
        match &parsed[0] {
            SocketMessage::SocketMessage(de) => {
                assert_eq!(de.m.as_str(), "timescale_update");
                assert_eq!(de.t, 1685633880);
                assert_eq!(de.t_ms, 1685633880000);
            }
            other => panic!("expected SocketMessage(SocketMessageDe), got {other:?}"),
        }
    }

    // ──────────────────────────────────────────────────────────────────
    // gen_session_id / gen_id
    // ──────────────────────────────────────────────────────────────────

    #[test]
    fn gen_session_id_produces_correct_format() {
        let session_type = "qc";
        let session_id = gen_session_id(session_type);
        // 2 (session_type) + 1 (_) + 12 (random alphanumeric chars)
        assert_eq!(session_id.len(), 15);
        assert!(session_id.starts_with(session_type));
        assert!(session_id.as_bytes()[2] == b'_');
    }

    #[test]
    fn gen_id_is_unique_across_many_calls() {
        let ids: Vec<String> = (0..100).map(|_| gen_id()).collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), 100, "gen_id should produce unique values");
    }

    #[test]
    fn gen_id_produces_only_alphanumeric() {
        for _ in 0..50 {
            let id = gen_id();
            assert!(
                id.chars().all(|c| c.is_ascii_alphanumeric()),
                "gen_id produced non-alphanumeric: {id}"
            );
        }
    }

    #[test]
    fn two_heartbeats_produce_exactly_two_typed_heartbeats_and_echoes() {
        let raw = "~m~5~m~~h~42~m~25~m~{\"m\":\"du\",\"p\":[\"cs_xxx\"]}~m~5~m~~h~43";
        let parsed = parse_packet(raw);
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0], SocketMessage::Heartbeat(42));
        assert!(matches!(parsed[1], SocketMessage::SocketMessage(_)));
        assert_eq!(parsed[2], SocketMessage::Heartbeat(43));

        let echoes = extract_heartbeat_echoes(raw);
        assert_eq!(echoes, vec!["~m~5~m~~h~42", "~m~5~m~~h~43"]);
    }

    #[test]
    fn embedded_tilde_m_and_tilde_h_survive() {
        let payload = serde_json::json!({
            "m": "quote",
            "p": ["embedded ~m~5~m~ and ~h~ text"]
        });
        let formatted = super::format_packet(&payload).expect("format succeeds");
        let text = match &formatted {
            tokio_tungstenite::tungstenite::protocol::Message::Text(t) => t.as_str(),
            _ => panic!("expected text"),
        };
        let parsed = parse_packet(text);
        assert_eq!(parsed.len(), 1);
        match &parsed[0] {
            SocketMessage::SocketMessage(de) => {
                assert_eq!(de.m.as_str(), "quote");
                assert_eq!(de.p[0], "embedded ~m~5~m~ and ~h~ text");
            }
            other => panic!("expected SocketMessage, got {other:?}"),
        }
    }

    #[test]
    fn unicode_byte_lengths_roundtrip() {
        let payload = serde_json::json!({
            "m": "study_data",
            "p": [{"name": "📈 Moving Average", "currency": "€"}]
        });
        let formatted = super::format_packet(&payload).expect("format succeeds");
        let text = match &formatted {
            tokio_tungstenite::tungstenite::protocol::Message::Text(t) => t.as_str(),
            _ => panic!("expected text"),
        };
        let parsed = parse_packet(text);
        assert_eq!(parsed.len(), 1);
        match &parsed[0] {
            SocketMessage::SocketMessage(de) => {
                assert_eq!(de.m.as_str(), "study_data");
                assert_eq!(de.p[0]["name"], "📈 Moving Average");
                assert_eq!(de.p[0]["currency"], "€");
            }
            other => panic!("expected SocketMessage, got {other:?}"),
        }
    }

    #[test]
    fn m_p_maps_to_socket_message_de() {
        let raw = r#"~m~23~m~{"m":"test","p":["a"]}"#;
        let parsed = parse_packet(raw);
        assert_eq!(parsed.len(), 1);
        match &parsed[0] {
            SocketMessage::SocketMessage(de) => {
                assert_eq!(de.m.as_str(), "test");
                assert_eq!(de.p[0], "a");
                assert_eq!(de.t, 0);
                assert_eq!(de.t_ms, 0);
            }
            other => panic!("expected SocketMessage(SocketMessageDe), got {other:?}"),
        }
    }

    // ──────────────────────────────────────────────────────────────────
    // symbol_init
    // ──────────────────────────────────────────────────────────────────

    #[test]
    fn symbol_init_minimal() {
        let test1 = symbol_init().instrument("NSE:NIFTY").call();
        assert!(test1.is_ok());
        assert_eq!(test1.unwrap(), r#"={"symbol":"NSE:NIFTY"}"#.to_string());
    }

    #[test]
    fn symbol_init_all_fields() {
        let result = symbol_init()
            .instrument("HOSE:FPT")
            .adjustment(MarketAdjustment::Dividends)
            .currency(Currency::USD)
            .session_type(SessionType::Extended)
            .replay("aaaaaaaaaaaa")
            .call();
        assert!(result.is_ok());
        let json_str = result.unwrap().replace('=', "");
        let parsed: Value = serde_json::from_str(&json_str).unwrap();
        let expected = json!({
            "adjustment": "dividends",
            "currency-id": "USD",
            "replay": "aaaaaaaaaaaa",
            "session": "extended",
            "symbol": "HOSE:FPT"
        });
        assert_eq!(parsed, expected);
    }

    // ──────────────────────────────────────────────────────────────────
    // Shared HTTP client tests
    // ──────────────────────────────────────────────────────────────────

    #[test]
    fn http_client_is_reusable() {
        let c1 = http_client();
        let c2 = http_client();
        let c3 = http_client();
        drop(c1);
        drop(c2);
        drop(c3);
    }

    #[test]
    fn http_client_supports_many_clones() {
        let clients: Vec<_> = (0..100).map(|_| http_client()).collect();
        assert_eq!(clients.len(), 100);
    }

    #[test]
    #[allow(deprecated)]
    fn build_request_no_cookie_returns_usable_client() {
        let client = build_request(None).expect("build_request without cookie");
        drop(client);
    }

    #[test]
    fn cookie_format_is_correct() {
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
    fn deprecated_build_request_with_cookie_still_works() {
        #[allow(deprecated)]
        let client =
            build_request(Some("sessionid=test; sessionid_sign=sig;")).expect("with cookie");
        drop(client);
    }

    // ──────────────────────────────────────────────────────────────────
    // extract_heartbeat_echoes
    // ──────────────────────────────────────────────────────────────────

    #[test]
    fn extract_heartbeat_single() {
        let raw = "~m~25~m~{\"m\":\"du\",\"p\":[\"cs_xxx\"]}~m~5~m~~h~42";
        let echoes = super::extract_heartbeat_echoes(raw);
        assert_eq!(echoes, vec!["~m~5~m~~h~42"]);
    }

    #[test]
    fn extract_heartbeat_multiple() {
        let raw = "~m~5~m~~h~42~m~25~m~{\"m\":\"du\",\"p\":[\"cs_xxx\"]}~m~5~m~~h~43";
        let echoes = super::extract_heartbeat_echoes(raw);
        assert_eq!(echoes, vec!["~m~5~m~~h~42", "~m~5~m~~h~43"]);
    }

    #[test]
    fn extract_heartbeat_none() {
        let raw = "~m~25~m~{\"m\":\"du\",\"p\":[\"cs_xxx\"]}";
        let echoes = super::extract_heartbeat_echoes(raw);
        assert!(echoes.is_empty());
    }

    #[test]
    fn extract_heartbeat_interleaved() {
        // "~h~99" = 5 bytes → ~m~5~m~, "~h~100" = 6 bytes → ~m~6~m~
        let raw = "~m~5~m~~h~99~m~25~m~{\"m\":\"qsd\",\"p\":[\"qs_xxx\"]}~m~6~m~~h~100";
        let echoes = super::extract_heartbeat_echoes(raw);
        assert_eq!(echoes, vec!["~m~5~m~~h~99", "~m~6~m~~h~100"]);
    }

    #[test]
    fn extract_heartbeat_standalone() {
        let raw = "~m~5~m~~h~42";
        let echoes = super::extract_heartbeat_echoes(raw);
        assert_eq!(echoes, vec!["~m~5~m~~h~42"]);
    }

    #[test]
    fn extract_heartbeat_empty_string() {
        let echoes = super::extract_heartbeat_echoes("");
        assert!(echoes.is_empty());
    }

    #[test]
    fn extract_heartbeat_consecutive() {
        let raw = "~m~5~m~~h~10~m~5~m~~h~20";
        let echoes = super::extract_heartbeat_echoes(raw);
        assert_eq!(echoes, vec!["~m~5~m~~h~10", "~m~5~m~~h~20"]);
    }

    #[test]
    fn extract_heartbeat_properly_framed() {
        // Server sends a standard heartbeat: ~m~5~m~~h~1
        // "~h~1" = 4 bytes → correct framing should be ~m~4~m~
        let raw = "~m~5~m~~h~1";
        let echoes = super::extract_heartbeat_echoes(raw);
        assert_eq!(echoes, vec!["~m~4~m~~h~1"]);
    }

    #[test]
    fn extract_heartbeat_variable_length_counter() {
        // Counter "42": "~h~42" = 5 bytes → ~m~5~m~
        let raw = "~m~5~m~~h~42";
        let echoes = super::extract_heartbeat_echoes(raw);
        assert_eq!(echoes, vec!["~m~5~m~~h~42"]);

        // Counter "123": "~h~123" = 6 bytes → ~m~6~m~
        let raw = "~m~6~m~~h~123";
        let echoes = super::extract_heartbeat_echoes(raw);
        assert_eq!(echoes, vec!["~m~6~m~~h~123"]);

        // Counter "9999": "~h~9999" = 7 bytes → ~m~7~m~
        let raw = "~m~7~m~~h~9999";
        let echoes = super::extract_heartbeat_echoes(raw);
        assert_eq!(echoes, vec!["~m~7~m~~h~9999"]);
    }

    #[test]
    fn extract_heartbeat_self_heals_bad_length() {
        // Server sends malformed frame: claims length 9 but actual "~h~42" = 5
        // We compute correct length from payload, not trusting server's length.
        let raw = "~m~9~m~~h~42";
        let echoes = super::extract_heartbeat_echoes(raw);
        assert_eq!(echoes, vec!["~m~5~m~~h~42"]);
    }

    #[test]
    fn extract_heartbeat_matches_exact_not_partial() {
        // A JSON message containing "~h~" as data, not a real heartbeat.
        // Should NOT be extracted as a heartbeat.
        let raw = "~m~30~m~{\"m\":\"set\",\"p\":[\"~h~\"]}";
        let echoes = super::extract_heartbeat_echoes(raw);
        assert!(echoes.is_empty());
    }

    #[test]
    fn test_parse_packet_utf16_code_units_vietnamese() {
        let json_val = serde_json::json!({
            "m": "symbol_resolved",
            "p": [
                "sds_sym_1",
                {
                    "name": "HOSE:FPT",
                    "local_description": "CÔNG TY CỔ PHẦN FPT"
                }
            ]
        });
        let json_str = json_val.to_string();
        let utf16_len = json_str.encode_utf16().count();
        let packet = format!("~m~{}~m~{}", utf16_len, json_str);

        let result = parse_packet(&packet);
        assert_eq!(result.len(), 1);
        match &result[0] {
            SocketMessage::SocketMessage(de) => {
                assert_eq!(de.m, "symbol_resolved");
                assert_eq!(de.p.len(), 2);
            }
            other => panic!("expected SocketMessage, got {other:?}"),
        }
    }
}
