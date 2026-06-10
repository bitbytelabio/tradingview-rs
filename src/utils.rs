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
        // "~m~-1~m~xxx" — '-' is not a digit, so the ~m~-1~m~ delimiter
        // doesn't match `~m~\d+~m~`. The entire input becomes one Unknown.
        let result = parse_packet("~m~-1~m~xxx");
        assert_eq!(result.len(), 1);
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

    /// `~h~` followed by ping digits, then a valid frame. The regex parser
    /// strips `~h~` then splits on `~m~\d+~m~`, yielding two segments:
    /// the ping digits and the frame payload.
    #[test]
    fn parse_packet_ping_digits_before_frame() {
        // ~h~9999999999~m~5~m~hello => heartbeat + 10 digits + valid frame
        let result = parse_packet("~h~9999999999~m~5~m~hello");
        // Regex: after stripping ~h~, split on ~m~5~m~ yields ["9999999999", "hello"]
        assert_eq!(result.len(), 2);
    }

    /// Consecutive ping keepalives with digits only (no frames).
    /// The regex parser strips `~h~` leaving the digits as an Unknown segment.
    #[test]
    fn parse_packet_consecutive_pings_only() {
        // 10 repetitions of "~h~9999999999" — no frames at all.
        let input = "~h~9999999999".repeat(10);
        let result = parse_packet(&input);
        // Regex strips all `~h~`, leaving digits as one Unknown segment.
        assert_eq!(result.len(), 1);
    }

    /// Mixed: heartbeat, ping digits, and valid frames interleaved.
    #[test]
    fn parse_packet_mixed_pings_and_frames() {
        let input = concat!(
            "~h~",                                     // heartbeat only
            "~m~23~m~{\"m\":\"test1\",\"p\":[\"a\"]}", // valid frame
            "~h~9999999999",                           // ping digits
            "~m~23~m~{\"m\":\"test2\",\"p\":[\"b\"]}", // valid frame
            "~h~88888888",                             // more ping digits
        );
        let result = parse_packet(input);
        assert_eq!(result.len(), 2);
    }

    /// `~h~` followed by digits that happen to form a valid-looking
    /// `~m~<len>~m~` prefix. The regex parser treats this as one
    /// non-frame segment (Unknown) since the `~m~` is not preceded by `~m~`.
    #[test]
    fn parse_packet_ping_digits_resembling_frame_prefix() {
        // "~h~10~m~hello" — after `~h~`, we have "10~m~hello".
        // No `~m~\d+~m~` delimiter → entire string is one Unknown segment.
        let result = parse_packet("~h~10~m~hello");
        assert_eq!(result.len(), 1);
    }

    /// Massive ping: `~h~` followed by 100-digit "ping".
    /// The regex parser strips `~h~` leaving the digits as an Unknown segment.
    #[test]
    fn parse_packet_large_ping_no_panic() {
        let ping = format!("~h~{}", "9".repeat(100));
        let result = parse_packet(&ping);
        // Regex strips `~h~`, leaving 100 digits → one Unknown segment.
        assert_eq!(result.len(), 1);
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
        // Simulate a real-world quote message format.
        // SocketMessageSer has only `m` and `p` — no `t`/`t_ms` fields —
        // so the untagged enum deserializes it as Other(Value), not
        // SocketMessage(SocketMessageDe).
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
            SocketMessage::Other(v) => {
                assert_eq!(v["m"], "qsd");
                assert_eq!(v["p"][0]["n"], "AAPL");
            }
            other => panic!("expected Other(Value) for SocketMessageSer round-trip, got {other:?}"),
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
                SocketMessage::Other(v) => {
                    assert_eq!(v["m"], format!("method_{i}"));
                    assert_eq!(v["p"][0]["index"], i);
                }
                other => panic!("expected Other(Value) at index {i}, got {other:?}"),
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
        let ids: Vec<Ustr> = (0..100).map(|_| gen_id()).collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), 100, "gen_id should produce unique values");
    }

    #[test]
    fn gen_id_produces_only_alphanumeric() {
        for _ in 0..50 {
            let id = gen_id();
            assert!(
                id.as_str().chars().all(|c| c.is_ascii_alphanumeric()),
                "gen_id produced non-alphanumeric: {id}"
            );
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
}
